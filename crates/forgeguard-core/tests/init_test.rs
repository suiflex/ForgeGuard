use std::fs;

use forgeguard_core::{
    detect_installed_agents, initialize_project, AgentTarget, ForgeGuardConfig, InitOptions,
};
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
            refresh: false,
            agents: vec![AgentTarget::All],
        },
    )
    .expect("initialize project");

    assert!(directory.path().join(".forgeguard/config.toml").exists());
    assert!(
        forgeguard_core::ForgeGuardConfig::load(directory.path())
            .expect("load initialized config")
            .focus
            .auto_poke
    );
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
    let engineering_skill = fs::read_to_string(
        directory
            .path()
            .join(".agents/skills/forgeguard-engineering/SKILL.md"),
    )
    .expect("read engineering skill");
    assert!(engineering_skill.contains("## Reject AI slop"));
    assert!(engineering_skill.contains("Do not invent requirements, APIs, schemas"));
    assert!(engineering_skill.contains("Do not disguise incomplete work with placeholders"));
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
        .join(".agents/skills/forgeguard-engineering/references/ml.md")
        .exists());
    assert!(directory
        .path()
        .join(".claude/skills/forgeguard-engineering/references/deep-learning.md")
        .exists());
    assert!(directory
        .path()
        .join(".agents/skills/forgeguard-engineering/references/mlops.md")
        .exists());
    assert!(directory
        .path()
        .join(".agents/skills/forgeguard-engineering/references/mobile.md")
        .exists());
    assert!(!directory.path().join(".gitignore").exists());
    assert!(!directory
        .path()
        .join(".agents/skills/forgeguard-engineering/references/clean-code.md")
        .exists());
    let algorithm_policy = fs::read_to_string(
        directory
            .path()
            .join(".agents/skills/forgeguard-engineering/references/algorithms.md"),
    )
    .expect("read algorithm policy");
    assert!(algorithm_policy.contains("Bound cache size, lifetime, and concurrent fan-out"));
    assert!(algorithm_policy.contains("actual bottleneck before optimizing"));
    let doctor = forgeguard_core::run_doctor(directory.path(), None).expect("run doctor");
    assert!(doctor.hooks.iter().all(|hook| hook.configured));
    let codex_hooks: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(directory.path().join(".codex/hooks.json")).expect("read Codex hooks"),
    )
    .expect("parse Codex hooks");
    assert_eq!(
        codex_hooks["hooks"]["Stop"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        codex_hooks["hooks"]["SessionStart"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        codex_hooks["hooks"]["PreToolUse"].as_array().map(Vec::len),
        Some(1)
    );
    let claude_hooks: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(directory.path().join(".claude/settings.json"))
            .expect("read Claude hooks"),
    )
    .expect("parse Claude hooks");
    assert_eq!(
        claude_hooks["hooks"]["UserPromptSubmit"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert!(!report.files_written.is_empty());
}

#[test]
fn reinitialization_adds_new_presets_without_overwriting_existing_commands() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write manifest");
    let mut config = ForgeGuardConfig::new(
        "sample",
        vec![forgeguard_core::CommandConfig {
            name: "test".to_owned(),
            command: "custom-test".to_owned(),
            required: true,
            enabled: true,
            timeout_seconds: 30,
        }],
    );
    config.version = 1;
    config.save(directory.path()).expect("save legacy config");

    let report = initialize_project(
        directory.path(),
        &InitOptions {
            agents: vec![AgentTarget::Codex],
            ..InitOptions::default()
        },
    )
    .expect("reinitialize project");
    let upgraded = ForgeGuardConfig::load(directory.path()).expect("load reconciled config");

    assert_eq!(upgraded.version, 1, "init must not change gate semantics");
    assert_eq!(
        upgraded
            .commands
            .iter()
            .find(|command| command.name == "test")
            .map(|command| command.command.as_str()),
        Some("custom-test")
    );
    assert!(upgraded
        .commands
        .iter()
        .any(|command| command.name == "dependency-audit" && !command.enabled));
    assert!(report
        .files_written
        .contains(&".forgeguard/config.toml".to_owned()));
}

#[test]
fn project_init_appends_installed_agent_directories_to_existing_gitignore() {
    let directory = tempdir().expect("temp directory");
    fs::write(directory.path().join(".gitignore"), "target/\n/.codex/\n").expect("write gitignore");
    let options = InitOptions {
        force: false,
        refresh: false,
        agents: vec![AgentTarget::All],
    };

    let report = initialize_project(directory.path(), &options).expect("first initialization");
    initialize_project(directory.path(), &options).expect("second initialization");

    assert_eq!(
        fs::read_to_string(directory.path().join(".gitignore")).expect("read gitignore"),
        "target/\n/.codex/\n.claude/\n.cursor/\n.agents/\n"
    );
    assert!(report.files_written.contains(&".gitignore".to_owned()));
}

#[test]
fn project_init_ignores_only_directories_for_selected_agents() {
    let directory = tempdir().expect("temp directory");
    fs::write(directory.path().join(".gitignore"), "target/\r\n").expect("write gitignore");

    initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: false,
            agents: vec![AgentTarget::Claude],
        },
    )
    .expect("initialize Claude");

    assert_eq!(
        fs::read_to_string(directory.path().join(".gitignore")).expect("read gitignore"),
        "target/\r\n.claude/\r\n"
    );
}

