use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    env, fs,
    hash::Hasher,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    config::{FocusConfig, ForgeGuardConfig, GuardMode, CONFIG_FILE},
    detect_project,
    git::{changed_paths_partitioned, repository_roots, worktree_fingerprint},
    model::GateSummary,
    report::{render_gate_compact, COMPACT_MAX_CHARS},
    run_changed_gate,
    scanner::{executable_placeholders, ExecutablePlaceholder},
    GateStatus,
};

const CACHE_DIR: &str = ".forgeguard/cache/stop";
/// A second hook registration for the same event (for example a global and a
/// project `settings.json` both installing the stop hook) fires within
/// milliseconds. Replaying the first decision inside this window keeps retry,
/// no-progress, and auto-poke budgets tied to agent turns instead of to the
/// number of installed hook entries.
const DUPLICATE_STOP_WINDOW_MILLIS: u64 = 1_500;
/// Extensions that cannot change build, lint, or test results, so a stop-hook
/// gate over only these paths runs the scanner without the configured
/// commands.
const DOCUMENTATION_EXTENSIONS: [&str; 17] = [
    "md", "mdx", "markdown", "rst", "adoc", "asciidoc", "org", "txt", "text", "png", "jpg", "jpeg",
    "gif", "webp", "svg", "ico", "pdf",
];
const TASK_DIR: &str = ".forgeguard/cache/tasks";
const REPORT_FILE: &str = ".forgeguard/reports/latest.json";
const MAX_OBJECTIVE_CHARS: usize = 4_000;
const MAX_EVIDENCE_ITEMS: usize = 8;
const MAX_EVIDENCE_CHARS: usize = 500;
const MAX_TASK_ITEMS: usize = 32;
const MAX_TASK_ITEM_CHARS: usize = 500;
const MAX_CONFIDENCE_HISTORY: usize = 16;
const FOCUS_FEEDBACK_RESERVE: usize = 320;
const MAX_AUTO_POKES: u32 = 5;
const CODE_AUTO_POKE_PHASES: [&str; 5] = [
    "reinspect the objective and remaining TODOs; fix any concrete gap",
    "run relevant tests and checks; fix every observed failure",
    "review the full diff for correctness, regressions, and scope drift",
    "verify contracts, edge cases, and failure handling against repository evidence",
    "run final verification and confirm no objective gap remains",
];
const GENERAL_AUTO_POKE_PHASES: [&str; 5] = [
    "reinspect the objective and remaining TODOs; fix any concrete gap",
    "verify the deliverable and every result against executed evidence",
    "review the deliverable for correctness, completeness, and scope drift",
    "verify claims, numbers, edge cases, and failure handling against evidence",
    "run final verification and confirm no objective gap remains",
];
const CLARIFICATION_CONTEXT: &str = "Before acting, resolve missing facts with read-only inspection. If unresolved ambiguity materially changes behavior, data, security, scope, cost, or external or irreversible state, {question_instruction} and wait for the answer. Otherwise use the safest reversible default and state it briefly.";

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
    Stop(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct HookReplay {
    state: String,
    at_millis: u64,
    stop: bool,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct HookCache {
    fingerprint: String,
    status: GateStatus,
    message: String,
    attempts: u32,
    stalled_attempts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Active,
    Ready,
    Completed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GoalContract {
    pub metric: Option<String>,
    pub baseline: Option<String>,
    pub target: Option<String>,
    #[serde(default)]
    pub guardrails: Vec<String>,
    #[serde(default)]
    pub verifications: Vec<String>,
}

impl GoalContract {
    pub fn hill_climbability(&self) -> u8 {
        let mut score = 0;
        score += u8::from(self.metric.is_some()) * 20;
        score += u8::from(self.baseline.is_some()) * 20;
        score += u8::from(self.target.is_some()) * 20;
        score += u8::from(!self.guardrails.is_empty()) * 20;
        score += u8::from(!self.verifications.is_empty()) * 20;
        score
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskTodo {
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskState {
    pub version: u32,
    pub session_id: String,
    pub objective: String,
    pub scopes: Vec<String>,
    pub semantic: bool,
    #[serde(default)]
    pub goal: GoalContract,
    #[serde(default)]
    pub todos: Vec<TaskTodo>,
    #[serde(default)]
    pub confidence: Option<u8>,
    #[serde(default)]
    pub confidence_history: Vec<u8>,
    #[serde(default)]
    pub auto_pokes: u32,
    #[serde(default)]
    pub poke_instruction: Option<String>,
    pub status: TaskStatus,
    pub evidence: Vec<String>,
    pub blocker: Option<String>,
}

pub fn evaluate_stop_hook(fallback_root: &Path, input: &str) -> Result<(HookDecision, bool)> {
    let payload: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    let Some(root) = find_project_root(fallback_root, &payload) else {
        return Ok((HookDecision::Pass, false));
    };
    let session = session_key(&payload);
    if !is_hook_project(&root)? {
        return evaluate_general_stop_hook(&root, &session, &global_focus_config());
    }
    let repositories = repository_roots(&root)?;
    if repositories.is_empty() {
        return Ok((HookDecision::Pass, false));
    }
    let worktree = workspace_fingerprint(&root, &repositories)?;
    let worktree_key = worktree.as_deref().unwrap_or("clean").to_owned();
    if let Some(decision) = replayed_decision(
        &root,
        &session,
        &stop_state_key(&root, &session, &worktree_key)?,
    ) {
        return Ok((decision, true));
    }
    let evaluated = evaluate_stop_state(&root, &session, &repositories, worktree)?;
    // The decision may have advanced the task, so the replay key describes the
    // state a duplicate hook registration observes, not the state this call saw.
    record_replay(
        &root,
        &session,
        &stop_state_key(&root, &session, &worktree_key)?,
        &evaluated.0,
    )?;
    Ok(evaluated)
}

fn evaluate_general_stop_hook(
    root: &Path,
    session: &str,
    focus: &FocusConfig,
) -> Result<(HookDecision, bool)> {
    let state = stop_state_key(root, session, "general")?;
    if let Some(decision) = replayed_decision(root, session, &state) {
        return Ok((decision, true));
    }
    let Some(mut task) = read_task(root, session)? else {
        return Ok((HookDecision::Pass, false));
    };
    let decision = match task.status {
        TaskStatus::Active if focus.enabled => active_auto_poke(root, &mut task, focus, "work")?
            .unwrap_or_else(|| HookDecision::Block(incomplete_task_message(session))),
        TaskStatus::Active => HookDecision::Pass,
        TaskStatus::Ready if focus.enabled => {
            match ready_auto_poke(root, &mut task, focus, &GENERAL_AUTO_POKE_PHASES, "work")? {
                Some(decision) => decision,
                None => {
                    task.status = TaskStatus::Completed;
                    write_task(root, &task)?;
                    HookDecision::Pass
                }
            }
        }
        TaskStatus::Ready => {
            task.status = TaskStatus::Completed;
            write_task(root, &task)?;
            HookDecision::Pass
        }
        TaskStatus::Completed | TaskStatus::Blocked => HookDecision::Pass,
    };
    record_replay(
        root,
        session,
        &stop_state_key(root, session, "general")?,
        &decision,
    )?;
    Ok((decision, false))
}

fn global_focus_config() -> FocusConfig {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    global_focus_config_at(home.as_deref())
}

fn global_focus_config_at(home: Option<&Path>) -> FocusConfig {
    home.and_then(|home| ForgeGuardConfig::load_global(home).ok())
        .map(|config| config.focus)
        .unwrap_or_default()
}

fn evaluate_stop_state(
    root: &Path,
    session: &str,
    repositories: &[PathBuf],
    worktree: Option<String>,
) -> Result<(HookDecision, bool)> {
    let root = root.to_path_buf();
    let session = session.to_owned();
    let mut task = read_task(&root, &session)?;
    if worktree.is_none() {
        match task.as_ref().map(|task| task.status) {
            None | Some(TaskStatus::Completed | TaskStatus::Blocked) => {
                return Ok((HookDecision::Pass, false));
            }
            Some(TaskStatus::Ready) => {
                let Some(config) = load_hook_config(&root)? else {
                    return Ok((HookDecision::Pass, false));
                };
                let task = task.as_mut().expect("matched ready task");
                if config.focus.enabled {
                    if let Some(decision) = ready_auto_poke(
                        &root,
                        task,
                        &config.focus,
                        &CODE_AUTO_POKE_PHASES,
                        "repository",
                    )? {
                        return Ok((decision, false));
                    }
                }
                task.status = TaskStatus::Completed;
                write_task(&root, task)?;
                return Ok((HookDecision::Pass, false));
            }
            Some(TaskStatus::Active) => {}
        }
    }
    let config = match load_hook_config(&root)? {
        Some(config) => config,
        None => return Ok((HookDecision::Pass, false)),
    };
    if config.focus.enabled {
        if let Some(task) = task
            .as_mut()
            .filter(|task| task.status == TaskStatus::Active)
        {
            if let Some(decision) = active_auto_poke(&root, task, &config.focus, "repository")? {
                return Ok((decision, false));
            }
        }
    }
    let fingerprint = format!(
        "{}:{}:{}",
        env!("CARGO_PKG_VERSION"),
        worktree.as_deref().unwrap_or("clean"),
        task_signature(task.as_ref())
    );
    let cache = read_cache(&root, &session);
    if config.mode != GuardMode::Lite {
        if let Some(message) = added_executable_placeholder(&root, repositories)? {
            return bounded_block(
                &root,
                &session,
                fingerprint,
                message,
                cache.as_ref(),
                &config.focus,
                false,
            );
        }
    }
    if let Some(cache) = &cache {
        if cache.fingerprint == fingerprint && cache.status == GateStatus::Blocked {
            return bounded_block(
                &root,
                &session,
                fingerprint,
                cache.message.clone(),
                Some(cache),
                &config.focus,
                true,
            );
        }
        if cache.fingerprint == fingerprint {
            return Ok((HookDecision::Pass, true));
        }
    }

    if worktree.is_none() {
        return bounded_block(
            &root,
            &session,
            fingerprint,
            incomplete_task_message(&session),
            cache.as_ref(),
            &config.focus,
            false,
        );
    }
    let report = run_workspace_gate(&root, repositories, &config)?;
    write_json(&root.join(REPORT_FILE), &report)?;
    if report.status == GateStatus::Blocked {
        return bounded_block(
            &root,
            &session,
            fingerprint,
            compact_failure(&report),
            cache.as_ref(),
            &config.focus,
            false,
        );
    }
    if config.focus.enabled {
        match task.as_ref() {
            Some(task) if task.status == TaskStatus::Active => {
                return bounded_block(
                    &root,
                    &session,
                    fingerprint,
                    incomplete_task_message(&session),
                    cache.as_ref(),
                    &config.focus,
                    false,
                );
            }
            None if config.mode == GuardMode::Strict => {
                return bounded_block(
                    &root,
                    &session,
                    fingerprint,
                    missing_task_message(&session),
                    cache.as_ref(),
                    &config.focus,
                    false,
                );
            }
            _ => {}
        }
        if let Some(task) = task
            .as_mut()
            .filter(|task| task.status == TaskStatus::Ready)
        {
            if let Some(decision) = ready_auto_poke(
                &root,
                task,
                &config.focus,
                &CODE_AUTO_POKE_PHASES,
                "repository",
            )? {
                return Ok((decision, false));
            }
        }
    }
    if let Some(mut task) = task.filter(|task| task.status == TaskStatus::Ready) {
        task.status = TaskStatus::Completed;
        write_task(&root, &task)?;
    }
    write_cache(
        &root,
        &session,
        &HookCache {
            fingerprint,
            status: report.status,
            message: String::new(),
            attempts: 0,
            stalled_attempts: 0,
        },
    )?;
    Ok((HookDecision::Pass, false))
}

fn added_executable_placeholder(
    workspace: &Path,
    repositories: &[PathBuf],
) -> Result<Option<String>> {
    for repository in repositories {
        let Some((path, placeholder)) = added_placeholder_in_repository(repository)? else {
            continue;
        };
        let prefix = repository.strip_prefix(workspace).unwrap_or(Path::new(""));
        return Ok(Some(format!(
            "ForgeGuard anti-slop hook blocked completion.\n- {}:{} added executable placeholder `{}`: {}\nReplace it with a complete implementation; committed placeholders and test code are ignored.",
            prefix.join(path).display(),
            placeholder.line,
            placeholder.kind,
            placeholder.evidence
        )));
    }
    Ok(None)
}

fn added_placeholder_in_repository(
    repository: &Path,
) -> Result<Option<(PathBuf, ExecutablePlaceholder)>> {
    let (_, existing) = changed_paths_partitioned(repository)?;
    for path in existing {
        if let Some(placeholder) = added_placeholder_in_file(repository, &path)? {
            return Ok(Some((path, placeholder)));
        }
    }
    Ok(None)
}

fn added_placeholder_in_file(
    repository: &Path,
    path: &Path,
) -> Result<Option<ExecutablePlaceholder>> {
    let Ok(source) = fs::read_to_string(repository.join(path)) else {
        return Ok(None);
    };
    let current = executable_placeholders(path, &source)?;
    let previous = source_at_head(repository, path)?;
    let mut previous_counts = HashMap::new();
    for placeholder in executable_placeholders(path, &previous)? {
        *previous_counts
            .entry((placeholder.kind, placeholder.fingerprint))
            .or_insert(0usize) += 1;
    }
    for placeholder in current {
        let count = previous_counts
            .entry((placeholder.kind, placeholder.fingerprint.clone()))
            .or_default();
        if *count == 0 {
            return Ok(Some(placeholder));
        }
        *count -= 1;
    }
    Ok(None)
}

fn source_at_head(root: &Path, path: &Path) -> Result<String> {
    let path = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let output = Command::new("git")
        .args(["show", "--no-textconv", &format!("HEAD:{path}")])
        .current_dir(root)
        .output()
        .context("failed to inspect committed source")?;
    Ok(if output.status.success() {
        String::from_utf8_lossy(&output.stdout).into_owned()
    } else {
        String::new()
    })
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
        (HookAgent::Codex, HookDecision::Block(reason)) => {
            json!({"decision": "block", "reason": reason}).to_string()
        }
        (HookAgent::Antigravity, HookDecision::Block(reason)) => json!({
            "decision": "continue",
            "reason": reason
        })
        .to_string(),
        (HookAgent::Cursor, HookDecision::Stop(_)) => "{}".to_owned(),
        (HookAgent::Antigravity, HookDecision::Stop(reason)) => {
            json!({"decision": "stop", "reason": reason}).to_string()
        }
        (_, HookDecision::Stop(reason)) => json!({
            "continue": false,
            "stopReason": reason,
            "systemMessage": reason
        })
        .to_string(),
    }
}

pub fn start_task(
    root: &Path,
    session_id: &str,
    objective: &str,
    scopes: &[String],
    semantic: bool,
) -> Result<TaskState> {
    start_task_with_contract(
        root,
        session_id,
        objective,
        scopes,
        semantic,
        GoalContract::default(),
        &[],
    )
}

pub fn start_task_with_contract(
    root: &Path,
    session_id: &str,
    objective: &str,
    scopes: &[String],
    semantic: bool,
    goal: GoalContract,
    todos: &[String],
) -> Result<TaskState> {
    validate_session_id(session_id)?;
    let objective = objective.trim();
    if objective.is_empty() {
        bail!("task objective must not be empty");
    }
    if objective.chars().count() > MAX_OBJECTIVE_CHARS {
        bail!("task objective exceeds {MAX_OBJECTIVE_CHARS} characters");
    }
    let mut scopes = scopes
        .iter()
        .map(|scope| normalize_scope(scope))
        .collect::<Result<Vec<_>>>()?;
    scopes.sort();
    scopes.dedup();
    let goal = normalize_goal_contract(goal)?;
    let todos = normalize_task_items("todo", todos, MAX_TASK_ITEMS)?
        .into_iter()
        .map(|text| TaskTodo { text, done: false })
        .collect();
    // Carrying the auto-poke budget over stops an agent from escaping it by
    // reframing the same work. A blocked task is different: the session already
    // reported its blocker and stopped, so the next objective starts fresh
    // instead of being stopped before its first turn.
    let previous = read_task(root, session_id)?;
    let (auto_pokes, confidence_history) = previous
        .filter(|task| {
            task.status != TaskStatus::Completed
                && !(task.status == TaskStatus::Blocked && task.objective != objective)
        })
        .map_or((0, Vec::new()), |task| {
            (task.auto_pokes, task.confidence_history)
        });
    let task = TaskState {
        version: 1,
        session_id: session_id.to_owned(),
        objective: objective.to_owned(),
        scopes,
        semantic,
        goal,
        todos,
        confidence: None,
        confidence_history,
        auto_pokes,
        poke_instruction: None,
        status: TaskStatus::Active,
        evidence: Vec::new(),
        blocker: None,
    };
    write_task(root, &task)?;
    Ok(task)
}

pub fn mark_task_ready(root: &Path, session_id: &str, evidence: &[String]) -> Result<TaskState> {
    mark_task_ready_with_confidence(root, session_id, evidence, None)
}

pub fn mark_task_ready_with_confidence(
    root: &Path,
    session_id: &str,
    evidence: &[String],
    confidence: Option<u8>,
) -> Result<TaskState> {
    validate_session_id(session_id)?;
    if evidence.is_empty() {
        bail!("at least one executed check or tool result is required as evidence");
    }
    if evidence.len() > MAX_EVIDENCE_ITEMS {
        bail!("task evidence exceeds {MAX_EVIDENCE_ITEMS} items");
    }
    let evidence = evidence
        .iter()
        .map(|item| {
            let item = item.trim();
            if item.is_empty() {
                bail!("task evidence must not be empty");
            }
            if item.chars().count() > MAX_EVIDENCE_CHARS {
                bail!("task evidence exceeds {MAX_EVIDENCE_CHARS} characters");
            }
            Ok(item.to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    let mut task = read_task(root, session_id)?
        .with_context(|| format!("no ForgeGuard task found for session {session_id}"))?;
    let pending = task.todos.iter().filter(|todo| !todo.done).count();
    if pending > 0 {
        bail!("{pending} task todo(s) remain incomplete");
    }
    if let Some(confidence) = confidence {
        if confidence > 100 {
            bail!("task confidence must be between 0 and 100");
        }
        task.confidence = Some(confidence);
        task.confidence_history.push(confidence);
        if task.confidence_history.len() > MAX_CONFIDENCE_HISTORY {
            task.confidence_history.remove(0);
        }
    }
    task.status = TaskStatus::Ready;
    task.evidence = evidence;
    task.poke_instruction = None;
    task.blocker = None;
    write_task(root, &task)?;
    Ok(task)
}

pub fn update_task_todos(
    root: &Path,
    session_id: &str,
    additions: &[String],
    completed: &[usize],
) -> Result<TaskState> {
    validate_session_id(session_id)?;
    let mut task = read_task(root, session_id)?
        .with_context(|| format!("no ForgeGuard task found for session {session_id}"))?;
    let additions = normalize_task_items(
        "todo",
        additions,
        MAX_TASK_ITEMS.saturating_sub(task.todos.len()),
    )?;
    let changed = !additions.is_empty() || !completed.is_empty();
    task.todos.extend(
        additions
            .into_iter()
            .map(|text| TaskTodo { text, done: false }),
    );
    for index in completed {
        if *index == 0 || *index > task.todos.len() {
            bail!("todo index {index} is outside 1..={}", task.todos.len());
        }
        task.todos[*index - 1].done = true;
    }
    if !changed {
        bail!("provide at least one todo addition or completed index");
    }
    task.status = TaskStatus::Active;
    task.confidence = None;
    task.poke_instruction = None;
    task.blocker = None;
    write_task(root, &task)?;
    Ok(task)
}

pub fn task_state(root: &Path, session_id: &str) -> Result<Option<TaskState>> {
    validate_session_id(session_id)?;
    read_task(root, session_id)
}

pub fn evaluate_context_hook(
    fallback_root: &Path,
    input: &str,
    agent: HookAgent,
) -> Result<Option<String>> {
    let payload: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    let Some(root) = find_project_root(fallback_root, &payload) else {
        return Ok(None);
    };
    let code_guard = is_hook_project(&root)?;
    let session = session_key(&payload);
    let task = read_task(&root, &session)?;
    if agent == HookAgent::Antigravity
        && payload
            .get("invocationNum")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            > 0
    {
        return Ok(None);
    }
    let mut context = match task {
        Some(task) => task_context(&task, code_guard),
        None if code_guard => format!(
            "ForgeGuard focus session {session}. For non-trivial code changes, register the objective and verifiable todos before editing: `forgeguard task start --session {session} --objective <goal> --todo <step> [--metric <metric> --baseline <value> --target <value> --verification <check>]`. Never state a correction, number, or completion claim without exact tool/check evidence; label it unverified otherwise."
        ),
        None => format!(
            "ForgeGuard general focus session {session}. For non-trivial work, register the objective and verifiable todos: `forgeguard task start --session {session} --objective <goal> --todo <step> [--metric <metric> --baseline <value> --target <value> --verification <check>]`. Never state a correction, number, or completion claim without exact tool/check evidence; label it unverified otherwise."
        ),
    };
    if code_guard {
        context.push_str(
            " Code Guard is active: follow inspect → design → implement → test → review → verify; repository scanning and configured commands run at completion.",
        );
    } else {
        context.push_str(
            " General Guard is active: objective, TODO, evidence, scope, and auto-poke apply; repository scanning and commands are disabled until `forgeguard init` creates `.forgeguard/config.toml`.",
        );
    }
    let question_instruction = match agent {
        HookAgent::Claude => "call `AskUserQuestion`",
        HookAgent::Codex => "use `request_user_input` when available, otherwise ask directly",
        HookAgent::Cursor | HookAgent::Antigravity => {
            "use the host's native structured user-input tool when available, otherwise ask directly"
        }
    };
    context.push(' ');
    context
        .push_str(&CLARIFICATION_CONTEXT.replace("{question_instruction}", question_instruction));
    Ok(Some(context))
}

pub fn render_context_hook(agent: HookAgent, input: &str, context: &str) -> String {
    let payload: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    let event = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("SessionStart");
    match agent {
        HookAgent::Codex | HookAgent::Claude => json!({
            "hookSpecificOutput": {
                "hookEventName": event,
                "additionalContext": context
            }
        })
        .to_string(),
        HookAgent::Cursor => json!({"additional_context": context}).to_string(),
        HookAgent::Antigravity => {
            json!({"injectSteps": [{"ephemeralMessage": context}]}).to_string()
        }
    }
}

pub fn evaluate_scope_hook(fallback_root: &Path, input: &str) -> Result<Option<String>> {
    let payload: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    let Some(root) = find_project_root(fallback_root, &payload) else {
        return Ok(None);
    };
    let session = session_key(&payload);
    let Some(task) = read_task(&root, &session)? else {
        return Ok(None);
    };
    if task.status != TaskStatus::Active || task.scopes.is_empty() {
        return Ok(None);
    }
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.pointer("/toolCall/args"))
        .unwrap_or(&Value::Null);
    let mut paths = Vec::new();
    collect_tool_paths(tool_input, None, &mut paths);
    paths.extend(patch_paths(tool_input));
    paths.sort();
    paths.dedup();
    let outside = paths
        .into_iter()
        .filter_map(|path| normalize_tool_path(&root, &path))
        .filter(|path| !task.scopes.iter().any(|scope| path_in_scope(path, scope)))
        .take(3)
        .collect::<Vec<_>>();
    if outside.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "ForgeGuard scope warning for objective `{}`: {} outside declared scope [{}]. Reinspect and expand scope explicitly only when required by the objective.",
        truncate(&task.objective, 120),
        outside.join(", "),
        task.scopes.join(", ")
    )))
}

pub fn render_scope_warning(agent: HookAgent, warning: &str) -> String {
    match agent {
        HookAgent::Codex | HookAgent::Claude => json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "additionalContext": warning
            }
        })
        .to_string(),
        HookAgent::Cursor => json!({"permission": "allow", "user_message": warning}).to_string(),
        HookAgent::Antigravity => json!({"decision": "allow", "reason": warning}).to_string(),
    }
}

/// In zero-config mode the gate still writes `cache/` and `reports/` under
/// `.forgeguard/`. Keep those artifacts out of version control without asking
/// the user to configure anything: a self-ignoring `.forgeguard/.gitignore`,
/// plus a `.forgeguard/` entry appended to an existing root `.gitignore`.
pub fn ignore_forgeguard_artifacts(root: &Path) -> Result<()> {
    let marker = root.join(".forgeguard/.gitignore");
    if !marker.exists() {
        if let Some(parent) = marker.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&marker, "*\n")?;
    }
    let root_ignore = root.join(".gitignore");
    if root_ignore.is_file() {
        let current = fs::read_to_string(&root_ignore).unwrap_or_default();
        let already = current
            .lines()
            .any(|line| line.trim().trim_end_matches('/') == ".forgeguard");
        if !already {
            let mut updated = current;
            if !updated.is_empty() && !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(".forgeguard/\n");
            fs::write(&root_ignore, updated)?;
        }
    }
    Ok(())
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

    let starts = candidates
        .into_iter()
        .filter_map(|candidate| {
            let start = if candidate.is_file() {
                candidate.parent()?
            } else {
                candidate.as_path()
            };
            start.is_dir().then(|| start.to_path_buf())
        })
        .collect::<Vec<_>>();
    starts
        .iter()
        .find_map(|start| {
            start
                .ancestors()
                .find(|path| path.join(".git").exists() || path.join(CONFIG_FILE).is_file())
                .map(Path::to_path_buf)
        })
        .or_else(|| starts.into_iter().next())
}

