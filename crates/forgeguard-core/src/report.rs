use std::fmt::Write;

use serde_json::json;

use crate::{doctor::DoctorReport, model::GateReport, rules::RULES, ProjectDetection, Severity};

pub const COMPACT_MAX_CHARS: usize = 2_000;
const COMPACT_MAX_FINDINGS: usize = 5;

pub fn render_detection(report: &ProjectDetection) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "ForgeGuard project detection");
    let _ = writeln!(output, "Root: {}", report.root.display());
    render_values(&mut output, "Languages", &report.languages);
    render_values(&mut output, "Frameworks", &report.frameworks);
    render_values(&mut output, "Package managers", &report.package_managers);
    render_values(&mut output, "Database tools", &report.database_tools);
    render_values(&mut output, "Test tools", &report.test_tools);
    if report.suggested_commands.is_empty() {
        let _ = writeln!(output, "Commands: none detected");
    } else {
        let _ = writeln!(output, "Commands:");
        for command in &report.suggested_commands {
            let _ = writeln!(output, "  - {}: {}", command.name, command.command);
        }
    }
    output
}

pub fn render_gate(report: &GateReport) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "ForgeGuard gate: {}", report.status.as_str());
    let _ = writeln!(
        output,
        "Findings: {} error(s), {} warning(s), {} info",
        report.summary.errors, report.summary.warnings, report.summary.info
    );
    if report.summary.findings_baselined > 0 {
        let _ = writeln!(
            output,
            "Baseline: {} existing finding(s) hidden",
            report.summary.findings_baselined
        );
    }
    let _ = writeln!(
        output,
        "Checks: {} passed, {} failed",
        report.summary.checks_passed, report.summary.checks_failed
    );

    if !report.findings.is_empty() {
        let _ = writeln!(output, "\nFindings:");
        for finding in &report.findings {
            let _ = writeln!(
                output,
                "  [{}] {} {}:{} — {}",
                finding.severity.as_str(),
                finding.rule_id,
                finding.path.display(),
                finding.line,
                finding.title
            );
            let _ = writeln!(output, "      Evidence: {}", finding.evidence);
            let _ = writeln!(output, "      Fix: {}", finding.recommendation);
        }
    }

    if !report.checks.is_empty() {
        let _ = writeln!(output, "\nChecks:");
        for check in &report.checks {
            let marker = if check.success { "PASS" } else { "FAIL" };
            let _ = writeln!(
                output,
                "  [{marker}] {} ({} ms): {}",
                check.name, check.duration_ms, check.command
            );
            if !check.success && !check.output.is_empty() {
                for line in check.output.lines().take(20) {
                    let _ = writeln!(output, "      {line}");
                }
            }
        }
    }
    output
}

pub fn render_gate_compact(report: &GateReport) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "ForgeGuard {}: {} error(s), {} warning(s), {} failed check(s).",
        report.status.as_str(),
        report.summary.errors,
        report.summary.warnings,
        report.summary.checks_failed
    );
    for finding in report.findings.iter().take(COMPACT_MAX_FINDINGS) {
        let _ = writeln!(
            output,
            "- {} {}:{}: {} Fix: {}",
            finding.rule_id,
            finding.path.display(),
            finding.line,
            finding.title,
            finding.recommendation
        );
    }
    let omitted = report.findings.len().saturating_sub(COMPACT_MAX_FINDINGS);
    if omitted > 0 {
        let _ = writeln!(output, "- {omitted} additional finding(s) omitted.");
    }
    for check in report.checks.iter().filter(|check| !check.success) {
        // forgeguard: allow FG-ALG-002 -- each output scanned once; O(total failed output lines)
        let evidence = check
            .output
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("command failed");
        let _ = writeln!(output, "- check {} failed: {evidence}", check.name);
    }
    truncate_chars(output.trim(), COMPACT_MAX_CHARS)
}

pub fn render_sarif(report: &GateReport) -> Result<String, serde_json::Error> {
    let rules = RULES
        .iter()
        .map(|rule| {
            json!({
                "id": rule.id,
                "name": rule.title,
                "shortDescription": {"text": rule.title},
                "defaultConfiguration": {"level": sarif_level(rule.default_severity)},
                "properties": {"confidence": rule.confidence.as_str()},
            })
        })
        .collect::<Vec<_>>();
    let results = report
        .findings
        .iter()
        .map(|finding| {
            json!({
                "ruleId": finding.rule_id,
                "level": sarif_level(finding.severity),
                "message": {
                    "text": format!("{} Evidence: {} Fix: {}", finding.title, finding.evidence, finding.recommendation),
                },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {"uri": finding.path.to_string_lossy()},
                        "region": {
                            "startLine": finding.line,
                            "endLine": finding.end_line.unwrap_or(finding.line),
                        },
                    }
                }],
                "partialFingerprints": {
                    "forgeguardFinding": format!("{}:{}:{}", finding.rule_id, finding.path.display(), finding.evidence),
                },
                "properties": {
                    "confidence": finding.confidence.as_str(),
                    "blocking": finding.blocking,
                },
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": {"name": "ForgeGuard", "rules": rules}},
            "results": results,
        }],
    }))
}

fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

pub fn render_doctor(report: &DoctorReport) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "ForgeGuard doctor: {}",
        if report.healthy && report.warnings.is_empty() {
            "healthy"
        } else if report.healthy {
            "healthy with warnings"
        } else {
            "needs attention"
        }
    );
    let _ = writeln!(
        output,
        "  Configuration: {}",
        marker(report.configuration_found)
    );
    let _ = writeln!(
        output,
        "  Git repository: {}",
        marker(report.git_repository)
    );
    for tool in &report.tools {
        let path = tool
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "not found".to_owned());
        let _ = writeln!(
            output,
            "  Tool {}: {} ({path})",
            tool.tool,
            marker(tool.available)
        );
    }
    for hook in &report.hooks {
        let state = if hook.configured {
            "ok"
        } else if hook.installed {
            "missing"
        } else {
            "not installed"
        };
        let _ = writeln!(
            output,
            "  Hook {}: {} ({})",
            hook.agent,
            state,
            hook.path.display()
        );
    }
    for warning in &report.warnings {
        let _ = writeln!(output, "  Warning: {warning}");
    }
    output
}

fn render_values(output: &mut String, title: &str, values: &[String]) {
    if values.is_empty() {
        let _ = writeln!(output, "{title}: none detected");
    } else {
        let _ = writeln!(output, "{title}: {}", values.join(", "));
    }
}

fn marker(value: bool) -> &'static str {
    if value {
        "ok"
    } else {
        "missing"
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let mut output: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    output.push('…');
    output
}