#[test]
fn installs_global_general_guard_configuration_and_skills() {
    let directory = tempdir().expect("temp directory");

    let report = forgeguard_core::initialize_global(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: false,
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
    assert!(directory
        .path()
        .join(".config/opencode/skills/forgeguard-engineering/references/general-work.md")
        .exists());
    assert!(directory
        .path()
        .join(".hermes/skills/forgeguard-engineering/SKILL.md")
        .exists());
    assert!(directory
        .path()
        .join(".openclaw/skills/forgeguard-engineering/SKILL.md")
        .exists());
    assert!(directory
        .path()
        .join(".openclaw/extensions/forgeguard/openclaw.plugin.json")
        .exists());
    let openclaw_config: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(directory.path().join(".openclaw/openclaw.json"))
            .expect("read OpenClaw config"),
    )
    .expect("parse OpenClaw config");
    assert_eq!(
        openclaw_config["plugins"]["entries"]["forgeguard"]["enabled"],
        true
    );
    assert_eq!(
        openclaw_config["plugins"]["entries"]["forgeguard"]["hooks"]["allowConversationAccess"],
        true
    );
    assert!(directory.path().join(".gemini/GEMINI.md").exists());
    assert!(directory
        .path()
        .join(".gemini/antigravity-cli/skills/forgeguard-engineering/SKILL.md")
        .exists());
    // `.gemini/config` is documented only for `mcp_config.json`, and Antigravity
    // publishes no user-level hook file, so a global install writes neither.
    assert!(!directory.path().join(".gemini/config").exists());
    assert!(!directory.path().join(".gemini/hooks.json").exists());
    let config = ForgeGuardConfig::load_global(directory.path()).expect("load global config");
    assert_eq!(config.project.name, "global");
    assert_eq!(config.focus, forgeguard_core::FocusConfig::default());
    assert!(!report.files_written.is_empty());
}

#[test]
fn stop_hook_timeout_covers_the_configured_command_budget() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .expect("write manifest");

    initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: false,
            agents: vec![AgentTarget::Claude],
        },
    )
    .expect("initialize project");

    let config = ForgeGuardConfig::load(directory.path()).expect("load config");
    let budget: u64 = config
        .commands
        .iter()
        .filter(|command| command.enabled)
        .map(|command| command.timeout_seconds)
        .sum();
    assert!(
        budget > 600,
        "the Rust preset must exceed one command budget"
    );
    let settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(directory.path().join(".claude/settings.json")).expect("read settings"),
    )
    .expect("parse settings");
    let timeout = settings["hooks"]["Stop"][0]["hooks"][0]["timeout"]
        .as_u64()
        .expect("stop hook timeout");
    assert!(
        timeout >= budget,
        "stop hook timeout {timeout} must cover the {budget}s command budget"
    );
}