/// Project scanning and commands are opt-in. General task supervision remains
/// available without initialization but never executes repository policy.
fn is_hook_project(root: &Path) -> Result<bool> {
    Ok(root.join(CONFIG_FILE).is_file())
}

fn workspace_fingerprint(workspace: &Path, repositories: &[PathBuf]) -> Result<Option<String>> {
    if repositories == [workspace] {
        return worktree_fingerprint(workspace);
    }

    let mut hasher = DefaultHasher::new();
    let mut changed = false;
    for repository in repositories {
        let Some(fingerprint) = worktree_fingerprint(repository)? else {
            continue;
        };
        changed = true;
        hasher.write(
            repository
                .strip_prefix(workspace)
                .unwrap_or(repository)
                .to_string_lossy()
                .as_bytes(),
        );
        hasher.write(fingerprint.as_bytes());
    }
    Ok(changed.then(|| format!("{:016x}", hasher.finish())))
}

fn run_workspace_gate(
    workspace: &Path,
    repositories: &[PathBuf],
    workspace_config: &ForgeGuardConfig,
) -> Result<crate::GateReport> {
    if repositories == [workspace] {
        let (changed, _) = changed_paths_partitioned(workspace)?;
        return run_changed_gate(
            workspace,
            workspace_config,
            !changes_require_commands(&changed),
            None,
        );
    }

    let mut combined = crate::GateReport {
        status: GateStatus::Passed,
        findings: Vec::new(),
        checks: Vec::new(),
        summary: GateSummary {
            errors: 0,
            warnings: 0,
            info: 0,
            blocking_findings: 0,
            findings_baselined: 0,
            checks_passed: 0,
            checks_failed: 0,
        },
    };
    for repository in repositories {
        if worktree_fingerprint(repository)?.is_none() {
            continue;
        }
        let configured = repository.join(CONFIG_FILE).is_file();
        let Some(mut config) = load_hook_config(repository)? else {
            continue;
        };
        if !configured {
            config.version = workspace_config.version;
            config.mode = workspace_config.mode;
            config.scan = workspace_config.scan.clone();
            config.policies = workspace_config.policies.clone();
            config.rules = workspace_config.rules.clone();
        }
        let (changed, _) = changed_paths_partitioned(repository)?;
        let report = run_changed_gate(
            repository,
            &config,
            !changes_require_commands(&changed),
            None,
        )?;
        write_json(&repository.join(REPORT_FILE), &report)?;
        merge_report(
            &mut combined,
            report,
            repository.strip_prefix(workspace).unwrap_or(repository),
        );
    }
    Ok(combined)
}

