use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{
    config::ScanConfig,
    git::ChangedScope,
    model::{EvidenceConfidence, Finding, Severity},
    scanner::is_supported_source,
};

pub(crate) fn changed_coverage_finding(
    root: &Path,
    config: &ScanConfig,
    scope: &ChangedScope,
) -> Result<Option<Finding>> {
    let (Some(report), Some(minimum)) = (&config.coverage_report, config.min_changed_coverage)
    else {
        return Ok(None);
    };
    if !scope.paths.iter().any(|path| is_supported_source(path)) {
        return Ok(None);
    }
    let report_path = root.join(report);
    if !report_path.is_file() {
        return Ok(Some(coverage_finding(
            report,
            format!("configured LCOV report is missing: {}", report.display()),
            minimum,
        )));
    }

    let source = fs::read_to_string(&report_path)
        .with_context(|| format!("failed to read {}", report_path.display()))?;
    let mut current = None;
    let mut covered = 0usize;
    let mut coverable = 0usize;
    for line in source.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            current = Some(normalize_path(root, Path::new(path)));
            continue;
        }
        let Some((line_number, hits)) = line
            .strip_prefix("DA:")
            .and_then(|record| record.split_once(','))
        else {
            continue;
        };
        let (Ok(line_number), Ok(hits)) = (
            line_number.parse::<usize>(),
            hits.split(',').next().unwrap_or_default().parse::<u64>(),
        ) else {
            continue;
        };
        let Some(path) = current.as_ref() else {
            continue;
        };
        if scope.lines.get(path).is_some_and(|ranges| {
            ranges
                .iter()
                .any(|(start, end)| (*start..=*end).contains(&line_number))
        }) {
            coverable += 1;
            covered += usize::from(hits > 0);
        }
    }
    if coverable == 0 {
        return Ok(None);
    }
    let percent = covered.saturating_mul(100) / coverable;
    Ok((percent < minimum as usize).then(|| {
        coverage_finding(
            report,
            format!("changed-line coverage is {percent}% ({covered}/{coverable})"),
            minimum,
        )
    }))
}

fn normalize_path(root: &Path, path: &Path) -> PathBuf {
    let relative = if path.is_absolute() {
        path.strip_prefix(root).unwrap_or(path)
    } else {
        path.strip_prefix(".").unwrap_or(path)
    };
    relative.to_path_buf()
}

fn coverage_finding(path: &Path, evidence: String, minimum: u8) -> Finding {
    Finding {
        rule_id: "FG-COV-001".to_owned(),
        title: "Changed-line coverage below policy".to_owned(),
        severity: Severity::Warning,
        confidence: EvidenceConfidence::Deterministic,
        blocking: false,
        path: path.to_path_buf(),
        line: 1,
        end_line: None,
        evidence,
        recommendation: format!(
            "Add focused tests and regenerate LCOV until changed-line coverage is at least {minimum}%."
        ),
    }
}
