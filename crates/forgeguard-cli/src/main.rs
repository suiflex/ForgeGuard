use std::{
    env,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use forgeguard_core::{
    config::{ForgeGuardConfig, UpdatePolicy, CONFIG_FILE},
    create_baseline_with_config, detect_installed_agents, detect_project, evaluate_context_hook,
    evaluate_scope_hook, evaluate_stop_hook,
    git::changed_files,
    initialize_global, initialize_project, mark_task_ready_with_confidence, render_context_hook,
    render_hook_decision, render_scope_warning,
    report::{render_detection, render_doctor, render_gate, render_gate_compact, render_sarif},
    run_doctor, run_gate, start_task_with_contract, task_state, update_task_todos, AgentTarget,
    GateOptions, GateReport, GateStatus, GoalContract, GuardMode, HookAgent, HookDecision,
    InitOptions, BASELINE_FILE, LANGUAGE_CAPABILITIES, RULES,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Parser)]
#[command(
    name = "forgeguard",
    version,
    about = "Token-efficient, language-agnostic engineering guardrails for AI coding agents"
)]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Install or refresh ForgeGuard for a repo or globally (--force to refresh after an upgrade).
    Init {
        /// Overwrite ForgeGuard-owned policy and skill files with the bundled
        /// versions and prune obsolete role-skill directories. Also regenerates
        /// .forgeguard/config.toml from detection defaults, resetting custom
        /// commands and mode. Use to refresh an existing install after an upgrade.
        #[arg(long)]
        force: bool,
        /// Install rules, skills, and hooks for supported agents under the user directory.
        #[arg(long)]
        global: bool,
        /// Agents to install for, comma-separated or repeated. Omit in a terminal
        /// to pick interactively; omit elsewhere to install only for the agents
        /// already configured in the target directory.
        #[arg(long, value_enum, value_delimiter = ',')]
        agent: Vec<AgentArg>,
        #[arg(long)]
        json: bool,
    },
    /// Detect languages, frameworks, database tools, tests, and quality commands.
    Detect {
        #[arg(long)]
        json: bool,
    },
    /// Show parser, structural-rule, and semantic-pack coverage.
    Capabilities {
        #[arg(long)]
        json: bool,
    },
    /// Check or change ForgeGuard mode.
    Mode {
        #[arg(value_enum)]
        mode: Option<ModeArg>,
        #[arg(long)]
        global: bool,
        #[arg(long)]
        json: bool,
    },
    /// Inspect or migrate ForgeGuard configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Check configuration and required local tools.
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// Run static engineering rules and configured quality commands.
    Gate {
        #[arg(long)]
        json: bool,
        #[arg(long, value_enum, default_value = "full", conflicts_with = "json")]
        output: OutputArg,
        #[arg(long)]
        no_run: bool,
        #[arg(long)]
        changed: bool,
    },
    /// Review only changed files with ForgeGuard static rules.
    Review {
        #[arg(long)]
        json: bool,
        #[arg(long, value_enum, default_value = "full", conflicts_with = "json")]
        output: OutputArg,
    },
    /// Record current static findings so gates report only new findings.
    Baseline {
        #[command(subcommand)]
        command: BaselineCommands,
    },
    /// Run lifecycle adapters used by supported AI coding agents.
    Hook {
        #[command(subcommand)]
        command: HookCommands,
    },
    /// Track a session objective, scope, and evidence for bounded agent work.
    Task {
        #[command(subcommand)]
        command: Box<TaskCommands>,
    },
    /// Check for a newer release, or change the update policy.
    ///
    /// With no `--mode`, checks and prints a notice; `auto`/`off` never
    /// install anything. `ask` mode also gates other TTY-run commands
    /// (`init`, `doctor`, `gate`, `review`, `baseline`) with a y/n prompt
    /// when a newer version is cached, and installs only on explicit "yes".
    Update {
        #[arg(long, value_enum)]
        mode: Option<UpdatePolicyArg>,
        #[arg(long)]
        global: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AgentArg {
    Codex,
    Claude,
    Cursor,
    #[value(name = "opencode")]
    OpenCode,
    Antigravity,
    Windsurf,
    Copilot,
    Cline,
    Roo,
    All,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputArg {
    Full,
    Compact,
    Quiet,
    Sarif,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ModeArg {
    Default,
    Lite,
    Strict,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum UpdatePolicyArg {
    Auto,
    Ask,
    Off,
}

impl From<UpdatePolicyArg> for UpdatePolicy {
    fn from(value: UpdatePolicyArg) -> Self {
        match value {
            UpdatePolicyArg::Auto => UpdatePolicy::Auto,
            UpdatePolicyArg::Ask => UpdatePolicy::Ask,
            UpdatePolicyArg::Off => UpdatePolicy::Off,
        }
    }
}

#[derive(Debug, Subcommand)]
enum ConfigCommands {
    /// Upgrade a version 1 configuration to version 2 without resetting commands.
    Migrate {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum HookCommands {
    /// Verify changed code when an agent attempts to stop.
    Stop {
        #[arg(long, value_enum)]
        agent: HookAgentArg,
    },
    /// Inject the active objective when a session starts, resumes, or compacts.
    Context {
        #[arg(long, value_enum)]
        agent: HookAgentArg,
    },
    /// Warn when a file edit falls outside the declared task path prefixes.
    Scope {
        #[arg(long, value_enum)]
        agent: HookAgentArg,
    },
}

#[derive(Debug, Subcommand)]
enum TaskCommands {
    /// Register the exact objective before a non-trivial code change.
    Start {
        #[arg(long)]
        session: String,
        #[arg(long)]
        objective: String,
        /// Repository-relative path prefix. Repeat for multiple scopes.
        #[arg(long = "scope")]
        scopes: Vec<String>,
        /// Ask the host's native goal evaluator to track semantic completion.
        #[arg(long)]
        semantic: bool,
        /// Progress metric, such as p95 latency or failing regression count.
        #[arg(long)]
        metric: Option<String>,
        /// Current measured state.
        #[arg(long)]
        baseline: Option<String>,
        /// Verifiable target state.
        #[arg(long)]
        target: Option<String>,
        /// Constraint that must not regress. Repeat for multiple guardrails.
        #[arg(long = "guardrail")]
        guardrails: Vec<String>,
        /// Exact verification method. Repeat for multiple checks.
        #[arg(long = "verification")]
        verifications: Vec<String>,
        /// Verifiable work item. Repeat for multiple todos.
        #[arg(long = "todo")]
        todos: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Mark implementation ready for ForgeGuard's deterministic completion gate.
    Ready {
        #[arg(long)]
        session: String,
        /// Exact executed check or tool result. Repeat for multiple evidence items.
        #[arg(long = "evidence")]
        evidence: Vec<String>,
        /// Model-reported confidence; tracked but never replaces evidence or gates.
        #[arg(long)]
        confidence: Option<u8>,
        #[arg(long)]
        json: bool,
    },
    /// Add todos or mark 1-based todo indexes complete.
    Todo {
        #[arg(long)]
        session: String,
        #[arg(long = "add")]
        additions: Vec<String>,
        #[arg(long = "done")]
        completed: Vec<usize>,
        #[arg(long)]
        json: bool,
    },
    /// Show the current session task state.
    Status {
        #[arg(long)]
        session: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum BaselineCommands {
    /// Write current static findings to .forgeguard/baseline.json.
    Create {
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum HookAgentArg {
    Codex,
    Claude,
    Cursor,
    Antigravity,
}

fn main() -> ExitCode {
    match execute() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn execute() -> Result<ExitCode> {
    let cli = Cli::parse();
    let root = cli
        .root
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", cli.root.display()))?;

    match cli.command {
        Commands::Init {
            force,
            global,
            agent,
            json,
        } => {
            // Interactive wizard only when nothing was specified and we own a
            // terminal. Explicit `--agent` always wins and is never second-guessed,
            // so existing scripts keep working unchanged.
            let interactive =
                agent.is_empty() && !global && !json && std::io::stdout().is_terminal();
            let (use_global, agents, add_gitignore) = if interactive {
                run_init_wizard(&root)?
            } else if agent.is_empty() {
                // Nothing specified and nothing to prompt: install for the agents
                // this directory already uses rather than writing every
                // integration into a repository that wanted one.
                let detect_root = if global {
                    home_directory()?
                } else {
                    root.clone()
                };
                let detected = detect_installed_agents(&detect_root, global);
                if detected.is_empty() {
                    return no_agent_detected(json);
                }
                (global, detected, false)
            } else {
                (
                    global,
                    agent.into_iter().map(AgentTarget::from).collect(),
                    false,
                )
            };
            let options = InitOptions { force, agents };
            if use_global {
                let home = home_directory()?;
                let report = initialize_global(&home, &options)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    render_init_result(
                        &report.agents,
                        &report.files_written,
                        &report.files_skipped,
                    );
                    println!(
                        "{}",
                        theme::point(
                            &format!("global install under {}", report.home.display()),
                            theme::ACCENT,
                        )
                    );
                    if io::stdin().is_terminal() {
                        configure_mode_interactive(&home, true)?;
                    }
                }
            } else {
                let report = initialize_project(&root, &options)?;
                if add_gitignore {
                    forgeguard_core::ignore_forgeguard_artifacts(&root)?;
                }
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    render_init_result(
                        &report.agents,
                        &report.files_written,
                        &report.files_skipped,
                    );
                    let mut done = format!("initialized at {}", root.display());
                    if add_gitignore {
                        done.push_str("; ignored .forgeguard/ in .gitignore");
                    }
                    println!("{}\n", theme::point(&done, theme::ACCENT));
                    print!("{}", render_detection(&report.detection));
                    if io::stdin().is_terminal() {
                        configure_mode_interactive(&root, false)?;
                    }
                }
            }
            if !json {
                maybe_gate_update(&root)?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Detect { json } => {
            let report = detect_project(&root)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", render_detection(&report));
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Capabilities { json } => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "workflow_support": "all initialized repositories",
                        "languages": LANGUAGE_CAPABILITIES,
                        "rules": RULES,
                    }))?
                );
            } else {
                println!("ForgeGuard capabilities");
                println!("Workflow support: all initialized repositories");
                for capability in LANGUAGE_CAPABILITIES {
                    println!(
                        "  {}: parser={}, structural={}, semantic={}",
                        capability.language,
                        capability.parser,
                        capability.structural_rules,
                        capability.semantic_pack
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Mode { mode, global, json } => execute_mode(&root, mode, global, json),
        Commands::Config {
            command: ConfigCommands::Migrate { json },
        } => {
            let mut config = ForgeGuardConfig::load(&root)?;
            let previous = config.migrate_to_v2()?;
            config.save(&root)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({"previous_version": previous, "version": config.version})
                );
            } else if previous == config.version {
                println!("ForgeGuard config already at version {}.", config.version);
            } else {
                println!("ForgeGuard config migrated from version {previous} to 2.");
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Doctor { json } => {
            let config = if root.join(CONFIG_FILE).exists() {
                Some(ForgeGuardConfig::load(&root)?)
            } else {
                None
            };
            let report = run_doctor(&root, config.as_ref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", render_doctor(&report));
                maybe_gate_update(&root)?;
            }
            Ok(if report.healthy {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            })
        }
        Commands::Gate {
            json,
            output,
            no_run,
            changed,
        } => {
            let config = ForgeGuardConfig::load(&root).context(
                "ForgeGuard is not initialized; run `forgeguard init` in the repository first",
            )?;
            if !json {
                maybe_gate_update(&root)?;
            }
            let paths = if changed {
                Some(changed_files(&root)?)
            } else {
                None
            };
            let report = run_gate(
                &root,
                &config,
                &GateOptions {
                    skip_commands: no_run,
                    paths,
                },
            )?;
            render_gate_output(&report, json, output)?;
            Ok(exit_code_for_status(report.status))
        }
        Commands::Review { json, output } => {
            let config = ForgeGuardConfig::load(&root).context(
                "ForgeGuard is not initialized; run `forgeguard init` in the repository first",
            )?;
            if !json {
                maybe_gate_update(&root)?;
            }
            let paths = Some(changed_files(&root)?);
            let report = run_gate(
                &root,
                &config,
                &GateOptions {
                    skip_commands: true,
                    paths,
                },
            )?;
            render_gate_output(&report, json, output)?;
            Ok(exit_code_for_status(report.status))
        }
        Commands::Baseline {
            command: BaselineCommands::Create { force, json },
        } => {
            let config = ForgeGuardConfig::load(&root).context(
                "ForgeGuard is not initialized; run `forgeguard init` in the repository first",
            )?;
            if !json {
                maybe_gate_update(&root)?;
            }
            let baseline = create_baseline_with_config(&root, &config, force)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "path": BASELINE_FILE,
                        "findings": baseline.total_findings(),
                    })
                );
            } else {
                println!(
                    "ForgeGuard baseline created: {} finding(s) at {BASELINE_FILE}",
                    baseline.total_findings()
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Hook {
            command: HookCommands::Stop { agent },
        } => execute_stop_hook(&root, agent.into()),
        Commands::Hook {
            command: HookCommands::Context { agent },
        } => execute_context_hook(&root, agent.into()),
        Commands::Hook {
            command: HookCommands::Scope { agent },
        } => execute_scope_hook(&root, agent.into()),
        Commands::Task { command } => execute_task(&root, *command),
        Commands::Update { mode, global, json } => execute_update(&root, mode, global, json),
    }
}

fn execute_update(
    root: &Path,
    mode: Option<UpdatePolicyArg>,
    global: bool,
    json: bool,
) -> Result<ExitCode> {
    if let Some(mode) = mode {
        let target = if global {
            home_directory()?
        } else {
            root.to_path_buf()
        };
        let mut config = load_or_create_config(&target, global)?;
        config.update.policy = mode.into();
        save_config(&target, global, &config)?;
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "scope": if global { "global" } else { "project" },
                    "update_policy": config.update.policy.as_str(),
                })
            );
        } else {
            println!(
                "ForgeGuard {} update policy set to {}.",
                if global { "global" } else { "project" },
                config.update.policy.as_str()
            );
        }
        return Ok(ExitCode::SUCCESS);
    }

    let home = home_directory()?;
    match forgeguard_core::update::refresh_for(&home, true, VERSION) {
        Some(notice) => println!("{notice}"),
        None => println!("ForgeGuard {VERSION} is up to date."),
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_mode(root: &Path, mode: Option<ModeArg>, global: bool, json: bool) -> Result<ExitCode> {
    let target = if global {
        home_directory()?
    } else {
        root.to_path_buf()
    };
    let mut config = load_or_create_config(&target, global)?;
    let selected = match mode {
        Some(mode) => mode.into(),
        None if io::stdin().is_terminal() && !json => prompt_for_mode(config.mode)?,
        None => config.mode,
    };
    config.mode = selected;
    save_config(&target, global, &config)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "scope": if global { "global" } else { "project" },
                "mode": config.mode.as_str(),
            })
        );
    } else {
        println!(
            "ForgeGuard {} mode set to {}.",
            if global { "global" } else { "project" },
            config.mode.as_str()
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn configure_mode_interactive(target: &Path, global: bool) -> Result<()> {
    let mut config = load_or_create_config(target, global)?;
    println!();
    let mode = prompt_for_mode(config.mode)?;
    config.mode = mode;
    save_config(target, global, &config)?;
    println!(
        "ForgeGuard {} mode set to {}.",
        if global { "global" } else { "project" },
        config.mode.as_str()
    );
    Ok(())
}

fn prompt_for_mode(default: GuardMode) -> Result<GuardMode> {
    println!("ForgeGuard mode");
    println!("  1) default - token-friendly; report static findings, block only failed required commands");
    println!("  2) lite    - report-only; never blocks");
    println!("  3) strict  - block failed required commands and error-level findings");
    print!("Choose mode [{}]: ", default.as_str());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    parse_mode(input.trim()).map(|mode| mode.unwrap_or(default))
}

fn parse_mode(value: &str) -> Result<Option<GuardMode>> {
    let mode = match value.trim().to_ascii_lowercase().as_str() {
        "" => return Ok(None),
        "1" | "default" => GuardMode::Default,
        "2" | "lite" => GuardMode::Lite,
        "3" | "strict" | "guard" => GuardMode::Strict,
        other => bail!("unknown mode `{other}`; expected default, lite, or strict"),
    };
    Ok(Some(mode))
}

fn load_or_create_config(target: &Path, global: bool) -> Result<ForgeGuardConfig> {
    if global {
        return match ForgeGuardConfig::load_global(target) {
            Ok(config) => Ok(config),
            Err(_) => Ok(ForgeGuardConfig::new("global", Vec::new())),
        };
    }
    match ForgeGuardConfig::load(target) {
        Ok(config) => Ok(config),
        Err(_) => {
            let detection = detect_project(target)?;
            let project_name = target
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("project");
            Ok(ForgeGuardConfig::new(
                project_name,
                detection.suggested_commands,
            ))
        }
    }
}

fn save_config(target: &Path, global: bool, config: &ForgeGuardConfig) -> Result<()> {
    if global {
        config.save_global(target)
    } else {
        config.save(target)
    }
}

fn execute_stop_hook(root: &Path, agent: HookAgent) -> Result<ExitCode> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read hook input")?;
    let home = home_directory().ok();
    if let Some(home) = &home {
        forgeguard_core::update::spawn_refresh_if_stale(home);
    }
    let decision = match evaluate_stop_hook(root, &input) {
        Ok((decision, _cache_hit)) => decision,
        Err(error) => {
            let detail = format!("{error:#}");
            let detail: String = detail.chars().take(500).collect();
            HookDecision::Block(format!(
                "ForgeGuard hook failed: {detail}. Fix hook setup or run `forgeguard gate --changed`."
            ))
        }
    };
    // A passing gate stays silent; only surface the optional notice when the hook
    // is already returning feedback, so clean turns keep zero noise.
    let decision = match decision {
        HookDecision::Block(reason) => {
            HookDecision::Block(append_update_notice(reason, home.as_deref()))
        }
        HookDecision::Pass => HookDecision::Pass,
        HookDecision::Stop(reason) => HookDecision::Stop(reason),
    };
    let output = render_hook_decision(agent, &decision);
    if !output.is_empty() {
        println!("{output}");
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_context_hook(root: &Path, agent: HookAgent) -> Result<ExitCode> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read hook input")?;
    if let Some(context) = evaluate_context_hook(root, &input, agent)? {
        println!("{}", render_context_hook(agent, &input, &context));
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_scope_hook(root: &Path, agent: HookAgent) -> Result<ExitCode> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read hook input")?;
    if let Some(warning) = evaluate_scope_hook(root, &input)? {
        println!("{}", render_scope_warning(agent, &warning));
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_task(root: &Path, command: TaskCommands) -> Result<ExitCode> {
    let (task, json) = match command {
        TaskCommands::Start {
            session,
            objective,
            scopes,
            semantic,
            metric,
            baseline,
            target,
            guardrails,
            verifications,
            todos,
            json,
        } => (
            start_task_with_contract(
                root,
                &session,
                &objective,
                &scopes,
                semantic,
                GoalContract {
                    metric,
                    baseline,
                    target,
                    guardrails,
                    verifications,
                },
                &todos,
            )?,
            json,
        ),
        TaskCommands::Ready {
            session,
            evidence,
            confidence,
            json,
        } => (
            mark_task_ready_with_confidence(root, &session, &evidence, confidence)?,
            json,
        ),
        TaskCommands::Todo {
            session,
            additions,
            completed,
            json,
        } => (
            update_task_todos(root, &session, &additions, &completed)?,
            json,
        ),
        TaskCommands::Status { session, json } => {
            let task = task_state(root, &session)?
                .with_context(|| format!("no ForgeGuard task found for session {session}"))?;
            (task, json)
        }
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&task)?);
    } else {
        println!(
            "ForgeGuard task {}: {}",
            task.session_id,
            serde_json::to_value(&task)?["status"]
                .as_str()
                .unwrap_or("unknown")
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn append_update_notice(reason: String, home: Option<&Path>) -> String {
    match home.and_then(|home| forgeguard_core::update::cached_notice_for(home, VERSION)) {
        Some(notice) => format!("{reason}\n\n{notice}"),
        None => reason,
    }
}

fn print_update_notice() {
    if let Ok(home) = home_directory() {
        if let Some(notice) = forgeguard_core::update::refresh_for(&home, false, VERSION) {
            println!("{notice}");
        }
    }
}

/// Project config wins over global config when both set an update policy;
/// falls back to `auto` when neither is initialized.
fn resolve_update_policy(root: &Path, home: &Path) -> UpdatePolicy {
    if let Ok(config) = ForgeGuardConfig::load(root) {
        return config.update.policy;
    }
    if let Ok(config) = ForgeGuardConfig::load_global(home) {
        return config.update.policy;
    }
    UpdatePolicy::Auto
}

/// Surface the update notice according to the resolved policy. `auto` stays
/// passive (current behavior); `ask` blocks with a y/n prompt on a real TTY
/// and, on "yes", runs the installer; `off` skips the check entirely. Never
/// blocks a non-interactive run (falls back to passive notice).
fn maybe_gate_update(root: &Path) -> Result<()> {
    let Ok(home) = home_directory() else {
        return Ok(());
    };
    match resolve_update_policy(root, &home) {
        UpdatePolicy::Off => {}
        UpdatePolicy::Auto => print_update_notice(),
        UpdatePolicy::Ask if io::stdin().is_terminal() => {
            if let Some(notice) = forgeguard_core::update::refresh_for(&home, false, VERSION) {
                println!("{notice}");
                print!("Update now? [y/N]: ");
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                    let status = forgeguard_core::update::run_install_command()?;
                    if !status.success() {
                        eprintln!("ForgeGuard update command exited with {status}.");
                    }
                }
            }
        }
        UpdatePolicy::Ask => print_update_notice(),
    }
    Ok(())
}

fn render_gate_output(report: &GateReport, json: bool, output: OutputArg) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        match output {
            OutputArg::Full => print!("{}", render_gate(report)),
            OutputArg::Compact => println!("{}", render_gate_compact(report)),
            OutputArg::Quiet => {}
            OutputArg::Sarif => println!("{}", render_sarif(report)?),
        }
    }
    Ok(())
}

fn exit_code_for_status(status: GateStatus) -> ExitCode {
    match status {
        GateStatus::Passed | GateStatus::Warning => ExitCode::SUCCESS,
        GateStatus::Blocked => ExitCode::from(2),
    }
}

impl From<AgentArg> for AgentTarget {
    fn from(value: AgentArg) -> Self {
        match value {
            AgentArg::Codex => Self::Codex,
            AgentArg::Claude => Self::Claude,
            AgentArg::Cursor => Self::Cursor,
            AgentArg::OpenCode => Self::OpenCode,
            AgentArg::Antigravity => Self::Antigravity,
            AgentArg::Windsurf => Self::Windsurf,
            AgentArg::Copilot => Self::Copilot,
            AgentArg::Cline => Self::Cline,
            AgentArg::Roo => Self::Roo,
            AgentArg::All => Self::All,
        }
    }
}

impl From<ModeArg> for GuardMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Default => Self::Default,
            ModeArg::Lite => Self::Lite,
            ModeArg::Strict => Self::Strict,
        }
    }
}

impl From<HookAgentArg> for HookAgent {
    fn from(value: HookAgentArg) -> Self {
        match value {
            HookAgentArg::Codex => Self::Codex,
            HookAgentArg::Claude => Self::Claude,
            HookAgentArg::Cursor => Self::Cursor,
            HookAgentArg::Antigravity => Self::Antigravity,
        }
    }
}

/// The installable agents, in menu order. `AgentArg::All` is offered separately
/// as the `all` shortcut, so it is not part of this table.
const AGENT_MENU: &[(&str, AgentTarget)] = &[
    ("codex", AgentTarget::Codex),
    ("claude", AgentTarget::Claude),
    ("cursor", AgentTarget::Cursor),
    ("opencode", AgentTarget::OpenCode),
    ("antigravity", AgentTarget::Antigravity),
    ("windsurf", AgentTarget::Windsurf),
    ("copilot", AgentTarget::Copilot),
    ("cline", AgentTarget::Cline),
    ("roo", AgentTarget::Roo),
];

/// What each menu entry actually writes, so the picker states the cost of a row
/// instead of making the reader guess from a bare name.
const AGENT_SUMMARY: &[(&str, &str)] = &[
    ("codex", "AGENTS.md, shared skill, Stop hook"),
    ("claude", "CLAUDE.md, own skill, Stop hook"),
    ("cursor", ".cursor/rules, shared skill, stop hook"),
    ("opencode", "AGENTS.md, shared skill"),
    ("antigravity", ".agents/rules, shared skill, Stop hook"),
    ("windsurf", "AGENTS.md only"),
    ("copilot", "AGENTS.md only"),
    ("cline", "AGENTS.md only"),
    ("roo", "AGENTS.md only"),
];

const SCOPE_PROJECT: &str = "This repository";
const SCOPE_GLOBAL: &str = "Global (user directory)";

fn run_init_wizard(root: &Path) -> Result<(bool, Vec<AgentTarget>, bool)> {
    println!("{}\n", theme::banner());

    let scope = inquire::Select::new(
        "Where do you want to install?",
        vec![SCOPE_PROJECT, SCOPE_GLOBAL],
    )
    .with_render_config(theme::render_config())
    .prompt()
    .context("init wizard cancelled")?;
    let use_global = scope == SCOPE_GLOBAL;

    let detect_root = if use_global {
        home_directory()?
    } else {
        root.to_path_buf()
    };

    let detected = detect_installed_agents(&detect_root, use_global);
    let found = agent_names(&detected);
    println!(
        "\n{}\n",
        theme::step(
            "init — detected",
            &[if found.is_empty() {
                "no agent configuration found; nothing is pre-selected".to_owned()
            } else {
                format!("{} — pre-selected below", found.join(", "))
            }],
            if found.is_empty() {
                theme::AMBER
            } else {
                theme::ACCENT
            },
        )
    );

    let agents = prompt_for_agents(&detected)?;

    // The gitignore entry only makes sense for a project checkout.
    let add_gitignore = if use_global {
        false
    } else {
        inquire::Confirm::new("Add .forgeguard/ to .gitignore?")
            .with_default(true)
            .with_render_config(theme::render_config())
            .prompt()
            .context("init wizard cancelled")?
    };

    println!();
    Ok((use_global, agents, add_gitignore))
}

/// Ask which agents to install for, with the ones already configured under
/// `detect_root` pre-checked. An empty pick used to mean "install everything",
/// which turned a stray Enter into every integration written at once; it now
/// re-asks and then cancels, because writing nothing is always recoverable.
fn prompt_for_agents(detected: &[AgentTarget]) -> Result<Vec<AgentTarget>> {
    let rows = agent_menu_rows();
    let defaults: Vec<usize> = AGENT_MENU
        .iter()
        .enumerate()
        .filter(|(_, (_, target))| detected.contains(target))
        .map(|(index, _)| index)
        .collect();

    let help = "↑↓ move · space toggle · → all · ← none · enter confirm";
    for attempt in 0..2 {
        // Each row carries its summary, which makes a useful menu but a wrapped
        // mess once echoed back as the answer. Echo the names alone.
        let formatter = &|picked: &[inquire::list_option::ListOption<&String>]| -> String {
            picked
                .iter()
                .map(|option| option.value.split_whitespace().next().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let picked = inquire::MultiSelect::new("Which agents?", rows.clone())
            .with_default(&defaults)
            .with_page_size(AGENT_MENU.len())
            .with_formatter(formatter)
            .with_render_config(theme::render_config())
            .with_help_message(if attempt == 0 {
                help
            } else {
                "nothing selected — pick at least one, or press Esc to cancel"
            })
            .prompt()
            .context("init wizard cancelled")?;
        let agents = agents_from_rows(&picked);
        if !agents.is_empty() {
            return Ok(agents);
        }
        println!("{}", theme::point("nothing selected", theme::AMBER));
    }
    bail!("no agent selected; nothing was installed")
}

/// Menu rows pair the target name with what selecting it writes, padded so the
/// summaries line up into a readable column.
fn agent_menu_rows() -> Vec<String> {
    let width = AGENT_MENU
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0);
    AGENT_MENU
        .iter()
        .map(|(name, _)| {
            let summary = AGENT_SUMMARY
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, summary)| *summary)
                .unwrap_or_default();
            format!("{name:<width$}  {summary}")
        })
        .collect()
}

/// Recover the targets behind the rows the user checked, in menu order. Rows are
/// generated from `AGENT_MENU`, so matching on the name prefix is exact.
fn agents_from_rows(rows: &[String]) -> Vec<AgentTarget> {
    let names: Vec<&str> = rows
        .iter()
        .map(|row| row.split_whitespace().next().unwrap_or_default())
        .collect();
    agents_from_names(&names)
}

/// Map the agent names the user checked onto concrete targets, in menu order.
fn agents_from_names(names: &[&str]) -> Vec<AgentTarget> {
    AGENT_MENU
        .iter()
        .filter(|(name, _)| names.contains(name))
        .map(|(_, target)| *target)
        .collect()
}

fn home_directory() -> Result<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .context("could not determine the user home directory")
}

/// Exit code for "I will not guess": no `--agent`, no terminal to ask in, and no
/// configured agent to infer from. Distinct from `2`, which a blocked gate uses.
const EXIT_NEEDS_AGENT_SELECTION: u8 = 3;

/// Refuse to install rather than pick every agent by default. A caller that is a
/// script or another agent reads this and re-runs with an explicit selection.
fn no_agent_detected(json: bool) -> Result<ExitCode> {
    let choices: Vec<&str> = AGENT_MENU
        .iter()
        .map(|(name, _)| *name)
        .chain(["all"])
        .collect();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "needs_agent_selection": true,
                "choices": choices,
            }))?
        );
    } else {
        eprintln!(
            "{}",
            theme::panel(
                &[
                    "forgeguard init --agent claude".to_owned(),
                    "forgeguard init --agent claude,codex".to_owned(),
                    String::new(),
                    format!("available: {}", choices.join(", ")),
                ],
                "pick an agent",
                theme::VIOLET,
            )
        );
        eprintln!(
            "{}",
            theme::point(
                "no agent configuration detected; nothing was installed",
                theme::AMBER,
            )
        );
    }
    Ok(ExitCode::from(EXIT_NEEDS_AGENT_SELECTION))
}

