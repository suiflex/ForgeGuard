use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{
    baseline::Baseline,
    config::ForgeGuardConfig,
    model::{GateReport, GateStatus, GateSummary, Severity},
    runner::run_checks,
    scanner::{scan_project, ScanOptions},
};

#[derive(Debug, Clone, Default)]
pub struct GateOptions {
    pub skip_commands: bool,
    pub paths: Option<Vec<PathBuf>>,
}

pub fn run_gate(
    root: &Path,
    config: &ForgeGuardConfig,
    options: &GateOptions,
) -> Result<GateReport> {
    let mut findings = scan_project(
        root,
        &config.scan,
        &ScanOptions {
            paths: options.paths.clone(),
        },
    )?;
    findings.dedup_by(|left, right| {
        left.rule_id == right.rule_id && left.path == right.path && left.line == right.line
    });
    findings.retain_mut(|finding| config.apply_rule(&finding.rule_id, &mut finding.severity));
    let findings_baselined = match Baseline::load(root)? {
        Some(baseline) => baseline.filter(&mut findings),
        None => 0,
    };
    let checks = if options.skip_commands {
        Vec::new()
    } else {
        run_checks(root, &config.commands)
    };

    let errors = findings
        .iter()
        .filter(|finding| finding.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|finding| finding.severity == Severity::Warning)
        .count();
    let info = findings
        .iter()
        .filter(|finding| finding.severity == Severity::Info)
        .count();
    for finding in &mut findings {
        finding.blocking = config.blocks_finding(&finding.rule_id, finding.severity);
    }
    let blocking_findings = findings.iter().filter(|finding| finding.blocking).count();
    let checks_passed = checks.iter().filter(|check| check.success).count();
    let checks_failed = checks.iter().filter(|check| !check.success).count();
    let required_check_failed = checks.iter().any(|check| check.required && !check.success);

    let status = if required_check_failed || blocking_findings > 0 {
        GateStatus::Blocked
    } else if errors + warnings + info + checks_failed > 0 {
        GateStatus::Warning
    } else {
        GateStatus::Passed
    };

    Ok(GateReport {
        status,
        findings,
        checks,
        summary: GateSummary {
            errors,
            warnings,
            info,
            blocking_findings,
            findings_baselined,
            checks_passed,
            checks_failed,
        },
    })
}
