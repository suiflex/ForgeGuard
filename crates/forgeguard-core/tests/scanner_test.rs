use std::fs;

use forgeguard_core::{config::ScanConfig, scan_project, ScanOptions, Severity};
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

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-001"));
    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-002"));
    assert!(findings
        .iter()
        .any(|finding| { finding.rule_id == "FG-DB-002" && finding.severity == Severity::Info }));
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

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-CON-001"));
    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-DB-005"));
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

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(!findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-002"));
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

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-001"));
    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-DB-002"));
}

#[test]
fn detects_single_line_map_with_linear_lookup() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("service.ts"),
        "const output = users.map((user) => roles.find((role) => role.id === user.roleId));\n",
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-002"));
}

#[test]
fn detects_filter_inside_map() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("service.ts"),
        "const output = users.map((user) => roles.filter((role) => role.userId === user.id));\n",
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-001"));
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

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-NET-001"));
}

#[test]
fn detects_chained_orm_query_inside_loop() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("repository.ts"),
        r#"
import { PrismaClient } from "@prisma/client";
const prisma = new PrismaClient();
for (const account of accounts) {
  await prisma.user.findMany({ where: { accountId: account.id } });
}
"#,
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-DB-001"));
}

#[test]
fn parses_every_supported_language_without_fallback() {
    let directory = tempdir().expect("temp directory");
    let sources = [
        ("app.js", "for (const value of values) { console.log(value); }\n"),
        ("app.jsx", "export const App = () => <main>Hello</main>;\n"),
        ("app.ts", "const value: number = 1;\n"),
        ("app.tsx", "export const App = () => <main>Hello</main>;\n"),
        ("app.rs", "fn run(values: &[i32]) { for value in values { println!(\"{value}\"); } }\n"),
        ("app.go", "package app\nfunc run(values []int) { for _, value := range values { println(value) } }\n"),
        ("app.py", "def run(values):\n    for value in values:\n        print(value)\n"),
        ("App.java", "class App { void run(int[] values) { for (int value : values) { System.out.println(value); } } }\n"),
        ("app.kt", "fun run(values: List<Int>) { for (value in values) { println(value) } }\n"),
        ("App.cs", "class App { void Run(int[] values) { foreach (var value in values) { System.Console.WriteLine(value); } } }\n"),
        ("app.c", "void run(int *values, int count) { for (int i = 0; i < count; i++) { values[i]++; } }\n"),
        ("app.cpp", "void run(int *values, int count) { for (int i = 0; i < count; i++) { values[i]++; } }\n"),
        ("app.rb", "def run(values)\n  values.each { |value| puts(value) }\nend\n"),
        ("app.php", "<?php\nfunction run($values) { foreach ($values as $value) { echo $value; } }\n"),
        ("app.swift", "func run(_ values: [Int]) { for value in values { print(value) } }\n"),
        ("app.dart", "void run(List<int> values) { for (final value in values) { print(value); } }\n"),
        ("app.sh", "for value in 1 2 3; do\n  printf '%s\\n' \"$value\"\ndone\n"),
    ];
    for (name, source) in sources {
        fs::write(directory.path().join(name), source).expect("write source");
    }

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(!findings
        .iter()
        .any(|finding| finding.rule_id == "FG-PARSE-001"));
}

#[test]
fn detects_database_access_in_rust_go_and_python_loops() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("repository.rs"),
        "fn load(users: &[User], db: &Db) { for user in users { db.query(user.id); } }\n",
    )
    .expect("write Rust source");
    fs::write(
        directory.path().join("repository.go"),
        "package app\nfunc load(users []User, db DB) { for _, user := range users { db.Query(user.ID) } }\n",
    )
    .expect("write Go source");
    fs::write(
        directory.path().join("repository.py"),
        "def load(users, db):\n    for user in users:\n        db.query(user.id)\n",
    )
    .expect("write Python source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    for path in ["repository.rs", "repository.go", "repository.py"] {
        assert!(findings.iter().any(|finding| finding.rule_id == "FG-DB-002"
            && finding.path.as_path() == std::path::Path::new(path)));
    }
}

#[test]
fn ignores_rule_names_inside_strings_and_comments() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("scanner.rs"),
        r#"
