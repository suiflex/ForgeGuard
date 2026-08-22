use std::fs;

use forgeguard_core::{config::ScanConfig, scan_project, ScanOptions};
use tempfile::tempdir;

const LABELS_PER_CELL: usize = 20;

#[test]
fn semantic_packs_meet_fixture_precision_and_recall_floor() {
    let cells = [
        Cell::new(
            "typescript-db",
            "ts",
            "import { PrismaClient } from \"@prisma/client\";\nconst prisma = new PrismaClient();\n",
            "prisma.user.findMany({})",
            "repository.findMany(id)",
            "FG-DB-001",
        ),
        Cell::new(
            "typescript-network",
            "ts",
            "import axios from \"axios\";\n",
            "axios.get(String(id))",
            "client.get(id)",
            "FG-NET-001",
        ),
        Cell::new(
            "python-db",
            "py",
            "from sqlalchemy.orm import Session\nsession = Session()\n",
            "session.execute(id)",
            "repository.findMany(id)",
            "FG-DB-001",
        ),
        Cell::new(
            "python-network",
            "py",
            "import requests\n",
            "requests.get(str(id))",
            "client.get(id)",
            "FG-NET-001",
        ),
        Cell::new(
            "rust-db",
            "rs",
            "use sqlx;\n",
            "sqlx::query(\"SELECT 1\")",
            "repository.find_many(id)",
            "FG-DB-001",
        ),
        Cell::new(
            "rust-network",
            "rs",
            "use reqwest;\n",
            "reqwest::get(id)",
            "client.get(id)",
            "FG-NET-001",
        ),
        Cell::new(
            "go-db",
            "go",
            "package benchmark\nimport \"database/sql\"\n",
            "sql.Open(\"sqlite\", id)",
            "repository.FindMany(id)",
            "FG-DB-001",
        ),
        Cell::new(
            "go-network",
            "go",
            "package benchmark\nimport \"net/http\"\n",
            "http.Get(id)",
            "client.Get(id)",
            "FG-NET-001",
        ),
    ];

    for cell in cells {
        let directory = tempdir().expect("temp directory");
        fs::write(
            directory
                .path()
                .join(format!("positive.{}", cell.extension)),
            cell.source(cell.positive),
        )
        .expect("write positive corpus");
        fs::write(
            directory
                .path()
                .join(format!("negative.{}", cell.extension)),
            cell.source(cell.negative),
        )
        .expect("write negative corpus");

        let findings = scan_project(
            directory.path(),
            &ScanConfig::default(),
            &ScanOptions::default(),
        )
        .expect("scan benchmark corpus");
        let true_positives = findings
            .iter()
            .filter(|finding| {
                finding.rule_id == cell.rule
                    && finding.path.to_string_lossy().starts_with("positive")
            })
            .count();
        let false_positives = findings
            .iter()
            .filter(|finding| {
                finding.rule_id == cell.rule
                    && finding.path.to_string_lossy().starts_with("negative")
            })
            .count();
        let false_negatives = LABELS_PER_CELL.saturating_sub(true_positives);
        let precision = ratio(true_positives, true_positives + false_positives);
        let recall = ratio(true_positives, true_positives + false_negatives);

        assert!(precision >= 0.95, "{} precision {precision:.3}", cell.name);
        assert!(recall >= 0.85, "{} recall {recall:.3}", cell.name);
    }
}

