use std::{
    env,
    io::{IsTerminal, Read, Write},
    path::PathBuf,
    process::ExitCode,
};

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
    /// Check for a newer ForgeGuard release (optional; never required).
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
        Commands::Hook {
            command: HookCommands::Stop { agent },
        } => execute_stop_hook(&root, agent.into()),
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

fn execute_stop_hook(root: &std::path::Path, agent: HookAgent) -> Result<ExitCode> {
    let mut input = String::new();
    std::io::stdin()
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
    };
    let output = render_hook_decision(agent, &decision);
    if !output.is_empty() {
        println!("{output}");
    }
    Ok(ExitCode::SUCCESS)
}

fn append_update_notice(reason: String, home: Option<&std::path::Path>) -> String {
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

// ponytail: line-based prompt; add TUI multiselect only if UX complaint.
fn run_init_wizard() -> Result<(bool, Vec<AgentTarget>, bool)> {
    println!("ForgeGuard init");
    let use_global = prompt_choice(
        "Where do you want to install?",
        &["This repository (default)", "Global (user directory)"],
    )? == 1;

    println!("\nWhich agents? (comma-separated numbers, or 'all')");
    for (index, (name, _)) in AGENT_MENU.iter().enumerate() {
        println!("  {}) {name}", index + 1);
    }
    let agents = parse_agent_selection(&prompt_line("> ")?);

    // The gitignore entry only makes sense for a project checkout.
    let add_gitignore = !use_global && prompt_yes_no("Add .forgeguard/ to .gitignore?", true)?;

    Ok((use_global, agents, add_gitignore))
}

/// Map a comma-separated selection (`"1,3"`, `"all"`, empty) onto concrete
/// agents. Unknown or out-of-range entries are ignored; an empty result falls
/// back to `All` so a stray Enter never installs nothing.
fn parse_agent_selection(input: &str) -> Vec<AgentTarget> {
    let input = input.trim();
    if input.is_empty() || input.eq_ignore_ascii_case("all") {
        return vec![AgentTarget::All];
    }
    let mut selected: Vec<AgentTarget> = Vec::new();
    for token in input.split(',') {
        let token = token.trim();
        if token.eq_ignore_ascii_case("all") {
            return vec![AgentTarget::All];
        }
        let Ok(number) = token.parse::<usize>() else {
            continue;
        };
        if let Some((_, target)) = AGENT_MENU.get(number.wrapping_sub(1)) {
            if !selected.contains(target) {
                selected.push(*target);
            }
        }
    }
    if selected.is_empty() {
        vec![AgentTarget::All]
    } else {
        selected
    }
}

fn prompt_choice(question: &str, options: &[&str]) -> Result<usize> {
    println!("\n{question}");
    for (index, option) in options.iter().enumerate() {
        println!("  {}) {option}", index + 1);
    }
    loop {
        let answer = prompt_line("> ")?;
        let answer = answer.trim();
        if answer.is_empty() {
            return Ok(0);
        }
        if let Ok(number) = answer.parse::<usize>() {
            if (1..=options.len()).contains(&number) {
                return Ok(number - 1);
            }
        }
        println!("Enter a number between 1 and {}.", options.len());
    }
}

fn prompt_yes_no(question: &str, default_yes: bool) -> Result<bool> {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    loop {
        let answer = prompt_line(&format!("\n{question} {hint} "))?;
        match answer.trim().to_ascii_lowercase().as_str() {
            "" => return Ok(default_yes),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Please answer y or n."),
        }
    }
}

fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .context("failed to read input")?;
    Ok(line)
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
    use super::{parse_agent_selection, AgentTarget};

    #[test]
    fn parses_number_list() {
        assert_eq!(
            parse_agent_selection("1,3"),
            vec![AgentTarget::Codex, AgentTarget::Cursor]
        );
    }

    #[test]
    fn all_and_empty_and_garbage_default_to_all() {
        assert_eq!(parse_agent_selection("all"), vec![AgentTarget::All]);
        assert_eq!(parse_agent_selection("  "), vec![AgentTarget::All]);
        assert_eq!(parse_agent_selection("9,foo"), vec![AgentTarget::All]);
    }

    #[test]
    fn dedups_and_skips_out_of_range() {
        assert_eq!(parse_agent_selection("2,2,7"), vec![AgentTarget::Claude]);
    }
}
