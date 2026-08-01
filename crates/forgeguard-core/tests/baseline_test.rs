use std::fs;

use forgeguard_core::{
    create_baseline, create_baseline_with_config, run_gate, ForgeGuardConfig, GateOptions,
    GateStatus, GuardMode, BASELINE_FILE,
};
use tempfile::tempdir;

const EXISTING_FINDING: &str = r#"
import { PrismaClient } from "@prisma/client";
const db = new PrismaClient();
for (const user of users) {
  await db.query("SELECT id FROM profiles WHERE user_id = ?", [user.id]);
}
"#;

#[test]
fn baseline_hides_existing_finding_after_line_move() {
    let directory = tempdir().expect("temp directory");
    let source = directory.path().join("repository.ts");
    fs::write(&source, EXISTING_FINDING).expect("write source");
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = GuardMode::Strict;

    let baseline =
        create_baseline_with_config(directory.path(), &config, false).expect("create baseline");
    assert_eq!(baseline.total_findings(), 1);

    fs::write(&source, format!("\n\n{EXISTING_FINDING}")).expect("move finding");
    let report = run_gate(
        directory.path(),
        &config,
        &GateOptions {
            skip_commands: true,
            paths: None,
        },
    )
    .expect("run gate");

    assert_eq!(report.status, GateStatus::Passed);
    assert!(report.findings.is_empty());
    assert_eq!(report.summary.findings_baselined, 1);
}

#[test]
fn baseline_reports_additional_matching_finding() {
    let directory = tempdir().expect("temp directory");
    let source = directory.path().join("repository.ts");
    fs::write(&source, EXISTING_FINDING).expect("write source");
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = GuardMode::Strict;
    create_baseline_with_config(directory.path(), &config, false).expect("create baseline");

    fs::write(
        &source,
        r#"
import { PrismaClient } from "@prisma/client";
const db = new PrismaClient();
for (const user of users) {
  await db.query("SELECT id FROM profiles WHERE user_id = ?", [user.id]);
  await db.query("SELECT id FROM profiles WHERE user_id = ?", [user.id]);
}
"#,
    )
    .expect("add matching finding");
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
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.summary.findings_baselined, 1);
}

#[test]
fn replacing_baseline_requires_force() {
    let directory = tempdir().expect("temp directory");
    fs::write(directory.path().join("repository.ts"), EXISTING_FINDING).expect("write source");
    let config = ForgeGuardConfig::new("sample", Vec::new());

    create_baseline(directory.path(), &config.scan, false).expect("create baseline");
    let error = create_baseline(directory.path(), &config.scan, false)
        .expect_err("replacement should require force");
    assert!(error.to_string().contains("--force"));
    create_baseline(directory.path(), &config.scan, true).expect("replace baseline");
}

#[test]
fn gate_rejects_unknown_baseline_version() {
    let directory = tempdir().expect("temp directory");
    fs::create_dir_all(directory.path().join(".forgeguard")).expect("create baseline directory");
    fs::write(
        directory.path().join(BASELINE_FILE),
        r#"{"version":99,"findings":[]}"#,
    )
    .expect("write baseline");
    let config = ForgeGuardConfig::new("sample", Vec::new());

    let error = run_gate(
        directory.path(),
        &config,
        &GateOptions {
            skip_commands: true,
            paths: None,
        },
    )
    .expect_err("unsupported baseline should fail");

    assert!(error.to_string().contains("unsupported baseline version"));
}
