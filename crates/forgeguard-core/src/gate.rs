use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{
    baseline::Baseline,
    config::ForgeGuardConfig,
    coverage::changed_coverage_finding,
    git::changed_scope,
    model::{GateReport, GateStatus, GateSummary, Severity},
    runner::{run_checks, run_checks_for_changes},
    scanner::{scan_changed_project, scan_project, ScanOptions},
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
    let findings = scan_project(
        root,
        &config.scan,
        &ScanOptions {
            paths: options.paths.clone(),
        },
    )?;
    finish_gate(root, config, options.skip_commands, findings, None)
}

pub fn run_changed_gate(
    root: &Path,
    config: &ForgeGuardConfig,
    skip_commands: bool,
    base: Option<&str>,
) -> Result<GateReport> {
    let scope = changed_scope(root, base)?;
    let mut findings = scan_changed_project(root, &config.scan, &scope)?;
    if let Some(finding) = changed_coverage_finding(root, &config.scan, &scope)? {
        findings.push(finding);
    }
    finish_gate(root, config, skip_commands, findings, Some(&scope.paths))
}

fn finish_gate(
    root: &Path,
    config: &ForgeGuardConfig,
    skip_commands: bool,
    mut findings: Vec<crate::Finding>,
    changed_paths: Option<&[PathBuf]>,
) -> Result<GateReport> {
    findings.dedup_by(|left, right| {
        left.rule_id == right.rule_id && left.path == right.path && left.line == right.line
    });
    findings.retain_mut(|finding| config.apply_rule(&finding.rule_id, &mut finding.severity));
    let findings_baselined = match Baseline::load(root)? {
        Some(baseline) => baseline.filter(&mut findings),
        None => 0,
    };
    let checks = if skip_commands {
        Vec::new()
    } else {
        match changed_paths {
            Some(paths) => run_checks_for_changes(root, &config.commands, Some(paths)),
            None => run_checks(root, &config.commands),
        }
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
