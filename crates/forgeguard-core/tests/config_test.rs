use std::fs;

use forgeguard_core::{CommandConfig, ForgeGuardConfig, GuardMode};
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
    assert_eq!(config.mode, GuardMode::Default);
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
