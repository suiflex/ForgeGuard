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
const OPENCLAW_PLUGIN_MANIFEST: &str = include_str!("../assets/openclaw/openclaw.plugin.json");
const OPENCLAW_PLUGIN_PACKAGE: &str = include_str!("../assets/openclaw/package.json");
const OPENCLAW_PLUGIN_ENTRY: &str = include_str!("../assets/openclaw/index.js");
const FORGEGUARD_GITIGNORE: &str = "cache/\nreports/\n";
const AGENT_HOOK_FILES: [&str; 4] = [
    ".claude/settings.json",
    ".codex/hooks.json",
    ".cursor/hooks.json",
    ".agents/hooks.json",
];
/// Antigravity CLI moved global skills out of `.gemini/skills`; `.gemini/config`
/// is documented only for `mcp_config.json`, never for skills or hooks.
const GLOBAL_ANTIGRAVITY_SKILL_DIRECTORY: &str =
    ".gemini/antigravity-cli/skills/forgeguard-engineering";
const OBSOLETE_GLOBAL_ANTIGRAVITY_SKILL_DIRECTORY: &str =
    ".gemini/config/skills/forgeguard-engineering";
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
        "references/general-work.md",
        include_str!("../assets/skills/engineering/references/general-work.md"),
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
    Hermes,
    OpenClaw,
    Antigravity,
    Windsurf,
    Copilot,
    Cline,
    Roo,
    All,
}

#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Reinstall every ForgeGuard-owned file and prune superseded directories.
    pub force: bool,
    /// Overwrite only the ForgeGuard-owned files that have drifted from the
    /// bundled versions. The non-interactive form of answering yes to the
    /// refresh prompt.
    pub refresh: bool,
    pub agents: Vec<AgentTarget>,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            force: false,
            refresh: false,
            agents: vec![AgentTarget::All],
        }
    }
}