#[test]
fn reinitialization_repairs_a_stop_hook_timeout_below_the_command_budget() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .expect("write manifest");
    let settings = directory.path().join(".claude/settings.json");
    fs::create_dir_all(settings.parent().expect("settings parent")).expect("create settings");
    // An entry written by an earlier version: correct command, stale timeout.
    fs::write(
        &settings,
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"forgeguard hook stop --agent claude","timeout":600}]}]}}"#,
    )
    .expect("write legacy settings");

    initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: false,
            agents: vec![AgentTarget::Claude],
        },
    )
    .expect("initialize project");

    let config = ForgeGuardConfig::load(directory.path()).expect("load config");
    let budget: u64 = config
        .commands
        .iter()
        .filter(|command| command.enabled)
        .map(|command| command.timeout_seconds)
        .sum();
    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).expect("read settings"))
            .expect("parse settings");
    let timeout = value["hooks"]["Stop"][0]["hooks"][0]["timeout"]
        .as_u64()
        .expect("stop hook timeout");
    assert!(
        timeout >= budget,
        "existing stop hook timeout {timeout} must be repaired to cover {budget}s"
    );
    assert_eq!(
        value.to_string().matches("forgeguard hook stop").count(),
        1,
        "repair must not duplicate the hook entry"
    );
}

#[test]
fn hook_install_preserves_existing_settings_and_is_idempotent() {
    let directory = tempdir().expect("temp directory");
    let settings = directory.path().join(".claude/settings.json");
    fs::create_dir_all(settings.parent().expect("settings parent")).expect("create settings");
    fs::write(
        &settings,
        r#"{"permissions":{"allow":["Bash(git status)"]},"hooks":{"Stop":[],"SessionStart":[{"hooks":[{"type":"command","command":"repo_root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0; forgeguard hook context --agent claude --root \"$repo_root\""}]}]}}"#,
    )
    .expect("write settings");

    let options = InitOptions {
        force: false,
        refresh: false,
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
    assert_eq!(
        source
            .matches("forgeguard hook context --agent claude")
            .count(),
        2
    );
    assert_eq!(
        value["hooks"]["SessionStart"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        value["hooks"]["UserPromptSubmit"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        source
            .matches("forgeguard hook scope --agent claude")
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
            refresh: false,
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
fn force_removes_obsolete_skill_reference() {
    let directory = tempdir().expect("temp directory");
    let obsolete = directory
        .path()
        .join(".agents/skills/forgeguard-engineering/references/clean-code.md");
    fs::create_dir_all(obsolete.parent().expect("obsolete reference parent"))
        .expect("create obsolete reference parent");
    fs::write(&obsolete, "legacy reference").expect("write obsolete reference");

    initialize_project(
        directory.path(),
        &InitOptions {
            force: true,
            refresh: false,
            agents: vec![AgentTarget::Codex],
        },
    )
    .expect("initialize project");

    assert!(!obsolete.exists());
}

#[test]
fn non_force_preserves_obsolete_skill_reference() {
    let directory = tempdir().expect("temp directory");
    let obsolete = directory
        .path()
        .join(".agents/skills/forgeguard-engineering/references/clean-code.md");
    fs::create_dir_all(obsolete.parent().expect("obsolete reference parent"))
        .expect("create obsolete reference parent");
    fs::write(&obsolete, "legacy reference").expect("write obsolete reference");

    initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: false,
            agents: vec![AgentTarget::Codex],
        },
    )
    .expect("initialize project");

    assert!(obsolete.exists());
}

#[test]
fn existing_policy_is_preserved_without_force() {
    let directory = tempdir().expect("temp directory");
    fs::write(directory.path().join("AGENTS.md"), "custom policy\n").expect("write policy");

    initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: false,
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
            refresh: false,
            agents: vec![AgentTarget::OpenCode],
        },
    )
    .expect("initialize OpenCode");

    assert!(directory.path().join("AGENTS.md").exists());
    assert!(directory
        .path()
        .join(".agents/skills/forgeguard-engineering/SKILL.md")
        .exists());
    assert!(fs::read_to_string(
        directory
            .path()
            .join(".agents/skills/forgeguard-engineering/SKILL.md")
    )
    .expect("read OpenCode skill")
    .contains("native structured user-input tool"));
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
        refresh: false,
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
    assert_eq!(
        source
            .matches("forgeguard hook context --agent antigravity")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("forgeguard hook scope --agent antigravity")
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
            refresh: false,
            agents: vec![AgentTarget::Codex],
        },
    )
    .expect("migrate global skills");

    assert!(!legacy.exists());
    assert!(target.join("keep.txt").exists());
}