/// Terminal branding: ANSI colors, banner, and bordered panels.
///
/// Stdlib only — no `owo-colors`, no `console`. Colors are 256-color
/// approximations of the shared palette so ForgeGuard, websift, and suitest read
/// as one product, and everything collapses to plain text when stdout is not a
/// terminal or `NO_COLOR` is set.
mod theme {
    use std::io::IsTerminal;

    use inquire::ui::{Attributes, Color, RenderConfig, StyleSheet, Styled};

    pub(super) const ACCENT: &str = "\x1b[38;5;114m"; // #4ade80
    pub(super) const AMBER: &str = "\x1b[38;5;221m"; // #fbbf24
    pub(super) const VIOLET: &str = "\x1b[38;5;146m"; // #a78bfa
    const BOLD_FG: &str = "\x1b[1;38;5;255m"; // #fafafa
    const RESET: &str = "\x1b[0m";

    fn enabled() -> bool {
        std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
    }

    fn paint(color: &str, text: &str) -> String {
        if enabled() {
            format!("{color}{text}{RESET}")
        } else {
            text.to_owned()
        }
    }

    /// Boxed wordmark. The icon is an outlined mini-box holding an anvil mark —
    /// ForgeGuard forges the guardrails an agent works inside. Box-drawing
    /// characters, not filled blocks, which render as a blob at small sizes.
    pub(super) fn banner() -> String {
        let rows = ["┌───┐", "│ ⌂ │  F O R G E G U A R D", "└───┘"];
        let width = rows
            .iter()
            .map(|row| row.chars().count())
            .max()
            .unwrap_or(0)
            + 4;
        let mut out = vec![format!("┌{}┐", "─".repeat(width))];
        for row in rows {
            let pad = width - 4 - row.chars().count();
            out.push(format!("│  {row}{}  │", " ".repeat(pad)));
        }
        out.push(format!("└{}┘", "─".repeat(width)));
        paint(ACCENT, &out.join("\n"))
    }

