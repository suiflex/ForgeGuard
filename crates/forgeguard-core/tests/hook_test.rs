use std::{fs, process::Command};

use forgeguard_core::{
    evaluate_context_hook, evaluate_scope_hook, evaluate_stop_hook, mark_task_ready,
    mark_task_ready_with_confidence, render_hook_decision, start_task, start_task_with_contract,
    task_state, update_task_todos, CommandConfig, ForgeGuardConfig, GoalContract, GuardMode,
    HookAgent, HookDecision, TaskStatus,
};
use tempfile::tempdir;

#[test]
fn blocked_hook_is_compact_cached_and_bounded() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = forgeguard_core::GuardMode::Strict;
    config.focus.auto_poke = false;
    config.save(directory.path()).expect("save config");
    for index in 0..40 {
        fs::write(
            directory.path().join(format!("repository-{index}.ts")),
            "import { PrismaClient } from '@prisma/client';\nconst db = new PrismaClient();\nfor (const user of users) { await db.query('SELECT id FROM users'); }\n",
        )
        .expect("write source");
    }
    let input = format!(
        r#"{{"workspacePaths":["{}"],"executionNum":1}}"#,
        directory.path().display()
    );

    let (decision, cache_hit) =
        evaluate_stop_hook(directory.path(), &input).expect("evaluate hook");
    let HookDecision::Block(message) = decision else {
        panic!("expected blocked decision");
    };
    assert!(!cache_hit);
    assert!(message.chars().count() <= 2_000);
    assert!(message.contains("additional finding(s) omitted"));
    assert!(message.contains("Full report: .forgeguard/reports/latest.json"));
    assert!(directory
        .path()
        .join(".forgeguard/reports/latest.json")
        .exists());

    let (cached, cache_hit) =
        evaluate_stop_hook(directory.path(), &input).expect("evaluate cached hook");
    assert!(matches!(cached, HookDecision::Block(_)));
    assert!(cache_hit);

    let repeated = format!(
        r#"{{"cwd":"{}","stop_hook_active":true}}"#,
        directory.path().display()
    );
    let (decision, cache_hit) =
        evaluate_stop_hook(directory.path(), &repeated).expect("evaluate repeated hook");
    assert!(matches!(decision, HookDecision::Stop(_)));
    assert!(cache_hit);

    let antigravity_repeated = format!(
        r#"{{"workspacePaths":["{}"],"executionNum":2}}"#,
        directory.path().display()
    );
    let (decision, cache_hit) = evaluate_stop_hook(directory.path(), &antigravity_repeated)
        .expect("evaluate repeated Antigravity hook");
    assert!(matches!(decision, HookDecision::Stop(_)));
    assert!(cache_hit);
}

#[test]
fn agent_protocols_are_silent_or_structured() {
    assert_eq!(
        render_hook_decision(HookAgent::Codex, &HookDecision::Pass),
        ""
    );
    assert_eq!(
        render_hook_decision(HookAgent::Claude, &HookDecision::Pass),
        ""
    );
    assert_eq!(
        render_hook_decision(HookAgent::Cursor, &HookDecision::Pass),
        "{}"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&render_hook_decision(
            HookAgent::Antigravity,
            &HookDecision::Pass
        ))
        .expect("Antigravity pass JSON")["decision"],
        "stop"
    );

    let reason = "Fix failing tests".to_owned();
    let claude = render_hook_decision(HookAgent::Claude, &HookDecision::Block(reason.clone()));
    let cursor = render_hook_decision(HookAgent::Cursor, &HookDecision::Block(reason.clone()));
    let codex = render_hook_decision(HookAgent::Codex, &HookDecision::Block(reason));
    let antigravity = render_hook_decision(
        HookAgent::Antigravity,
        &HookDecision::Block("Fix failing tests".to_owned()),
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&claude).expect("Claude JSON")["decision"],
        "block"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&cursor).expect("Cursor JSON")
            ["followup_message"],
        "Fix failing tests"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&codex).expect("Codex JSON")["systemMessage"],
        serde_json::Value::Null
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&codex).expect("Codex JSON")["decision"],
        "block"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&antigravity).expect("Antigravity JSON")
            ["decision"],
        "continue"
    );

    let codex_stop = render_hook_decision(
        HookAgent::Codex,
        &HookDecision::Stop("No progress".to_owned()),
    );
    let codex_stop: serde_json::Value = serde_json::from_str(&codex_stop).expect("Codex stop JSON");
    assert_eq!(codex_stop["continue"], false);
    assert_eq!(codex_stop["stopReason"], "No progress");
}

