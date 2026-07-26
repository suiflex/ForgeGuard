use std::fs;

use forgeguard_core::{run_gate, ForgeGuardConfig, GateOptions, GateStatus, GuardMode};
use tempfile::tempdir;

#[test]
fn guard_mode_blocks_error_level_findings() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("repository.ts"),
        r#"
for (const user of users) {
  await db.query("SELECT id FROM profiles WHERE user_id = ?", [user.id]);
}
"#,
    )
    .expect("write source");
    let config = ForgeGuardConfig::new("sample", Vec::new());

    let report = run_gate(
        directory.path(),
        &config,
        &GateOptions {
            skip_commands: true,
            paths: None,
        },
    )
    .expect("run gate");

    assert_eq!(report.status, GateStatus::Blocked);
}

#[test]
fn lite_mode_reports_without_blocking_static_findings() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("repository.ts"),
        r#"
for (const user of users) {
  await db.query("SELECT id FROM profiles WHERE user_id = ?", [user.id]);
}
"#,
    )
    .expect("write source");
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = GuardMode::Lite;

    let report = run_gate(
        directory.path(),
        &config,
        &GateOptions {
            skip_commands: true,
            paths: None,
        },
    )
    .expect("run gate");

    assert_eq!(report.status, GateStatus::Warning);
}

#[test]
fn explicit_empty_path_scope_scans_nothing() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("repository.ts"),
        "for (const user of users) { await db.query('SELECT * FROM users'); }\n",
    )
    .expect("write source");
    let config = ForgeGuardConfig::new("sample", Vec::new());

    let report = run_gate(
        directory.path(),
        &config,
        &GateOptions {
            skip_commands: true,
            paths: Some(Vec::new()),
        },
    )
    .expect("run gate");

    assert_eq!(report.status, GateStatus::Passed);
    assert!(report.findings.is_empty());
}