    /// A row that sits on the connector column, marker at column zero.
    pub(super) fn point(text: &str, color: &str) -> String {
        paint(color, &format!("◇ {text}"))
    }

    /// One step of the flow: a labeled rule, then body lines under a shared gutter.
    pub(super) fn step(label: &str, lines: &[String], color: &str) -> String {
        let rule = "─".repeat(30usize.saturating_sub(label.len()).max(3));
        let mut out = vec![point(&format!("{label} {rule}"), color), gutter("", color)];
        for line in lines {
            out.push(gutter(&paint(BOLD_FG, line), color));
        }
        out.push(gutter("", color));
        out.join("\n")
    }

    fn gutter(text: &str, color: &str) -> String {
        let bar = paint(color, "│");
        if text.is_empty() {
            bar
        } else {
            format!("{bar} {text}")
        }
    }

    /// Bordered panel around a block of lines.
    pub(super) fn panel(lines: &[String], title: &str, color: &str) -> String {
        let width = lines
            .iter()
            .map(|line| line.chars().count())
            .chain(std::iter::once(title.chars().count()))
            .max()
            .unwrap_or(0)
            .max(20);
        let mut out = vec![paint(
            color,
            &format!(
                "┌─ {title} {}┐",
                "─".repeat((width + 2).saturating_sub(title.chars().count() + 3))
            ),
        )];
        for line in lines {
            let pad = " ".repeat(width - line.chars().count());
            out.push(format!(
                "{} {}{pad} {}",
                paint(color, "│"),
                paint(BOLD_FG, line),
                paint(color, "│")
            ));
        }
        out.push(paint(color, &format!("└{}┘", "─".repeat(width + 2))));
        out.join("\n")
    }

