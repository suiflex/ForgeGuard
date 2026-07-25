use std::fmt::Write;

use crate::{doctor::DoctorReport, model::GateReport, ProjectDetection};

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

pub fn render_doctor(report: &DoctorReport) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "ForgeGuard doctor: {}",
        if report.healthy { "healthy" } else { "needs attention" }
    );
    let _ = writeln!(
        output,
        "  Configuration: {}",
        marker(report.configuration_found)
    );
    let _ = writeln!(output, "  Git repository: {}", marker(report.git_repository));
    for tool in &report.tools {
        let path = tool
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "not found".to_owned());
        let _ = writeln!(output, "  Tool {}: {} ({path})", tool.tool, marker(tool.available));
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
    if value { "ok" } else { "missing" }
}
