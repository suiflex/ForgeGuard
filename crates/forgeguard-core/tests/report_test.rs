use std::fs;

use forgeguard_core::{report::render_sarif, run_gate, ForgeGuardConfig, GateOptions, GuardMode};
use tempfile::tempdir;

#[test]
fn sarif_contains_rule_location_confidence_and_blocking_state() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("repository.ts"),
        r#"import { PrismaClient } from "@prisma/client";
const prisma = new PrismaClient();
for (const id of ids) { prisma.user.findMany({ where: { id } }); }
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

    let sarif: serde_json::Value =
        serde_json::from_str(&render_sarif(&report).expect("render SARIF")).expect("parse SARIF");
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(sarif["runs"][0]["results"][0]["ruleId"], "FG-DB-001");
    assert_eq!(
        sarif["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
            ["uri"],
        "repository.ts"
    );
    assert_eq!(
        sarif["runs"][0]["results"][0]["properties"]["confidence"],
        "semantic"
    );
    assert_eq!(
        sarif["runs"][0]["results"][0]["properties"]["blocking"],
        true
    );
}