    /// Bind the prompt widgets to the same accent the rest of the flow uses.
    pub(super) fn render_config() -> RenderConfig<'static> {
        if !enabled() {
            return RenderConfig::empty();
        }
        let accent = Color::LightGreen;
        RenderConfig::default()
            .with_prompt_prefix(Styled::new("◇").with_fg(accent))
            .with_answered_prompt_prefix(Styled::new("◇").with_fg(accent))
            .with_highlighted_option_prefix(Styled::new("›").with_fg(accent))
            .with_selected_checkbox(Styled::new("◼").with_fg(accent))
            .with_unselected_checkbox(Styled::new("◻").with_fg(Color::DarkGrey))
            .with_answer(StyleSheet::new().with_fg(accent))
            .with_help_message(StyleSheet::new().with_fg(Color::DarkGrey))
            .with_option(StyleSheet::empty())
            .with_selected_option(Some(
                StyleSheet::new()
                    .with_fg(accent)
                    .with_attr(Attributes::BOLD),
            ))
    }
}

fn agent_names(agents: &[AgentTarget]) -> Vec<&'static str> {
    AGENT_MENU
        .iter()
        .filter(|(_, target)| agents.contains(target))
        .map(|(name, _)| *name)
        .collect()
}

/// Collapse a write list into one line per top-level directory. A single-agent
/// install touches sixteen paths, and printing each one buries the two facts that
/// matter: which agent, and which trees changed.
fn summarize_paths(paths: &[String]) -> Vec<String> {
    let mut groups: Vec<(String, usize)> = Vec::new();
    for path in paths {
        let key = match path.split_once('/') {
            Some((directory, _)) => format!("{directory}/"),
            None => path.clone(),
        };
        // A map would cost the insertion order the rendered output depends on.
        // forgeguard: allow FG-ALG-002 -- groups are top-level directories, of which ForgeGuard writes at most six
        match groups.iter_mut().find(|(name, _)| *name == key) {
            Some((_, count)) => *count += 1,
            None => groups.push((key, 1)),
        }
    }
    groups
        .into_iter()
        .map(|(name, count)| {
            if count > 1 {
                format!("{name} ({count} files)")
            } else {
                name
            }
        })
        .collect()
}

