use std::fs;

use forgeguard_core::{initialize_project, AgentTarget, InitOptions};
use tempfile::tempdir;

#[test]
fn installs_configuration_for_all_supported_agents() {
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
            agents: vec![AgentTarget::All],
        },
    )
    .expect("initialize project");

    assert!(directory.path().join(".forgeguard/config.toml").exists());
    assert!(directory.path().join("AGENTS.md").exists());
    assert!(directory.path().join("CLAUDE.md").exists());
    assert!(directory
        .path()
        .join(".cursor/rules/forgeguard.mdc")
        .exists());
    assert!(directory.path().join(".codex/hooks.json").exists());
    assert!(directory.path().join(".claude/settings.json").exists());
    assert!(directory.path().join(".cursor/hooks.json").exists());
    assert!(directory
        .path()
        .join(".agents/rules/forgeguard.md")
        .exists());
    assert!(directory.path().join(".agents/hooks.json").exists());
    assert_eq!(
        fs::read_to_string(directory.path().join(".forgeguard/.gitignore"))
            .expect("read ForgeGuard gitignore"),
        "cache/\nreports/\n"
    );
    assert!(directory
        .path()
        .join(".agents/skills/forgeguard-engineering/SKILL.md")
        .exists());
    assert!(directory
        .path()
        .join(".agents/skills/forgeguard-engineering/agents/openai.yaml")
        .exists());
    assert!(directory
        .path()
        .join(".claude/skills/forgeguard-engineering/references/ai.md")
        .exists());
    assert!(directory
        .path()
        .join(".agents/skills/forgeguard-engineering/references/mobile.md")
        .exists());
    let algorithm_policy = fs::read_to_string(
        directory
            .path()
            .join(".agents/skills/forgeguard-engineering/references/algorithms.md"),
    )
    .expect("read algorithm policy");
    assert!(algorithm_policy.contains("Performance-critical review output"));
    assert!(algorithm_policy.contains("Correctness → Security → Data Integrity"));
    let doctor = forgeguard_core::run_doctor(directory.path(), None).expect("run doctor");
    assert!(doctor.hooks.iter().all(|hook| hook.configured));
    assert!(!report.files_written.is_empty());
}

#[test]
fn installs_global_skills_without_project_configuration() {
    let directory = tempdir().expect("temp directory");

    let report = forgeguard_core::initialize_global(
        directory.path(),
        &InitOptions {
            force: false,
            agents: vec![AgentTarget::All],
        },
    )
    .expect("install global skills");

    assert!(directory.path().join(".codex/AGENTS.md").exists());
    assert!(directory.path().join(".claude/CLAUDE.md").exists());
    assert!(directory
        .path()
        .join(".cursor/rules/forgeguard.mdc")
        .exists());
    assert!(directory.path().join(".codex/hooks.json").exists());
    assert!(directory.path().join(".claude/settings.json").exists());
    assert!(directory.path().join(".cursor/hooks.json").exists());
    assert!(directory
        .path()
        .join(".agents/skills/forgeguard-engineering/SKILL.md")
        .exists());
    assert!(directory
        .path()
        .join(".claude/skills/forgeguard-engineering/SKILL.md")
        .exists());
    assert!(directory.path().join(".config/opencode/AGENTS.md").exists());
    assert!(directory
        .path()
        .join(".config/opencode/skills/forgeguard-engineering/SKILL.md")
        .exists());
    assert!(directory.path().join(".gemini/GEMINI.md").exists());
    assert!(directory
        .path()
        .join(".gemini/config/skills/forgeguard-engineering/SKILL.md")
        .exists());
    assert!(directory.path().join(".gemini/config/hooks.json").exists());
    assert!(!directory.path().join(".forgeguard/config.toml").exists());
    assert!(!report.files_written.is_empty());
}

