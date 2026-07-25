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
        }],
    );
    config.mode = GuardMode::Strict;

    config.save(directory.path()).expect("save config");
    let loaded = ForgeGuardConfig::load(directory.path()).expect("load config");

    assert_eq!(loaded, config);
}