#[test]
fn task_state_blocks_early_stop_then_completes_with_evidence() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = forgeguard_core::GuardMode::Strict;
    config.focus.auto_poke = false;
    config.save(directory.path()).expect("save config");
    fs::write(
        directory.path().join("service.ts"),
        "export const value = 1;\n",
    )
    .expect("write source");
    start_task(
        directory.path(),
        "session-1",
        "Add the service value and verify it",
        &["service.ts".to_owned()],
        false,
    )
    .expect("start task");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"session-1"}}"#,
        directory.path().display()
    );

    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("early stop");
    assert!(matches!(decision, HookDecision::Block(_)));

    mark_task_ready(
        directory.path(),
        "session-1",
        &["forgeguard gate passed".to_owned()],
    )
    .expect("mark ready");
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("completed stop");
    assert_eq!(decision, HookDecision::Pass);
    assert_eq!(
        task_state(directory.path(), "session-1")
            .expect("read task")
            .expect("task")
            .status,
        TaskStatus::Completed
    );
}

#[test]
fn auto_poke_runs_bounded_evidence_phases_before_completion() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = forgeguard_core::GuardMode::Strict;
    config.focus.auto_poke = true;
    config.focus.max_auto_pokes = 99;
    config.save(directory.path()).expect("save config");
    start_task_with_contract(
        directory.path(),
        "session-poke",
        "Finish implementation, tests, and review",
        &[],
        false,
        GoalContract {
            metric: Some("verified completion checks".to_owned()),
            baseline: Some("implementation incomplete".to_owned()),
            target: Some("all checks pass".to_owned()),
            guardrails: vec!["no regression".to_owned()],
            verifications: vec!["cargo test".to_owned()],
        },
        &["implement requested behavior".to_owned()],
    )
    .expect("start task");
    update_task_todos(directory.path(), "session-poke", &[], &[1]).expect("complete todo");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"session-poke"}}"#,
        directory.path().display()
    );
    let phases = [
        "remaining TODOs",
        "relevant tests and checks",
        "review the full diff",
        "verify contracts",
        "final verification",
    ];

    for (index, phase) in phases.into_iter().enumerate() {
        mark_task_ready_with_confidence(
            directory.path(),
            "session-poke",
            &[format!("phase {} evidence", index + 1)],
            Some(100),
        )
        .expect("mark phase ready");
        let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("auto-poke stop");
        let HookDecision::Block(message) = decision else {
            panic!("expected auto-poke block");
        };
        assert!(message.contains(&format!("auto-poke {}/5", index + 1)));
        assert!(message.contains(phase));
        let task = task_state(directory.path(), "session-poke")
            .expect("read task")
            .expect("task");
        assert_eq!(task.status, TaskStatus::Active);
        assert_eq!(task.auto_pokes, (index + 1) as u32);
        assert!(task.evidence.is_empty());
        let context = evaluate_context_hook(directory.path(), &input, HookAgent::Codex)
            .expect("evaluate auto-poke context")
            .expect("auto-poke context");
        assert!(context.contains(phase));
    }

    mark_task_ready_with_confidence(
        directory.path(),
        "session-poke",
        &["final gate passed".to_owned()],
        Some(100),
    )
    .expect("mark final ready");
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("final stop");
    assert_eq!(decision, HookDecision::Pass);
    assert_eq!(
        task_state(directory.path(), "session-poke")
            .expect("read task")
            .expect("task")
            .status,
        TaskStatus::Completed
    );
}