#[test]
fn security_rules_meet_fixture_precision_and_recall_floor() {
    let cases = [
        SecurityCase::positive(
            "xss-raw",
            "ts",
            "function render(input) { element.innerHTML = input; }\n",
            "FG-SEC-006",
        ),
        SecurityCase::negative(
            "xss-escaped",
            "ts",
            "function render(input) { element.innerHTML = escapeHtml(input); }\n",
            "FG-SEC-006",
        ),
        SecurityCase::positive(
            "path-request",
            "ts",
            "import fs from 'fs'; function read() { fs.readFile(req.query.path); }\n",
            "FG-SEC-007",
        ),
        SecurityCase::negative(
            "path-sanitized-filename",
            "ts",
            "import fs from 'fs'; function read() { fs.readFile(sanitizeFilename(req.query.path)); }\n",
            "FG-SEC-007",
        ),
        SecurityCase::positive(
            "path-basename-is-only-normalization",
            "ts",
            "import fs from 'fs'; function read() { fs.readFile(path.basename(req.query.path)); }\n",
            "FG-SEC-007",
        ),
        SecurityCase::positive(
            "yaml-unsafe",
            "py",
            "import yaml\ndef decode(value):\n    return yaml.load(value)\n",
            "FG-SEC-005",
        ),
        SecurityCase::negative(
            "yaml-safe",
            "py",
            "import yaml\ndef decode(value):\n    return yaml.load(value, Loader=yaml.SafeLoader)\n",
            "FG-SEC-005",
        ),
        SecurityCase::positive(
            "crypto-sha1",
            "ts",
            "function hash(value) { return createHash('sha1').update(value); }\n",
            "FG-SEC-004",
        ),
        SecurityCase::negative(
            "crypto-sha256",
            "ts",
            "function hash(value) { return createHash('sha256').update(value); }\n",
            "FG-SEC-004",
        ),
        SecurityCase::positive(
            "auth-missing",
            "ts",
            "app.post('/users', createUser);\n",
            "FG-AUTH-001",
        ),
        SecurityCase::negative(
            "auth-handler",
            "ts",
            "function createUser(req) { policy.check(req.user); save(req.body); }\napp.post('/users', createUser);\n",
            "FG-AUTH-001",
        ),
        SecurityCase::positive(
            "exception-empty",
            "ts",
            "try { work(); } catch (error) {}\n",
            "FG-ERR-001",
        ),
        SecurityCase::negative(
            "exception-rethrow",
            "ts",
            "try { work(); } catch (error) { throw error; }\n",
            "FG-ERR-001",
        ),
    ];
    let directory = tempdir().expect("temp directory");
    for case in &cases {
        fs::write(
            directory
                .path()
                .join(format!("{}.{}", case.name, case.extension)),
            case.source,
        )
        .expect("write security corpus");
    }

    let findings = scan_project(
        directory.path(),
        &ScanConfig::default(),
        &ScanOptions::default(),
    )
    .expect("scan security corpus");
    let mut true_positives = 0;
    let mut false_positives = 0;
    let mut false_negatives = 0;
    let mut missed = Vec::new();
    let mut noisy = Vec::new();
    for case in &cases {
        let found = findings.iter().any(|finding| {
            finding.rule_id == case.rule && finding.path.to_string_lossy().starts_with(case.name)
        });
        match (case.expected, found) {
            (true, true) => true_positives += 1,
            (true, false) => {
                false_negatives += 1;
                missed.push(case.name);
            }
            (false, true) => {
                false_positives += 1;
                noisy.push(case.name);
            }
            (false, false) => {}
        }
    }
    let precision = ratio(true_positives, true_positives + false_positives);
    let recall = ratio(true_positives, true_positives + false_negatives);

    assert!(
        precision >= 0.95,
        "security precision {precision:.3}; noisy={noisy:?}"
    );
    assert!(
        recall >= 0.90,
        "security recall {recall:.3}; missed={missed:?}"
    );
}

struct SecurityCase {
    name: &'static str,
    extension: &'static str,
    source: &'static str,
    rule: &'static str,
    expected: bool,
}

impl SecurityCase {
    const fn positive(
        name: &'static str,
        extension: &'static str,
        source: &'static str,
        rule: &'static str,
    ) -> Self {
        Self {
            name,
            extension,
            source,
            rule,
            expected: true,
        }
    }

    const fn negative(
        name: &'static str,
        extension: &'static str,
        source: &'static str,
        rule: &'static str,
    ) -> Self {
        Self {
            name,
            extension,
            source,
            rule,
            expected: false,
        }
    }
}

struct Cell {
    name: &'static str,
    extension: &'static str,
    prelude: &'static str,
    positive: &'static str,
    negative: &'static str,
    rule: &'static str,
}

impl Cell {
    const fn new(
        name: &'static str,
        extension: &'static str,
        prelude: &'static str,
        positive: &'static str,
        negative: &'static str,
        rule: &'static str,
    ) -> Self {
        Self {
            name,
            extension,
            prelude,
            positive,
            negative,
            rule,
        }
    }

    fn source(&self, call: &str) -> String {
        let calls = (0..LABELS_PER_CELL)
            .map(|_| format!("{call};"))
            .collect::<Vec<_>>()
            .join(" ");
        match self.extension {
            "py" => format!(
                "{}for id in ids:\n{}\n",
                self.prelude,
                calls
                    .split(';')
                    .filter(|line| !line.is_empty())
                    .map(|line| format!("    {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            "rs" => format!(
                "{}fn run(ids: &[&str]) {{ for id in ids {{ {calls} }} }}\n",
                self.prelude
            ),
            "go" => format!(
                "{}func run(ids []string) {{ for _, id := range ids {{ {calls} }} }}\n",
                self.prelude
            ),
            _ => format!("{}for (const id of ids) {{ {calls} }}\n", self.prelude),
        }
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        1.0
    } else {
        numerator as f64 / denominator as f64
    }
}
