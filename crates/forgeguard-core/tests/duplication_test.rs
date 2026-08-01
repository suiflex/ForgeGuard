use std::fs;

use forgeguard_core::{config::ScanConfig, scan_project, ScanOptions};
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

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-DRY-001"));
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
            && finding.path.as_path() == std::path::Path::new("changed.rs")
    }));
}

#[test]
fn unsupported_parser_language_still_receives_duplicate_check() {
    let directory = tempdir().expect("temp directory");
    let source = r#"
function normalize_customer(customer)
  local normalized_name = string.lower(customer.name)
  local normalized_email = string.lower(customer.email)
  local is_active = customer.deleted_at == nil
  local display_name = normalized_name .. " <" .. normalized_email .. ">"
  return {name = normalized_name, email = normalized_email, active = is_active, display = display_name}
end
"#;
    fs::write(directory.path().join("customer.lua"), source).expect("write first source");
    fs::write(directory.path().join("account.lua"), source).expect("write second source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-DRY-001"));
}

#[test]
fn reports_alpha_renamed_functions_but_preserves_literals() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("customer.rs"),
        r#"pub fn customer_score(users: &[i32]) -> i32 {
    let mut total = 0;
    for user in users {
        let adjusted = user * 2;
        total += adjusted;
    }
    total
}
"#,
    )
    .expect("write first source");
    fs::write(
        directory.path().join("account.rs"),
        r#"pub fn account_score(accounts: &[i32]) -> i32 {
    let mut sum = 0;
    for account in accounts {
        let weighted = account * 2;
        sum += weighted;
    }
    sum
}
"#,
    )
    .expect("write renamed source");
    fs::write(
        directory.path().join("different.rs"),
        r#"pub fn different_score(accounts: &[i32]) -> i32 {
    let mut sum = 0;
    for account in accounts {
        let weighted = account * 3;
        sum += weighted;
    }
    sum
}
"#,
    )
    .expect("write different source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-DRY-002"));
    assert!(!findings.iter().any(|finding| {
        finding.rule_id == "FG-DRY-002"
            && finding.path.as_path() == std::path::Path::new("different.rs")
    }));
}
