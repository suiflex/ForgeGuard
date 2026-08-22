use std::{fs, process::Command};

use forgeguard_core::{
    config::RuleConfig, run_changed_gate, run_gate, ForgeGuardConfig, GateOptions, GateStatus,
    GuardMode, Severity,
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

#[test]
fn changed_gate_ignores_old_findings_and_enforces_lcov_on_new_lines() {
    let directory = tempdir().expect("temp directory");
    git(directory.path(), &["init", "--quiet"]);
    git(
        directory.path(),
        &["config", "user.email", "test@example.com"],
    );
    git(
        directory.path(),
        &["config", "user.name", "ForgeGuard Test"],
    );
    fs::write(
        directory.path().join("app.ts"),
        "for (const row of rows) { for (const value of row) console.log(value); }\nexport const value = 1;\n",
    )
    .expect("write base source");
    git(directory.path(), &["add", "app.ts"]);
    git(directory.path(), &["commit", "--quiet", "-m", "base"]);
    fs::write(
        directory.path().join("app.ts"),
        "for (const row of rows) { for (const value of row) console.log(value); }\nexport const value = 2;\nexport const next = 3;\n",
    )
    .expect("change source");
    fs::write(
        directory.path().join("lcov.info"),
        "TN:\nSF:app.ts\nDA:1,0\nDA:2,1\nDA:3,0\nend_of_record\n",
    )
    .expect("write coverage");
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = GuardMode::Strict;
    config.scan.coverage_report = Some("lcov.info".into());
    config.scan.min_changed_coverage = Some(80);

    let report = run_changed_gate(directory.path(), &config, true, None).expect("changed gate");

    assert!(report
        .findings
        .iter()
        .any(|finding| finding.rule_id == "FG-COV-001"));
    assert!(!report
        .findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-001"));
    assert_eq!(report.status, GateStatus::Blocked);
}

#[test]
fn changed_coverage_policy_ignores_documentation_only_changes() {
    let directory = tempdir().expect("temp directory");
    git(directory.path(), &["init", "--quiet"]);
    git(
        directory.path(),
        &["config", "user.email", "test@example.com"],
    );
    git(
        directory.path(),
        &["config", "user.name", "ForgeGuard Test"],
    );
    fs::write(directory.path().join("README.md"), "before\n").expect("write readme");
    git(directory.path(), &["add", "README.md"]);
    git(directory.path(), &["commit", "--quiet", "-m", "base"]);
    fs::write(directory.path().join("README.md"), "after\n").expect("change readme");
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.scan.coverage_report = Some("missing-lcov.info".into());
    config.scan.min_changed_coverage = Some(80);

    let report = run_changed_gate(directory.path(), &config, true, None).expect("changed gate");

    assert!(!report
        .findings
        .iter()
        .any(|finding| finding.rule_id == "FG-COV-001"));
}

fn git(root: &std::path::Path, args: &[&str]) {
    assert!(Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .expect("run git")
        .success());
}