#[test]
fn hook_install_preserves_existing_settings_and_is_idempotent() {
    let directory = tempdir().expect("temp directory");
    let settings = directory.path().join(".claude/settings.json");
    fs::create_dir_all(settings.parent().expect("settings parent")).expect("create settings");
    fs::write(
        &settings,
        r#"{"permissions":{"allow":["Bash(git status)"]},"hooks":{"Stop":[]}}"#,
    )
    .expect("write settings");

    let options = InitOptions {
        force: false,
        agents: vec![AgentTarget::Claude],
    };
    initialize_project(directory.path(), &options).expect("first initialization");
    initialize_project(directory.path(), &options).expect("second initialization");

    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(settings).expect("read merged settings"))
            .expect("parse merged settings");
    assert_eq!(
        value["permissions"]["allow"][0],
        serde_json::Value::String("Bash(git status)".to_owned())
    );
    let source = value.to_string();
    assert_eq!(
        source
            .matches("forgeguard hook stop --agent claude")
            .count(),
        1
    );
}

#[test]
fn force_removes_only_legacy_forgeguard_skills() {
    let directory = tempdir().expect("temp directory");
    let legacy = directory.path().join(".codex/skills/forgeguard-clean-code");
    let unrelated = directory.path().join(".codex/skills/custom");
    fs::create_dir_all(&legacy).expect("create legacy skill");
    fs::create_dir_all(&unrelated).expect("create unrelated skill");
    fs::write(legacy.join("SKILL.md"), "legacy").expect("write legacy skill");
    fs::write(unrelated.join("SKILL.md"), "custom").expect("write unrelated skill");

    forgeguard_core::initialize_global(
        directory.path(),
        &InitOptions {
            force: true,
            agents: vec![AgentTarget::Codex],
        },
    )
    .expect("migrate global skills");

    assert!(!legacy.exists());
    assert!(unrelated.join("SKILL.md").exists());
    assert!(directory
        .path()
        .join(".agents/skills/forgeguard-engineering/SKILL.md")
        .exists());
}

#[test]
fn existing_policy_is_preserved_without_force() {
    let directory = tempdir().expect("temp directory");
    fs::write(directory.path().join("AGENTS.md"), "custom policy\n").expect("write policy");

    initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            agents: vec![AgentTarget::Codex],
        },
    )
    .expect("initialize project");

    assert_eq!(
        fs::read_to_string(directory.path().join("AGENTS.md")).expect("read policy"),
        "custom policy\n"
    );
}

#[test]
fn opencode_target_uses_shared_standards_without_unrelated_hooks() {
    let directory = tempdir().expect("temp directory");

    initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            agents: vec![AgentTarget::OpenCode],
        },
    )
    .expect("initialize OpenCode");

    assert!(directory.path().join("AGENTS.md").exists());
    assert!(directory
        .path()
        .join(".agents/skills/forgeguard-engineering/SKILL.md")
        .exists());
    assert!(!directory.path().join(".codex/hooks.json").exists());
    assert!(!directory.path().join(".agents/hooks.json").exists());
}

#[test]
fn antigravity_hook_merge_is_idempotent() {
    let directory = tempdir().expect("temp directory");
    let hooks = directory.path().join(".agents/hooks.json");
    fs::create_dir_all(hooks.parent().expect("hook parent")).expect("create hook directory");
    fs::write(
        &hooks,
        r#"{"existing":{"enabled":true,"Stop":[{"command":"existing-check"}]}}"#,
    )
    .expect("write hooks");
    let options = InitOptions {
        force: false,
        agents: vec![AgentTarget::Antigravity],
    };

    initialize_project(directory.path(), &options).expect("first initialization");
    initialize_project(directory.path(), &options).expect("second initialization");

    let source = fs::read_to_string(hooks).expect("read hooks");
    let value: serde_json::Value = serde_json::from_str(&source).expect("parse hooks");
    assert_eq!(
        value["existing"]["Stop"][0]["command"],
        serde_json::Value::String("existing-check".to_owned())
    );
    assert_eq!(
        source
            .matches("forgeguard hook stop --agent antigravity")
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn force_unlinks_legacy_symlink_without_removing_target() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temp directory");
    let target = directory.path().join("user-owned");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "keep").expect("write target");
    let legacy = directory.path().join(".codex/skills/forgeguard-clean-code");
    fs::create_dir_all(legacy.parent().expect("legacy parent")).expect("create skills directory");
    symlink(&target, &legacy).expect("create legacy symlink");

    forgeguard_core::initialize_global(
        directory.path(),
        &InitOptions {
            force: true,
            agents: vec![AgentTarget::Codex],
        },
    )
    .expect("migrate global skills");

    assert!(!legacy.exists());
    assert!(target.join("keep.txt").exists());
}
