use std::{env, io::Read, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use forgeguard_core::{
    config::{ForgeGuardConfig, CONFIG_FILE},
    detect_project, evaluate_stop_hook,
    git::changed_files,
    initialize_global, initialize_project, render_hook_decision,
    report::{render_detection, render_doctor, render_gate, render_gate_compact},
    run_doctor, run_gate, AgentTarget, GateOptions, GateReport, GateStatus, HookAgent,
    HookDecision, InitOptions,
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
    /// Install ForgeGuard skills globally or initialize a repository.
    Init {
        #[arg(long)]
        force: bool,
        /// Install rules, skills, and hooks for supported agents under the user directory.
        #[arg(long)]
        global: bool,
        #[arg(long, value_enum, default_value = "all")]
        agent: AgentArg,
        #[arg(long)]
        json: bool,
    },
    /// Detect languages, frameworks, database tools, tests, and quality commands.
    Detect {
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
    /// Run lifecycle adapters used by supported AI coding agents.
    Hook {
        #[command(subcommand)]
        command: HookCommands,
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
    All,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputArg {
    Full,
    Compact,
    Quiet,
}

#[derive(Debug, Subcommand)]
enum HookCommands {
    /// Verify changed code when an agent attempts to stop.
    Stop {
        #[arg(long, value_enum)]
        agent: HookAgentArg,
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
            let options = InitOptions {
                force,
                agent: agent.into(),
            };
            if global {
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
                }
            } else {
                let report = initialize_project(&root, &options)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!("ForgeGuard initialized at {}", root.display());
                    render_file_changes(&report.files_written, &report.files_skipped);
                    println!();
                    print!("{}", render_detection(&report.detection));
                }
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
        Commands::Hook {
            command: HookCommands::Stop { agent },
        } => execute_stop_hook(&root, agent.into()),
    }
}

fn execute_stop_hook(root: &std::path::Path, agent: HookAgent) -> Result<ExitCode> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("failed to read hook input")?;
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
    let output = render_hook_decision(agent, &decision);
    if !output.is_empty() {
        println!("{output}");
    }
    Ok(ExitCode::SUCCESS)
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