fn render_init_result(agents: &[AgentTarget], written: &[String], skipped: &[String]) {
    let mut body = vec![format!("agents   {}", agent_names(agents).join(", "))];
    for (index, line) in summarize_paths(written).into_iter().enumerate() {
        body.push(format!(
            "{:<8} {line}",
            if index == 0 { "wrote" } else { "" }
        ));
    }
    for (index, line) in summarize_paths(skipped).into_iter().enumerate() {
        body.push(format!(
            "{:<8} {line}",
            if index == 0 { "kept" } else { "" }
        ));
    }
    println!("{}", theme::panel(&body, "installed", theme::ACCENT));
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{
        agent_menu_rows, agents_from_names, agents_from_rows, summarize_paths, AgentTarget,
        BaselineCommands, Cli, Commands, ConfigCommands, OutputArg,
    };

    #[test]
    fn maps_checked_names_in_menu_order() {
        assert_eq!(
            agents_from_names(&["cursor", "codex"]),
            vec![AgentTarget::Codex, AgentTarget::Cursor]
        );
    }

    #[test]
    fn empty_pick_installs_nothing() {
        assert!(agents_from_names(&[]).is_empty());
    }

    #[test]
    fn menu_rows_pair_each_agent_with_what_it_writes() {
        let rows = agent_menu_rows();

        assert!(rows[1].starts_with("claude "));
        assert!(rows[1].ends_with("CLAUDE.md, own skill, Stop hook"));
        assert!(rows.iter().all(|row| row.contains("  ")));
    }

    #[test]
    fn menu_rows_round_trip_back_to_targets() {
        let rows = agent_menu_rows();
        let picked = vec![rows[1].clone(), rows[6].clone()];

        assert_eq!(
            agents_from_rows(&picked),
            vec![AgentTarget::Claude, AgentTarget::Copilot]
        );
    }

    #[test]
    fn writes_collapse_to_one_line_per_directory() {
        let written = vec![
            ".forgeguard/config.toml".to_owned(),
            ".forgeguard/.gitignore".to_owned(),
            "CLAUDE.md".to_owned(),
            ".claude/settings.json".to_owned(),
            ".claude/skills/forgeguard-engineering/SKILL.md".to_owned(),
        ];

        assert_eq!(
            summarize_paths(&written),
            vec![
                ".forgeguard/ (2 files)".to_owned(),
                "CLAUDE.md".to_owned(),
                ".claude/ (2 files)".to_owned(),
            ]
        );
    }

    #[test]
    fn ignores_unknown_names() {
        assert_eq!(
            agents_from_names(&["claude", "bogus"]),
            vec![AgentTarget::Claude]
        );
    }

    #[test]
    fn parses_forced_json_baseline_creation() {
        let cli = Cli::try_parse_from(["forgeguard", "baseline", "create", "--force", "--json"])
            .expect("parse baseline command");

        assert!(matches!(
            cli.command,
            Commands::Baseline {
                command: BaselineCommands::Create {
                    force: true,
                    json: true
                }
            }
        ));
    }

    #[test]
    fn parses_config_migration_and_sarif_output() {
        let migrate = Cli::try_parse_from(["forgeguard", "config", "migrate", "--json"])
            .expect("parse config migration");
        assert!(matches!(
            migrate.command,
            Commands::Config {
                command: ConfigCommands::Migrate { json: true }
            }
        ));

        let sarif = Cli::try_parse_from(["forgeguard", "gate", "--output", "sarif"])
            .expect("parse SARIF output");
        assert!(matches!(
            sarif.command,
            Commands::Gate {
                output: OutputArg::Sarif,
                ..
            }
        ));
    }
}
