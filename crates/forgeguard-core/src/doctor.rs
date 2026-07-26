use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    config::{ForgeGuardConfig, CONFIG_FILE},
    init::{
        ANTIGRAVITY_HOOK_COMMAND, CLAUDE_HOOK_COMMAND, CODEX_HOOK_COMMAND, CURSOR_HOOK_COMMAND,
        LEGACY_SKILL_NAMES,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStatus {
    pub tool: String,
    pub available: bool,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub configuration_found: bool,
    pub git_repository: bool,
    pub tools: Vec<ToolStatus>,
    pub hooks: Vec<HookStatus>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub healthy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookStatus {
    pub agent: String,
    pub installed: bool,
    pub configured: bool,
    pub path: PathBuf,
}

pub fn run_doctor(root: &Path, config: Option<&ForgeGuardConfig>) -> Result<DoctorReport> {
    let configuration_found = root.join(CONFIG_FILE).exists();
    let git_repository = root.join(".git").exists();
    let mut tool_names = vec!["git".to_owned()];
    if let Some(config) = config {
        for command in &config.commands {
            if let Some(tool) = first_command_token(&command.command) {
                tool_names.push(tool.to_owned());
            }
        }
    }
    tool_names.sort();
    tool_names.dedup();

    let tools: Vec<ToolStatus> = tool_names
        .into_iter()
        .map(|tool| {
            let path = find_in_path(&tool);
            ToolStatus {
                tool,
                available: path.is_some(),
                path,
            }
        })
        .collect();
    let healthy =
        configuration_found && git_repository && tools.iter().all(|status| status.available);
    let hooks = hook_statuses(root);
    let mut warnings = skill_warnings(root);
    warnings.extend(hook_warnings(&hooks));

    Ok(DoctorReport {
        configuration_found,
        git_repository,
        tools,
        hooks,
        warnings,
        healthy,
    })
}

fn hook_statuses(root: &Path) -> Vec<HookStatus> {
    let shared_skill = root
        .join(".agents/skills/forgeguard-engineering/SKILL.md")
        .is_file();
    [
        (
            "codex",
            ".codex/hooks.json",
            CODEX_HOOK_COMMAND,
            shared_skill && root.join("AGENTS.md").is_file(),
        ),
        (
            "claude",
            ".claude/settings.json",
            CLAUDE_HOOK_COMMAND,
            root.join(".claude/skills/forgeguard-engineering/SKILL.md")
                .is_file(),
        ),
        (
            "cursor",
            ".cursor/hooks.json",
            CURSOR_HOOK_COMMAND,
            shared_skill && root.join(".cursor/rules/forgeguard.mdc").is_file(),
        ),
        (
            "antigravity",
            ".agents/hooks.json",
            ANTIGRAVITY_HOOK_COMMAND,
            shared_skill && root.join(".agents/rules/forgeguard.md").is_file(),
        ),
    ]
    .into_iter()
    .map(|(agent, relative, command, installed)| {
        let path = root.join(relative);
        let configured = fs_contains(&path, command);
        HookStatus {
            agent: agent.to_owned(),
            installed,
            configured,
            path,
        }
    })
    .collect()
}

fn hook_warnings(hooks: &[HookStatus]) -> Vec<String> {
    hooks
        .iter()
        .filter(|status| status.installed && !status.configured)
        .map(|status| {
            format!(
                "ForgeGuard {} hook missing; run `forgeguard init --agent {}`.",
                status.agent, status.agent
            )
        })
        .collect()
}

fn fs_contains(path: &Path, expected: &str) -> bool {
    std::fs::read_to_string(path).is_ok_and(|source| source.contains(expected))
}

fn skill_warnings(root: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    let legacy_codex = LEGACY_SKILL_NAMES
        .iter()
        .copied()
        .chain(Some("engineering"))
        .any(|name| {
            root.join(format!(".codex/skills/forgeguard-{name}"))
                .exists()
        });
    if legacy_codex {
        warnings.push(
            "Legacy Codex ForgeGuard skills found; run `forgeguard init --agent codex --force`."
                .to_owned(),
        );
    }

    let legacy_claude = LEGACY_SKILL_NAMES.iter().any(|name| {
        root.join(format!(".claude/skills/forgeguard-{name}"))
            .exists()
    });
    if legacy_claude {
        warnings.push(
            "Legacy Claude ForgeGuard skills found; run `forgeguard init --agent claude --force`."
                .to_owned(),
        );
    }
    warnings
}

fn first_command_token(command: &str) -> Option<&str> {
    command
        .split_whitespace()
        .find(|token| !token.contains('=') && *token != "env" && *token != "test")
        .map(|token| token.trim_matches(&['\'', '"'][..]))
}

fn find_in_path(tool: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .flat_map(|directory| candidate_paths(&directory, tool))
        .find(|candidate| candidate.is_file())
}

fn candidate_paths(directory: &Path, tool: &str) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        vec![
            directory.join(tool),
            directory.join(format!("{tool}.exe")),
            directory.join(format!("{tool}.cmd")),
            directory.join(format!("{tool}.bat")),
        ]
    }
    #[cfg(not(windows))]
    {
        vec![directory.join(tool)]
    }
}