#[test]
fn auto_poke_uses_hill_goal_todos_and_confidence_before_completion() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = forgeguard_core::GuardMode::Strict;
    config.focus.auto_poke = true;
    config.focus.max_auto_pokes = 5;
    config.save(directory.path()).expect("save config");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"session-state"}}"#,
        directory.path().display()
    );

    start_task(
        directory.path(),
        "session-state",
        "Improve performance",
        &[],
        false,
    )
    .expect("start abstract task");
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("abstract stop");
    let HookDecision::Block(message) = decision else {
        panic!("abstract goal must continue");
    };
    assert!(message.contains("hill-climbability contract is 0/100"));

    let goal = GoalContract {
        metric: Some("p95 latency /search".to_owned()),
        baseline: Some("900 ms".to_owned()),
        target: Some("below 300 ms".to_owned()),
        guardrails: vec!["error rate does not increase".to_owned()],
        verifications: vec!["regression tests pass".to_owned()],
    };
    assert_eq!(goal.hill_climbability(), 100);
    start_task_with_contract(
        directory.path(),
        "session-state",
        "Reduce /search p95 latency without regressions",
        &[],
        false,
        goal,
        &[
            "measure baseline".to_owned(),
            "optimize endpoint".to_owned(),
        ],
    )
    .expect("reframe task");
    assert_eq!(
        task_state(directory.path(), "session-state")
            .expect("read reframed task")
            .expect("reframed task")
            .auto_pokes,
        1
    );
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("todo stop");
    let HookDecision::Block(message) = decision else {
        panic!("pending todos must continue");
    };
    assert!(message.contains("2 todo(s) remain"));

    update_task_todos(directory.path(), "session-state", &[], &[1]).expect("complete first todo");
    assert!(mark_task_ready(directory.path(), "session-state", &["partial".to_owned()]).is_err());
    update_task_todos(directory.path(), "session-state", &[], &[2]).expect("complete second todo");
    mark_task_ready_with_confidence(
        directory.path(),
        "session-state",
        &["tests passed".to_owned()],
        Some(50),
    )
    .expect("mark low-confidence ready");
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("confidence stop");
    let HookDecision::Block(message) = decision else {
        panic!("low confidence must continue");
    };
    assert!(message.contains("confidence is 50/100"));
    let task = task_state(directory.path(), "session-state")
        .expect("read task")
        .expect("task");
    assert_eq!(task.confidence_history, vec![50]);
    assert_eq!(task.confidence, None);
    assert!(mark_task_ready_with_confidence(
        directory.path(),
        "session-state",
        &["invalid confidence".to_owned()],
        Some(101),
    )
    .is_err());

    for confidence in [60, 70] {
        mark_task_ready_with_confidence(
            directory.path(),
            "session-state",
            &["more evidence".to_owned()],
            Some(confidence),
        )
        .expect("mark still-low confidence");
        let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("bounded poke");
        assert!(matches!(decision, HookDecision::Block(_)));
    }
    mark_task_ready_with_confidence(
        directory.path(),
        "session-state",
        &["confidence remains low".to_owned()],
        Some(70),
    )
    .expect("mark exhausted confidence");
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("exhausted poke");
    assert!(matches!(decision, HookDecision::Stop(_)));
    assert_eq!(
        task_state(directory.path(), "session-state")
            .expect("read blocked task")
            .expect("blocked task")
            .status,
        TaskStatus::Blocked
    );
}

#[test]
fn task_context_is_session_scoped_and_scope_drift_is_warned() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    start_task(
        directory.path(),
        "session-a",
        "Change only core source",
        &["src".to_owned()],
        true,
    )
    .expect("start task");
    let context_input = format!(
        r#"{{"cwd":"{}","session_id":"session-a","hook_event_name":"SessionStart"}}"#,
        directory.path().display()
    );
    let context = evaluate_context_hook(directory.path(), &context_input, HookAgent::Codex)
        .expect("evaluate context")
        .expect("context");
    assert!(context.contains("Change only core source"));
    assert!(context.contains("native goal evaluator"));

    let outside = format!(
        r#"{{"cwd":"{}","session_id":"session-a","tool_input":{{"file_path":"tests/api.rs"}}}}"#,
        directory.path().display()
    );
    let warning = evaluate_scope_hook(directory.path(), &outside)
        .expect("evaluate scope")
        .expect("scope warning");
    assert!(warning.contains("tests/api.rs"));

    let inside = format!(
        r#"{{"cwd":"{}","session_id":"session-a","tool_input":{{"file_path":"src/api.rs"}}}}"#,
        directory.path().display()
    );
    assert!(evaluate_scope_hook(directory.path(), &inside)
        .expect("evaluate scope")
        .is_none());
    assert!(task_state(directory.path(), "session-b")
        .expect("read other session")
        .is_none());

    let later_invocation = format!(
        r#"{{"workspacePaths":["{}"],"conversationId":"new-session","invocationNum":1}}"#,
        directory.path().display()
    );
    assert!(
        evaluate_context_hook(directory.path(), &later_invocation, HookAgent::Antigravity)
            .expect("evaluate later Antigravity invocation")
            .is_none()
    );
}