#[test]
fn detects_only_agents_with_configuration_present() {
    let directory = tempdir().expect("temp directory");

    assert!(detect_installed_agents(directory.path(), false).is_empty());

    fs::create_dir_all(directory.path().join(".claude")).expect("create .claude");
    fs::create_dir_all(directory.path().join(".roo/rules")).expect("create .roo/rules");
    fs::create_dir_all(directory.path().join(".hermes")).expect("create .hermes");
    fs::create_dir_all(directory.path().join(".openclaw")).expect("create .openclaw");
    fs::create_dir_all(directory.path().join(".github")).expect("create .github");
    fs::write(
        directory.path().join(".github/copilot-instructions.md"),
        "# rules\n",
    )
    .expect("write copilot instructions");

    let detected = detect_installed_agents(directory.path(), false);

    assert_eq!(
        detected,
        vec![
            AgentTarget::Claude,
            AgentTarget::Hermes,
            AgentTarget::OpenClaw,
            AgentTarget::Copilot,
            AgentTarget::Roo,
        ]
    );
}

#[test]
fn global_hermes_and_openclaw_use_their_native_skill_directories() {
    let directory = tempdir().expect("temp directory");
    fs::create_dir_all(directory.path().join(".openclaw")).expect("create OpenClaw home");
    fs::write(
        directory.path().join(".openclaw/openclaw.json"),
        r#"{"plugins":{"allow":["existing"],"entries":{"forgeguard":{"hooks":{"timeoutMs":1234}}}},"custom":true}"#,
    )
    .expect("seed OpenClaw config");

    let report = forgeguard_core::initialize_global(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: false,
            agents: vec![AgentTarget::Hermes, AgentTarget::OpenClaw],
        },
    )
    .expect("install global skills");

    assert_eq!(
        report.agents,
        vec![AgentTarget::Hermes, AgentTarget::OpenClaw]
    );
    assert!(directory
        .path()
        .join(".hermes/skills/forgeguard-engineering/SKILL.md")
        .is_file());
    assert!(directory
        .path()
        .join(".openclaw/skills/forgeguard-engineering/SKILL.md")
        .is_file());
    assert!(directory
        .path()
        .join(".openclaw/extensions/forgeguard/index.js")
        .is_file());
    let config: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(directory.path().join(".openclaw/openclaw.json"))
            .expect("read merged OpenClaw config"),
    )
    .expect("parse merged OpenClaw config");
    assert_eq!(config["custom"], true);
    assert_eq!(
        config["plugins"]["allow"],
        serde_json::json!(["existing", "forgeguard"])
    );
    assert_eq!(
        config["plugins"]["entries"]["forgeguard"]["hooks"]["timeoutMs"],
        1234
    );
    assert_eq!(config["plugins"]["entries"]["forgeguard"]["enabled"], true);
    assert!(!directory.path().join("AGENTS.md").exists());
}

#[test]
fn agents_md_only_targets_write_no_extra_directories() {
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
            refresh: false,
            agents: vec![
                AgentTarget::Windsurf,
                AgentTarget::Copilot,
                AgentTarget::Cline,
                AgentTarget::Roo,
            ],
        },
    )
    .expect("install AGENTS.md-only targets");

    // One shared policy file plus ForgeGuard's own config, and nothing else:
    // these four agents read AGENTS.md natively and expose no hook API.
    assert_eq!(
        report.files_written,
        vec![
            ".forgeguard/config.toml".to_owned(),
            ".forgeguard/.gitignore".to_owned(),
            "AGENTS.md".to_owned(),
        ]
    );
    for unexpected in [".windsurf", ".clinerules", ".roo", ".github", ".agents"] {
        assert!(
            !directory.path().join(unexpected).exists(),
            "{unexpected} should not be created"
        );
    }
}