/// What an install did, split so the caller can tell "already correct" from
/// "exists but stale".
#[derive(Debug, Default)]
struct InstallLog {
    written: Vec<String>,
    skipped: Vec<String>,
    /// ForgeGuard-owned files that exist but no longer match the bundled
    /// version. Replacing a file the user may have edited is their decision, so
    /// these are reported rather than silently rewritten.
    outdated: Vec<String>,
    /// User-owned files left exactly as they were found.
    kept: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitReport {
    pub detection: ProjectDetection,
    /// The targets actually installed, with `All` already expanded.
    pub agents: Vec<AgentTarget>,
    pub files_written: Vec<String>,
    pub files_skipped: Vec<String>,
    /// Existing ForgeGuard-owned files that differ from the bundled versions.
    pub files_outdated: Vec<String>,
    /// Existing user-owned files ForgeGuard refused to rewrite.
    pub files_kept: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalInitReport {
    pub home: PathBuf,
    pub agents: Vec<AgentTarget>,
    pub files_written: Vec<String>,
    pub files_skipped: Vec<String>,
    pub files_outdated: Vec<String>,
    pub files_kept: Vec<String>,
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
    AgentTarget::Hermes,
    AgentTarget::OpenClaw,
    AgentTarget::Antigravity,
    AgentTarget::Windsurf,
    AgentTarget::Copilot,
    AgentTarget::Cline,
    AgentTarget::Roo,
];

/// Paths that show an agent is actually used here, checked in the order of
/// `ALL_AGENT_TARGETS`. A target matches when any of its markers exists.
///
/// Markers are the agent's own configuration, so a repository that has never run
/// `init` still reports the agents it uses. Where ForgeGuard writes into the same
/// tree it only ever does so for that agent's own target, which keeps a repeat
/// `init` on the same selection. `.agents/skills` is deliberately *not* a marker:
/// Codex, Cursor, and OpenCode share it, so treating it as one would make every
/// re-run silently add Antigravity.
const PROJECT_AGENT_MARKERS: &[(AgentTarget, &[&str])] = &[
    (AgentTarget::Codex, &[".codex"]),
    (AgentTarget::Claude, &[".claude"]),
    (AgentTarget::Cursor, &[".cursor", ".cursorrules"]),
    (AgentTarget::OpenCode, &[".opencode", "opencode.json"]),
    (AgentTarget::Hermes, &[".hermes"]),
    (AgentTarget::OpenClaw, &[".openclaw", "openclaw.json"]),
    (
        AgentTarget::Antigravity,
        &[".agents/rules", ".agents/hooks.json", ".agent/rules"],
    ),
    (
        AgentTarget::Windsurf,
        &[".windsurf", ".devin", ".windsurfrules"],
    ),
    (
        AgentTarget::Copilot,
        &[".github/copilot-instructions.md", ".github/instructions"],
    ),
    (AgentTarget::Cline, &[".clinerules"]),
    (AgentTarget::Roo, &[".roo", ".roorules"]),
];

/// Home-directory equivalents of `PROJECT_AGENT_MARKERS`. Copilot is absent
/// because its instructions are repository-scoped, and Cline because the
/// `~/.agents` directory it reads is also created by a global Codex or Cursor
/// install, which would make it detect itself on the next run.
const GLOBAL_AGENT_MARKERS: &[(AgentTarget, &[&str])] = &[
    (AgentTarget::Codex, &[".codex"]),
    (AgentTarget::Claude, &[".claude"]),
    (AgentTarget::Cursor, &[".cursor"]),
    (AgentTarget::OpenCode, &[".config/opencode"]),
    (AgentTarget::Hermes, &[".hermes"]),
    (AgentTarget::OpenClaw, &[".openclaw"]),
    (AgentTarget::Antigravity, &[".gemini"]),
    (AgentTarget::Windsurf, &[".codeium/windsurf", ".devin"]),
    (AgentTarget::Roo, &[".roo"]),
];

/// Report which agents leave configuration under `root`, so `init` can install
/// for those alone instead of writing every integration into every repository.
pub fn detect_installed_agents(root: &Path, global: bool) -> Vec<AgentTarget> {
    let markers = if global {
        GLOBAL_AGENT_MARKERS
    } else {
        PROJECT_AGENT_MARKERS
    };
    markers
        .iter()
        .filter(|(_, paths)| paths.iter().any(|path| root.join(path).exists()))
        .map(|(target, _)| *target)
        .collect()
}

pub fn initialize_global(home: &Path, options: &InitOptions) -> Result<GlobalInitReport> {
    let home = home
        .canonicalize()
        .with_context(|| format!("failed to resolve home directory {}", home.display()))?;
    let mut log = InstallLog::default();

    // Global configuration belongs to General Guard and is created once. In
    // particular, refreshes must not rewrite values the user has tuned.
    write_config(
        &home,
        &ForgeGuardConfig::new("global", Vec::new()),
        &mut log,
    )?;

    install_agents(
        &home,
        InstallScope::Global,
        &options.agents,
        options.force || options.refresh,
        &mut log,
    )?;

    Ok(GlobalInitReport {
        home,
        agents: expand_agent_targets(&options.agents),
        files_written: log.written,
        files_skipped: log.skipped,
        files_outdated: log.outdated,
        files_kept: log.kept,
    })
}

pub fn initialize_project(root: &Path, options: &InitOptions) -> Result<InitReport> {
    let detection = detect_project(root)?;
    let project_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let config = ForgeGuardConfig::new(project_name, detection.suggested_commands.clone());

    let mut log = InstallLog::default();
    let overwrite = options.force || options.refresh;
    // Configuration is created once and then belongs to the repository: it holds
    // the operating mode and any command the user tuned. Reinstalling files
    // ForgeGuard ships is no reason to throw that away.
    write_config(root, &config, &mut log)?;
    write_file(
        root,
        &root.join(".forgeguard/.gitignore"),
        FORGEGUARD_GITIGNORE,
        overwrite,
        &mut log,
    )?;

    install_agents(
        root,
        InstallScope::Project,
        &options.agents,
        overwrite,
        &mut log,
    )?;
    refresh_stop_hook_timeouts(root, &mut log.written)?;
    ignore_project_agent_directories(root, &options.agents, &mut log)?;

    Ok(InitReport {
        detection,
        agents: expand_agent_targets(&options.agents),
        files_written: log.written,
        files_skipped: log.skipped,
        files_outdated: log.outdated,
        files_kept: log.kept,
    })
}

fn install_agents(
    root: &Path,
    scope: InstallScope,
    requested: &[AgentTarget],
    overwrite: bool,
    log: &mut InstallLog,
) -> Result<()> {
    for target in expand_agent_targets(requested) {
        match target {
            AgentTarget::Codex => install_codex(root, scope, overwrite, log)?,
            AgentTarget::Claude => install_claude(root, scope, overwrite, log)?,
            AgentTarget::Cursor => install_cursor(root, scope, overwrite, log)?,
            AgentTarget::OpenCode => install_opencode(root, scope, overwrite, log)?,
            AgentTarget::Hermes => install_shared_skill_agent(root, target, scope, overwrite, log)?,
            AgentTarget::OpenClaw => install_openclaw(root, scope, overwrite, log)?,
            AgentTarget::Antigravity => install_antigravity(root, scope, overwrite, log)?,
            AgentTarget::Windsurf
            | AgentTarget::Copilot
            | AgentTarget::Cline
            | AgentTarget::Roo => install_agents_md(root, target, scope, overwrite, log)?,
            AgentTarget::All => unreachable!("all is expanded before installation"),
        }
    }
    Ok(())
}

fn expand_agent_targets(requested: &[AgentTarget]) -> Vec<AgentTarget> {
    // ponytail: O(n²) contains-check over a tiny fixed agent list; a set is not worth it.
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
    log: &mut InstallLog,
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
                | AgentTarget::Hermes
                | AgentTarget::OpenClaw
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
        record_path(root, &path, &mut log.written);
    }
    Ok(())
}

fn write_config(root: &Path, config: &ForgeGuardConfig, log: &mut InstallLog) -> Result<()> {
    let path = root.join(".forgeguard/config.toml");
    if path.exists() {
        let mut existing = ForgeGuardConfig::load(root)?;
        if existing.reconcile_commands(&config.commands) > 0 {
            existing.save(root)?;
            record_path(root, &path, &mut log.written);
        }
        return Ok(());
    }
    let content =
        toml::to_string_pretty(config).context("failed to serialize ForgeGuard config")?;
    write_file(root, &path, &content, false, log)
}

fn install_codex(
    root: &Path,
    scope: InstallScope,
    overwrite: bool,
    log: &mut InstallLog,
) -> Result<()> {
    if overwrite {
        remove_legacy_skills(root, ".codex/skills", true)?;
    }
    let policy_path = match scope {
        InstallScope::Project => root.join("AGENTS.md"),
        InstallScope::Global => root.join(".codex/AGENTS.md"),
    };
    write_file(root, &policy_path, AGENTS_TEMPLATE, overwrite, log)?;
    install_skill(
        root,
        ".agents/skills/forgeguard-engineering",
        overwrite,
        log,
    )?;
    install_grouped_hook(
        root,
        &root.join(".codex/hooks.json"),
        "Stop",
        None,
        CODEX_HOOK_COMMAND,
        log,
    )?;
    install_grouped_hook(
        root,
        &root.join(".codex/hooks.json"),
        "SessionStart",
        Some("startup|resume|compact"),
        CODEX_CONTEXT_HOOK_COMMAND,
        log,
    )?;
    install_grouped_hook(
        root,
        &root.join(".codex/hooks.json"),
        "PreToolUse",
        Some("apply_patch|Edit|Write"),
        CODEX_SCOPE_HOOK_COMMAND,
        log,
    )
}

fn install_claude(
    root: &Path,
    scope: InstallScope,
    overwrite: bool,
    log: &mut InstallLog,
) -> Result<()> {
    if overwrite {
        remove_legacy_skills(root, ".claude/skills", false)?;
    }
    let policy_path = match scope {
        InstallScope::Project => root.join("CLAUDE.md"),
        InstallScope::Global => root.join(".claude/CLAUDE.md"),
    };
    write_file(root, &policy_path, CLAUDE_TEMPLATE, overwrite, log)?;
    install_skill(
        root,
        ".claude/skills/forgeguard-engineering",
        overwrite,
        log,
    )?;
    install_grouped_hook(
        root,
        &root.join(".claude/settings.json"),
        "Stop",
        None,
        CLAUDE_HOOK_COMMAND,
        log,
    )?;
    install_grouped_hook(
        root,
        &root.join(".claude/settings.json"),
        "SessionStart",
        Some("startup|resume|compact"),
        CLAUDE_CONTEXT_HOOK_COMMAND,
        log,
    )?;
    install_grouped_hook(
        root,
        &root.join(".claude/settings.json"),
        "UserPromptSubmit",
        None,
        CLAUDE_CONTEXT_HOOK_COMMAND,
        log,
    )?;
    install_grouped_hook(
        root,
        &root.join(".claude/settings.json"),
        "PreToolUse",
        Some("Edit|Write|MultiEdit|NotebookEdit"),
        CLAUDE_SCOPE_HOOK_COMMAND,
        log,
    )
}

fn install_cursor(
    root: &Path,
    scope: InstallScope,
    overwrite: bool,
    log: &mut InstallLog,
) -> Result<()> {
    let rule_path = match scope {
        InstallScope::Project => root.join(".cursor/rules/forgeguard.mdc"),
        InstallScope::Global => root.join(".cursor/rules/forgeguard.mdc"),
    };
    write_file(root, &rule_path, CURSOR_TEMPLATE, overwrite, log)?;
    install_skill(
        root,
        ".agents/skills/forgeguard-engineering",
        overwrite,
        log,
    )?;
    let path = root.join(".cursor/hooks.json");
    install_cursor_hook(root, &path, "stop", None, CURSOR_HOOK_COMMAND, log)?;
    install_cursor_hook(
        root,
        &path,
        "sessionStart",
        None,
        CURSOR_CONTEXT_HOOK_COMMAND,
        log,
    )?;
    install_cursor_hook(
        root,
        &path,
        "preToolUse",
        Some("Write|StrReplace|Delete|ApplyPatch"),
        CURSOR_SCOPE_HOOK_COMMAND,
        log,
    )
}

fn install_opencode(
    root: &Path,
    scope: InstallScope,
    overwrite: bool,
    log: &mut InstallLog,
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
    write_file(root, &policy_path, AGENTS_TEMPLATE, overwrite, log)?;
    install_skill(root, skill_directory, overwrite, log)
}

fn install_shared_skill_agent(
    root: &Path,
    target: AgentTarget,
    scope: InstallScope,
    overwrite: bool,
    log: &mut InstallLog,
) -> Result<()> {
    let skill_directory = match (target, scope) {
        (AgentTarget::Hermes, InstallScope::Global) => ".hermes/skills/forgeguard-engineering",
        (AgentTarget::OpenClaw, InstallScope::Global) => ".openclaw/skills/forgeguard-engineering",
        (_, InstallScope::Project) => ".agents/skills/forgeguard-engineering",
        _ => unreachable!("only Hermes and OpenClaw use this installer"),
    };
    if matches!(scope, InstallScope::Project) {
        write_file(
            root,
            &root.join("AGENTS.md"),
            AGENTS_TEMPLATE,
            overwrite,
            log,
        )?;
    }
    install_skill(root, skill_directory, overwrite, log)
}

fn install_openclaw(
    root: &Path,
    scope: InstallScope,
    overwrite: bool,
    log: &mut InstallLog,
) -> Result<()> {
    if matches!(scope, InstallScope::Project) {
        return install_shared_skill_agent(root, AgentTarget::OpenClaw, scope, overwrite, log);
    }
    install_skill(
        root,
        ".openclaw/skills/forgeguard-engineering",
        overwrite,
        log,
    )?;
    for (relative, content) in [
        ("openclaw.plugin.json", OPENCLAW_PLUGIN_MANIFEST),
        ("package.json", OPENCLAW_PLUGIN_PACKAGE),
        ("index.js", OPENCLAW_PLUGIN_ENTRY),
    ] {
        write_file(
            root,
            &root.join(".openclaw/extensions/forgeguard").join(relative),
            content,
            overwrite,
            log,
        )?;
    }
    configure_openclaw_plugin(root, log)
}

fn configure_openclaw_plugin(root: &Path, log: &mut InstallLog) -> Result<()> {
    let path = root.join(".openclaw/openclaw.json");
    let mut document = read_json_object(&path).with_context(|| {
        format!(
            "cannot enable the ForgeGuard OpenClaw plugin in {}; preserve the file and enable `forgeguard` manually",
            path.display()
        )
    })?;
    let original = document.clone();
    let plugins = object_field(&mut document, "plugins", &path)?;
    if let Some(allow) = plugins.get_mut("allow") {
        let allow = allow.as_array_mut().with_context(|| {
            format!(
                "expected `plugins.allow` to be an array in {}",
                path.display()
            )
        })?;
        if !allow
            .iter()
            .any(|value| value.as_str() == Some("forgeguard"))
        {
            allow.push(json!("forgeguard"));
        }
    }
    let entries = plugins
        .entry("entries")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .with_context(|| {
            format!(
                "expected `plugins.entries` to be an object in {}",
                path.display()
            )
        })?;
    let forgeguard = entries
        .entry("forgeguard")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .with_context(|| {
            format!(
                "expected `plugins.entries.forgeguard` to be an object in {}",
                path.display()
            )
        })?;
    forgeguard.insert("enabled".to_owned(), json!(true));
    let hooks = forgeguard
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .with_context(|| {
            format!(
                "expected `plugins.entries.forgeguard.hooks` to be an object in {}",
                path.display()
            )
        })?;
    hooks.insert("allowConversationAccess".to_owned(), json!(true));
    hooks.insert("allowPromptInjection".to_owned(), json!(true));
    if document == original {
        record_path(root, &path, &mut log.skipped);
        return Ok(());
    }
    write_json_document(root, &path, &document, &mut log.written)
}

fn install_antigravity(
    root: &Path,
    scope: InstallScope,
    overwrite: bool,
    log: &mut InstallLog,
) -> Result<()> {
    let (policy_path, skill_directory) = match scope {
        InstallScope::Project => (
            root.join(".agents/rules/forgeguard.md"),
            ".agents/skills/forgeguard-engineering",
        ),
        InstallScope::Global => (
            root.join(".gemini/GEMINI.md"),
            GLOBAL_ANTIGRAVITY_SKILL_DIRECTORY,
        ),
    };
    if overwrite {
        remove_directory(root, OBSOLETE_GLOBAL_ANTIGRAVITY_SKILL_DIRECTORY)?;
    }
    write_file(root, &policy_path, AGENTS_TEMPLATE, overwrite, log)?;
    install_skill(root, skill_directory, overwrite, log)?;

    // Only the workspace agent has a documented local hook file. Antigravity
    // publishes no user-level hook path, so a global install stops at rules and
    // skills rather than writing a file no product reads.
    let InstallScope::Project = scope else {
        return Ok(());
    };
    let hook_path = root.join(".agents/hooks.json");
    install_antigravity_simple_hook(root, &hook_path, "Stop", ANTIGRAVITY_HOOK_COMMAND, log)?;
    install_antigravity_simple_hook(
        root,
        &hook_path,
        "PreInvocation",
        ANTIGRAVITY_CONTEXT_HOOK_COMMAND,
        log,
    )?;
    install_antigravity_tool_hook(
        root,
        &hook_path,
        "write_to_file|replace_file_content|multi_replace_file_content",
        ANTIGRAVITY_SCOPE_HOOK_COMMAND,
        log,
    )
}

/// Windsurf, Copilot, Cline, and Roo all read a workspace `AGENTS.md` natively and
/// expose no hook API ForgeGuard can drive, so supporting a project costs one
/// shared policy file and nothing else. They do not receive the engineering skill:
/// none of them has a documented skill directory, and writing four near-duplicate
/// rules files is the clutter this selection work exists to remove.
///
/// User-level rules are not shared the same way, so a global install follows each
/// agent's own documented path — or writes nothing when the agent documents none,
/// rather than leaving a file no product reads.
fn install_agents_md(
    root: &Path,
    target: AgentTarget,
    scope: InstallScope,
    overwrite: bool,
    log: &mut InstallLog,
) -> Result<()> {
    let relative = match scope {
        InstallScope::Project => "AGENTS.md",
        InstallScope::Global => match global_policy_path(target) {
            Some(path) => path,
            None => return Ok(()),
        },
    };
    write_file(root, &root.join(relative), AGENTS_TEMPLATE, overwrite, log)
}

/// Where each `AGENTS.md`-only agent reads user-level rules from. `None` means the
/// agent documents no user-level location, so a global install writes nothing for
/// it rather than guessing.
fn global_policy_path(target: AgentTarget) -> Option<&'static str> {
    match target {
        // Cline lists `~/.agents/AGENTS.md` among its rule sources.
        AgentTarget::Cline => Some(".agents/AGENTS.md"),
        // Windsurf/Devin reads AGENTS.md per workspace; globally it reads one file.
        AgentTarget::Windsurf => Some(".codeium/windsurf/memories/global_rules.md"),
        // Roo reads every file in its global rules directory.
        AgentTarget::Roo => Some(".roo/rules/forgeguard.md"),
        // Copilot instructions are repository-scoped only.
        _ => None,
    }
}

