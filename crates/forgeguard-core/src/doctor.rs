use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::{ForgeGuardConfig, CONFIG_FILE};

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
    pub healthy: bool,
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
    let healthy = configuration_found
        && git_repository
        && tools.iter().all(|status| status.available);

    Ok(DoctorReport {
        configuration_found,
        git_repository,
        tools,
        healthy,
    })
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
