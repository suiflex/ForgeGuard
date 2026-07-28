use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    config::ScanConfig,
    model::{Finding, Severity},
    scanner::{scan_project, ScanOptions},
};

pub const BASELINE_FILE: &str = ".forgeguard/baseline.json";
const BASELINE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    pub version: u32,
    pub findings: Vec<BaselineEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineEntry {
    pub rule_id: String,
    pub severity: Severity,
    pub path: String,
    pub evidence: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct BaselineKey {
    rule_id: String,
    severity: Severity,
    path: String,
    evidence: String,
}

impl Baseline {
    pub fn from_findings(findings: &[Finding]) -> Self {
        let mut counts = BTreeMap::new();
        for finding in findings {
            *counts.entry(key_for_finding(finding)).or_insert(0) += 1;
        }
        Self {
            version: BASELINE_VERSION,
            findings: counts
                .into_iter()
                .map(|(key, count)| BaselineEntry {
                    rule_id: key.rule_id,
                    severity: key.severity,
                    path: key.path,
                    evidence: key.evidence,
                    count,
                })
                .collect(),
        }
    }

    pub fn load(root: &Path) -> Result<Option<Self>> {
        let path = root.join(BASELINE_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let baseline: Self = serde_json::from_str(&source)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        if baseline.version != BASELINE_VERSION {
            bail!(
                "unsupported baseline version {} in {}; expected {}",
                baseline.version,
                path.display(),
                BASELINE_VERSION
            );
        }
        if baseline.findings.iter().any(|entry| entry.count == 0) {
            bail!("invalid zero-count finding in {}", path.display());
        }
        Ok(Some(baseline))
    }

    pub fn total_findings(&self) -> usize {
        self.findings
            .iter()
            .fold(0, |total, entry| total.saturating_add(entry.count))
    }

    pub fn filter(&self, findings: &mut Vec<Finding>) -> usize {
        let mut remaining = HashMap::new();
        for entry in &self.findings {
            let count = remaining.entry(key_for_entry(entry)).or_insert(0usize);
            *count = count.saturating_add(entry.count);
        }

        let mut filtered = 0;
        findings.retain(|finding| {
            let Some(count) = remaining.get_mut(&key_for_finding(finding)) else {
                return true;
            };
            if *count == 0 {
                return true;
            }
            *count -= 1;
            filtered += 1;
            false
        });
        filtered
    }
}

pub fn create_baseline(root: &Path, config: &ScanConfig, force: bool) -> Result<Baseline> {
    let path = root.join(BASELINE_FILE);
    if path.exists() && !force {
        bail!(
            "{} already exists; use `forgeguard baseline create --force` to replace it",
            path.display()
        );
    }

    let findings = scan_project(root, config, &ScanOptions::default())?;
    let baseline = Baseline::from_findings(&findings);
    let parent = path
        .parent()
        .context("ForgeGuard baseline path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let mut output =
        serde_json::to_string_pretty(&baseline).context("failed to serialize baseline")?;
    output.push('\n');
    fs::write(&path, output).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(baseline)
}

fn key_for_finding(finding: &Finding) -> BaselineKey {
    BaselineKey {
        rule_id: finding.rule_id.clone(),
        severity: finding.severity,
        path: portable_path(&finding.path),
        evidence: finding.evidence.clone(),
    }
}

fn key_for_entry(entry: &BaselineEntry) -> BaselineKey {
    BaselineKey {
        rule_id: entry.rule_id.clone(),
        severity: entry.severity,
        path: entry.path.clone(),
        evidence: entry.evidence.clone(),
    }
}

fn portable_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
