use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{config::ForgeGuardConfig, detect_project, ProjectDetection};

const AGENTS_TEMPLATE: &str = include_str!("../assets/templates/AGENTS.md");
const CLAUDE_TEMPLATE: &str = include_str!("../assets/templates/CLAUDE.md");
const CURSOR_TEMPLATE: &str = include_str!("../assets/templates/CURSOR.md");
const FORGEGUARD_GITIGNORE: &str = "cache/\nreports/\n";
const AGENT_HOOK_FILES: [&str; 5] = [
    ".claude/settings.json",
    ".codex/hooks.json",
    ".cursor/hooks.json",
    ".agents/hooks.json",
    ".gemini/config/hooks.json",
];
const DEFAULT_STOP_HOOK_TIMEOUT_SECONDS: u64 = 600;
const MAX_STOP_HOOK_TIMEOUT_SECONDS: u64 = 3_600;
/// Scan and report time on top of the configured command budget.
const STOP_HOOK_TIMEOUT_MARGIN_SECONDS: u64 = 120;
pub(crate) const CODEX_HOOK_COMMAND: &str = "forgeguard hook stop --agent codex";
pub(crate) const CLAUDE_HOOK_COMMAND: &str = "forgeguard hook stop --agent claude";
pub(crate) const CURSOR_HOOK_COMMAND: &str = "forgeguard hook stop --agent cursor";
pub(crate) const ANTIGRAVITY_HOOK_COMMAND: &str = "forgeguard hook stop --agent antigravity";
pub(crate) const CODEX_CONTEXT_HOOK_COMMAND: &str = "forgeguard hook context --agent codex";
pub(crate) const CLAUDE_CONTEXT_HOOK_COMMAND: &str = "forgeguard hook context --agent claude";
pub(crate) const CURSOR_CONTEXT_HOOK_COMMAND: &str = "forgeguard hook context --agent cursor";
pub(crate) const ANTIGRAVITY_CONTEXT_HOOK_COMMAND: &str =
    "forgeguard hook context --agent antigravity";
pub(crate) const CODEX_SCOPE_HOOK_COMMAND: &str = "forgeguard hook scope --agent codex";
pub(crate) const CLAUDE_SCOPE_HOOK_COMMAND: &str = "forgeguard hook scope --agent claude";
pub(crate) const CURSOR_SCOPE_HOOK_COMMAND: &str = "forgeguard hook scope --agent cursor";
pub(crate) const ANTIGRAVITY_SCOPE_HOOK_COMMAND: &str = "forgeguard hook scope --agent antigravity";

const SKILL_ASSETS: &[(&str, &str)] = &[
    (
        "SKILL.md",
        include_str!("../assets/skills/engineering/SKILL.md"),
    ),
    (
        "references/frontend.md",
        include_str!("../assets/skills/engineering/references/frontend.md"),
    ),
    (
        "references/mobile.md",
        include_str!("../assets/skills/engineering/references/mobile.md"),
    ),
    (
        "references/backend.md",
        include_str!("../assets/skills/engineering/references/backend.md"),
    ),
    (
        "references/algorithms.md",
        include_str!("../assets/skills/engineering/references/algorithms.md"),
    ),
    (
        "references/database.md",
        include_str!("../assets/skills/engineering/references/database.md"),
    ),
    (
        "references/ai.md",
        include_str!("../assets/skills/engineering/references/ai.md"),
    ),
    (
        "references/ml.md",
        include_str!("../assets/skills/engineering/references/ml.md"),
    ),
    (
        "references/deep-learning.md",
        include_str!("../assets/skills/engineering/references/deep-learning.md"),
    ),
    (
        "references/mlops.md",
        include_str!("../assets/skills/engineering/references/mlops.md"),
    ),
    (
        "references/testing.md",
        include_str!("../assets/skills/engineering/references/testing.md"),
    ),
    (
        "agents/openai.yaml",
        include_str!("../assets/skills/engineering/agents/openai.yaml"),
    ),
];

const OBSOLETE_SKILL_ASSETS: &[&str] = &["references/clean-code.md"];