#[test]
fn force_prunes_the_superseded_global_antigravity_skill_directory() {
    let directory = tempdir().expect("temp directory");
    let obsolete = directory
        .path()
        .join(".gemini/config/skills/forgeguard-engineering");
    fs::create_dir_all(&obsolete).expect("seed obsolete skill directory");
    fs::write(obsolete.join("SKILL.md"), "stale\n").expect("write stale skill");

    forgeguard_core::initialize_global(
        directory.path(),
        &InitOptions {
            force: true,
            refresh: false,
            agents: vec![AgentTarget::Antigravity],
        },
    )
    .expect("refresh global antigravity install");

    assert!(!obsolete.exists());
    assert!(directory
        .path()
        .join(".gemini/antigravity-cli/skills/forgeguard-engineering/SKILL.md")
        .exists());
}

#[test]
fn detection_is_stable_across_repeated_initialization() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write manifest");
    fs::create_dir_all(directory.path().join(".codex")).expect("create .codex");

    let first = detect_installed_agents(directory.path(), false);
    assert_eq!(first, vec![AgentTarget::Codex]);

    initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: false,
            agents: first.clone(),
        },
    )
    .expect("install for codex");

    // Codex shares `.agents/skills` with Cursor and OpenCode, so ForgeGuard's own
    // output must not make the next run believe Antigravity is in use too.
    assert!(directory.path().join(".agents/skills").exists());
    assert_eq!(detect_installed_agents(directory.path(), false), first);
}

#[test]
fn global_agents_md_targets_follow_each_documented_user_path() {
    let directory = tempdir().expect("temp directory");

    forgeguard_core::initialize_global(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: false,
            agents: vec![
                AgentTarget::Windsurf,
                AgentTarget::Copilot,
                AgentTarget::Cline,
                AgentTarget::Roo,
            ],
        },
    )
    .expect("install global AGENTS.md-only targets");

    // Each agent reads user-level rules from its own place; a shared
    // `~/.agents/AGENTS.md` would be read by Cline alone.
    assert!(directory.path().join(".agents/AGENTS.md").is_file());
    assert!(directory
        .path()
        .join(".codeium/windsurf/memories/global_rules.md")
        .is_file());
    assert!(directory.path().join(".roo/rules/forgeguard.md").is_file());
    // Copilot documents no user-level location, so nothing is written for it.
    assert!(!directory.path().join(".github").exists());
}

/// Seed a project installed for Claude, then drift two ForgeGuard-owned files.
fn project_with_drift() -> tempfile::TempDir {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write manifest");
    initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: false,
            agents: vec![AgentTarget::Claude],
        },
    )
    .expect("install for claude");
    fs::write(directory.path().join("CLAUDE.md"), "hand written\n").expect("edit policy");
    fs::write(
        directory
            .path()
            .join(".claude/skills/forgeguard-engineering/SKILL.md"),
        "from an older release\n",
    )
    .expect("age the skill");
    directory
}

#[test]
fn drifted_files_are_reported_and_left_alone() {
    let directory = project_with_drift();

    let report = initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: false,
            agents: vec![AgentTarget::Claude],
        },
    )
    .expect("re-run init");

    // CLAUDE.md is the user's file, so it is never a refresh candidate; only
    // the skill, which ForgeGuard owns outright, can be offered for replacement.
    assert_eq!(
        report.files_outdated,
        vec![".claude/skills/forgeguard-engineering/SKILL.md".to_owned()]
    );
    assert_eq!(report.files_kept, vec!["CLAUDE.md".to_owned()]);
    // Reported, never rewritten: replacing a file the user may have edited is
    // their decision, so a plain init leaves both exactly as they were.
    assert_eq!(
        fs::read_to_string(directory.path().join("CLAUDE.md")).expect("read policy"),
        "hand written\n"
    );
    // Files that already match the bundle are not drift.
    assert!(!report
        .files_outdated
        .iter()
        .any(|path| path.contains("references/")));
}

