use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    config::{ForgeGuardConfig, CONFIG_FILE},
    git::{changed_files, worktree_fingerprint},
    report::{render_gate_compact, COMPACT_MAX_CHARS},
    run_gate, GateOptions, GateStatus,
};

const CACHE_FILE: &str = ".forgeguard/cache/stop.json";
const REPORT_FILE: &str = ".forgeguard/reports/latest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookAgent {
    Codex,
    Claude,
    Cursor,
    Antigravity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    Pass,
    Block(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct HookCache {
    fingerprint: String,
    status: GateStatus,
    message: String,
}

pub fn evaluate_stop_hook(fallback_root: &Path, input: &str) -> Result<(HookDecision, bool)> {
    let payload: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    let Some(root) = find_project_root(fallback_root, &payload) else {
        return Ok((HookDecision::Pass, false));
    };
    let Some(worktree) = worktree_fingerprint(&root)? else {
        return Ok((HookDecision::Pass, false));
    };
    let fingerprint = format!("{}:{worktree}", env!("CARGO_PKG_VERSION"));
    let repeated_stop = payload
        .get("stop_hook_active")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || payload
            .get("loop_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0)
        || payload
            .get("executionNum")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 1);

    if let Some(cache) = read_cache(&root) {
        if cache.fingerprint == fingerprint {
            let decision = if cache.status == GateStatus::Blocked && !repeated_stop {
                HookDecision::Block(cache.message)
            } else {
                HookDecision::Pass
            };
            return Ok((decision, true));
        }
    }

    let config = ForgeGuardConfig::load(&root)?;
    let report = run_gate(
        &root,
        &config,
        &GateOptions {
            skip_commands: false,
            paths: Some(changed_files(&root)?),
        },
    )?;
    write_json(&root.join(REPORT_FILE), &report)?;
    let message = if report.status == GateStatus::Blocked {
        compact_failure(&report)
    } else {
        String::new()
    };
    write_json(
        &root.join(CACHE_FILE),
        &HookCache {
            fingerprint,
            status: report.status,
            message: message.clone(),
        },
    )?;

    Ok((
        if report.status == GateStatus::Blocked {
            HookDecision::Block(message)
        } else {
            HookDecision::Pass
        },
        false,
    ))
}

pub fn render_hook_decision(agent: HookAgent, decision: &HookDecision) -> String {
    match (agent, decision) {
        (HookAgent::Cursor, HookDecision::Pass) => "{}".to_owned(),
        (HookAgent::Antigravity, HookDecision::Pass) => json!({"decision": "stop"}).to_string(),
        (_, HookDecision::Pass) => String::new(),
        (HookAgent::Claude, HookDecision::Block(reason)) => {
            json!({"decision": "block", "reason": reason}).to_string()
        }
        (HookAgent::Cursor, HookDecision::Block(reason)) => {
            json!({"followup_message": reason}).to_string()
        }
        (HookAgent::Codex, HookDecision::Block(reason)) => json!({
            "continue": true,
            "systemMessage": reason
        })
        .to_string(),
        (HookAgent::Antigravity, HookDecision::Block(reason)) => json!({
            "decision": "continue",
            "reason": reason
        })
        .to_string(),
    }
}

fn find_project_root(fallback_root: &Path, payload: &Value) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
        candidates.push(PathBuf::from(cwd));
    }
    if let Some(roots) = payload.get("workspace_roots").and_then(Value::as_array) {
        candidates.extend(roots.iter().filter_map(Value::as_str).map(PathBuf::from));
    }
    if let Some(roots) = payload.get("workspacePaths").and_then(Value::as_array) {
        candidates.extend(roots.iter().filter_map(Value::as_str).map(PathBuf::from));
    }
    candidates.push(fallback_root.to_path_buf());

    candidates.into_iter().find_map(|candidate| {
        let start = if candidate.is_file() {
            candidate.parent()?
        } else {
            candidate.as_path()
        };
        start
            .ancestors()
            .find(|path| path.join(CONFIG_FILE).is_file())
            .map(Path::to_path_buf)
    })
}

fn compact_failure(report: &crate::GateReport) -> String {
    let suffix = "\nFull report: .forgeguard/reports/latest.json";
    let available = COMPACT_MAX_CHARS.saturating_sub(suffix.chars().count());
    let compact = render_gate_compact(report);
    let mut output: String = compact.chars().take(available).collect();
    output.push_str(suffix);
    output
}

fn read_cache(root: &Path) -> Option<HookCache> {
    let source = fs::read_to_string(root.join(CACHE_FILE)).ok()?;
    serde_json::from_str(&source).ok()
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .context("ForgeGuard report path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let output = serde_json::to_vec_pretty(value).context("failed to serialize ForgeGuard data")?;
    fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))
}