/// Build, lint, and test commands only observe code and manifests. When every
/// changed path is documentation or an asset, running them costs minutes and
/// can block a stop on a failure the change did not cause. Anything not
/// recognized as documentation still runs the commands.
fn changes_require_commands(paths: &[PathBuf]) -> bool {
    if paths.is_empty() {
        return true;
    }
    !paths.iter().all(|path| is_documentation_path(path))
}

fn is_documentation_path(path: &Path) -> bool {
    // A Markdown or text file under a test tree is a fixture: the commands read
    // it, so a change to it must still run them.
    if path.components().any(|component| {
        component.as_os_str().to_str().is_some_and(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "test" | "tests" | "__tests__" | "spec" | "specs" | "fixtures" | "testdata"
            )
        })
    }) {
        return false;
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            DOCUMENTATION_EXTENSIONS
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

fn merge_report(combined: &mut crate::GateReport, mut report: crate::GateReport, prefix: &Path) {
    combined.status = match (combined.status, report.status) {
        (GateStatus::Blocked, _) | (_, GateStatus::Blocked) => GateStatus::Blocked,
        (GateStatus::Warning, _) | (_, GateStatus::Warning) => GateStatus::Warning,
        _ => GateStatus::Passed,
    };
    for finding in &mut report.findings {
        finding.path = prefix.join(&finding.path);
    }
    for check in &mut report.checks {
        check.name = format!("{}: {}", prefix.display(), check.name);
    }
    combined.findings.extend(report.findings);
    combined.checks.extend(report.checks);
    combined.summary.errors += report.summary.errors;
    combined.summary.warnings += report.summary.warnings;
    combined.summary.info += report.summary.info;
    combined.summary.blocking_findings += report.summary.blocking_findings;
    combined.summary.findings_baselined += report.summary.findings_baselined;
    combined.summary.checks_passed += report.summary.checks_passed;
    combined.summary.checks_failed += report.summary.checks_failed;
}

fn compact_failure(report: &crate::GateReport) -> String {
    let suffix = "\nFull report: .forgeguard/reports/latest.json";
    let available = COMPACT_MAX_CHARS
        .saturating_sub(suffix.chars().count())
        .saturating_sub(FOCUS_FEEDBACK_RESERVE);
    let compact = render_gate_compact(report);
    let mut output: String = compact.chars().take(available).collect();
    output.push_str(suffix);
    output
}

fn load_hook_config(root: &Path) -> Result<Option<ForgeGuardConfig>> {
    if root.join(CONFIG_FILE).exists() {
        return ForgeGuardConfig::load(root).map(Some);
    }
    let detection = detect_project(root)?;
    if detection.languages.is_empty() {
        return Ok(None);
    }
    ignore_forgeguard_artifacts(root)?;
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    Ok(Some(ForgeGuardConfig::new(
        name,
        detection.suggested_commands,
    )))
}

fn bounded_block(
    root: &Path,
    session: &str,
    fingerprint: String,
    message: String,
    previous: Option<&HookCache>,
    focus: &FocusConfig,
    cache_hit: bool,
) -> Result<(HookDecision, bool)> {
    // Both budgets measure the same thing: attempts against unchanged state.
    // Counting attempts across changed fingerprints would stop a session that
    // makes real progress on every turn.
    let repeated = previous.filter(|cache| cache.fingerprint == fingerprint);
    let attempts = repeated.map_or(1, |cache| cache.attempts.saturating_add(1));
    let stalled_attempts = repeated.map_or(0, |cache| cache.stalled_attempts.saturating_add(1));
    let max_retries = focus.max_retries.max(1);
    let no_progress_limit = focus.no_progress_limit.max(1);
    let exhausted = attempts > max_retries || stalled_attempts >= no_progress_limit;
    let bounded_message = if exhausted {
        format!(
            "{message}\nForgeGuard stopped after {attempts} attempt(s), including {stalled_attempts} without repository or task-state progress. Report the blocker; do not claim completion."
        )
    } else {
        format!(
            "{message}\nForgeGuard focus attempt {attempts}/{max_retries}; no-progress {stalled_attempts}/{no_progress_limit}. Continue only from verified repository/tool evidence."
        )
    };
    let bounded_message = truncate(&bounded_message, COMPACT_MAX_CHARS);
    write_cache(
        root,
        session,
        &HookCache {
            fingerprint,
            status: GateStatus::Blocked,
            message,
            attempts,
            stalled_attempts,
        },
    )?;
    if exhausted {
        if let Some(mut task) = read_task(root, session)? {
            task.status = TaskStatus::Blocked;
            task.blocker = Some(bounded_message.clone());
            write_task(root, &task)?;
        }
        Ok((HookDecision::Stop(bounded_message), cache_hit))
    } else {
        Ok((HookDecision::Block(bounded_message), cache_hit))
    }
}

fn active_auto_poke(
    root: &Path,
    task: &mut TaskState,
    focus: &FocusConfig,
    evidence_context: &str,
) -> Result<Option<HookDecision>> {
    if !focus.auto_poke {
        return Ok(None);
    }
    let hill_climbability = task.goal.hill_climbability();
    let min_hill_climbability = focus.min_hill_climbability.min(100);
    let instruction = if hill_climbability < min_hill_climbability {
        format!(
            "goal hill-climbability contract is {hill_climbability}/100, below {min_hill_climbability}. Reframe it with explicit `--metric`, `--baseline`, `--target`, `--guardrail`, and `--verification` fields"
        )
    } else {
        let pending = task
            .todos
            .iter()
            .enumerate()
            .filter(|(_, todo)| !todo.done)
            .take(3)
            .map(|(index, todo)| format!("#{} {}", index + 1, todo.text))
            .collect::<Vec<_>>();
        if task.todos.is_empty() {
            format!(
                "todo list is empty; decompose the objective with `forgeguard task todo --session {} --add <verifiable-step>`",
                task.session_id
            )
        } else if pending.is_empty() {
            format!(
                "all todos are complete; submit executed evidence and model confidence with `forgeguard task ready --session {} --confidence <0-100> --evidence <exact-check-result>`",
                task.session_id
            )
        } else {
            format!(
                "{} todo(s) remain: {}. Continue work, then mark completed indexes with `forgeguard task todo --session {} --done <index>`",
                task.todos.iter().filter(|todo| !todo.done).count(),
                pending.join("; "),
                task.session_id
            )
        }
    };
    continue_auto_poke(root, task, focus, &instruction, true, evidence_context)
}

fn ready_auto_poke(
    root: &Path,
    task: &mut TaskState,
    focus: &FocusConfig,
    phases: &[&str; 5],
    evidence_context: &str,
) -> Result<Option<HookDecision>> {
    if !focus.auto_poke {
        return Ok(None);
    }
    let hill_climbability = task.goal.hill_climbability();
    let min_hill_climbability = focus.min_hill_climbability.min(100);
    if hill_climbability < min_hill_climbability {
        return continue_auto_poke(
            root,
            task,
            focus,
            &format!(
                "goal hill-climbability contract is {hill_climbability}/100, below {min_hill_climbability}; reframe the goal contract before completion"
            ),
            true,
            evidence_context,
        );
    }
    let min_confidence = focus.min_confidence.min(100);
    if task.confidence.unwrap_or(0) < min_confidence {
        let confidence = task
            .confidence
            .map_or_else(|| "unreported".to_owned(), |value| value.to_string());
        return continue_auto_poke(
            root,
            task,
            focus,
            &format!(
                "model confidence is {confidence}/100, below {min_confidence}; inspect concrete uncertainty and submit fresh evidence plus confidence"
            ),
            true,
            evidence_context,
        );
    }
    let limit = focus.max_auto_pokes.min(MAX_AUTO_POKES);
    if task.auto_pokes >= limit {
        return Ok(None);
    }
    let phase = phases[task.auto_pokes as usize];
    continue_auto_poke(root, task, focus, phase, false, evidence_context)
}

fn continue_auto_poke(
    root: &Path,
    task: &mut TaskState,
    focus: &FocusConfig,
    instruction: &str,
    stop_when_exhausted: bool,
    evidence_context: &str,
) -> Result<Option<HookDecision>> {
    let limit = focus.max_auto_pokes.min(MAX_AUTO_POKES);
    if task.auto_pokes >= limit {
        if !stop_when_exhausted {
            return Ok(None);
        }
        let message = format!(
            "ForgeGuard auto-poke stopped after {limit} continuation request(s): {instruction}. Report the blocker; do not claim completion."
        );
        task.status = TaskStatus::Blocked;
        task.blocker = Some(message.clone());
        write_task(root, task)?;
        return Ok(Some(HookDecision::Stop(message)));
    }
    task.auto_pokes += 1;
    task.status = TaskStatus::Active;
    task.evidence.clear();
    task.confidence = None;
    let instruction = truncate(instruction, 700);
    task.poke_instruction = Some(instruction.clone());
    task.blocker = None;
    write_task(root, task)?;
    Ok(Some(HookDecision::Block(format!(
        "ForgeGuard auto-poke {current}/{limit}: {instruction}. Continue from exact {evidence_context}/tool evidence.",
        current = task.auto_pokes,
    ))))
}

fn missing_task_message(session: &str) -> String {
    format!(
        "ForgeGuard focus contract missing. Register the exact objective and verifiable todo with `forgeguard task start --session {session} --objective <goal> --todo <step> [--scope <path-prefix>]`, then finish with executed evidence."
    )
}

fn incomplete_task_message(session: &str) -> String {
    format!(
        "ForgeGuard objective is still active. Inspect remaining work, run verification, then call `forgeguard task ready --session {session} --evidence <exact-check-result>`. Do not invent or silently revise facts, numbers, or completion claims."
    )
}

fn task_context(task: &TaskState, code_guard: bool) -> String {
    let scopes = if task.scopes.is_empty() {
        if code_guard {
            "repository scope not constrained".to_owned()
        } else {
            "work scope not constrained".to_owned()
        }
    } else {
        format!("scope prefixes: {}", task.scopes.join(", "))
    };
    let semantic = if task.semantic {
        " Use the host's native goal evaluator when available, but treat executed checks as source of truth."
    } else {
        ""
    };
    let auto_poke = if task.status == TaskStatus::Active {
        task.poke_instruction
            .as_deref()
            .map(|instruction| {
                format!(
                    " Active auto-poke {}: {}.",
                    task.auto_pokes,
                    truncate(instruction, 300)
                )
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    let completed_todos = task.todos.iter().filter(|todo| todo.done).count();
    let confidence = task
        .confidence
        .map_or_else(|| "unreported".to_owned(), |value| format!("{value}/100"));
    format!(
        "ForgeGuard objective [{}]: {}. Status: {:?}; {scopes}; hill-climbability contract: {}/100; todos: {completed_todos}/{}; model confidence: {confidence}.{auto_poke} Never state a correction, number, or completion claim without exact tool/check evidence; label it unverified otherwise.{semantic}",
        task.session_id,
        truncate(&task.objective, 300),
        task.status,
        task.goal.hill_climbability(),
        task.todos.len()
    )
}

fn task_signature(task: Option<&TaskState>) -> String {
    let mut hasher = DefaultHasher::new();
    if let Some(task) = task {
        if let Ok(source) = serde_json::to_vec(task) {
            hasher.write(&source);
        }
    }
    format!("{:016x}", hasher.finish())
}

fn session_key(payload: &Value) -> String {
    let raw = [
        "session_id",
        "sessionId",
        "conversation_id",
        "conversationId",
        "transcript_path",
        "transcriptPath",
    ]
    .into_iter()
    .find_map(|key| payload.get(key).and_then(Value::as_str))
    .unwrap_or("default");
    if raw.len() <= 128
        && !raw.is_empty()
        && raw
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.".contains(character))
    {
        return raw.to_owned();
    }
    let hash = raw
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("session-{hash:016x}")
}

fn validate_session_id(session_id: &str) -> Result<()> {
    if session_id.is_empty()
        || session_id.len() > 128
        || !session_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.".contains(character))
    {
        bail!("session id must contain 1-128 ASCII letters, digits, '-', '_', or '.'");
    }
    Ok(())
}

fn normalize_goal_contract(goal: GoalContract) -> Result<GoalContract> {
    Ok(GoalContract {
        metric: normalize_optional_task_item("goal metric", goal.metric)?,
        baseline: normalize_optional_task_item("goal baseline", goal.baseline)?,
        target: normalize_optional_task_item("goal target", goal.target)?,
        guardrails: normalize_task_items("goal guardrail", &goal.guardrails, MAX_TASK_ITEMS)?,
        verifications: normalize_task_items(
            "goal verification",
            &goal.verifications,
            MAX_TASK_ITEMS,
        )?,
    })
}

fn normalize_optional_task_item(label: &str, value: Option<String>) -> Result<Option<String>> {
    value
        .map(|value| {
            normalize_task_items(label, &[value], 1)
                .map(|mut values| values.pop().expect("one item"))
        })
        .transpose()
}

fn normalize_task_items(label: &str, items: &[String], max_items: usize) -> Result<Vec<String>> {
    if items.len() > max_items {
        bail!("{label} exceeds {max_items} items");
    }
    items
        .iter()
        .map(|item| {
            let item = item.trim();
            if item.is_empty() {
                bail!("{label} must not be empty");
            }
            if item.chars().count() > MAX_TASK_ITEM_CHARS {
                bail!("{label} exceeds {MAX_TASK_ITEM_CHARS} characters");
            }
            Ok(item.to_owned())
        })
        .collect()
}

fn normalize_scope(scope: &str) -> Result<String> {
    let scope = Path::new(scope.trim());
    if scope.as_os_str().is_empty() || scope.is_absolute() {
        bail!("task scope must be a non-empty repository-relative path prefix");
    }
    let mut normalized = PathBuf::new();
    for component in scope.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            _ => bail!("task scope must not escape the repository"),
        }
    }
    if normalized.as_os_str().is_empty() {
        return Ok(".".to_owned());
    }
    Ok(normalized.to_string_lossy().replace('\\', "/"))
}

fn normalize_tool_path(root: &Path, path: &str) -> Option<String> {
    let path = Path::new(path.trim());
    let relative = if path.is_absolute() {
        match path.strip_prefix(root) {
            Ok(relative) => relative,
            Err(_) => return Some(format!("outside-workspace:{path}", path = path.display())),
        }
    } else {
        path
    };
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            _ => return Some(format!("outside-workspace:{}", path.display())),
        }
    }
    (!normalized.as_os_str().is_empty()).then(|| normalized.to_string_lossy().replace('\\', "/"))
}