fn messages(items: &[Item]) {
    for item in items {
        let rule = "db.query(\"SELECT * FROM users\")";
        // await prisma.user.findMany()
        println!("{rule} {}", item.id);
    }
}
"#,
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(!findings
        .iter()
        .any(|finding| matches!(finding.rule_id.as_str(), "FG-DB-001" | "FG-DB-005")));
}

#[test]
fn rust_set_membership_is_not_reported_as_linear_lookup() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("membership.rs"),
        r#"
fn retain(values: &[i32], allowed: &std::collections::HashSet<i32>) {
    for value in values {
        if allowed.contains(value) {
            println!("{value}");
        }
    }
}
"#,
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(!findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-002"));
}

#[test]
fn statically_bounded_take_is_not_reported_as_quadratic() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("report.rs"),
        r#"
fn render(checks: &[Check]) {
    for check in checks {
        for line in check.output.lines().take(20) {
            println!("{line}");
        }
    }
}
"#,
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(!findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-001"));
}

#[test]
fn detects_python_requests_call_inside_loop() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("client.py"),
        "import requests\ndef fetch(urls):\n    for url in urls:\n        requests.get(url)\n",
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-NET-001"));
}

#[test]
fn detects_network_call_inside_python_comprehension() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("client.py"),
        "import requests\ndef fetch(urls):\n    return [requests.get(url) for url in urls]\n",
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-NET-001"));
}

#[test]
fn literal_range_bound_is_not_reported_as_quadratic() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("grid.py"),
        "def grid():\n    for i in range(3):\n        for j in range(3):\n            print(i, j)\n",
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(!findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-001"));
}

#[test]
fn variable_range_bound_is_still_reported_as_quadratic() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("matrix.py"),
        "def matrix(n, m):\n    for i in range(n):\n        for j in range(m):\n            print(i, j)\n",
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-001"));
}

#[test]
fn malformed_source_reports_parse_skip_without_structural_claims() {
    let directory = tempdir().expect("temp directory");
    fs::write(directory.path().join("broken.rs"), "fn broken(\n").expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-PARSE-001"));
    assert!(!findings.iter().any(|finding| {
        finding.rule_id.starts_with("FG-ALG")
            || finding.rule_id.starts_with("FG-DB")
            || finding.rule_id.starts_with("FG-NET")
            || finding.rule_id.starts_with("FG-CON")
    }));
}

#[test]
fn unsupported_languages_receive_no_structural_findings() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("repository.lua"),
        "for _, user in ipairs(users) do\n  db.query(user.id)\nend\n",
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings.is_empty());
}

#[test]
fn detects_nested_loops_in_added_language_profiles() {
    let directory = tempdir().expect("temp directory");
    let sources = [
        ("Nested.java", "class Nested { void run(int[][] rows) { for (int[] row : rows) { for (int value : row) { System.out.println(value); } } } }\n"),
        ("nested.kt", "fun run(rows: List<List<Int>>) { for (row in rows) { for (value in row) { println(value) } } }\n"),
        ("Nested.cs", "class Nested { void Run(int[][] rows) { foreach (var row in rows) { foreach (var value in row) { System.Console.WriteLine(value); } } } }\n"),
        ("nested.c", "void run(int **rows, int size) { for (int i = 0; i < size; i++) { for (int j = 0; j < size; j++) { rows[i][j]++; } } }\n"),
        ("nested.cpp", "void run(int **rows, int size) { for (int i = 0; i < size; i++) { for (int j = 0; j < size; j++) { rows[i][j]++; } } }\n"),
        ("nested.rb", "rows.each do |row|\n  row.each { |value| puts(value) }\nend\n"),
        ("nested.php", "<?php\nforeach ($rows as $row) { foreach ($row as $value) { echo $value; } }\n"),
        ("nested.swift", "func run(_ rows: [[Int]]) { for row in rows { for value in row { print(value) } } }\n"),
        ("nested.dart", "void run(List<List<int>> rows) { for (final row in rows) { for (final value in row) { print(value); } } }\n"),
        ("nested.sh", "for row in 1 2; do\n  for value in 1 2; do\n    printf '%s\\n' \"$row:$value\"\n  done\ndone\n"),
    ];
    for (name, source) in sources {
        fs::write(directory.path().join(name), source).expect("write source");
    }

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    for (name, _) in sources {
        assert!(
            findings.iter().any(|finding| {
                finding.rule_id == "FG-ALG-001"
                    && finding.path.as_path() == std::path::Path::new(name)
            }),
            "missing nested-loop finding for {name}"
        );
    }
}

