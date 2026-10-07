use std::{
    env,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use forgeguard_core::{
    analyze_impact, architecture, available_servers,
    config::{ForgeGuardConfig, UpdatePolicy, CONFIG_FILE},
    create_baseline_with_config, delete_project, detect_installed_agents, detect_project,
    evaluate_context_hook, evaluate_scope_hook, evaluate_stop_hook, export_artifact, find_symbols,
    index_repository, index_status, initialize_global, initialize_project,
    is_general_hook_invocation, list_projects, mark_task_ready_with_evidence,
    memory::{ensure_current, refresh_changed, run_query, search_symbols},
    render_context_hook, render_hook_decision, render_scope_warning,
    report::{render_detection, render_doctor, render_gate, render_gate_compact, render_sarif},
    run_changed_gate, run_doctor, run_gate, start_task_with_profile, symbol_card, task_state,
    trace_path, update_task_todos, watch_repository, AgentTarget, Detail, Direction, GateOptions,
    GateReport, GateStatus, GoalContract, GuardMode, HookAgent, HookDecision, IndexOptions,
    InitOptions, LspOptions, MemoryStats, RetrievalOptions, Store, TaskProfile, WatchOptions,
    ARTIFACT_FILE, BASELINE_FILE, BEST_LEVEL, FAST_LEVEL, LANGUAGE_CAPABILITIES, RULES,
};

mod mcp;

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
        /// Overwrite every ForgeGuard-owned policy and skill file with the
        /// bundled version and prune obsolete role-skill directories, whether or
        /// not it had drifted. `.forgeguard/config.toml` is left alone.
        #[arg(long)]
        force: bool,
        /// Replace the ForgeGuard-owned files that have drifted from the bundled
        /// versions, without prompting. Configuration is never touched.
        #[arg(long)]
        refresh: bool,
        /// Install rules, skills, and hooks for supported agents under the user directory.
        #[arg(long)]
        global: bool,
        /// Agents to install for, comma-separated or repeated. Omit in a terminal
        /// to pick interactively; omit elsewhere to install only for the agents
        /// already configured in the target directory.
        #[arg(long, value_enum, value_delimiter = ',')]
        agent: Vec<AgentArg>,
        /// Build the code graph after installing. The wizard asks; this is how a
        /// script or CI run answers. `--no-index` declines it.
        #[arg(long, overrides_with = "no_index")]
        index: bool,
        #[arg(long = "no-index", overrides_with = "index")]
        no_index: bool,
        /// Register `forgeguard mcp serve` in this repository for the installed
        /// agents. `--no-mcp` declines it.
        #[arg(long, overrides_with = "no_mcp")]
        mcp: bool,
        #[arg(long = "no-mcp", overrides_with = "mcp")]
        no_mcp: bool,
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
    /// Check or change Code Guard mode for an initialized repository.
    Mode {
        #[arg(value_enum)]
        mode: Option<ModeArg>,
        /// Deprecated compatibility flag; Code Guard modes are repository-only.
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
        /// Compare new code against this Git revision (for example origin/main).
        #[arg(long, requires = "changed")]
        base: Option<String>,
    },
    /// Review only changed files with ForgeGuard static rules.
    Review {
        #[arg(long)]
        json: bool,
        #[arg(long, value_enum, default_value = "full", conflicts_with = "json")]
        output: OutputArg,
        /// Compare new code against this Git revision (for example origin/main).
        #[arg(long)]
        base: Option<String>,
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
    /// Track a general or code objective, scope, and evidence for bounded agent work.
    Task {
        #[command(subcommand)]
        command: Box<TaskCommands>,
    },
    /// Update ForgeGuard to the latest release, check for updates, or change the update policy.
    ///
    /// Running `forgeguard update` checks for a newer release and installs it
    /// directly if available. Pass `--check` to check without installing.
    Update {
        /// Only check for updates without installing
        #[arg(long)]
        check: bool,
        /// Update policy to set (`auto`, `ask`, `off`)
        #[arg(long, value_enum)]
        mode: Option<UpdatePolicyArg>,
        /// Apply update policy globally
        #[arg(long)]
        global: bool,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Serve ForgeGuard over MCP stdio or register it with an agent harness.
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
    /// Query the persistent code graph instead of re-reading source files.
    ///
    /// Every subcommand prints JSON: the caller is normally an agent.
    Memory {
        #[command(subcommand)]
        command: MemoryCommands,
    },
}

#[derive(Debug, Subcommand)]
enum MemoryCommands {
    /// Build or refresh the index. Unchanged files are not reparsed.
    Index {
        /// Reparse every file, ignoring cached hashes.
        #[arg(long)]
        force: bool,
        /// Ask installed language servers about call sites the static pass could
        /// not attribute to a type. Needs the servers on PATH; costs seconds.
        #[arg(long)]
        lsp: bool,
        /// Wall-clock budget for the language-server pass, in seconds.
        #[arg(long, default_value_t = 30, requires = "lsp")]
        lsp_budget: u64,
        /// Per-request timeout, in seconds. A cold server indexing a large
        /// workspace answers its first request slowly.
        #[arg(long, default_value_t = 10, requires = "lsp")]
        lsp_timeout: u64,
        /// Index only what Git reports as changed.
        #[arg(long, conflicts_with = "force")]
        changed: bool,
        /// Git revision to compare against when --changed is set.
        #[arg(long, requires = "changed")]
        base: Option<String>,
    },
    /// Find symbols by exact name, qualified name, or substring.
    Find {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Retrieve one symbol at the requested level of detail.
    Symbol {
        query: String,
        #[arg(long, value_enum, default_value = "structure")]
        detail: DetailArg,
        /// Maximum source bytes to return; source is dropped, not truncated.
        #[arg(long, default_value_t = 8192)]
        max_bytes: usize,
    },
    /// Rank symbols by BM25 over name, signature, and path.
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Walk the call chain: who reaches a symbol and what it reaches.
    Trace {
        query: String,
        #[arg(long, value_enum, default_value = "both")]
        direction: DirectionArg,
        /// Hops to follow, 1 to 5.
        #[arg(long, default_value_t = 2)]
        depth: usize,
    },
    /// Run a read-only Cypher-like query over the graph.
    Query {
        query: String,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Map a Git diff to changed symbols, callers, dependents, tests, and risk.
    Impact {
        #[arg(long)]
        base: Option<String>,
    },
    /// Summarise modules, layers, entry points, routes, and dependencies.
    Architecture,
    /// Report what the memory layer indexed, answered, and avoided reading.
    Stats,
    /// List repositories with an index on this machine.
    Projects,
    /// Report whether this repository's index exists and matches HEAD.
    Status,
    /// Delete this repository's index. Source files are never touched.
    Delete {
        /// Required: deleting an index cannot be undone without a re-index.
        #[arg(long)]
        yes: bool,
    },
    /// Re-index on an interval until interrupted.
    Watch {
        #[arg(long, default_value_t = 5)]
        interval: u64,
        /// Stop after this many ticks instead of running until interrupted.
        #[arg(long)]
        iterations: Option<usize>,
    },
    /// Write the shareable graph artifact teammates can commit.
    Export {
        #[arg(long)]
        output: Option<PathBuf>,
        /// Compress for size (9) instead of speed (3).
        #[arg(long, default_value_t = true)]
        best: bool,
    },
    /// List the language servers ForgeGuard knows about and found on PATH.
    Servers,
    /// Load a committed graph artifact into the local cache.
    Import,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DirectionArg {
    Inbound,
    Outbound,
    Both,
}

impl From<DirectionArg> for Direction {
    fn from(value: DirectionArg) -> Self {
        match value {
            DirectionArg::Inbound => Self::Inbound,
            DirectionArg::Outbound => Self::Outbound,
            DirectionArg::Both => Self::Both,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DetailArg {
    Metadata,
    Structure,
    Snippet,
    Full,
}

impl From<DetailArg> for Detail {
    fn from(value: DetailArg) -> Self {
        match value {
            DetailArg::Metadata => Self::Metadata,
            DetailArg::Structure => Self::Structure,
            DetailArg::Snippet => Self::Snippet,
            DetailArg::Full => Self::Full,
        }
    }
}

#[derive(Debug, Subcommand)]
enum McpCommands {
    /// Serve the gate, doctor, and task status tools over MCP stdio.
    Serve,
    /// Register `forgeguard mcp serve` with an agent harness.
    Register {
        /// Harness id such as claude-code, codex, cursor, or opencode.
        #[arg(long)]
        client: String,
        /// Write the harness's project configuration under --root.
        #[arg(long)]
        project: bool,
        #[arg(long)]
        dry_run: bool,
        /// Replace an existing, different `forgeguard` entry.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AgentArg {
    Codex,
    Claude,
    Cursor,
    #[value(name = "opencode")]
    OpenCode,
    Hermes,
    #[value(name = "openclaw")]
    OpenClaw,
    Omp,
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
        /// Run only for work outside an initialized Code Guard repository.
        #[arg(long)]
        global: bool,
    },
    /// Inject the active objective when a session starts, resumes, or compacts.
    Context {
        #[arg(long, value_enum)]
        agent: HookAgentArg,
        /// Run only for work outside an initialized Code Guard repository.
        #[arg(long)]
        global: bool,
    },
    /// Warn when a file edit falls outside the declared task path prefixes.
    Scope {
        #[arg(long, value_enum)]
        agent: HookAgentArg,
        /// Run only for work outside an initialized Code Guard repository.
        #[arg(long)]
        global: bool,
    },
}

#[derive(Debug, Subcommand)]
enum TaskCommands {
    /// Register the exact objective before non-trivial general or code work.
    Start {
        #[arg(long)]
        session: String,
        #[arg(long)]
        objective: String,
        /// Workflow profile. Built-ins include product, qa, security, business-analysis,
        /// database, architecture, content-creator, and statistics; custom names are allowed.
        #[arg(long, default_value = "general")]
        profile: String,
        /// Repository-relative path prefix. Repeat for multiple scopes.
        #[arg(long = "scope")]
        scopes: Vec<String>,
        /// Non-file resource prefix such as mcp:playwright, url:https://example.com,
        /// or database:production/analytics. Repeat for multiple resources.
        #[arg(long = "resource")]
        resources: Vec<String>,
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
        /// Verifiable completion criterion. Required for non-general profiles.
        #[arg(long = "acceptance")]
        acceptance_criteria: Vec<String>,
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
        /// Evidence provenance such as mcp:playwright, command:cargo-test, or human:reviewer.
        #[arg(long)]
        source: Option<String>,
        /// Artifact reference such as artifact:trace.zip. Repeat for multiple artifacts.
        #[arg(long = "artifact")]
        artifacts: Vec<String>,
        /// 1-based acceptance criterion proved by this evidence. Repeat as needed.
        #[arg(long = "criterion")]
        criteria: Vec<usize>,
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
    #[value(name = "openclaw")]
    OpenClaw,
    #[value(name = "opencode")]
    OpenCode,
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

// forgeguard: allow FG-CPLX-001 -- central CLI dispatcher; this change only routes hook scope
fn execute() -> Result<ExitCode> {
    let cli = Cli::parse();
    let root = cli
        .root
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", cli.root.display()))?;

    match cli.command {
        Commands::Init {
            force,
            refresh,
            global,
            agent,
            index,
            no_index,
            mcp,
            no_mcp,
            json,
        } => {
            // A flag answers the question the wizard would have asked, so a
            // scripted install reaches the same state as an interactive one.
            let index_flag = flag_choice(index, no_index);
            let mcp_flag = flag_choice(mcp, no_mcp);
            // Interactive wizard only when nothing was specified and we own a
            // terminal. Explicit `--agent` always wins and is never second-guessed,
            // so existing scripts keep working unchanged.
            let interactive = should_run_init_wizard(
                agent.is_empty(),
                global,
                json,
                std::io::stdin().is_terminal(),
                std::io::stdout().is_terminal(),
            );
            let mut choices = if interactive {
                run_init_wizard(&root, index_flag, mcp_flag)?
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
                WizardChoices::plain(global, detected)
            } else {
                WizardChoices::plain(global, agent.into_iter().map(AgentTarget::from).collect())
            };
            if !interactive && !choices.use_global {
                choices.index_now = index_flag.unwrap_or(false);
                choices.register_mcp = mcp_flag.unwrap_or(false);
            }
            let WizardChoices {
                use_global,
                agents,
                add_gitignore,
                index_now,
                register_mcp,
            } = choices;
            let options = InitOptions {
                force,
                refresh,
                agents: agents.clone(),
            };
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
                    report_kept(&report.files_kept);
                    if offer_refresh(&report.files_outdated)? {
                        let report = initialize_global(
                            &home,
                            &InitOptions {
                                refresh: true,
                                ..options.clone()
                            },
                        )?;
                        render_init_result(
                            &report.agents,
                            &report.files_written,
                            &report.files_skipped,
                        );
                    }
                }
            } else {
                let report = initialize_project(&root, &options)?;
                // Registered per repository on purpose: one global entry would
                // point every checkout at whichever directory the harness
                // happened to start in, and each repository owns its own graph.
                // `report.agents` rather than the requested list: `initialize_project`
                // has already expanded `--agent all` into concrete targets.
                let mcp_configs = if register_mcp {
                    mcp::register_agents(&root, &report.agents, json)
                } else {
                    Vec::new()
                };
                if add_gitignore {
                    // The prompt was answered yes, so a repository without a
                    // `.gitignore` gets one rather than silently committing the
                    // generated configuration.
                    let ignore = root.join(".gitignore");
                    if !ignore.exists() {
                        std::fs::write(&ignore, "")?;
                    }
                    forgeguard_core::ignore_forgeguard_artifacts(&root)?;
                    forgeguard_core::ignore_repository_paths(&root, &mcp_configs)?;
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
                    report_kept(&report.files_kept);
                    if offer_refresh(&report.files_outdated)? {
                        let report = initialize_project(
                            &root,
                            &InitOptions {
                                refresh: true,
                                ..options.clone()
                            },
                        )?;
                        render_init_result(
                            &report.agents,
                            &report.files_written,
                            &report.files_skipped,
                        );
                        println!();
                    }
                    print!("{}", render_detection(&report.detection));
                    if io::stdin().is_terminal() {
                        configure_mode_interactive(&root)?;
                    }
                }
                if index_now {
                    build_initial_memory(&root, json)?;
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
            let detected = detect_project(&root)?;
            let commands_added = config.reconcile_commands(&detected.suggested_commands);
            config.save(&root)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "previous_version": previous,
                        "version": config.version,
                        "commands_added": commands_added,
                    })
                );
            } else if previous == config.version {
                println!(
                    "ForgeGuard config already at version {}; added {commands_added} new command preset(s).",
                    config.version
                );
            } else {
                println!(
                    "ForgeGuard config migrated from version {previous} to 2; added {commands_added} new command preset(s)."
                );
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
            base,
        } => {
            let config = ForgeGuardConfig::load(&root).context(
                "ForgeGuard is not initialized; run `forgeguard init` in the repository first",
            )?;
            if !json {
                maybe_gate_update(&root)?;
            }
            let report = if changed {
                run_changed_gate(&root, &config, no_run, base.as_deref())?
            } else {
                run_gate(
                    &root,
                    &config,
                    &GateOptions {
                        skip_commands: no_run,
                        paths: None,
                    },
                )?
            };
            render_gate_output(&report, json, output)?;
            Ok(exit_code_for_status(report.status))
        }
        Commands::Review { json, output, base } => {
            let config = ForgeGuardConfig::load(&root).context(
                "ForgeGuard is not initialized; run `forgeguard init` in the repository first",
            )?;
            if !json {
                maybe_gate_update(&root)?;
            }
            let report = run_changed_gate(&root, &config, true, base.as_deref())?;
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
            command: HookCommands::Stop { agent, global },
        } => execute_stop_hook(&root, agent.into(), global),
        Commands::Hook {
            command: HookCommands::Context { agent, global },
        } => execute_context_hook(&root, agent.into(), global),
        Commands::Hook {
            command: HookCommands::Scope { agent, global },
        } => execute_scope_hook(&root, agent.into(), global),
        Commands::Task { command } => execute_task(&root, *command),
        Commands::Update {
            check,
            mode,
            global,
            json,
        } => execute_update(&root, check, mode, global, json),
        Commands::Mcp {
            command: McpCommands::Serve,
        } => mcp::serve(root).map(|()| ExitCode::SUCCESS),
        Commands::Mcp {
            command:
                McpCommands::Register {
                    client,
                    project,
                    dry_run,
                    force,
                },
        } => mcp::register(&root, &client, project, dry_run, force).map(|()| ExitCode::SUCCESS),
        Commands::Memory { command } => execute_memory(&root, command),
    }
}

// forgeguard: allow FG-CPLX-001 -- flat subcommand dispatcher; each arm is one call
fn execute_memory(root: &Path, command: MemoryCommands) -> Result<ExitCode> {
    let config = ForgeGuardConfig::scan_settings(root)?;
    match command {
        MemoryCommands::Index {
            force,
            lsp,
            lsp_budget,
            lsp_timeout,
            changed,
            base,
        } => {
            let report = if changed {
                refresh_changed(root, &config, base.as_deref())?
            } else {
                index_repository(
                    root,
                    &config,
                    &IndexOptions {
                        force,
                        paths: None,
                        lsp: lsp.then(|| LspOptions {
                            budget: std::time::Duration::from_secs(lsp_budget.max(1)),
                            timeout: std::time::Duration::from_secs(lsp_timeout.max(1)),
                            ..LspOptions::default()
                        }),
                    },
                )?
            };
            print_json(&report)
        }
        MemoryCommands::Find { query, limit } => {
            let store = load_memory(root)?;
            print_json(&find_symbols(root, &store, &query, limit)?)
        }
        MemoryCommands::Search { query, limit } => {
            let store = load_memory(root)?;
            print_json(&search_symbols(root, &store, &query, limit)?)
        }
        MemoryCommands::Symbol {
            query,
            detail,
            max_bytes,
        } => {
            let store = load_memory(root)?;
            let options = RetrievalOptions {
                detail: detail.into(),
                max_bytes,
            };
            match symbol_card(root, &store, &query, &options)? {
                Some(card) => print_json(&card),
                None => {
                    eprintln!("no indexed symbol matches {query}");
                    Ok(ExitCode::FAILURE)
                }
            }
        }
        MemoryCommands::Trace {
            query,
            direction,
            depth,
        } => {
            let store = load_memory(root)?;
            match trace_path(root, &store, &query, direction.into(), depth)? {
                Some(report) => print_json(&report),
                None => {
                    eprintln!("no indexed symbol matches {query}");
                    Ok(ExitCode::FAILURE)
                }
            }
        }
        MemoryCommands::Query { query, limit } => {
            let store = load_memory(root)?;
            print_json(&run_query(root, &store, &query, limit)?)
        }
        MemoryCommands::Impact { base } => {
            let store = load_memory(root)?;
            print_json(&analyze_impact(root, &store, base.as_deref())?)
        }
        MemoryCommands::Architecture => {
            let store = load_memory(root)?;
            print_json(&architecture(root, &store)?)
        }
        MemoryCommands::Stats => print_json(&MemoryStats::load(root)),
        MemoryCommands::Projects => print_json(&list_projects()?),
        MemoryCommands::Status => print_json(&index_status(root)?),
        MemoryCommands::Delete { yes } => {
            if !yes {
                bail!("pass --yes to delete the index for {}", root.display());
            }
            print_json(&serde_json::json!({ "deleted": delete_project(root)? }))
        }
        MemoryCommands::Watch {
            interval,
            iterations,
        } => {
            let options = WatchOptions {
                interval: std::time::Duration::from_secs(interval.max(1)),
                iterations,
                base: None,
            };
            // One JSON object per changed tick keeps the stream parseable by a
            // harness that is tailing it.
            watch_repository(root, &config, &options, |tick| {
                if tick.changed {
                    if let Ok(line) = serde_json::to_string(tick) {
                        println!("{line}");
                    }
                }
            })?;
            Ok(ExitCode::SUCCESS)
        }
        MemoryCommands::Export { output, best } => {
            let level = if best { BEST_LEVEL } else { FAST_LEVEL };
            print_json(&export_artifact(root, output.as_deref(), level)?)
        }
        MemoryCommands::Servers => {
            let servers = available_servers()
                .iter()
                .map(|server| {
                    serde_json::json!({
                        "family": server.family,
                        "command": server.command,
                        "args": server.args,
                    })
                })
                .collect::<Vec<_>>();
            print_json(&serde_json::json!({ "installed": servers }))
        }
        MemoryCommands::Import => {
            let mut store = Store::open(root)?;
            let imported = forgeguard_core::memory::import_artifact(root, &mut store)?;
            if !imported {
                bail!(
                    "no graph artifact at {}",
                    root.join(ARTIFACT_FILE).display()
                );
            }
            let report = index_repository(root, &config, &IndexOptions::default())?;
            print_json(&report)
        }
    }
}

/// Answer from a graph that matches the working tree, the way the MCP surface
/// already does, so a CLI caller never reads a stale answer either.
fn load_memory(root: &Path) -> Result<Store> {
    let config = ForgeGuardConfig::scan_settings(root)?;
    let store = ensure_current(root, &config)?;
    if store.is_empty()? {
        bail!("no code memory index; run `forgeguard memory index` first");
    }
    Ok(store)
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<ExitCode> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(ExitCode::SUCCESS)
}

// forgeguard: allow FG-CPLX-001 -- legacy update flow is unchanged; this PR only adjusts unrelated CLI paths
fn execute_update(
    root: &Path,
    check: bool,
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
    let update_info = forgeguard_core::update::check_for_update_for(&home, true, VERSION);

    if check {
        match update_info {
            Some(info) if info.update_available => {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "update_available",
                            "current": info.current,
                            "latest": info.latest,
                            "update_available": true,
                        })
                    );
                } else {
                    println!(
                        "A newer version of ForgeGuard is available: {} (current: {}).",
                        info.latest, info.current
                    );
                    println!("Run `forgeguard update` to install the update.");
                }
            }
            Some(info) => {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "up_to_date",
                            "version": info.current,
                            "update_available": false,
                        })
                    );
                } else {
                    println!("ForgeGuard {} is up to date.", info.current);
                }
            }
            None => {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "unknown",
                            "version": VERSION,
                            "update_available": false,
                        })
                    );
                } else {
                    println!(
                        "ForgeGuard {VERSION} is up to date (could not check remote release)."
                    );
                }
            }
        }
        return Ok(ExitCode::SUCCESS);
    }

    match update_info {
        Some(info) if info.update_available => {
            if !json {
                println!(
                    "Updating ForgeGuard from v{} to v{}...",
                    info.current, info.latest
                );
            }
            let status = forgeguard_core::update::run_install_command()?;
            if status.success() {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "updated",
                            "from": info.current,
                            "to": info.latest,
                        })
                    );
                } else {
                    println!("ForgeGuard successfully updated to v{}.", info.latest);
                }
                Ok(ExitCode::SUCCESS)
            } else {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Update installer exited with {status}"),
                        })
                    );
                } else {
                    eprintln!("ForgeGuard update command exited with {status}.");
                }
                Ok(ExitCode::from(1))
            }
        }
        Some(info) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "up_to_date",
                        "version": info.current,
                    })
                );
            } else {
                println!("ForgeGuard {} is up to date.", info.current);
            }
            Ok(ExitCode::SUCCESS)
        }
        None => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "up_to_date",
                        "version": VERSION,
                    })
                );
            } else {
                println!("ForgeGuard {VERSION} is up to date (could not check remote release).");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn execute_mode(root: &Path, mode: Option<ModeArg>, global: bool, json: bool) -> Result<ExitCode> {
    if global {
        bail!(
            "`forgeguard mode --global` is no longer supported; lite/default/strict apply only to Code Guard repositories. Run `forgeguard mode <mode>` inside an initialized repository"
        );
    }
    if !root.join(CONFIG_FILE).is_file() {
        bail!("Code Guard mode requires an initialized repository; run `forgeguard init` first");
    }
    let mut config = ForgeGuardConfig::load(root)?;
    let selected = match mode {
        Some(mode) => mode.into(),
        None if io::stdin().is_terminal() && !json => prompt_for_mode(config.mode)?,
        None => config.mode,
    };
    config.mode = selected;
    config.save(root)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "scope": "project",
                "mode": config.mode.as_str(),
            })
        );
    } else {
        println!("ForgeGuard project mode set to {}.", config.mode.as_str());
    }
    Ok(ExitCode::SUCCESS)
}