fn path_in_scope(path: &str, scope: &str) -> bool {
    (scope == "." && !path.starts_with("outside-workspace:"))
        || path == scope
        || path
            .strip_prefix(scope)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn collect_tool_paths(value: &Value, key: Option<&str>, paths: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                collect_tool_paths(value, Some(key), paths);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_tool_paths(value, key, paths);
            }
        }
        Value::String(path)
            if key.is_some_and(|key| {
                matches!(
                    key.to_ascii_lowercase().as_str(),
                    "path"
                        | "file"
                        | "files"
                        | "filepath"
                        | "file_path"
                        | "targetfile"
                        | "target_file"
                        | "absolutepath"
                )
            }) =>
        {
            paths.push(path.to_owned());
        }
        _ => {}
    }
}

fn patch_paths(value: &Value) -> Vec<String> {
    let Some(command) = value.get("command").and_then(Value::as_str) else {
        return Vec::new();
    };
    command
        .lines()
        .filter_map(|line| {
            ["*** Add File: ", "*** Update File: ", "*** Delete File: "]
                .into_iter()
                .find_map(|prefix| line.strip_prefix(prefix))
                .map(str::to_owned)
        })
        .collect()
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    value.chars().take(max_chars).collect::<String>() + "…"
}

fn task_path(root: &Path, session: &str) -> PathBuf {
    root.join(TASK_DIR).join(format!("{session}.json"))
}