pub(crate) const LEGACY_SKILL_NAMES: &[&str] = &[
    "algorithm-engineering",
    "clean-code",
    "backend-engineering",
    "frontend-engineering",
    "database-engineering",
    "ai-engineering",
    "testing-verification",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentTarget {
    Codex,
    Claude,
    Cursor,
    OpenCode,
    Antigravity,
    All,
}

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub force: bool,
    pub agents: Vec<AgentTarget>,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            force: false,
            agents: vec![AgentTarget::All],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitReport {
    pub detection: ProjectDetection,
    pub files_written: Vec<String>,
    pub files_skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalInitReport {
    pub home: PathBuf,
    pub files_written: Vec<String>,
    pub files_skipped: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum InstallScope {
    Project,
    Global,
}

const ALL_AGENT_TARGETS: &[AgentTarget] = &[
    AgentTarget::Codex,
    AgentTarget::Claude,
    AgentTarget::Cursor,
    AgentTarget::OpenCode,
    AgentTarget::Antigravity,
];

pub fn initialize_global(home: &Path, options: &InitOptions) -> Result<GlobalInitReport> {
    let home = home
        .canonicalize()
        .with_context(|| format!("failed to resolve home directory {}", home.display()))?;
    let mut files_written = Vec::new();
    let mut files_skipped = Vec::new();

    install_agents(
        &home,
        InstallScope::Global,
        &options.agents,
        options.force,
        &mut files_written,
        &mut files_skipped,
    )?;

    Ok(GlobalInitReport {
        home,
        files_written,
        files_skipped,
    })
}

pub fn initialize_project(root: &Path, options: &InitOptions) -> Result<InitReport> {
    let detection = detect_project(root)?;
    let project_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let config = ForgeGuardConfig::new(project_name, detection.suggested_commands.clone());

    let mut files_written = Vec::new();
    let mut files_skipped = Vec::new();
    write_config(
        root,
        &config,
        options.force,
        &mut files_written,
        &mut files_skipped,
    )?;
    write_file(
        root,
        &root.join(".forgeguard/.gitignore"),
        FORGEGUARD_GITIGNORE,
        options.force,
        &mut files_written,
        &mut files_skipped,
    )?;

    install_agents(
        root,
        InstallScope::Project,
        &options.agents,
        options.force,
        &mut files_written,
        &mut files_skipped,
    )?;
    refresh_stop_hook_timeouts(root, &mut files_written)?;
    ignore_project_agent_directories(root, &options.agents, &mut files_written)?;

    Ok(InitReport {
        detection,
        files_written,
        files_skipped,
    })
}

fn install_agents(
    root: &Path,
    scope: InstallScope,
    requested: &[AgentTarget],
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    for target in expand_agent_targets(requested) {
        match target {
            AgentTarget::Codex => install_codex(root, scope, force, written, skipped)?,
            AgentTarget::Claude => install_claude(root, scope, force, written, skipped)?,
            AgentTarget::Cursor => install_cursor(root, scope, force, written, skipped)?,
            AgentTarget::OpenCode => install_opencode(root, scope, force, written, skipped)?,
            AgentTarget::Antigravity => install_antigravity(root, scope, force, written, skipped)?,
            AgentTarget::All => unreachable!("all is expanded before installation"),
        }
    }
    Ok(())
}

fn expand_agent_targets(requested: &[AgentTarget]) -> Vec<AgentTarget> {
    // ponytail: O(n²) contains-check, but n <= 5 agents; a set is not worth it.
    let mut targets: Vec<AgentTarget> = Vec::new();
    let push = |target: AgentTarget, targets: &mut Vec<AgentTarget>| {
        if !targets.contains(&target) {
            targets.push(target);
        }
    };
    for target in requested {
        if *target == AgentTarget::All {
            // forgeguard: allow FG-ALG-001 -- at most five requested agents expand across five targets
            for expanded in ALL_AGENT_TARGETS {
                push(*expanded, &mut targets);
            }
        } else {
            push(*target, &mut targets);
        }
    }
    targets
}

fn ignore_project_agent_directories(
    root: &Path,
    requested: &[AgentTarget],
    written: &mut Vec<String>,
) -> Result<()> {
    let path = root.join(".gitignore");
    if !path.is_file() {
        return Ok(());
    }

    let targets = expand_agent_targets(requested);
    let mut entries = Vec::new();
    if targets.contains(&AgentTarget::Codex) {
        entries.push(".codex/");
    }
    if targets.contains(&AgentTarget::Claude) {
        entries.push(".claude/");
    }
    if targets.contains(&AgentTarget::Cursor) {
        entries.push(".cursor/");
    }
    if targets.iter().any(|target| {
        matches!(
            target,
            AgentTarget::Codex
                | AgentTarget::Cursor
                | AgentTarget::OpenCode
                | AgentTarget::Antigravity
        )
    }) {
        entries.push(".agents/");
    }

    let mut content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut changed = false;
    for entry in entries {
        let expected = entry.trim_end_matches('/');
        if content
            .lines()
            .any(|line| line.trim().trim_matches('/') == expected)
        {
            continue;
        }
        if !content.is_empty() && !content.ends_with('\n') {
            content.push_str(newline);
        }
        content.push_str(entry);
        content.push_str(newline);
        changed = true;
    }
    if changed {
        fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))?;
        record_path(root, &path, written);
    }
    Ok(())
}

fn write_config(
    root: &Path,
    config: &ForgeGuardConfig,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let path = root.join(".forgeguard/config.toml");
    let content =
        toml::to_string_pretty(config).context("failed to serialize ForgeGuard config")?;
    write_file(root, &path, &content, force, written, skipped)
}

fn install_codex(
    root: &Path,
    scope: InstallScope,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    if force {
        remove_legacy_skills(root, ".codex/skills", true)?;
    }
    let policy_path = match scope {
        InstallScope::Project => root.join("AGENTS.md"),
        InstallScope::Global => root.join(".codex/AGENTS.md"),
    };
    write_file(root, &policy_path, AGENTS_TEMPLATE, force, written, skipped)?;
    install_skill(
        root,
        ".agents/skills/forgeguard-engineering",
        force,
        written,
        skipped,
    )?;
    install_grouped_hook(
        root,
        &root.join(".codex/hooks.json"),
        "Stop",
        None,
        CODEX_HOOK_COMMAND,
        written,
        skipped,
    )?;
    install_grouped_hook(
        root,
        &root.join(".codex/hooks.json"),
        "SessionStart",
        Some("startup|resume|compact"),
        CODEX_CONTEXT_HOOK_COMMAND,
        written,
        skipped,
    )?;
    install_grouped_hook(
        root,
        &root.join(".codex/hooks.json"),
        "PreToolUse",
        Some("apply_patch|Edit|Write"),
        CODEX_SCOPE_HOOK_COMMAND,
        written,
        skipped,
    )
}

fn install_claude(
    root: &Path,
    scope: InstallScope,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    if force {
        remove_legacy_skills(root, ".claude/skills", false)?;
    }
    let policy_path = match scope {
        InstallScope::Project => root.join("CLAUDE.md"),
        InstallScope::Global => root.join(".claude/CLAUDE.md"),
    };
    write_file(root, &policy_path, CLAUDE_TEMPLATE, force, written, skipped)?;
    install_skill(
        root,
        ".claude/skills/forgeguard-engineering",
        force,
        written,
        skipped,
    )?;
    install_grouped_hook(
        root,
        &root.join(".claude/settings.json"),
        "Stop",
        None,
        CLAUDE_HOOK_COMMAND,
        written,
        skipped,
    )?;
    install_grouped_hook(
        root,
        &root.join(".claude/settings.json"),
        "SessionStart",
        Some("startup|resume|compact"),
        CLAUDE_CONTEXT_HOOK_COMMAND,
        written,
        skipped,
    )?;
    install_grouped_hook(
        root,
        &root.join(".claude/settings.json"),
        "UserPromptSubmit",
        None,
        CLAUDE_CONTEXT_HOOK_COMMAND,
        written,
        skipped,
    )?;
    install_grouped_hook(
        root,
        &root.join(".claude/settings.json"),
        "PreToolUse",
        Some("Edit|Write|MultiEdit|NotebookEdit"),
        CLAUDE_SCOPE_HOOK_COMMAND,
        written,
        skipped,
    )
}

fn install_cursor(
    root: &Path,
    scope: InstallScope,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let rule_path = match scope {
        InstallScope::Project => root.join(".cursor/rules/forgeguard.mdc"),
        InstallScope::Global => root.join(".cursor/rules/forgeguard.mdc"),
    };
    write_file(root, &rule_path, CURSOR_TEMPLATE, force, written, skipped)?;
    install_skill(
        root,
        ".agents/skills/forgeguard-engineering",
        force,
        written,
        skipped,
    )?;
    let path = root.join(".cursor/hooks.json");
    install_cursor_hook(
        root,
        &path,
        "stop",
        None,
        CURSOR_HOOK_COMMAND,
        written,
        skipped,
    )?;
    install_cursor_hook(
        root,
        &path,
        "sessionStart",
        None,
        CURSOR_CONTEXT_HOOK_COMMAND,
        written,
        skipped,
    )?;
    install_cursor_hook(
        root,
        &path,
        "preToolUse",
        Some("Write|StrReplace|Delete|ApplyPatch"),
        CURSOR_SCOPE_HOOK_COMMAND,
        written,
        skipped,
    )
}

fn install_opencode(
    root: &Path,
    scope: InstallScope,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let (policy_path, skill_directory) = match scope {
        InstallScope::Project => (
            root.join("AGENTS.md"),
            ".agents/skills/forgeguard-engineering",
        ),
        InstallScope::Global => (
            root.join(".config/opencode/AGENTS.md"),
            ".config/opencode/skills/forgeguard-engineering",
        ),
    };
    write_file(root, &policy_path, AGENTS_TEMPLATE, force, written, skipped)?;
    install_skill(root, skill_directory, force, written, skipped)
}

fn install_antigravity(
    root: &Path,
    scope: InstallScope,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let (policy_path, skill_directory, hook_path) = match scope {
        InstallScope::Project => (
            root.join(".agents/rules/forgeguard.md"),
            ".agents/skills/forgeguard-engineering",
            root.join(".agents/hooks.json"),
        ),
        InstallScope::Global => (
            root.join(".gemini/GEMINI.md"),
            ".gemini/config/skills/forgeguard-engineering",
            root.join(".gemini/config/hooks.json"),
        ),
    };
    write_file(root, &policy_path, AGENTS_TEMPLATE, force, written, skipped)?;
    install_skill(root, skill_directory, force, written, skipped)?;
    install_antigravity_simple_hook(
        root,
        &hook_path,
        "Stop",
        ANTIGRAVITY_HOOK_COMMAND,
        written,
        skipped,
    )?;
    install_antigravity_simple_hook(
        root,
        &hook_path,
        "PreInvocation",
        ANTIGRAVITY_CONTEXT_HOOK_COMMAND,
        written,
        skipped,
    )?;
    install_antigravity_tool_hook(
        root,
        &hook_path,
        "write_to_file|replace_file_content|multi_replace_file_content",
        ANTIGRAVITY_SCOPE_HOOK_COMMAND,
        written,
        skipped,
    )
}

fn install_skill(
    root: &Path,
    directory: &str,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    for (relative, content) in SKILL_ASSETS {
        let path = root.join(directory).join(relative);
        write_file(root, &path, content, force, written, skipped)?;
    }
    if force {
        remove_obsolete_skill_assets(root, directory)?;
    }
    Ok(())
}

fn remove_obsolete_skill_assets(root: &Path, directory: &str) -> Result<()> {
    for relative in OBSOLETE_SKILL_ASSETS {
        let path = root.join(directory).join(relative);
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_file() || metadata.file_type().is_symlink() {
            fs::remove_file(&path).with_context(|| {
                format!("failed to remove obsolete skill asset {}", path.display())
            })?;
        } else {
            bail!(
                "expected obsolete skill asset to be a file: {}",
                path.display()
            );
        }
    }
    Ok(())
}

/// The stop hook runs the configured commands, so the host must wait at least
/// as long as those commands may take. A host timeout shorter than the command
/// budget kills the gate mid-run: the decision is lost and the next stop repeats
/// the whole suite.
fn stop_hook_timeout_seconds(root: &Path) -> u64 {
    let Ok(config) = ForgeGuardConfig::load(root) else {
        return DEFAULT_STOP_HOOK_TIMEOUT_SECONDS;
    };
    let budget = config
        .commands
        .iter()
        .filter(|command| command.enabled)
        .fold(0_u64, |total, command| {
            total.saturating_add(command.timeout_seconds)
        })
        .saturating_add(STOP_HOOK_TIMEOUT_MARGIN_SECONDS);
    budget.clamp(
        DEFAULT_STOP_HOOK_TIMEOUT_SECONDS,
        MAX_STOP_HOOK_TIMEOUT_SECONDS,
    )
}

/// Hook entries installed by an earlier version keep their original timeout,
/// because every installer skips a command it already finds. A repository whose
/// command budget grew past that timeout would have its gate killed mid-run, so
/// initialization repairs existing stop-hook entries too.
fn refresh_stop_hook_timeouts(root: &Path, written: &mut Vec<String>) -> Result<()> {
    let timeout = stop_hook_timeout_seconds(root);
    for relative in AGENT_HOOK_FILES {
        let path = root.join(relative);
        if !path.is_file() {
            continue;
        }
        let mut document = read_json_object(&path)?;
        if !set_stop_hook_timeout(&mut document, timeout) {
            continue;
        }
        write_json_document(root, &path, &document, written)?;
    }
    Ok(())
}

fn set_stop_hook_timeout(value: &mut Value, timeout: u64) -> bool {
    match value {
        Value::Object(fields) => {
            let is_stop_hook = fields
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|command| command.contains("forgeguard hook stop"));
            let mut changed = false;
            if is_stop_hook && fields.get("timeout") != Some(&json!(timeout)) {
                fields.insert("timeout".to_owned(), json!(timeout));
                changed = true;
            }
            for nested in fields.values_mut() {
                changed |= set_stop_hook_timeout(nested, timeout);
            }
            changed
        }
        Value::Array(values) => values.iter_mut().fold(false, |changed, nested| {
            changed | set_stop_hook_timeout(nested, timeout)
        }),
        _ => false,
    }
}

