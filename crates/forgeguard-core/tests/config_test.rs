use std::fs;

use forgeguard_core::{CommandConfig, ForgeGuardConfig, GuardMode, Severity};
use tempfile::tempdir;

#[test]
fn configuration_round_trips_through_toml() {
    let directory = tempdir().expect("temp directory");
    let mut config = ForgeGuardConfig::new(
        "example",
        vec![CommandConfig {
            name: "test".to_owned(),
            command: "cargo test".to_owned(),
            required: true,
            enabled: true,
            timeout_seconds: 30,
        }],
    );
    config.mode = GuardMode::Strict;

    config.save(directory.path()).expect("save config");
    let loaded = ForgeGuardConfig::load(directory.path()).expect("load config");

    assert_eq!(loaded, config);
}

#[test]
fn new_configuration_defaults_to_default_mode() {
    let config = ForgeGuardConfig::new("example", Vec::new());
    assert_eq!(config.version, 2);
    assert_eq!(config.mode, GuardMode::Default);
    assert!(config.focus.enabled);
    assert_eq!(config.focus.max_retries, 3);
    assert_eq!(config.focus.no_progress_limit, 2);
    assert!(config.focus.auto_poke);
    assert_eq!(config.focus.max_auto_pokes, 3);
    assert_eq!(config.focus.min_confidence, 80);
    assert_eq!(config.focus.min_hill_climbability, 80);
}

#[test]
fn legacy_guard_mode_loads_as_strict() {
    let directory = tempdir().expect("temp directory");
    fs::create_dir_all(directory.path().join(".forgeguard")).expect("create config dir");
    fs::write(
        directory.path().join(".forgeguard/config.toml"),
        r#"version = 1
mode = "guard"

[project]
name = "legacy"

[scan]
enabled = true
max_file_bytes = 1000000
include_tests = false
extra_excludes = []
duplicate_block_lines = 6
"#,
    )
    .expect("write legacy config");

    let config = ForgeGuardConfig::load(directory.path()).expect("load legacy config");
    assert_eq!(config.mode, GuardMode::Strict);
    assert!(config.focus.enabled);
    assert!(config.focus.auto_poke);
}

#[test]
fn global_configuration_round_trips_through_toml() {
    let directory = tempdir().expect("temp directory");
    let config = ForgeGuardConfig::new("global", Vec::new());

    config
        .save_global(directory.path())
        .expect("save global config");
    let loaded = ForgeGuardConfig::load_global(directory.path()).expect("load global config");

    assert_eq!(loaded, config);
    assert!(directory.path().join(".forgeguard/config.toml").exists());
}

#[test]
fn config_v2_loads_global_and_per_rule_policy() {
    let directory = tempdir().expect("temp directory");
    fs::create_dir_all(directory.path().join(".forgeguard")).expect("create config dir");
    fs::write(
        directory.path().join(".forgeguard/config.toml"),
        r#"version = 2
mode = "strict"

[project]
name = "policy"

[policies]
warnings_block = false

[rules.FG-NET-001]
enabled = true
severity = "error"
block = true
"#,
    )
    .expect("write config");

    let config = ForgeGuardConfig::load(directory.path()).expect("load config");
    assert_eq!(config.policies.warnings_block, Some(false));
    let rule = config.rules.get("FG-NET-001").expect("rule policy");
    assert_eq!(rule.enabled, Some(true));
    assert_eq!(rule.severity, Some(Severity::Error));
    assert_eq!(rule.block, Some(true));
}

#[test]
fn migration_to_v2_preserves_commands_focus_and_mode() {
    let mut config = ForgeGuardConfig::new(
        "migration",
        vec![CommandConfig {
            name: "test".to_owned(),
            command: "cargo test".to_owned(),
            required: true,
            enabled: true,
            timeout_seconds: 45,
        }],
    );
    config.version = 1;
    config.mode = GuardMode::Strict;
    config.focus.max_retries = 7;
    let commands = config.commands.clone();
    let focus = config.focus.clone();

    assert_eq!(config.migrate_to_v2().expect("migrate config"), 1);
    assert_eq!(config.version, 2);
    assert_eq!(config.mode, GuardMode::Strict);
    assert_eq!(config.commands, commands);
    assert_eq!(config.focus, focus);
    assert_eq!(config.migrate_to_v2().expect("idempotent migration"), 2);
}

#[test]
fn command_reconciliation_appends_missing_presets_without_overwriting_user_edits() {
    let custom = CommandConfig {
        name: "test".to_owned(),
        command: "custom-test --fast".to_owned(),
        required: false,
        enabled: true,
        timeout_seconds: 12,
    };
    let mut config = ForgeGuardConfig::new("upgrade", vec![custom.clone()]);
    let detected = [
        CommandConfig {
            name: "test".to_owned(),
            command: "cargo test".to_owned(),
            required: true,
            enabled: true,
            timeout_seconds: 600,
        },
        CommandConfig {
            name: "dependency-audit".to_owned(),
            command: "cargo audit".to_owned(),
            required: false,
            enabled: false,
            timeout_seconds: 600,
        },
    ];

    assert_eq!(config.reconcile_commands(&detected), 1);
    assert_eq!(config.commands[0], custom);
    assert_eq!(config.commands[1].name, "dependency-audit");
    assert_eq!(config.reconcile_commands(&detected), 0);
}

#[test]
fn unsupported_config_version_fails_closed() {
    let directory = tempdir().expect("temp directory");
    fs::create_dir_all(directory.path().join(".forgeguard")).expect("create config dir");
    fs::write(
        directory.path().join(".forgeguard/config.toml"),
        r#"version = 99

[project]
name = "future"
"#,
    )
    .expect("write config");

    let error = ForgeGuardConfig::load(directory.path()).expect_err("reject unknown version");
    assert!(error.to_string().contains("unsupported config version 99"));
}

#[test]
fn changed_coverage_policy_requires_a_valid_report_configuration() {
    let directory = tempdir().expect("temp directory");
    fs::create_dir_all(directory.path().join(".forgeguard")).expect("create config dir");
    fs::write(
        directory.path().join(".forgeguard/config.toml"),
        r#"version = 2
[project]
name = "coverage"
[scan]
min_changed_coverage = 101
"#,
    )
    .expect("write config");

    let error = ForgeGuardConfig::load(directory.path()).expect_err("reject invalid threshold");
    assert!(error.to_string().contains("between 0 and 100"));
}