fn install_skill(
    root: &Path,
    directory: &str,
    overwrite: bool,
    log: &mut InstallLog,
) -> Result<()> {
    for (relative, content) in SKILL_ASSETS {
        let path = root.join(directory).join(relative);
        write_file(root, &path, content, overwrite, log)?;
    }
    if overwrite {
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
    log: &mut InstallLog,
) -> Result<()> {
    let mut document = read_json_object(path)?;
    if document
        .pointer(&format!("/hooks/{event}"))
        .is_some_and(|hooks| contains_hook_command(hooks, command))
    {
        record_path(root, path, &mut log.skipped);
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
    write_json_document(root, path, &document, &mut log.written)
}

fn install_cursor_hook(
    root: &Path,
    path: &Path,
    event: &str,
    matcher: Option<&str>,
    command: &str,
    log: &mut InstallLog,
) -> Result<()> {
    let mut document = read_json_object(path)?;
    if contains_string(&document, command) {
        record_path(root, path, &mut log.skipped);
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
    write_json_document(root, path, &document, &mut log.written)
}

fn install_antigravity_simple_hook(
    root: &Path,
    path: &Path,
    event: &str,
    command: &str,
    log: &mut InstallLog,
) -> Result<()> {
    let mut document = read_json_object(path)?;
    if contains_string(&document, command) {
        record_path(root, path, &mut log.skipped);
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
    write_json_document(root, path, &document, &mut log.written)
}

fn install_antigravity_tool_hook(
    root: &Path,
    path: &Path,
    matcher: &str,
    command: &str,
    log: &mut InstallLog,
) -> Result<()> {
    let mut document = read_json_object(path)?;
    if contains_string(&document, command) {
        record_path(root, path, &mut log.skipped);
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
    write_json_document(root, path, &document, &mut log.written)
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

/// Drop a directory ForgeGuard used to install into. A symlink is unlinked rather
/// than followed, so a user who points the path elsewhere does not lose that tree.
fn remove_directory(root: &Path, relative: &str) -> Result<()> {
    let path = root.join(relative);
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return Ok(());
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(&path).with_context(|| format!("failed to remove {}", path.display()))
    } else {
        fs::remove_file(&path).with_context(|| format!("failed to remove {}", path.display()))
    }
}

/// Files that carry the user's own instructions alongside ForgeGuard's line.
/// ForgeGuard seeds them when they are absent and never touches them again:
/// rewriting one destroys work that was never ours, so no flag reaches them.
/// Everything else ForgeGuard installs lives under a name or directory of its
/// own, where the bundled copy is the only copy and refreshing is safe.
const USER_OWNED_FILES: &[&str] = &[
    "CLAUDE.md",
    "AGENTS.md",
    ".claude/CLAUDE.md",
    ".codex/AGENTS.md",
    ".config/opencode/AGENTS.md",
    ".gemini/GEMINI.md",
    ".agents/AGENTS.md",
    ".codeium/windsurf/memories/global_rules.md",
];

fn is_user_owned(relative: &str) -> bool {
    let relative = relative.replace('\\', "/");
    USER_OWNED_FILES.contains(&relative.as_str())
}

fn write_file(
    root: &Path,
    path: &Path,
    content: &str,
    overwrite: bool,
    log: &mut InstallLog,
) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();
    if log.written.contains(&relative) {
        return Ok(());
    }
    // A symlink belongs to whoever made it. Writing through one edits the file
    // at the other end instead: a repository that points AGENTS.md at CLAUDE.md
    // would have its CLAUDE.md replaced by the AGENTS template, and the consent
    // prompt would have named the wrong file. Leave it, and say so.
    if fs::symlink_metadata(path).is_ok_and(|entry| entry.file_type().is_symlink()) {
        record(&mut log.skipped, relative);
        return Ok(());
    }
    if path.exists() {
        // A file matching the bundle needs nothing; one that differs is either
        // an older release or something the user edited, and only the caller
        // knows which. Either way it is reported, not quietly replaced.
        let current = fs::read_to_string(path).unwrap_or_default();
        if current == content {
            record(&mut log.skipped, relative);
            return Ok(());
        }
        // Checked before `overwrite` so that neither --force nor --refresh can
        // reach the write below: the user's own file is not ours to replace.
        if is_user_owned(&relative) {
            record(&mut log.kept, relative.clone());
            record(&mut log.skipped, relative);
            return Ok(());
        }
        if !overwrite {
            record(&mut log.outdated, relative.clone());
            record(&mut log.skipped, relative);
            return Ok(());
        }
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid output path {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))?;
    if !path.exists() {
        bail!("file was not created: {}", path.display());
    }
    record(&mut log.written, relative);
    Ok(())
}

fn record(records: &mut Vec<String>, value: String) {
    if !records.contains(&value) {
        records.push(value);
    }
}
