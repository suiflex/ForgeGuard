use std::{fs, process::Command};

use forgeguard_core::run_doctor;
use tempfile::tempdir;

#[test]
fn reports_legacy_codex_skill_layout() {
    let directory = tempdir().expect("temp directory");
    fs::create_dir_all(directory.path().join(".codex/skills/forgeguard-clean-code"))
        .expect("create legacy skill");

    let report = run_doctor(directory.path(), None).expect("run doctor");

    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("Legacy Codex")));
}

#[test]
fn current_skill_layout_has_no_migration_warning() {
    let directory = tempdir().expect("temp directory");
    let current = directory
        .path()
        .join(".agents/skills/forgeguard-engineering");
    fs::create_dir_all(&current).expect("create current skill");
    fs::write(current.join("SKILL.md"), "current").expect("write current skill");

    let report = run_doctor(directory.path(), None).expect("run doctor");

    assert!(report.warnings.is_empty());
}

#[test]
fn nested_repository_satisfies_workspace_git_check() {
    let directory = tempdir().expect("temp directory");
    let repository = directory.path().join("service");
    fs::create_dir_all(&repository).expect("create repository");
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&repository)
        .status()
        .expect("run git init");
    assert!(status.success());

    let report = run_doctor(directory.path(), None).expect("run doctor");

    assert!(report.git_repository);
}