#[test]
fn detects_database_calls_in_common_object_oriented_languages() {
    let directory = tempdir().expect("temp directory");
    let sources = [
        ("Repository.java", "class Repository { void load(User[] users, Db db) { for (User user : users) { db.query(user.id); } } }\n"),
        ("repository.kt", "fun load(users: List<User>, db: Db) { for (user in users) { db.query(user.id) } }\n"),
        ("Repository.cs", "class Repository { void Load(User[] users, Db db) { foreach (var user in users) { db.QueryAsync(user.Id); } } }\n"),
        ("repository.rb", "users.each do |user|\n  db.query(user.id)\nend\n"),
        ("repository.php", "<?php\nforeach ($users as $user) { $db->query($user->id); }\n"),
        ("repository.swift", "func load(_ users: [User], db: Database) { for user in users { db.query(user.id) } }\n"),
        ("repository.dart", "void load(List<User> users, Database db) { for (final user in users) { db.query(user.id); } }\n"),
    ];
    for (name, source) in sources {
        fs::write(directory.path().join(name), source).expect("write source");
    }

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    for (name, _) in sources {
        assert!(
            findings.iter().any(|finding| {
                finding.rule_id == "FG-DB-002"
                    && finding.path.as_path() == std::path::Path::new(name)
            }),
            "missing database-in-loop finding for {name}"
        );
    }
}

#[test]
fn sql_files_still_report_select_all() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("users.sql"),
        "-- users\nSELECT * FROM users;\n",
    )
    .expect("write SQL");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| { finding.rule_id == "FG-DB-005" && finding.line == 2 }));
}

#[test]
fn sql_findings_keep_line_numbers_across_multiple_matches() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("queries.sql"),
        "SELECT * FROM users;\n\nSELECT * FROM roles;\nSELECT id FROM teams;\nSELECT * FROM jobs;\n",
    )
    .expect("write SQL");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");
    let lines = findings
        .iter()
        .filter(|finding| finding.rule_id == "FG-DB-005")
        .map(|finding| finding.line)
        .collect::<Vec<_>>();

    assert_eq!(lines, vec![1, 3, 5]);
}

#[test]
fn reasoned_inline_suppression_only_hides_heuristics() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("service.ts"),
        r#"
import { PrismaClient } from "@prisma/client";
const db = new PrismaClient();
for (const user of users) {
  // forgeguard: allow FG-ALG-002 -- roles has a documented maximum of 4
  const role = roles.find((candidate) => candidate.id === user.roleId);
  // forgeguard: allow FG-DB-001 -- error-level rules cannot be suppressed
  await db.query("SELECT id FROM users WHERE id = ?", [user.id]);
}
"#,
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(!findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-002"));
    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-DB-001"));
}

#[test]
fn scans_svelte_component_script_with_correct_line_numbers() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("Users.svelte"),
        r#"<script lang="ts">
  export let users = [];
  export let db;
  async function load() {
    for (const user of users) {
      await db.query("SELECT id FROM profiles WHERE user_id = ?", [user.id]);
    }
  }
</script>

<ul>
  {#each users as user}
    <li>{user.name}</li>
  {/each}
</ul>
"#,
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    let db = findings
        .iter()
        .find(|finding| finding.rule_id == "FG-DB-002" && finding.severity == Severity::Info)
        .expect("db-in-loop flagged inside svelte <script>");
    // The db.query call sits on line 6 of the original file; masking must
    // preserve line numbers so the finding points back at the real source.
    assert_eq!(db.line, 6);
}

#[test]
fn detects_zig_and_solidity_nested_loops() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("pair.zig"),
        r#"pub fn f(a: []const u8, b: []const u8) void {
    for (a) |x| {
        for (b) |y| {
            _ = x;
            _ = y;
        }
    }
}
"#,
    )
    .expect("write zig");
    fs::write(
        directory.path().join("Pair.sol"),
        r#"contract C {
    function f(uint[] memory a, uint[] memory b) public pure {
        for (uint i = 0; i < a.length; i++) {
            for (uint j = 0; j < b.length; j++) {
                a[i] = b[j];
            }
        }
    }
}
"#,
    )
    .expect("write solidity");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-001"
            && finding.path.to_string_lossy().ends_with("pair.zig")));
    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ALG-001"
            && finding.path.to_string_lossy().ends_with("Pair.sol")));
}

