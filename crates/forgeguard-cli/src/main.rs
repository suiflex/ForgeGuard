use std::{
    env,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use forgeguard_core::{
    config::{ForgeGuardConfig, CONFIG_FILE},
    create_baseline, detect_project, evaluate_context_hook, evaluate_scope_hook,
    evaluate_stop_hook,
    git::changed_files,
    initialize_global, initialize_project, mark_task_ready_with_confidence, render_context_hook,
    render_hook_decision, render_scope_warning,
    report::{render_detection, render_doctor, render_gate, render_gate_compact},
    run_doctor, run_gate, start_task_with_contract, task_state, update_task_todos, AgentTarget,
    GateOptions, GateReport, GateStatus, GoalContract, GuardMode, HookAgent, HookDecision,
    InitOptions, BASELINE_FILE,
};

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
        /// Agents to install for. Omit in a terminal to pick interactively.
        #[arg(long, value_enum)]
        agent: Option<AgentArg>,
        #[arg(long)]
        json: bool,
    },
    /// Detect languages, frameworks, database tools, tests, and quality commands.
    Detect {
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
    /// Check for a newer release (checks only; installs nothing).
    ///
    /// Only checks and prints a notice; it installs nothing. To upgrade, re-run
    /// the installer, then `forgeguard init --force` to refresh skills and
    /// policies. Use `forgeguard --version` to see the installed version.
    Update,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AgentArg {
    Codex,
    Claude,
    Cursor,
    #[value(name = "opencode")]
    OpenCode,
    Antigravity,
    All,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputArg {
    Full,
    Compact,
    Quiet,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ModeArg {
    Default,
    Lite,
    Strict,
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
            // terminal. Explicit flags, --json, or a pipe keep the old behavior
            // (default `all`) so scripts and CI are never prompted.
            let interactive =
                agent.is_none() && !global && !json && std::io::stdout().is_terminal();
            let (use_global, agents, add_gitignore) = if interactive {
                run_init_wizard()?
            } else {
                (global, vec![agent.unwrap_or(AgentArg::All).into()], false)
            };
            let options = InitOptions { force, agents };
            if use_global {
                let home = home_directory()?;
                let report = initialize_global(&home, &options)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!(
                        "ForgeGuard global skills installed under {}",
                        report.home.display()
                    );
                    render_file_changes(&report.files_written, &report.files_skipped);
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
                    println!("ForgeGuard initialized at {}", root.display());
                    render_file_changes(&report.files_written, &report.files_skipped);
                    if add_gitignore {
                        println!("  ignored .forgeguard/ in .gitignore");
                    }
                    println!();
                    print!("{}", render_detection(&report.detection));
                    if io::stdin().is_terminal() {
                        configure_mode_interactive(&root, false)?;
                    }
                }
            }
            if !json {
                print_update_notice();
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
        Commands::Mode { mode, global, json } => execute_mode(&root, mode, global, json),
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
                print_update_notice();
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
            let baseline = create_baseline(&root, &config.scan, force)?;
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
        Commands::Update => {
            let home = home_directory()?;
            match forgeguard_core::update::refresh(&home, true) {
                Some(notice) => println!("{notice}"),
                None => println!(
                    "ForgeGuard {} is up to date.",
                    forgeguard_core::update::current_version()
                ),
            }
            Ok(ExitCode::SUCCESS)
        }
    }
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
    match home.and_then(forgeguard_core::update::cached_notice) {
        Some(notice) => format!("{reason}\n\n{notice}"),
        None => reason,
    }
}

fn print_update_notice() {
    if let Ok(home) = home_directory() {
        if let Some(notice) = forgeguard_core::update::refresh(&home, false) {
            println!("{notice}");
        }
    }
}

fn render_gate_output(report: &GateReport, json: bool, output: OutputArg) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        match output {
            OutputArg::Full => print!("{}", render_gate(report)),
            OutputArg::Compact => println!("{}", render_gate_compact(report)),
            OutputArg::Quiet => {}
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

/// The five installable agents, in menu order. `AgentArg::All` is offered
/// separately as the `all` shortcut, so it is not part of this table.
const AGENT_MENU: &[(&str, AgentTarget)] = &[
    ("codex", AgentTarget::Codex),
    ("claude", AgentTarget::Claude),
    ("cursor", AgentTarget::Cursor),
    ("opencode", AgentTarget::OpenCode),
    ("antigravity", AgentTarget::Antigravity),
];

const SCOPE_PROJECT: &str = "This repository";
const SCOPE_GLOBAL: &str = "Global (user directory)";

fn run_init_wizard() -> Result<(bool, Vec<AgentTarget>, bool)> {
    let scope = inquire::Select::new(
        "Where do you want to install?",
        vec![SCOPE_PROJECT, SCOPE_GLOBAL],
    )
    .prompt()
    .context("init wizard cancelled")?;
    let use_global = scope == SCOPE_GLOBAL;

    let names: Vec<&str> = AGENT_MENU.iter().map(|(name, _)| *name).collect();
    let picked = inquire::MultiSelect::new("Which agents?", names)
        .with_help_message("↑↓ move, space toggle, → all, ← none, enter confirm")
        .prompt()
        .context("init wizard cancelled")?;
    let agents = agents_from_names(&picked);

    // The gitignore entry only makes sense for a project checkout.
    let add_gitignore = if use_global {
        false
    } else {
        inquire::Confirm::new("Add .forgeguard/ to .gitignore?")
            .with_default(true)
            .prompt()
            .context("init wizard cancelled")?
    };

    Ok((use_global, agents, add_gitignore))
}

/// Map the agent names the user checked onto concrete targets. An empty pick
/// falls back to `All` so confirming nothing never installs nothing.
fn agents_from_names(names: &[&str]) -> Vec<AgentTarget> {
    let selected: Vec<AgentTarget> = AGENT_MENU
        .iter()
        .filter(|(name, _)| names.contains(name))
        .map(|(_, target)| *target)
        .collect();
    if selected.is_empty() {
        vec![AgentTarget::All]
    } else {
        selected
    }
}

fn home_directory() -> Result<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .context("could not determine the user home directory")
}

fn render_file_changes(written: &[String], skipped: &[String]) {
    for path in written {
        println!("  created {path}");
    }
    for path in skipped {
        println!("  skipped {path} (already exists)");
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{agents_from_names, AgentTarget, BaselineCommands, Cli, Commands};

    #[test]
    fn maps_checked_names_in_menu_order() {
        assert_eq!(
            agents_from_names(&["cursor", "codex"]),
            vec![AgentTarget::Codex, AgentTarget::Cursor]
        );
    }

    #[test]
    fn empty_pick_defaults_to_all() {
        assert_eq!(agents_from_names(&[]), vec![AgentTarget::All]);
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
}