fn read_task(root: &Path, session: &str) -> Result<Option<TaskState>> {
    let path = task_path(root, session);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    serde_json::from_str(&source)
        .with_context(|| format!("failed to parse {}", path.display()))
        .map(Some)
}

fn write_task(root: &Path, task: &TaskState) -> Result<()> {
    write_json(&task_path(root, &task.session_id), task)
}

fn replay_path(root: &Path, session: &str) -> PathBuf {
    root.join(CACHE_DIR).join(format!("{session}.replay.json"))
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn stop_state_key(root: &Path, session: &str, worktree: &str) -> Result<String> {
    Ok(format!(
        "{worktree}:{}",
        task_signature(read_task(root, session)?.as_ref())
    ))
}

fn replayed_decision(root: &Path, session: &str, state: &str) -> Option<HookDecision> {
    let source = fs::read_to_string(replay_path(root, session)).ok()?;
    let replay: HookReplay = serde_json::from_str(&source).ok()?;
    if replay.state != state {
        return None;
    }
    if now_millis().saturating_sub(replay.at_millis) >= DUPLICATE_STOP_WINDOW_MILLIS {
        return None;
    }
    Some(if replay.stop {
        HookDecision::Stop(replay.message)
    } else {
        HookDecision::Block(replay.message)
    })
}

fn record_replay(root: &Path, session: &str, state: &str, decision: &HookDecision) -> Result<()> {
    let (stop, message) = match decision {
        HookDecision::Pass => return Ok(()),
        HookDecision::Block(message) => (false, message.clone()),
        HookDecision::Stop(message) => (true, message.clone()),
    };
    write_json(
        &replay_path(root, session),
        &HookReplay {
            state: state.to_owned(),
            at_millis: now_millis(),
            stop,
            message,
        },
    )
}

fn cache_path(root: &Path, session: &str) -> PathBuf {
    root.join(CACHE_DIR).join(format!("{session}.json"))
}

fn read_cache(root: &Path, session: &str) -> Option<HookCache> {
    let source = fs::read_to_string(cache_path(root, session)).ok()?;
    serde_json::from_str(&source).ok()
}

fn write_cache(root: &Path, session: &str, cache: &HookCache) -> Result<()> {
    write_json(&cache_path(root, session), cache)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .context("ForgeGuard report path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let output = serde_json::to_vec_pretty(value).context("failed to serialize ForgeGuard data")?;
    fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn global_general_guard_config_reads_focus_and_ignores_mode() {
        let home = tempdir().expect("temporary home");
        let mut config = ForgeGuardConfig::new("global", Vec::new());
        config.mode = GuardMode::Strict;
        config.focus.max_auto_pokes = 1;
        config.save_global(home.path()).expect("save global config");

        let focus = global_focus_config_at(Some(home.path()));

        assert_eq!(focus.max_auto_pokes, 1);
        assert_eq!(focus.min_confidence, 80);
    }

    #[test]
    fn general_guard_uses_supplied_focus_limits_without_a_mode() {
        let directory = tempdir().expect("temporary directory");
        start_task(
            directory.path(),
            "global-focus",
            "Review a non-code artifact",
            &[],
            false,
        )
        .expect("start task");
        let focus = FocusConfig {
            max_auto_pokes: 0,
            ..FocusConfig::default()
        };

        let (decision, _) = evaluate_general_stop_hook(directory.path(), "global-focus", &focus)
            .expect("evaluate General Guard");

        assert!(matches!(decision, HookDecision::Stop(_)));
        let task = task_state(directory.path(), "global-focus")
            .expect("read task")
            .expect("task");
        assert_eq!(task.auto_pokes, 0);
        assert_eq!(task.status, TaskStatus::Blocked);
    }

    #[test]
    fn disabled_global_focus_bypasses_general_guard_lifecycle() {
        let directory = tempdir().expect("temporary directory");
        start_task(
            directory.path(),
            "disabled-focus",
            "Review a non-code artifact",
            &[],
            false,
        )
        .expect("start task");
        let focus = FocusConfig {
            enabled: false,
            ..FocusConfig::default()
        };

        let (decision, _) = evaluate_general_stop_hook(directory.path(), "disabled-focus", &focus)
            .expect("evaluate General Guard");

        assert_eq!(decision, HookDecision::Pass);
        assert_eq!(
            task_state(directory.path(), "disabled-focus")
                .expect("read task")
                .expect("task")
                .status,
            TaskStatus::Active
        );
    }
}