fn install_grouped_hook(
    root: &Path,
    path: &Path,
    event: &str,
    matcher: Option<&str>,
    command: &str,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let mut document = read_json_object(path)?;
    if document
        .pointer(&format!("/hooks/{event}"))
        .is_some_and(|hooks| contains_hook_command(hooks, command))
    {
        record_path(root, path, skipped);
        return Ok(());
    }
    let hooks = object_field(&mut document, "hooks", path)?;
    let command_hook = json!({
        "type": "command",
        "command": command,
        "timeout": if event == "Stop" { stop_hook_timeout_seconds(root) } else { 5 }
    });
    let mut handler = json!({"hooks": [command_hook]});
    if let Some(matcher) = matcher {
        handler["matcher"] = json!(matcher);
    }
    array_field(hooks, event, path)?.push(handler);
    write_json_document(root, path, &document, written)
}

fn install_cursor_hook(
    root: &Path,
    path: &Path,
    event: &str,
    matcher: Option<&str>,
    command: &str,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let mut document = read_json_object(path)?;
    if contains_string(&document, command) {
        record_path(root, path, skipped);
        return Ok(());
    }
    document
        .as_object_mut()
        .expect("validated JSON object")
        .entry("version")
        .or_insert(json!(1));
    let hooks = object_field(&mut document, "hooks", path)?;
    let mut handler = json!({
        "command": command,
        "timeout": if event == "stop" { stop_hook_timeout_seconds(root) } else { 5 }
    });
    if let Some(matcher) = matcher {
        handler["matcher"] = json!(matcher);
    }
    array_field(hooks, event, path)?.push(handler);
    write_json_document(root, path, &document, written)
}