#[test]
fn flags_large_inline_data_literal_but_not_small_one() {
    let directory = tempdir().expect("temp directory");

    // 60 objects on one line each -> exceeds the 50-element threshold.
    let mut big = String::from("export const users = [\n");
    for i in 0..60 {
        big.push_str(&format!("  {{ id: {i}, name: \"user{i}\" }},\n"));
    }
    big.push_str("];\n");
    fs::write(directory.path().join("mock.ts"), &big).expect("write big");

    fs::write(
        directory.path().join("small.ts"),
        "export const roles = [\n  { id: 1, name: \"admin\" },\n  { id: 2, name: \"user\" },\n];\n",
    )
    .expect("write small");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ARCH-001"
            && finding.path.to_string_lossy().ends_with("mock.ts")));
    assert!(!findings
        .iter()
        .any(|finding| finding.rule_id == "FG-ARCH-001"
            && finding.path.to_string_lossy().ends_with("small.ts")));
}

#[test]
fn semantic_packs_propagate_database_and_network_wrappers() {
    let directory = tempdir().expect("temp directory");
    let sources = [
        (
            "service.ts",
            r#"import { PrismaClient } from "@prisma/client";
const prisma = new PrismaClient();
async function loadUser(id) { return prisma.user.findMany({ where: { id } }); }
for (const id of ids) { await loadUser(id); }
"#,
            "FG-DB-001",
        ),
        (
            "service.py",
            "import requests as http\ndef fetch_user(url):\n    return http.get(url)\nfor url in urls:\n    fetch_user(url)\n",
            "FG-NET-001",
        ),
        (
            "service.rs",
            "use sqlx;\nfn load_user(id: i64) { sqlx::query(\"SELECT id FROM users\"); }\nfn run(ids: &[i64]) { for id in ids { load_user(*id); } }\n",
            "FG-DB-001",
        ),
        (
            "service.go",
            "package service\nimport \"net/http\"\nfunc fetchUser(url string) { http.Get(url) }\nfunc run(urls []string) { for _, url := range urls { fetchUser(url) } }\n",
            "FG-NET-001",
        ),
    ];
    for (name, source, _) in sources {
        fs::write(directory.path().join(name), source).expect("write semantic source");
    }

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    for (name, _, rule_id) in sources {
        assert!(
            findings.iter().any(|finding| {
                finding.rule_id == rule_id && finding.path.as_path() == std::path::Path::new(name)
            }),
            "missing semantic finding {rule_id} for {name}"
        );
    }
}

#[test]
fn receiver_name_collision_is_only_heuristic_evidence() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("memory.ts"),
        r#"import fakeAxios from "not-axios";
const local = "axios";
for (const id of ids) { repository.findMany(id); client.get(id); fakeAxios.get(id); local.get(id); }
"#,
    )
    .expect("write source");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan project");

    assert!(!findings
        .iter()
        .any(|finding| { matches!(finding.rule_id.as_str(), "FG-DB-001" | "FG-NET-001") }));
    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-DB-002"));
    assert!(findings
        .iter()
        .any(|finding| finding.rule_id == "FG-NET-002"));
}

#[test]
fn changed_scope_resolves_wrapper_from_unchanged_file() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("database.ts"),
        r#"import { PrismaClient } from "@prisma/client";
const prisma = new PrismaClient();
export async function loadUser(id) { return prisma.user.findMany({ where: { id } }); }
"#,
    )
    .expect("write unchanged wrapper");
    fs::write(
        directory.path().join("service.ts"),
        "import { loadUser } from './database';\nfor (const id of ids) { await loadUser(id); }\n",
    )
    .expect("write changed caller");

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions {
            paths: Some(vec![std::path::PathBuf::from("service.ts")]),
        },
    )
    .expect("scan changed file");

    assert!(findings.iter().any(|finding| {
        finding.rule_id == "FG-DB-001"
            && finding.path.as_path() == std::path::Path::new("service.ts")
    }));
}
