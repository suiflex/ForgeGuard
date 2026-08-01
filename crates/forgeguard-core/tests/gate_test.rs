use std::fs;

use forgeguard_core::{
    config::RuleConfig, run_gate, ForgeGuardConfig, GateOptions, GateStatus, GuardMode, Severity,
};
use tempfile::tempdir;

#[test]
fn default_mode_reports_static_findings_without_blocking() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("repository.ts"),
        r#"
import { PrismaClient } from "@prisma/client";
const db = new PrismaClient();
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

    assert_eq!(report.status, GateStatus::Warning);
}

#[test]
fn strict_mode_blocks_error_level_findings() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("repository.ts"),
        r#"
import { PrismaClient } from "@prisma/client";
const db = new PrismaClient();
for (const user of users) {
  await db.query("SELECT id FROM profiles WHERE user_id = ?", [user.id]);
}
"#,
    )
    .expect("write source");
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = GuardMode::Strict;

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

#[test]
fn config_v2_strict_blocks_warnings_but_v1_keeps_error_only_behavior() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("nested.ts"),
        "for (const row of rows) { for (const value of row) { console.log(value); } }\n",
    )
    .expect("write source");
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = GuardMode::Strict;

    let report = run_gate(
        directory.path(),
        &config,
        &GateOptions {
            skip_commands: true,
            paths: None,
        },
    )
    .expect("run v2 gate");
    assert_eq!(report.status, GateStatus::Blocked);
    assert_eq!(report.summary.blocking_findings, 1);

    config.version = 1;
    let report = run_gate(
        directory.path(),
        &config,
        &GateOptions {
            skip_commands: true,
            paths: None,
        },
    )
    .expect("run v1 gate");
    assert_eq!(report.status, GateStatus::Warning);
    assert_eq!(report.summary.blocking_findings, 0);
}

#[test]
fn per_rule_policy_can_disable_override_and_block() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("nested.ts"),
        "for (const row of rows) { for (const value of row) { console.log(value); } }\n",
    )
    .expect("write source");
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.rules.insert(
        "FG-ALG-001".to_owned(),
        RuleConfig {
            enabled: None,
            severity: Some(Severity::Error),
            block: Some(true),
        },
    );

    let report = run_gate(
        directory.path(),
        &config,
        &GateOptions {
            skip_commands: true,
            paths: None,
        },
    )
    .expect("run configured gate");
    assert_eq!(report.status, GateStatus::Blocked);
    assert_eq!(report.findings[0].severity, Severity::Error);

    config.rules.get_mut("FG-ALG-001").expect("rule").enabled = Some(false);
    let report = run_gate(
        directory.path(),
        &config,
        &GateOptions {
            skip_commands: true,
            paths: None,
        },
    )
    .expect("run disabled gate");
    assert_eq!(report.status, GateStatus::Passed);
    assert!(report.findings.is_empty());
}