#[test]
fn stop_retry_budget_is_isolated_by_session() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = forgeguard_core::GuardMode::Strict;
    config.save(directory.path()).expect("save config");
    fs::write(
        directory.path().join("service.ts"),
        "import { PrismaClient } from '@prisma/client';\nconst db = new PrismaClient();\nfor (const user of users) { await db.query('SELECT id FROM users'); }\n",
    )
    .expect("write source");
    let input = |session: &str| {
        format!(
            r#"{{"cwd":"{}","session_id":"{session}"}}"#,
            directory.path().display()
        )
    };

    let (first_a, _) =
        evaluate_stop_hook(directory.path(), &input("session-a")).expect("first session-a stop");
    assert!(matches!(first_a, HookDecision::Block(_)));
    let (second_a, _) =
        evaluate_stop_hook(directory.path(), &input("session-a")).expect("second session-a stop");
    assert!(matches!(second_a, HookDecision::Block(_)));
    let (first_b, _) =
        evaluate_stop_hook(directory.path(), &input("session-b")).expect("first session-b stop");
    let HookDecision::Block(message) = first_b else {
        panic!("session-b must start with a fresh retry budget");
    };
    assert!(message.contains("attempt 1/3"));
}

#[test]
fn cursor_transcript_path_isolates_retry_budget_without_a_session_id() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = forgeguard_core::GuardMode::Strict;
    config.save(directory.path()).expect("save config");
    fs::write(
        directory.path().join("service.ts"),
        "import { PrismaClient } from '@prisma/client';\nconst db = new PrismaClient();\nfor (const user of users) { await db.query('SELECT id FROM users'); }\n",
    )
    .expect("write source");
    let input = |transcript: &str| {
        format!(
            r#"{{"cwd":"{}","transcript_path":"{transcript}"}}"#,
            directory.path().display()
        )
    };

    evaluate_stop_hook(directory.path(), &input("/tmp/cursor-a.jsonl"))
        .expect("first cursor-a stop");
    evaluate_stop_hook(directory.path(), &input("/tmp/cursor-a.jsonl"))
        .expect("second cursor-a stop");
    let (first_b, _) = evaluate_stop_hook(directory.path(), &input("/tmp/cursor-b.jsonl"))
        .expect("first cursor-b stop");
    let HookDecision::Block(message) = first_b else {
        panic!("cursor-b must start with a fresh retry budget");
    };
    assert!(message.contains("attempt 1/3"));
}

#[test]
fn task_contract_rejects_untrusted_paths_and_unsupported_evidence() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    assert!(start_task(directory.path(), "../escape", "Unsafe session", &[], false,).is_err());
    assert!(start_task(
        directory.path(),
        "safe-session",
        "Unsafe scope",
        &["../outside".to_owned()],
        false,
    )
    .is_err());
    start_task(
        directory.path(),
        "safe-session",
        "Valid task",
        &["src".to_owned()],
        false,
    )
    .expect("start valid task");
    assert!(mark_task_ready(directory.path(), "safe-session", &[]).is_err());
}

