use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{config::ForgeGuardConfig, detect_project, ProjectDetection};

const AGENTS_TEMPLATE: &str = include_str!("../assets/templates/AGENTS.md");
const CLAUDE_TEMPLATE: &str = include_str!("../assets/templates/CLAUDE.md");

const SKILLS: &[(&str, &str)] = &[
    (
        "algorithm-engineering",
        include_str!("../assets/skills/algorithm-engineering/SKILL.md"),
    ),
    (
        "clean-code",
        include_str!("../assets/skills/clean-code/SKILL.md"),
    ),
    (
        "backend-engineering",
        include_str!("../assets/skills/backend-engineering/SKILL.md"),
    ),
    (
        "frontend-engineering",
        include_str!("../assets/skills/frontend-engineering/SKILL.md"),
    ),
    (
        "database-engineering",
        include_str!("../assets/skills/database-engineering/SKILL.md"),
    ),
    (
        "ai-engineering",
        include_str!("../assets/skills/ai-engineering/SKILL.md"),
    ),
    (
        "testing-verification",
        include_str!("../assets/skills/testing-verification/SKILL.md"),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentTarget {
    Codex,
    Claude,
    All,
}

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub force: bool,
    pub agent: AgentTarget,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            force: false,
            agent: AgentTarget::All,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitReport {
    pub detection: ProjectDetection,
    pub files_written: Vec<String>,
    pub files_skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalInitReport {
    pub home: PathBuf,
    pub files_written: Vec<String>,
    pub files_skipped: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum InstallScope {
    Project,
    Global,
}

pub fn initialize_global(home: &Path, options: &InitOptions) -> Result<GlobalInitReport> {
    let home = home
        .canonicalize()
        .with_context(|| format!("failed to resolve home directory {}", home.display()))?;
    let mut files_written = Vec::new();
    let mut files_skipped = Vec::new();

    match options.agent {
        AgentTarget::Codex => {
            install_codex(
                &home,
                InstallScope::Global,
                options.force,
                &mut files_written,
                &mut files_skipped,
            )?
        }
        AgentTarget::Claude => {
            install_claude(
                &home,
                InstallScope::Global,
                options.force,
                &mut files_written,
                &mut files_skipped,
            )?
        }
        AgentTarget::All => {
            install_codex(
                &home,
                InstallScope::Global,
                options.force,
                &mut files_written,
                &mut files_skipped,
            )?;
            install_claude(
                &home,
                InstallScope::Global,
                options.force,
                &mut files_written,
                &mut files_skipped,
            )?;
        }
    }

    Ok(GlobalInitReport {
        home,
        files_written,
        files_skipped,
    })
}

pub fn initialize_project(root: &Path, options: &InitOptions) -> Result<InitReport> {
    let detection = detect_project(root)?;
    let project_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let config = ForgeGuardConfig::new(project_name, detection.suggested_commands.clone());

    let mut files_written = Vec::new();
    let mut files_skipped = Vec::new();
    write_config(root, &config, options.force, &mut files_written, &mut files_skipped)?;

    match options.agent {
        AgentTarget::Codex => install_codex(
            root,
            InstallScope::Project,
            options.force,
            &mut files_written,
            &mut files_skipped,
        )?,
        AgentTarget::Claude => install_claude(
            root,
            InstallScope::Project,
            options.force,
            &mut files_written,
            &mut files_skipped,
        )?,
        AgentTarget::All => {
            install_codex(
                root,
                InstallScope::Project,
                options.force,
                &mut files_written,
                &mut files_skipped,
            )?;
            install_claude(
                root,
                InstallScope::Project,
                options.force,
                &mut files_written,
                &mut files_skipped,
            )?;
        }
    }

    Ok(InitReport {
        detection,
        files_written,
        files_skipped,
    })
}

fn write_config(
    root: &Path,
    config: &ForgeGuardConfig,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let path = root.join(".forgeguard/config.toml");
    let content = toml::to_string_pretty(config).context("failed to serialize ForgeGuard config")?;
    write_file(root, &path, &content, force, written, skipped)
}

fn install_codex(
    root: &Path,
    scope: InstallScope,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let policy_path = match scope {
        InstallScope::Project => root.join("AGENTS.md"),
        InstallScope::Global => root.join(".codex/AGENTS.md"),
    };
    write_file(root, &policy_path, AGENTS_TEMPLATE, force, written, skipped)?;
    for (name, content) in SKILLS {
        let path = root.join(format!(".codex/skills/forgeguard-{name}/SKILL.md"));
        write_file(root, &path, content, force, written, skipped)?;
    }
    Ok(())
}

fn install_claude(
    root: &Path,
    scope: InstallScope,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let policy_path = match scope {
        InstallScope::Project => root.join("CLAUDE.md"),
        InstallScope::Global => root.join(".claude/CLAUDE.md"),
    };
    write_file(root, &policy_path, CLAUDE_TEMPLATE, force, written, skipped)?;
    for (name, content) in SKILLS {
        let path = root.join(format!(".claude/skills/forgeguard-{name}/SKILL.md"));
        write_file(root, &path, content, force, written, skipped)?;
    }
    Ok(())
}

fn write_file(
    root: &Path,
    path: &Path,
    content: &str,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let relative = path.strip_prefix(root).unwrap_or(path).display().to_string();
    if path.exists() && !force {
        skipped.push(relative);
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid output path {}", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))?;
    if !path.exists() {
        bail!("file was not created: {}", path.display());
    }
    written.push(relative);
    Ok(())
}