fn configure_mode_interactive(target: &Path) -> Result<()> {
    let mut config = ForgeGuardConfig::load(target)?;
    println!();
    let mode = prompt_for_mode(config.mode)?;
    config.mode = mode;
    config.save(target)?;
    println!("ForgeGuard project mode set to {}.", config.mode.as_str());
    Ok(())
}

fn prompt_for_mode(default: GuardMode) -> Result<GuardMode> {
    println!("Code Guard mode");
    println!("  1) default - token-friendly; report static findings, block only failed required commands");
    println!("  2) lite    - static report-only; required command failures still block");
    println!("  3) strict  - block failed required commands and warning/error findings");
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

fn execute_stop_hook(root: &Path, agent: HookAgent, global: bool) -> Result<ExitCode> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read hook input")?;
    if global && !is_general_hook_invocation(root, &input)? {
        return Ok(ExitCode::SUCCESS);
    }
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

fn execute_context_hook(root: &Path, agent: HookAgent, global: bool) -> Result<ExitCode> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read hook input")?;
    if global && !is_general_hook_invocation(root, &input)? {
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(context) = evaluate_context_hook(root, &input, agent)? {
        println!("{}", render_context_hook(agent, &input, &context));
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_scope_hook(root: &Path, agent: HookAgent, global: bool) -> Result<ExitCode> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read hook input")?;
    if global && !is_general_hook_invocation(root, &input)? {
        return Ok(ExitCode::SUCCESS);
    }
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
            profile,
            scopes,
            resources,
            semantic,
            metric,
            baseline,
            target,
            guardrails,
            verifications,
            todos,
            acceptance_criteria,
            json,
        } => (
            start_task_with_profile(
                root,
                &session,
                &objective,
                &scopes,
                &resources,
                semantic,
                GoalContract {
                    metric,
                    baseline,
                    target,
                    guardrails,
                    verifications,
                },
                &todos,
                TaskProfile::new(&profile)?,
                &acceptance_criteria,
            )?,
            json,
        ),
        TaskCommands::Ready {
            session,
            evidence,
            source,
            artifacts,
            criteria,
            confidence,
            json,
        } => (
            mark_task_ready_with_evidence(
                root,
                &session,
                &evidence,
                confidence,
                source.as_deref(),
                &artifacts,
                &criteria,
            )?,
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
            AgentArg::Hermes => Self::Hermes,
            AgentArg::OpenClaw => Self::OpenClaw,
            AgentArg::Omp => Self::Omp,
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
            HookAgentArg::OpenClaw => Self::OpenClaw,
            HookAgentArg::OpenCode => Self::OpenCode,
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
    ("hermes", AgentTarget::Hermes),
    ("openclaw", AgentTarget::OpenClaw),
    ("omp", AgentTarget::Omp),
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
    ("hermes", "AGENTS.md, native skill"),
    ("openclaw", "AGENTS.md, native skill"),
    ("omp", "AGENTS.md, shared skill"),
    ("antigravity", ".agents/rules, shared skill, Stop hook"),
    ("windsurf", "AGENTS.md only"),
    ("copilot", "AGENTS.md only"),
    ("cline", "AGENTS.md only"),
    ("roo", "AGENTS.md only"),
];

const SCOPE_PROJECT: &str = "This repository";
const SCOPE_GLOBAL: &str = "Global (user directory)";

/// What the install has to do, whether it came from the wizard or from flags.
struct WizardChoices {
    use_global: bool,
    agents: Vec<AgentTarget>,
    add_gitignore: bool,
    index_now: bool,
    register_mcp: bool,
}

impl WizardChoices {
    /// Flags decided everything: install and nothing else, so scripts and CI
    /// keep the behaviour they had before the wizard asked these questions.
    fn plain(use_global: bool, agents: Vec<AgentTarget>) -> Self {
        Self {
            use_global,
            agents,
            add_gitignore: false,
            index_now: false,
            register_mcp: false,
        }
    }
}

/// `--index`/`--no-index` style pairs: a flag answers, nothing leaves it open.
const fn flag_choice(yes: bool, no: bool) -> Option<bool> {
    match (yes, no) {
        (true, _) => Some(true),
        (_, true) => Some(false),
        _ => None,
    }
}

const fn should_run_init_wizard(
    no_agents: bool,
    global: bool,
    json: bool,
    stdin_terminal: bool,
    stdout_terminal: bool,
) -> bool {
    no_agents && !global && !json && stdin_terminal && stdout_terminal
}

fn confirm(question: &str, help: &str) -> Result<bool> {
    inquire::Confirm::new(question)
        .with_default(true)
        .with_help_message(help)
        .with_render_config(theme::render_config())
        .prompt()
        .context("init wizard cancelled")
}

fn run_init_wizard(
    root: &Path,
    index_flag: Option<bool>,
    mcp_flag: Option<bool>,
) -> Result<WizardChoices> {
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

    println!(
        "{}",
        theme::point(
            "Picker: ↑/↓ move the highlight; Space toggles a checkbox; Enter confirms checked agents.",
            theme::ACCENT,
        )
    );
    println!(
        "{}",
        theme::point(
            "Skip the picker with `forgeguard init --agent <name>` (for example, `--agent opencode`).",
            theme::ACCENT,
        )
    );
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

    // Only a project checkout has code to index, and the graph is what lets an
    // agent ask for a symbol instead of reading whole files.
    let (index_now, register_mcp) = if use_global {
        (false, false)
    } else {
        let index_now = match index_flag {
            Some(answer) => answer,
            None => confirm(
                "Build the code memory index now?",
                "lets agents query symbols instead of reading files",
            )?,
        };
        let register_mcp = match mcp_flag {
            Some(answer) => answer,
            None => confirm(
                "Register the ForgeGuard MCP server here?",
                "writes this repository's MCP config for the agents you picked",
            )?,
        };
        (index_now, register_mcp)
    };

    println!();
    Ok(WizardChoices {
        use_global,
        agents,
        add_gitignore,
        index_now,
        register_mcp,
    })
}

/// Build the code graph right after install, so the first agent session can
/// query symbols instead of paying to read files.
fn build_initial_memory(root: &Path, quiet: bool) -> Result<()> {
    let config = ForgeGuardConfig::scan_settings(root)?;
    if quiet {
        index_repository(root, &config, &IndexOptions::default())?;
        return Ok(());
    }
    println!("{}", theme::point("indexing code memory…", theme::ACCENT));
    let report = index_repository(root, &config, &IndexOptions::default())?;
    println!(
        "{}",
        theme::point(
            &format!(
                "indexed {} symbols across {} files in {}ms",
                report.symbols, report.files, report.duration_millis
            ),
            theme::ACCENT,
        )
    );
    Ok(())
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

    let help = "↑↓ navigate · Space toggles checked state · → all · ← none · Enter confirms";
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
        let picked = inquire::MultiSelect::new(
            "Select agents (checked agents will be installed)",
            rows.clone(),
        )
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

    /// The shield-and-hammer mark from `assets/brand/logo-mark.svg`, sampled to
    /// twelve by twelve. `g` is the shield, `d` the hammer struck through it, a
    /// space is outside the mark. Regenerate with `sh tests/logo.sh`.
    const LOGO: [&str; 12] = [
        "gdgggggggggg",
        "dddggggggggg",
        "ggddddgggggg",
        "ggggggdggggg",
        "gggggggdgggg",
        "gggggggdgggg",
        "ggggdddggggg",
        "ggggdggggggg",
        " ggggdddddg ",
        "  ggggggdg  ",
        "   gggggd   ",
        "     gg     ",
    ];
    const SHIELD: &str = "\x1b[38;5;114m";
    const SHIELD_BG: &str = "\x1b[48;5;114m";
    const STRUCK: &str = "\x1b[38;5;234m";
    const STRUCK_BG: &str = "\x1b[48;5;234m";
    const DEFAULT_BG: &str = "\x1b[49m";

    /// Draw the mark two pixel rows per line: `▀` paints the upper half in the
    /// foreground and the lower half in the background, so a text cell carries
    /// two pixels. Anything outside the mark keeps the terminal's own
    /// background rather than punching a coloured hole in it.
    fn logo_rows() -> Vec<String> {
        let cell = |upper: u8, lower: u8| match (upper, lower) {
            (b' ', b' ') => " ".to_owned(),
            (b' ', lower) => {
                let colour = if lower == b'g' { SHIELD } else { STRUCK };
                format!("{colour}{DEFAULT_BG}▄{RESET}")
            }
            (upper, b' ') => {
                let colour = if upper == b'g' { SHIELD } else { STRUCK };
                format!("{colour}{DEFAULT_BG}▀{RESET}")
            }
            (upper, lower) => {
                let top = if upper == b'g' { SHIELD } else { STRUCK };
                let bottom = if lower == b'g' { SHIELD_BG } else { STRUCK_BG };
                format!("{top}{bottom}▀{RESET}")
            }
        };
        LOGO.chunks(2)
            .map(|pair| {
                let upper = pair[0].as_bytes();
                let lower = pair[1].as_bytes();
                (0..upper.len())
                    .map(|column| cell(upper[column], lower[column]))
                    .collect()
            })
            .collect()
    }

    /// The mark beside the wordmark, split the way the logo splits it: `Forge`
    /// plain, `Guard` in the accent. Falls back to plain text whenever colour is
    /// off, because the mark is made of colour and would otherwise be a smear of
    /// half-blocks.
    pub(super) fn banner() -> String {
        if !enabled() {
            return "ForgeGuard".to_owned();
        }
        let wordmark = format!("{BOLD_FG}Forge{RESET}{ACCENT}Guard{RESET}");
        logo_rows()
            .into_iter()
            .enumerate()
            .map(|(index, row)| {
                if index == 2 {
                    format!("  {row}   {wordmark}")
                } else {
                    format!("  {row}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
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
        // The title sits inside the top border and needs at least one dash after
        // it, so it claims a column more than a body line of the same length.
        // Without that the top border runs one character past the sides.
        let width = lines
            .iter()
            .map(|line| line.chars().count())
            .chain(std::iter::once(title.chars().count() + 1))
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

/// A newer release ships newer policy and skill files, but the copies on disk
/// may also carry edits the user made. Replacing them is therefore the user's
/// call, not the installer's: name every file, ask, and default to keeping them
/// so a stray Enter never costs anyone their work.
///
/// Returns whether the caller should reinstall with `refresh` set.
/// Files ForgeGuard found already written by the user. It never rewrites these,
/// so the only thing left to do is say so — silence would read as "installed".
fn report_kept(kept: &[String]) {
    if kept.is_empty() {
        return;
    }
    println!("{}", theme::panel(kept, "yours — left as-is", theme::AMBER));
}

fn offer_refresh(outdated: &[String]) -> Result<bool> {
    if outdated.is_empty() {
        return Ok(false);
    }
    let summary = format!(
        "{} ForgeGuard file{} differ{} from this version",
        outdated.len(),
        if outdated.len() == 1 { "" } else { "s" },
        if outdated.len() == 1 { "s" } else { "" },
    );

    // Without a terminal there is nobody to ask, so say what is stale and how to
    // act on it rather than overwriting on the user's behalf.
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        println!(
            "{}",
            theme::panel(
                outdated,
                "outdated — run `forgeguard init --refresh`",
                theme::AMBER
            )
        );
        return Ok(false);
    }

    println!("{}", theme::panel(outdated, &summary, theme::AMBER));
    inquire::Confirm::new("Replace them with the bundled versions?")
        .with_default(false)
        .with_help_message(
            "your edits to these files would be lost; configuration is never touched",
        )
        .with_render_config(theme::render_config())
        .prompt()
        .context("refresh prompt cancelled")
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
    use std::{
        fs, process,
        time::{SystemTime, UNIX_EPOCH},
    };

    use clap::Parser;
    use forgeguard_core::{config::ForgeGuardConfig, GuardMode};

    use super::{
        agent_menu_rows, agents_from_names, agents_from_rows, execute_mode, should_run_init_wizard,
        summarize_paths, AgentTarget, BaselineCommands, Cli, Commands, ConfigCommands,
        HookCommands, McpCommands, ModeArg, OutputArg,
    };

    fn temporary_project(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "forgeguard-{label}-{}-{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should follow the Unix epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn code_guard_mode_requires_an_initialized_repository() {
        let root = temporary_project("uninitialized-mode");
        fs::create_dir_all(&root).expect("temporary directory should be created");

        let error = execute_mode(&root, Some(ModeArg::Strict), false, true)
            .expect_err("an uninitialized repository must be rejected");

        assert!(error.to_string().contains("run `forgeguard init` first"));
        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn code_guard_mode_is_saved_only_in_the_repository() {
        let root = temporary_project("repository-mode");
        fs::create_dir_all(&root).expect("temporary directory should be created");
        ForgeGuardConfig::new("mode-test", vec![])
            .save(&root)
            .expect("project config should be created");

        execute_mode(&root, Some(ModeArg::Strict), false, true)
            .expect("repository mode should be updated");

        assert_eq!(
            ForgeGuardConfig::load(&root)
                .expect("project config should load")
                .mode,
            GuardMode::Strict
        );
        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn legacy_global_mode_flag_is_parseable_but_rejected() {
        let cli = Cli::try_parse_from(["forgeguard", "mode", "strict", "--global"])
            .expect("legacy command shape should remain parseable");
        assert!(matches!(cli.command, Commands::Mode { global: true, .. }));

        let error = execute_mode(std::path::Path::new("."), Some(ModeArg::Strict), true, true)
            .expect_err("global Code Guard mode must be rejected");
        assert!(error.to_string().contains("Code Guard repositories"));
    }

    #[test]
    fn global_lifecycle_hook_scope_is_parseable() {
        for action in ["stop", "context", "scope"] {
            let cli =
                Cli::try_parse_from(["forgeguard", "hook", action, "--agent", "codex", "--global"])
                    .expect("global hook command should parse");
            assert!(matches!(
                cli.command,
                Commands::Hook {
                    command: HookCommands::Stop { global: true, .. }
                        | HookCommands::Context { global: true, .. }
                        | HookCommands::Scope { global: true, .. }
                }
            ));
        }
    }

    #[test]
    fn maps_checked_names_in_menu_order() {
        assert_eq!(
            agents_from_names(&["openclaw", "cursor", "codex", "hermes"]),
            vec![
                AgentTarget::Codex,
                AgentTarget::Cursor,
                AgentTarget::Hermes,
                AgentTarget::OpenClaw,
            ]
        );
    }

    #[test]
    fn empty_pick_installs_nothing() {
        assert!(agents_from_names(&[]).is_empty());
    }

    #[test]
    fn init_wizard_requires_input_and_output_terminals() {
        assert!(should_run_init_wizard(true, false, false, true, true));
        assert!(!should_run_init_wizard(true, false, false, false, true));
        assert!(!should_run_init_wizard(true, false, false, true, false));
    }

    #[test]
    fn menu_rows_pair_each_agent_with_what_it_writes() {
        let rows = agent_menu_rows();

        assert!(rows[1].starts_with("claude "));
        assert!(rows[1].ends_with("CLAUDE.md, own skill, Stop hook"));
        assert!(rows.iter().all(|row| row.contains("  ")));
    }

    #[test]
    fn every_menu_entry_declares_what_it_writes() {
        // agent_menu_rows falls back to an empty summary, so a target added to
        // AGENT_MENU without an AGENT_SUMMARY entry would render a blank column
        // instead of failing. Catch that here.
        for (name, _) in super::AGENT_MENU {
            assert!(
                super::AGENT_SUMMARY
                    .iter()
                    .any(|(key, summary)| key == name && !summary.is_empty()),
                "{name} has no menu summary"
            );
        }
    }

    #[test]
    fn menu_rows_round_trip_back_to_targets() {
        let rows = agent_menu_rows();
        let picked = vec![rows[1].clone(), rows[9].clone()];

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

        let review = Cli::try_parse_from(["forgeguard", "review", "--base", "origin/main"])
            .expect("parse base review");
        assert!(matches!(
            review.command,
            Commands::Review {
                base: Some(ref base),
                ..
            } if base == "origin/main"
        ));
    }

    #[test]
    fn parses_mcp_serve_and_register() {
        let serve = Cli::try_parse_from(["forgeguard", "mcp", "serve"]).expect("parse mcp serve");
        assert!(matches!(
            serve.command,
            Commands::Mcp {
                command: McpCommands::Serve
            }
        ));

        let register = Cli::try_parse_from([
            "forgeguard",
            "mcp",
            "register",
            "--client",
            "claude-code",
            "--project",
            "--dry-run",
        ])
        .expect("parse mcp register");
        assert!(matches!(
            register.command,
            Commands::Mcp {
                command: McpCommands::Register {
                    ref client,
                    project: true,
                    dry_run: true,
                    force: false,
                }
            } if client == "claude-code"
        ));
        assert!(Cli::try_parse_from(["forgeguard", "mcp", "register"]).is_err());
    }

    #[test]
    fn parses_cross_role_task_contract_and_evidence() {
        let start = Cli::try_parse_from([
            "forgeguard",
            "task",
            "start",
            "--session",
            "qa-session",
            "--objective",
            "Verify checkout",
            "--profile",
            "qa",
            "--resource",
            "mcp:playwright",
            "--acceptance",
            "guest checkout succeeds",
        ])
        .expect("parse role task");
        assert!(matches!(start.command, Commands::Task { .. }));

        Cli::try_parse_from([
            "forgeguard",
            "task",
            "ready",
            "--session",
            "qa-session",
            "--evidence",
            "trace captured",
            "--source",
            "mcp:playwright",
            "--artifact",
            "artifact:trace.zip",
            "--criterion",
            "1",
        ])
        .expect("parse structured evidence");
    }
}
