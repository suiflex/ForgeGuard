use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    config::{ForgeGuardConfig, ScanConfig},
    model::{Finding, Severity},
    scanner::{scan_project, ScanOptions},
};

pub const BASELINE_FILE: &str = ".forgeguard/baseline.json";
const BASELINE_VERSION: u32 = 2;

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
    path: String,
    evidence: String,
}

impl Baseline {
    pub fn from_findings(findings: &[Finding]) -> Self {
        let mut counts = BTreeMap::new();
        for finding in findings {
            let (_, count) = counts
                .entry(key_for_finding(finding))
                .or_insert((finding.severity, 0usize));
            *count = count.saturating_add(1);
        }
        Self {
            version: BASELINE_VERSION,
            findings: counts
                .into_iter()
                .map(|(key, (severity, count))| BaselineEntry {
                    rule_id: key.rule_id,
                    severity,
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
        if !matches!(baseline.version, 1 | BASELINE_VERSION) {
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
    ensure_baseline_can_be_created(root, force)?;
    let findings = scan_project(root, config, &ScanOptions::default())?;
    write_baseline(root, &findings)
}

pub fn create_baseline_with_config(
    root: &Path,
    config: &ForgeGuardConfig,
    force: bool,
) -> Result<Baseline> {
    ensure_baseline_can_be_created(root, force)?;
    let mut findings = scan_project(root, &config.scan, &ScanOptions::default())?;
    findings.retain_mut(|finding| config.apply_rule(&finding.rule_id, &mut finding.severity));
    write_baseline(root, &findings)
}

fn ensure_baseline_can_be_created(root: &Path, force: bool) -> Result<()> {
    let path = root.join(BASELINE_FILE);
    if path.exists() && !force {
        bail!(
            "{} already exists; use `forgeguard baseline create --force` to replace it",
            path.display()
        );
    }
    Ok(())
}

fn write_baseline(root: &Path, findings: &[Finding]) -> Result<Baseline> {
    let path = root.join(BASELINE_FILE);
    let baseline = Baseline::from_findings(findings);
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
        path: portable_path(&finding.path),
        evidence: finding.evidence.clone(),
    }
}

fn key_for_entry(entry: &BaselineEntry) -> BaselineKey {
    BaselineKey {
        rule_id: entry.rule_id.clone(),
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