fn install_antigravity_simple_hook(
    root: &Path,
    path: &Path,
    event: &str,
    command: &str,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let mut document = read_json_object(path)?;
    if contains_string(&document, command) {
        record_path(root, path, skipped);
        return Ok(());
    }
    let root_object = document.as_object_mut().expect("validated JSON object");
    let hook = root_object
        .entry("forgeguard-quality-gate")
        .or_insert_with(|| json!({}));
    let hook = hook.as_object_mut().with_context(|| {
        format!(
            "expected `forgeguard-quality-gate` to be an object in {}",
            path.display()
        )
    })?;
    array_field(hook, event, path)?.push(json!({
        "type": "command",
        "command": command,
        "timeout": if event == "Stop" { stop_hook_timeout_seconds(root) } else { 5 }
    }));
    write_json_document(root, path, &document, written)
}

fn install_antigravity_tool_hook(
    root: &Path,
    path: &Path,
    matcher: &str,
    command: &str,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let mut document = read_json_object(path)?;
    if contains_string(&document, command) {
        record_path(root, path, skipped);
        return Ok(());
    }
    let root_object = document.as_object_mut().expect("validated JSON object");
    let hook = root_object
        .entry("forgeguard-quality-gate")
        .or_insert_with(|| json!({}));
    let hook = hook.as_object_mut().with_context(|| {
        format!(
            "expected `forgeguard-quality-gate` to be an object in {}",
            path.display()
        )
    })?;
    array_field(hook, "PreToolUse", path)?.push(json!({
        "matcher": matcher,
        "hooks": [{
            "type": "command",
            "command": command,
            "timeout": 5
        }]
    }));
    write_json_document(root, path, &document, written)
}

