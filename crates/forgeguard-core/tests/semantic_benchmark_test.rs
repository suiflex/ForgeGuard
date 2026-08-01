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
