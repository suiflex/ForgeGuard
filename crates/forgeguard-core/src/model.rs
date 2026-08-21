use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceConfidence {
    Deterministic,
    Semantic,
    Structural,
    #[default]
    Heuristic,
}

impl EvidenceConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::Semantic => "semantic",
            Self::Structural => "structural",
            Self::Heuristic => "heuristic",
        }
    }
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub rule_id: String,
    pub title: String,
    pub severity: Severity,
    #[serde(default)]
    pub confidence: EvidenceConfidence,
    #[serde(default)]
    pub blocking: bool,
    pub path: PathBuf,
    pub line: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    pub evidence: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateStatus {
    Passed,
    Warning,
    Blocked,
}

impl GateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Warning => "warning",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub command: String,
    pub required: bool,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateSummary {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub blocking_findings: usize,
    pub findings_baselined: usize,
    pub checks_passed: usize,
    pub checks_failed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateReport {
    pub status: GateStatus,
    pub findings: Vec<Finding>,
    pub checks: Vec<CheckResult>,
    pub summary: GateSummary,
}