fn read_json_object(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: Value = serde_json::from_str(&source)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if !value.is_object() {
        bail!("expected a JSON object in {}", path.display());
    }
    Ok(value)
}

fn object_field<'a>(
    document: &'a mut Value,
    key: &str,
    path: &Path,
) -> Result<&'a mut serde_json::Map<String, Value>> {
    let value = document
        .as_object_mut()
        .expect("validated JSON object")
        .entry(key)
        .or_insert_with(|| json!({}));
    value
        .as_object_mut()
        .with_context(|| format!("expected `{key}` to be an object in {}", path.display()))
}

fn array_field<'a>(
    document: &'a mut serde_json::Map<String, Value>,
    key: &str,
    path: &Path,
) -> Result<&'a mut Vec<Value>> {
    let value = document.entry(key).or_insert_with(|| json!([]));
    value
        .as_array_mut()
        .with_context(|| format!("expected `{key}` to be an array in {}", path.display()))
}

fn contains_string(value: &Value, expected: &str) -> bool {
    match value {
        Value::String(value) => value == expected,
        Value::Array(values) => values.iter().any(|value| contains_string(value, expected)),
        Value::Object(values) => values
            .values()
            .any(|value| contains_string(value, expected)),
        _ => false,
    }
}

fn contains_hook_command(value: &Value, expected: &str) -> bool {
    match value {
        Value::String(value) => value.contains(expected),
        Value::Array(values) => values
            .iter()
            .any(|value| contains_hook_command(value, expected)),
        Value::Object(values) => values
            .values()
            .any(|value| contains_hook_command(value, expected)),
        _ => false,
    }
}

fn write_json_document(
    root: &Path,
    path: &Path,
    document: &Value,
    written: &mut Vec<String>,
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid output path {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let mut output =
        serde_json::to_string_pretty(document).context("failed to serialize hook configuration")?;
    output.push('\n');
    fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))?;
    record_path(root, path, written);
    Ok(())
}

fn record_path(root: &Path, path: &Path, records: &mut Vec<String>) {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();
    if !records.contains(&relative) {
        records.push(relative);
    }
}

fn remove_legacy_skills(root: &Path, directory: &str, include_engineering: bool) -> Result<()> {
    for name in LEGACY_SKILL_NAMES
        .iter()
        .copied()
        .chain(include_engineering.then_some("engineering"))
    {
        let path = root.join(directory).join(format!("forgeguard-{name}"));
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn write_file(
    root: &Path,
    path: &Path,
    content: &str,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();
    if written.contains(&relative) {
        return Ok(());
    }
    if path.exists() && !force {
        if !skipped.contains(&relative) {
            skipped.push(relative);
        }
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid output path {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))?;
    if !path.exists() {
        bail!("file was not created: {}", path.display());
    }
    if !written.contains(&relative) {
        written.push(relative);
    }
    Ok(())
}
