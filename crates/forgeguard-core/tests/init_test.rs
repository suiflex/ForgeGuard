use std::fs;

use forgeguard_core::{initialize_project, AgentTarget, InitOptions};
use tempfile::tempdir;

#[test]
fn installs_configuration_and_both_agent_skill_sets() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write manifest");

    let report = initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            agent: AgentTarget::All,
        },
    )
    .expect("initialize project");

    assert!(directory.path().join(".forgeguard/config.toml").exists());
    assert!(directory.path().join("AGENTS.md").exists());
    assert!(directory.path().join("CLAUDE.md").exists());
    assert!(directory
        .path()
        .join(".codex/skills/forgeguard-algorithm-engineering/SKILL.md")
        .exists());
    assert!(directory
        .path()
        .join(".claude/skills/forgeguard-ai-engineering/SKILL.md")
        .exists());
    assert!(!report.files_written.is_empty());
}

#[test]
fn installs_global_skills_without_project_configuration() {
    let directory = tempdir().expect("temp directory");

    let report = forgeguard_core::initialize_global(
        directory.path(),
        &InitOptions {
            force: false,
            agent: AgentTarget::All,
        },
    )
    .expect("install global skills");

    assert!(directory.path().join(".codex/AGENTS.md").exists());
    assert!(directory.path().join(".claude/CLAUDE.md").exists());
    assert!(directory
        .path()
        .join(".codex/skills/forgeguard-algorithm-engineering/SKILL.md")
        .exists());
    assert!(!directory.path().join(".forgeguard/config.toml").exists());
    assert!(!report.files_written.is_empty());
}
