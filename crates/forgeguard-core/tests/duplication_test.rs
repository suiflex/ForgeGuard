use std::fs;

use forgeguard_core::{scan_project, config::ScanConfig, ScanOptions};
use tempfile::tempdir;

#[test]
fn reports_cross_file_duplicate_blocks() {
    let directory = tempdir().expect("temp directory");
    let source = r#"
pub fn normalize_customer(customer: Customer) -> CustomerView {
    let normalized_name = customer.name.trim().to_lowercase();
    let normalized_email = customer.email.trim().to_lowercase();
    let is_active = customer.deleted_at.is_none();
    let display_name = format!("{} <{}>", normalized_name, normalized_email);
    CustomerView { normalized_name, normalized_email, is_active, display_name }
}
"#;
    fs::write(directory.path().join("customer.rs"), source).expect("write first source");
    fs::write(directory.path().join("account.rs"), source).expect("write second source");

    let findings = scan_project(directory.path(), &ScanConfig::default(), &ScanOptions::default())
        .expect("scan project");

    assert!(findings.iter().any(|finding| finding.rule_id == "FG-DRY-001"));
}

#[test]
fn changed_scope_compares_against_unchanged_files() {
    let directory = tempdir().expect("temp directory");
    let source = r#"
pub fn normalize_customer(customer: Customer) -> CustomerView {
    let normalized_name = customer.name.trim().to_lowercase();
    let normalized_email = customer.email.trim().to_lowercase();
    let is_active = customer.deleted_at.is_none();
    let display_name = format!("{} <{}>", normalized_name, normalized_email);
    CustomerView { normalized_name, normalized_email, is_active, display_name }
}
"#;
    fs::write(directory.path().join("existing.rs"), source).expect("write existing source");
    fs::write(directory.path().join("changed.rs"), source).expect("write changed source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions {
            paths: Some(vec![std::path::PathBuf::from("changed.rs")]),
        },
    )
    .expect("scan project");

    assert!(findings.iter().any(|finding| {
        finding.rule_id == "FG-DRY-001"
            && finding.path == std::path::PathBuf::from("changed.rs")
    }));
}