#[test]
fn auto_gate_reports_without_blocking_without_config_and_ignores_artifacts() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    // A pre-existing root .gitignore must be appended to, not clobbered.
    fs::write(directory.path().join(".gitignore"), "node_modules/\n").expect("write gitignore");
    fs::write(
        directory.path().join("service.ts"),
        "import { PrismaClient } from '@prisma/client';\nconst db = new PrismaClient();\nfor (const user of users) { await db.query('SELECT id FROM users WHERE id = 1'); }\n",
    )
    .expect("write source");

    let input = format!(
        r#"{{"cwd":"{}","executionNum":1}}"#,
        directory.path().display()
    );
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("evaluate hook");
    assert_eq!(decision, HookDecision::Pass);

    // No manual setup, yet artifacts are kept out of version control.
    let marker = fs::read_to_string(directory.path().join(".forgeguard/.gitignore"))
        .expect("read forgeguard gitignore");
    assert_eq!(marker.trim(), "*");
    let root_ignore =
        fs::read_to_string(directory.path().join(".gitignore")).expect("read root gitignore");
    assert!(root_ignore.contains("node_modules/"));
    assert!(root_ignore
        .lines()
        .any(|line| line.trim().trim_end_matches('/') == ".forgeguard"));
    assert!(!directory.path().join(".forgeguard/config.toml").exists());
}

#[test]
fn auto_gate_passes_for_repo_without_code() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    fs::write(directory.path().join("README.md"), "# docs only\n").expect("write readme");

    let input = format!(
        r#"{{"cwd":"{}","executionNum":1}}"#,
        directory.path().display()
    );
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("evaluate hook");
    assert_eq!(decision, HookDecision::Pass);
    assert!(!directory.path().join(".forgeguard").exists());
}

#[test]
fn initialized_non_git_workspace_gates_each_nested_repository() {
    let directory = tempdir().expect("temp directory");
    let mut workspace_config = ForgeGuardConfig::new("workspace", Vec::new());
    workspace_config.focus.enabled = false;
    workspace_config
        .save(directory.path())
        .expect("save workspace config");

    let service_a = directory.path().join("service-a");
    let service_b = directory.path().join("service-b");
    fs::create_dir_all(&service_a).expect("create service A");
    fs::create_dir_all(&service_b).expect("create service B");
    git_init(&service_a);
    git_init(&service_b);
    fs::write(service_a.join("service.ts"), "export const ready = true;\n")
        .expect("write TypeScript service");
    fs::write(service_b.join("main.rs"), "fn main() {}\n").expect("write Rust service");

    ForgeGuardConfig::new(
        "service-b",
        vec![CommandConfig {
            name: "test".to_owned(),
            command: "test ! -f main.rs".to_owned(),
            required: true,
            enabled: true,
            timeout_seconds: 10,
        }],
    )
    .save(&service_b)
    .expect("save service B config");

    let input = format!(r#"{{"cwd":"{}"}}"#, directory.path().display());
    let (decision, _) =
        evaluate_stop_hook(directory.path(), &input).expect("evaluate workspace hook");

    assert!(matches!(decision, HookDecision::Block(_)));
    assert!(!service_a.join(".forgeguard/config.toml").exists());
    assert!(service_a.join(".forgeguard/reports/latest.json").is_file());
    assert!(service_b.join(".forgeguard/reports/latest.json").is_file());
}

#[test]
fn zero_config_nested_repository_inherits_workspace_config_version() {
    let directory = tempdir().expect("temp directory");
    let mut workspace_config = ForgeGuardConfig::new("workspace", Vec::new());
    workspace_config.version = 1;
    workspace_config.mode = GuardMode::Strict;
    workspace_config.focus.enabled = false;
    workspace_config
        .save(directory.path())
        .expect("save workspace config");

    let repository = directory.path().join("service");
    fs::create_dir_all(&repository).expect("create service");
    git_init(&repository);
    fs::write(
        repository.join("nested.ts"),
        "for (const row of rows) { for (const value of row) { console.log(value); } }\n",
    )
    .expect("write warning-only source");

    let input = format!(r#"{{"cwd":"{}"}}"#, directory.path().display());
    let (decision, _) =
        evaluate_stop_hook(directory.path(), &input).expect("evaluate workspace hook");

    assert_eq!(decision, HookDecision::Pass);
}

fn git_init(root: &std::path::Path) {
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root)
        .status()
        .expect("run git init");
    assert!(status.success());
}