#[test]
fn refresh_replaces_drifted_files_without_touching_configuration() {
    let directory = project_with_drift();
    let configuration = directory.path().join(".forgeguard/config.toml");
    let tuned = fs::read_to_string(&configuration)
        .expect("read config")
        .replace("mode = \"default\"", "mode = \"strict\"");
    fs::write(&configuration, &tuned).expect("tune config");

    let report = initialize_project(
        directory.path(),
        &InitOptions {
            force: false,
            refresh: true,
            agents: vec![AgentTarget::Claude],
        },
    )
    .expect("refresh");

    assert!(report.files_outdated.is_empty());
    assert!(report
        .files_written
        .contains(&".claude/skills/forgeguard-engineering/SKILL.md".to_owned()));
    // A refresh updates what ForgeGuard ships and stops there.
    assert!(!report.files_written.contains(&"CLAUDE.md".to_owned()));
    assert_eq!(
        fs::read_to_string(directory.path().join("CLAUDE.md")).expect("read policy"),
        "hand written\n"
    );
    assert_eq!(
        fs::read_to_string(&configuration).expect("read config"),
        tuned,
        "refresh must not regenerate configuration"
    );
}

#[test]
fn force_reinstalls_files_but_keeps_configuration() {
    let directory = project_with_drift();
    let configuration = directory.path().join(".forgeguard/config.toml");
    let tuned = fs::read_to_string(&configuration)
        .expect("read config")
        .replace("mode = \"default\"", "mode = \"strict\"");
    fs::write(&configuration, &tuned).expect("tune config");

    initialize_project(
        directory.path(),
        &InitOptions {
            force: true,
            refresh: false,
            agents: vec![AgentTarget::Claude],
        },
    )
    .expect("force reinstall");

    // The operating mode and any tuned command belong to the repository; a
    // reinstall of the files ForgeGuard ships is no reason to discard them.
    assert_eq!(
        fs::read_to_string(&configuration).expect("read config"),
        tuned
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("CLAUDE.md")).expect("read policy"),
        "hand written\n"
    );
}

/// The whole point of the guard: --force and --refresh exist to reinstall what
/// ForgeGuard ships, and neither is a licence to rewrite the user's own file.
#[test]
fn a_user_edited_policy_file_survives_force_and_refresh() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write manifest");
    fs::write(directory.path().join("CLAUDE.md"), "# My rules\n").expect("seed claude policy");
    fs::write(directory.path().join("AGENTS.md"), "# My rules\n").expect("seed codex policy");

    for options in [
        InitOptions {
            force: true,
            refresh: false,
            agents: vec![AgentTarget::Claude, AgentTarget::Codex],
        },
        InitOptions {
            force: false,
            refresh: true,
            agents: vec![AgentTarget::Claude, AgentTarget::Codex],
        },
    ] {
        let report = initialize_project(directory.path(), &options).expect("install");
        assert_eq!(
            report.files_kept,
            vec!["CLAUDE.md".to_owned(), "AGENTS.md".to_owned()]
        );
        for policy in ["CLAUDE.md", "AGENTS.md"] {
            assert_eq!(
                fs::read_to_string(directory.path().join(policy)).expect("read policy"),
                "# My rules\n",
                "{policy} was rewritten"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn a_symlinked_policy_file_is_never_written_through() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write manifest");
    // Pointing AGENTS.md at CLAUDE.md is a common way to keep one policy file.
    fs::write(directory.path().join("CLAUDE.md"), "hand written\n").expect("seed policy");
    std::os::unix::fs::symlink("CLAUDE.md", directory.path().join("AGENTS.md"))
        .expect("link AGENTS.md at CLAUDE.md");

    initialize_project(
        directory.path(),
        &InitOptions {
            force: true,
            refresh: false,
            agents: vec![AgentTarget::Codex],
        },
    )
    .expect("install for codex");

    // Writing the Codex policy through the link would replace the file at the
    // other end, and the refresh prompt would have named AGENTS.md while
    // destroying CLAUDE.md.
    assert_eq!(
        fs::read_to_string(directory.path().join("CLAUDE.md")).expect("read policy"),
        "hand written\n"
    );
    assert!(fs::symlink_metadata(directory.path().join("AGENTS.md"))
        .expect("stat link")
        .file_type()
        .is_symlink());
}
