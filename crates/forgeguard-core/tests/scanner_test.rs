use std::fs;

use forgeguard_core::{scan_project, config::ScanConfig, ScanOptions, Severity};
use tempfile::tempdir;

#[test]
fn finds_repeated_lookup_database_io_and_nested_iteration() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("service.ts"),
        r#"
export async function enrich(users, roles, db) {
  for (const user of users) {
    const role = roles.find((candidate) => candidate.id === user.roleId);
    const profile = await db.query("SELECT id FROM profiles WHERE user_id = ?", [user.id]);
    for (const permission of role.permissions) {
      console.log(permission);
    }
  }
}
"#,
    )
    .expect("write source");

    let findings = scan_project(directory.path(), &ScanConfig::default(), &ScanOptions::default())
        .expect("scan project");

    assert!(findings.iter().any(|finding| finding.rule_id == "FG-ALG-001"));
    assert!(findings.iter().any(|finding| finding.rule_id == "FG-ALG-002"));
    assert!(findings.iter().any(|finding| {
        finding.rule_id == "FG-DB-001" && finding.severity == Severity::Error
    }));
}

#[test]
fn finds_unbounded_parallel_execution_and_select_all() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("jobs.ts"),
        r#"
const rows = await db.query("SELECT * FROM jobs");
await Promise.all(rows.map((row) => processRow(row)));
"#,
    )
    .expect("write source");

    let findings = scan_project(directory.path(), &ScanConfig::default(), &ScanOptions::default())
        .expect("scan project");

    assert!(findings.iter().any(|finding| finding.rule_id == "FG-CON-001"));
    assert!(findings.iter().any(|finding| finding.rule_id == "FG-DB-005"));
}

#[test]
fn indexed_lookup_does_not_trigger_repeated_lookup_rule() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("service.ts"),
        r#"
const roleById = new Map(roles.map((role) => [role.id, role]));
const result = users.map((user) => ({ ...user, role: roleById.get(user.roleId) }));
"#,
    )
    .expect("write source");

    let findings = scan_project(directory.path(), &ScanConfig::default(), &ScanOptions::default())
        .expect("scan project");

    assert!(!findings.iter().any(|finding| finding.rule_id == "FG-ALG-002"));
}

#[test]
fn detects_python_loop_database_access() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("service.py"),
        r#"
async def load_profiles(users, db):
    for user in users:
        profile = await db.fetch(user.id)
        for permission in profile.permissions:
            print(permission)
"#,
    )
    .expect("write source");

    let findings = scan_project(directory.path(), &ScanConfig::default(), &ScanOptions::default())
        .expect("scan project");

    assert!(findings.iter().any(|finding| finding.rule_id == "FG-ALG-001"));
    assert!(findings.iter().any(|finding| finding.rule_id == "FG-DB-001"));
}

#[test]
fn detects_single_line_map_with_linear_lookup() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("service.ts"),
        "const output = users.map((user) => roles.find((role) => role.id === user.roleId));\n",
    )
    .expect("write source");

    let findings = scan_project(directory.path(), &ScanConfig::default(), &ScanOptions::default())
        .expect("scan project");

    assert!(findings.iter().any(|finding| finding.rule_id == "FG-ALG-002"));
}

#[test]
fn detects_external_request_inside_loop() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("client.ts"),
        r#"
for (const user of users) {
  await fetch(`/api/users/${user.id}`);
}
"#,
    )
    .expect("write source");

    let findings = scan_project(directory.path(), &ScanConfig::default(), &ScanOptions::default())
        .expect("scan project");

    assert!(findings.iter().any(|finding| finding.rule_id == "FG-NET-001"));
}

#[test]
fn detects_chained_orm_query_inside_loop() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("repository.ts"),
        r#"
for (const account of accounts) {
  await prisma.user.findMany({ where: { accountId: account.id } });
}
"#,
    )
    .expect("write source");

    let findings = scan_project(directory.path(), &ScanConfig::default(), &ScanOptions::default())
        .expect("scan project");

    assert!(findings.iter().any(|finding| finding.rule_id == "FG-DB-001"));
}
