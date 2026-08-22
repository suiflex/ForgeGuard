use std::{fs, process::Command};

use forgeguard_core::{
    evaluate_context_hook, evaluate_scope_hook, evaluate_stop_hook, is_general_hook_invocation,
    mark_task_ready, mark_task_ready_with_confidence, mark_task_ready_with_evidence,
    render_hook_decision, start_task, start_task_with_contract, start_task_with_profile,
    task_state, update_task_todos, CommandConfig, ForgeGuardConfig, GoalContract, GuardMode,
    HookAgent, HookDecision, TaskProfile, TaskStatus,
};
use tempfile::tempdir;

#[test]
fn global_hooks_defer_to_an_initialized_repository() {
    let directory = tempdir().expect("temp directory");
    let input = format!(r#"{{"cwd":"{}"}}"#, directory.path().display());

    assert!(is_general_hook_invocation(directory.path(), &input).expect("general invocation"));

    ForgeGuardConfig::new("sample", Vec::new())
        .save(directory.path())
        .expect("save project config");

    assert!(!is_general_hook_invocation(directory.path(), &input).expect("code invocation"));
}

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
    // Later agent turns, not duplicate hook entries: step past the replay window.
    sleep_past_duplicate_window();
    let (decision, cache_hit) =
        evaluate_stop_hook(directory.path(), &repeated).expect("evaluate repeated hook");
    let HookDecision::Block(message) = decision else {
        panic!("the second unchanged turn is still inside the retry budget");
    };
    assert!(message.contains("no-progress 1/2"));
    assert!(cache_hit);

    sleep_past_duplicate_window();
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
    assert_eq!(
        render_hook_decision(HookAgent::OpenClaw, &HookDecision::Pass),
        ""
    );

    let reason = "Fix failing tests".to_owned();
    let claude = render_hook_decision(HookAgent::Claude, &HookDecision::Block(reason.clone()));
    let cursor = render_hook_decision(HookAgent::Cursor, &HookDecision::Block(reason.clone()));
    let codex = render_hook_decision(HookAgent::Codex, &HookDecision::Block(reason));
    let antigravity = render_hook_decision(
        HookAgent::Antigravity,
        &HookDecision::Block("Fix failing tests".to_owned()),
    );
    let openclaw = render_hook_decision(
        HookAgent::OpenClaw,
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
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&openclaw).expect("OpenClaw JSON")["action"],
        "revise"
    );

    let codex_stop = render_hook_decision(
        HookAgent::Codex,
        &HookDecision::Stop("No progress".to_owned()),
    );
    let codex_stop: serde_json::Value = serde_json::from_str(&codex_stop).expect("Codex stop JSON");
    assert_eq!(codex_stop["continue"], false);
    assert_eq!(codex_stop["stopReason"], "No progress");
    let openclaw_stop = render_hook_decision(
        HookAgent::OpenClaw,
        &HookDecision::Stop("No progress".to_owned()),
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&openclaw_stop).expect("OpenClaw stop JSON")
            ["action"],
        "finalize"
    );
}

#[test]
fn camel_case_session_id_is_shared_by_context_and_stop_hooks() {
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
    let session = "1f1646e8-1234-4567-89ab-0123456789ab";
    start_task(
        directory.path(),
        session,
        "Verify the service value",
        &["service.ts".to_owned()],
        false,
    )
    .expect("start task");
    let input = format!(
        r#"{{"cwd":"{}","sessionId":"{session}"}}"#,
        directory.path().display()
    );

    let context = evaluate_context_hook(directory.path(), &input, HookAgent::Codex)
        .expect("evaluate context hook")
        .expect("task context");
    assert!(context.contains("Verify the service value"));
    mark_task_ready(directory.path(), session, &["service verified".to_owned()])
        .expect("mark task ready");
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("evaluate stop hook");
    assert_eq!(decision, HookDecision::Pass);
    assert_eq!(
        task_state(directory.path(), session)
            .expect("read task")
            .expect("task")
            .status,
        TaskStatus::Completed
    );
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
        assert!(message.contains("repository/tool evidence"));
        assert!(!message.contains("work/tool evidence"));
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
    ForgeGuardConfig::new("sample", Vec::new())
        .save(directory.path())
        .expect("save config");
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
        r#"{{"cwd":"{}","session_id":"session-a","tool_name":"apply_patch","tool_input":{{"file_path":"src/api.rs"}}}}"#,
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
fn general_profiles_require_acceptance_and_provenance() {
    let directory = tempdir().expect("temp directory");
    let goal = GoalContract {
        metric: Some("approved requirements".to_owned()),
        baseline: Some("0 approved".to_owned()),
        target: Some("2 approved".to_owned()),
        guardrails: vec!["no unsupported claims".to_owned()],
        verifications: vec!["stakeholder review".to_owned()],
    };
    assert!(start_task_with_profile(
        directory.path(),
        "product-missing-acceptance",
        "Design checkout requirements",
        &[],
        &[],
        false,
        goal.clone(),
        &[],
        TaskProfile::new("product").expect("product profile"),
        &[],
    )
    .is_err());

    start_task_with_profile(
        directory.path(),
        "product-session",
        "Design checkout requirements",
        &[],
        &["mcp:productboard".to_owned()],
        false,
        goal,
        &["Review customer evidence".to_owned()],
        TaskProfile::new("product").expect("product profile"),
        &[
            "Primary user and problem are explicit".to_owned(),
            "Every requirement is testable".to_owned(),
        ],
    )
    .expect("start product task");
    update_task_todos(directory.path(), "product-session", &[], &[1])
        .expect("complete product todo");

    let missing_source = mark_task_ready_with_evidence(
        directory.path(),
        "product-session",
        &["requirements reviewed".to_owned()],
        Some(90),
        None,
        &[],
        &[1, 2],
    )
    .expect_err("role evidence must declare provenance");
    assert!(missing_source.to_string().contains("evidence source"));

    let missing_criterion = mark_task_ready_with_evidence(
        directory.path(),
        "product-session",
        &["customer notes reviewed".to_owned()],
        Some(90),
        Some("mcp:productboard"),
        &["artifact:product-brief".to_owned()],
        &[1],
    )
    .expect_err("all acceptance criteria need evidence");
    assert!(missing_criterion.to_string().contains("criterion #2"));

    let task = mark_task_ready_with_evidence(
        directory.path(),
        "product-session",
        &["customer notes and requirements reviewed".to_owned()],
        Some(90),
        Some("mcp:productboard"),
        &["artifact:product-brief".to_owned()],
        &[1, 2],
    )
    .expect("record structured product evidence");
    assert_eq!(task.profile.as_str(), "product");
    assert_eq!(task.evidence_records[0].source, "mcp:productboard");
    assert_eq!(task.evidence_records[0].criteria, vec![1, 2]);
}

#[test]
fn general_profiles_restore_role_review_and_warn_on_resource_drift() {
    let directory = tempdir().expect("temp directory");
    start_task_with_profile(
        directory.path(),
        "dba-session",
        "Review the production query plan without mutations",
        &[],
        &[
            "mcp:postgres/read-only".to_owned(),
            "database:production/analytics".to_owned(),
        ],
        false,
        GoalContract {
            metric: Some("reviewed query plans".to_owned()),
            baseline: Some("0 reviewed".to_owned()),
            target: Some("1 reviewed".to_owned()),
            guardrails: vec!["no production writes".to_owned()],
            verifications: vec!["EXPLAIN plan captured".to_owned()],
        },
        &["Inspect the query plan".to_owned()],
        TaskProfile::new("database").expect("database profile"),
        &["Query plan and rollback risk are documented".to_owned()],
    )
    .expect("start DBA task");
    let context_input = format!(
        r#"{{"cwd":"{}","session_id":"dba-session"}}"#,
        directory.path().display()
    );
    let context = evaluate_context_hook(directory.path(), &context_input, HookAgent::Codex)
        .expect("evaluate context")
        .expect("DBA context");
    assert!(context.contains("profile: database"));
    assert!(context.contains("acceptance criteria: 0/1 evidenced"));

    let outside = format!(
        r#"{{"cwd":"{}","session_id":"dba-session","tool_name":"mcp__postgres__execute_sql","tool_input":{{"database":"staging"}}}}"#,
        directory.path().display()
    );
    let warning = evaluate_scope_hook(directory.path(), &outside)
        .expect("evaluate resource scope")
        .expect("resource warning");
    assert!(warning.contains("mcp:postgres/execute_sql"));
    assert!(warning.contains("database:staging"));

    update_task_todos(directory.path(), "dba-session", &[], &[1]).expect("complete DBA todo");
    mark_task_ready_with_evidence(
        directory.path(),
        "dba-session",
        &["EXPLAIN plan captured; no statement executed".to_owned()],
        Some(95),
        Some("mcp:postgres/read-only"),
        &["artifact:explain-plan.json".to_owned()],
        &[1],
    )
    .expect("mark DBA task ready");
    let (decision, _) =
        evaluate_stop_hook(directory.path(), &context_input).expect("evaluate DBA review phase");
    let HookDecision::Block(message) = decision else {
        panic!("DBA profile must run a role review phase");
    };
    assert!(message.contains("environment, authorization"));
}

#[test]
fn task_state_v1_defaults_new_general_guard_fields() {
    let directory = tempdir().expect("temp directory");
    let task_directory = directory.path().join(".forgeguard/cache/tasks");
    fs::create_dir_all(&task_directory).expect("create task directory");
    fs::write(
        task_directory.join("legacy.json"),
        r#"{
          "version": 1,
          "session_id": "legacy",
          "objective": "Legacy objective",
          "scopes": [],
          "semantic": false,
          "goal": {},
          "todos": [],
          "confidence": null,
          "confidence_history": [],
          "auto_pokes": 0,
          "poke_instruction": null,
          "status": "active",
          "evidence": [],
          "blocker": null
        }"#,
    )
    .expect("write v1 task");

    let task = task_state(directory.path(), "legacy")
        .expect("read legacy task")
        .expect("legacy task");
    assert_eq!(task.profile.as_str(), "general");
    assert!(task.resources.is_empty());
    assert!(task.acceptance_criteria.is_empty());
    assert!(task.evidence_records.is_empty());
}

#[test]
fn custom_profiles_use_general_review_without_compiled_role_changes() {
    let directory = tempdir().expect("temp directory");
    start_task_with_profile(
        directory.path(),
        "custom-session",
        "Review a legal operations checklist",
        &[],
        &[],
        false,
        GoalContract {
            metric: Some("reviewed checklist items".to_owned()),
            baseline: Some("0 reviewed".to_owned()),
            target: Some("1 reviewed".to_owned()),
            guardrails: vec!["no unsupported legal claims".to_owned()],
            verifications: vec!["qualified reviewer sign-off".to_owned()],
        },
        &["Review the checklist".to_owned()],
        TaskProfile::new("legal-operations").expect("custom profile"),
        &["Every checklist item has a source".to_owned()],
    )
    .expect("start custom task");
    update_task_todos(directory.path(), "custom-session", &[], &[1]).expect("complete custom todo");
    mark_task_ready_with_evidence(
        directory.path(),
        "custom-session",
        &["reviewer checked every item".to_owned()],
        Some(90),
        Some("human:qualified-reviewer"),
        &[],
        &[1],
    )
    .expect("mark custom task ready");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"custom-session"}}"#,
        directory.path().display()
    );
    let (decision, _) =
        evaluate_stop_hook(directory.path(), &input).expect("evaluate custom profile review");
    let HookDecision::Block(message) = decision else {
        panic!("custom profile must receive a general review phase");
    };
    assert!(message.contains("reinspect the objective"));
}

#[test]
fn agent_context_requires_questions_only_for_material_ambiguity() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    ForgeGuardConfig::new("sample", Vec::new())
        .save(directory.path())
        .expect("save config");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"clarify","hook_event_name":"UserPromptSubmit"}}"#,
        directory.path().display()
    );

    for (agent, expected) in [
        (HookAgent::Claude, "call `AskUserQuestion`"),
        (HookAgent::Codex, "use `request_user_input` when available"),
        (HookAgent::Cursor, "native structured user-input tool"),
        (HookAgent::Antigravity, "native structured user-input tool"),
    ] {
        let context = evaluate_context_hook(directory.path(), &input, agent)
            .expect("evaluate agent context")
            .expect("agent context");
        assert!(context.contains(expected));
        assert!(context.contains("safest reversible default"));
    }
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
fn uninitialized_repository_uses_only_general_guard() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    fs::write(directory.path().join(".gitignore"), "node_modules/\n").expect("write gitignore");
    fs::write(
        directory.path().join("service.ts"),
        "import { PrismaClient } from '@prisma/client';\nconst db = new PrismaClient();\nfor (const user of users) { await db.query('SELECT id FROM users WHERE id = 1'); }\n",
    )
    .expect("write source");
    start_task(
        directory.path(),
        "session-uninitialized",
        "Write documentation only",
        &["docs".to_owned()],
        false,
    )
    .expect("write stale task");

    let input = format!(
        r#"{{"cwd":"{}","session_id":"session-uninitialized","executionNum":1,"tool_input":{{"file_path":"service.ts"}}}}"#,
        directory.path().display()
    );
    assert!(
        evaluate_context_hook(directory.path(), &input, HookAgent::Codex)
            .expect("evaluate context hook")
            .expect("general context")
            .contains("General Guard is active")
    );
    assert!(evaluate_scope_hook(directory.path(), &input)
        .expect("evaluate scope hook")
        .expect("general scope warning")
        .contains("service.ts"));
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("evaluate hook");
    assert!(matches!(decision, HookDecision::Block(_)));

    // General Guard stores task state but never opts into repository policy.
    assert!(!directory.path().join(".forgeguard/config.toml").exists());
    assert!(!directory
        .path()
        .join(".forgeguard/reports/latest.json")
        .exists());
    let root_ignore =
        fs::read_to_string(directory.path().join(".gitignore")).expect("read root gitignore");
    assert_eq!(root_ignore, "node_modules/\n");
}

#[test]
fn initialized_repository_activates_every_hook() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = GuardMode::Strict;
    config.focus.auto_poke = false;
    config.save(directory.path()).expect("save config");
    fs::write(
        directory.path().join("service.ts"),
        "import { PrismaClient } from '@prisma/client';\nconst db = new PrismaClient();\nfor (const user of users) { await db.query('SELECT id FROM users WHERE id = 1'); }\n",
    )
    .expect("write source");
    start_task(
        directory.path(),
        "session-initialized",
        "Change only source",
        &["src".to_owned()],
        false,
    )
    .expect("start task");

    let input = format!(
        r#"{{"cwd":"{}","session_id":"session-initialized","tool_input":{{"file_path":"service.ts"}}}}"#,
        directory.path().display()
    );
    assert!(
        evaluate_context_hook(directory.path(), &input, HookAgent::Codex)
            .expect("evaluate context hook")
            .expect("context")
            .contains("inspect → design → implement → test → review → verify")
    );
    assert!(evaluate_scope_hook(directory.path(), &input)
        .expect("evaluate scope hook")
        .expect("scope warning")
        .contains("service.ts"));
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("evaluate hook");
    assert!(matches!(decision, HookDecision::Block(_)));
    assert!(directory
        .path()
        .join(".forgeguard/reports/latest.json")
        .is_file());
}

#[test]
fn stop_hook_blocks_only_new_production_placeholders() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.focus.enabled = false;
    config.save(directory.path()).expect("save config");
    fs::write(
        directory.path().join("service.rs"),
        "fn value() -> u8 { 1 }\n",
    )
    .expect("write source");
    git_commit_all(directory.path());

    fs::write(
        directory.path().join("service.rs"),
        "fn value() -> u8 { todo!() }\n",
    )
    .expect("add placeholder");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"new-placeholder"}}"#,
        directory.path().display()
    );
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("evaluate hook");
    let HookDecision::Block(message) = decision else {
        panic!("new production placeholder must block completion");
    };
    assert!(message.contains("anti-slop hook blocked completion"));
    assert!(message.contains("service.rs:1"));

    fs::write(
        directory.path().join("service.rs"),
        "fn value() -> u8 { 2 }\n",
    )
    .expect("complete implementation");
    let fixed = format!(
        r#"{{"cwd":"{}","session_id":"fixed-placeholder"}}"#,
        directory.path().display()
    );
    assert_eq!(
        evaluate_stop_hook(directory.path(), &fixed)
            .expect("evaluate fixed hook")
            .0,
        HookDecision::Pass
    );

    fs::write(
        directory.path().join("service.rs"),
        "fn value() -> u8 { panic!(\"TODO\") }\n",
    )
    .expect("add panic placeholder");
    let panic_placeholder = format!(
        r#"{{"cwd":"{}","session_id":"panic-placeholder"}}"#,
        directory.path().display()
    );
    assert!(matches!(
        evaluate_stop_hook(directory.path(), &panic_placeholder)
            .expect("evaluate panic hook")
            .0,
        HookDecision::Block(_)
    ));
    fs::write(
        directory.path().join("service.rs"),
        "fn value() -> u8 { 2 }\n",
    )
    .expect("remove panic placeholder");
    fs::write(
        directory.path().join("service.ts"),
        "export function pending() { throw new Error('TODO'); }\n",
    )
    .expect("add TypeScript placeholder");
    let typescript = format!(
        r#"{{"cwd":"{}","session_id":"typescript-placeholder"}}"#,
        directory.path().display()
    );
    assert!(matches!(
        evaluate_stop_hook(directory.path(), &typescript)
            .expect("evaluate TypeScript hook")
            .0,
        HookDecision::Block(_)
    ));
    fs::write(
        directory.path().join("service.ts"),
        "// throw new Error('TODO');\nexport const ready = 1;\n",
    )
    .expect("replace placeholder with comment");
    let comment = format!(
        r#"{{"cwd":"{}","session_id":"placeholder-comment"}}"#,
        directory.path().display()
    );
    assert_eq!(
        evaluate_stop_hook(directory.path(), &comment)
            .expect("evaluate comment hook")
            .0,
        HookDecision::Pass
    );

    fs::write(
        directory.path().join("service.rs"),
        "fn value() -> u8 { todo!(\"old\") }\n",
    )
    .expect("write committed placeholder");
    git_commit_all(directory.path());
    fs::write(
        directory.path().join("service.rs"),
        "fn value() -> u8 { todo!(\"new\") }\n",
    )
    .expect("replace committed placeholder");
    let replaced = format!(
        r#"{{"cwd":"{}","session_id":"replaced-placeholder"}}"#,
        directory.path().display()
    );
    assert!(matches!(
        evaluate_stop_hook(directory.path(), &replaced)
            .expect("evaluate replaced hook")
            .0,
        HookDecision::Block(_)
    ));
    fs::write(
        directory.path().join("service.rs"),
        "fn value() -> u8 { todo!( \"old\" ) }\nfn ready() -> u8 { 3 }\n",
    )
    .expect("change beside committed placeholder");
    fs::create_dir(directory.path().join("tests")).expect("create tests");
    fs::write(
        directory.path().join("tests/pending_test.rs"),
        "fn fixture() { unimplemented!() }\n",
    )
    .expect("write test placeholder");
    let existing = format!(
        r#"{{"cwd":"{}","session_id":"existing-placeholder"}}"#,
        directory.path().display()
    );
    assert_eq!(
        evaluate_stop_hook(directory.path(), &existing)
            .expect("evaluate existing hook")
            .0,
        HookDecision::Pass
    );
}

#[test]
fn documentation_only_change_skips_configured_commands() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let failing = CommandConfig {
        name: "test".to_owned(),
        command: "exit 1".to_owned(),
        required: true,
        enabled: true,
        timeout_seconds: 10,
    };
    let mut config = ForgeGuardConfig::new("sample", vec![failing]);
    config.focus.enabled = false;
    config.save(directory.path()).expect("save config");
    fs::write(
        directory.path().join("service.ts"),
        "export const value = 1;\n",
    )
    .expect("write source");
    git_commit_all(directory.path());
    fs::write(directory.path().join("guide.md"), "# guide\n").expect("write documentation");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"session-docs"}}"#,
        directory.path().display()
    );

    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("documentation stop");
    assert_eq!(decision, HookDecision::Pass);

    // A removed source file changes build, lint, and test results even though
    // the path no longer exists on disk.
    fs::remove_file(directory.path().join("service.ts")).expect("remove source");
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("removal stop");
    let HookDecision::Block(message) = decision else {
        panic!("a removed source file must run the configured commands");
    };
    assert!(message.contains("test failed"));

    fs::write(
        directory.path().join("service.ts"),
        "export const value = 2;\n",
    )
    .expect("rewrite source");
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("source stop");
    let HookDecision::Block(message) = decision else {
        panic!("a source change must run the configured commands");
    };
    assert!(message.contains("test failed"));
}

#[test]
fn markdown_under_a_test_tree_is_a_fixture_and_still_runs_commands() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let failing = CommandConfig {
        name: "test".to_owned(),
        command: "exit 1".to_owned(),
        required: true,
        enabled: true,
        timeout_seconds: 10,
    };
    let mut config = ForgeGuardConfig::new("sample", vec![failing]);
    config.focus.enabled = false;
    config.save(directory.path()).expect("save config");
    fs::create_dir_all(directory.path().join("tests/fixtures")).expect("create fixtures");
    fs::write(
        directory.path().join("tests/fixtures/expected.md"),
        "# expected\n",
    )
    .expect("write fixture");
    git_commit_all(directory.path());
    fs::write(
        directory.path().join("tests/fixtures/expected.md"),
        "# expected output\n",
    )
    .expect("edit fixture");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"session-fixture"}}"#,
        directory.path().display()
    );

    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("fixture stop");
    let HookDecision::Block(message) = decision else {
        panic!("a fixture change must run the configured commands");
    };
    assert!(message.contains("test failed"));
}

#[test]
fn duplicate_hook_registration_does_not_consume_the_retry_budget() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = GuardMode::Strict;
    config.focus.auto_poke = false;
    config.save(directory.path()).expect("save config");
    fs::write(
        directory.path().join("service.ts"),
        "export const value = 1;\n",
    )
    .expect("write source");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"session-duplicate"}}"#,
        directory.path().display()
    );

    // A global and a project hook entry fire the same event back to back.
    let (first, _) = evaluate_stop_hook(directory.path(), &input).expect("first registration");
    let (second, cache_hit) =
        evaluate_stop_hook(directory.path(), &input).expect("duplicate registration");
    assert_eq!(first, second);
    assert!(cache_hit);
    assert!(matches!(second, HookDecision::Block(message) if message.contains("attempt 1/3")));
}

#[test]
fn retry_budget_resets_when_the_worktree_advances() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.mode = GuardMode::Strict;
    config.focus.auto_poke = false;
    config.save(directory.path()).expect("save config");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"session-progress"}}"#,
        directory.path().display()
    );

    for index in 0..5 {
        fs::write(
            directory.path().join(format!("service-{index}.ts")),
            "export const value = 1;\n",
        )
        .expect("write source");
        start_task(
            directory.path(),
            "session-progress",
            &format!("Ship increment {index}"),
            &[],
            false,
        )
        .expect("start task");
        let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("progress stop");
        let HookDecision::Block(message) = decision else {
            panic!("a session that advances every turn must not be stopped");
        };
        assert!(message.contains("attempt 1/3"));
        assert!(message.contains("no-progress 0/2"));
    }
}

#[test]
fn a_new_objective_after_a_blocked_task_starts_with_a_fresh_poke_budget() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    let mut config = ForgeGuardConfig::new("sample", Vec::new());
    config.focus.auto_poke = true;
    config.focus.max_auto_pokes = 1;
    config.save(directory.path()).expect("save config");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"session-recover"}}"#,
        directory.path().display()
    );

    start_task(
        directory.path(),
        "session-recover",
        "Improve latency",
        &[],
        false,
    )
    .expect("start abstract task");
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("first poke");
    assert!(matches!(decision, HookDecision::Block(_)));
    fs::write(
        directory.path().join("service.ts"),
        "export const value = 1;\n",
    )
    .expect("write source");
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("exhausted poke");
    assert!(matches!(decision, HookDecision::Stop(_)));
    assert_eq!(
        task_state(directory.path(), "session-recover")
            .expect("read task")
            .expect("task")
            .status,
        TaskStatus::Blocked
    );

    start_task(
        directory.path(),
        "session-recover",
        "Document the release process",
        &[],
        false,
    )
    .expect("start new objective");
    let task = task_state(directory.path(), "session-recover")
        .expect("read task")
        .expect("task");
    assert_eq!(task.auto_pokes, 0);
    assert_eq!(task.status, TaskStatus::Active);
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
fn global_hooks_supervise_uninitialized_non_code_work() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    fs::write(
        directory.path().join(".mcp.json"),
        r#"{"mcpServers":{"database":{"command":"database-proxy"}}}"#,
    )
    .expect("write MCP config");
    start_task(
        directory.path(),
        "mcp-session",
        "Inspect database data through MCP",
        &["data".to_owned()],
        false,
    )
    .expect("write stale task");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"mcp-session","tool_input":{{"file_path":"outside.txt"}}}}"#,
        directory.path().display()
    );

    assert!(
        evaluate_context_hook(directory.path(), &input, HookAgent::Codex)
            .expect("evaluate context hook")
            .expect("general context")
            .contains("General Guard is active")
    );
    let (decision, cache_hit) =
        evaluate_stop_hook(directory.path(), &input).expect("evaluate stop hook");
    assert!(matches!(decision, HookDecision::Block(_)));
    assert!(!cache_hit);
    let (_, cache_hit) =
        evaluate_stop_hook(directory.path(), &input).expect("evaluate duplicate stop hook");
    assert!(cache_hit);
    assert_eq!(
        task_state(directory.path(), "mcp-session")
            .expect("read task")
            .expect("task")
            .auto_pokes,
        1
    );
    assert!(evaluate_scope_hook(directory.path(), &input)
        .expect("evaluate scope hook")
        .is_some());
    assert!(!directory
        .path()
        .join(".forgeguard/reports/latest.json")
        .exists());
}

#[test]
fn general_guard_works_outside_a_git_repository() {
    let directory = tempdir().expect("temp directory");
    start_task(
        directory.path(),
        "general-session",
        "Review a marketing brief",
        &[],
        false,
    )
    .expect("start general task");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"general-session"}}"#,
        directory.path().display()
    );

    let context = evaluate_context_hook(directory.path(), &input, HookAgent::Codex)
        .expect("evaluate context hook")
        .expect("general context");
    assert!(context.contains("General Guard is active"));
    assert!(context.contains("work scope not constrained"));
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("evaluate stop hook");
    let HookDecision::Block(message) = decision else {
        panic!("general task must continue");
    };
    assert!(message.contains("work/tool evidence"));
    assert!(!message.contains("repository/tool evidence"));
    assert!(!directory.path().join(".forgeguard/config.toml").exists());
    assert!(!directory
        .path()
        .join(".forgeguard/reports/latest.json")
        .exists());
}

#[test]
fn general_guard_uses_non_code_review_phases() {
    let directory = tempdir().expect("temp directory");
    start_task_with_contract(
        directory.path(),
        "general-review",
        "Review a marketing brief",
        &[],
        false,
        GoalContract {
            metric: Some("approved claims".to_owned()),
            baseline: Some("0".to_owned()),
            target: Some("3".to_owned()),
            guardrails: vec!["no unsupported claims".to_owned()],
            verifications: vec!["source notes checked".to_owned()],
        },
        &["Review every claim".to_owned()],
    )
    .expect("start general task");
    update_task_todos(directory.path(), "general-review", &[], &[1]).expect("complete todo");
    mark_task_ready_with_confidence(
        directory.path(),
        "general-review",
        &["source notes: 3 claims checked".to_owned()],
        Some(90),
    )
    .expect("mark ready");
    let input = format!(
        r#"{{"cwd":"{}","session_id":"general-review"}}"#,
        directory.path().display()
    );
    let (first, _) = evaluate_stop_hook(directory.path(), &input).expect("first review phase");
    assert!(matches!(first, HookDecision::Block(_)));

    mark_task_ready_with_confidence(
        directory.path(),
        "general-review",
        &["source notes: 3 claims rechecked".to_owned()],
        Some(90),
    )
    .expect("mark ready again");
    let (second, _) = evaluate_stop_hook(directory.path(), &input).expect("second review phase");
    let HookDecision::Block(message) = second else {
        panic!("general review must continue");
    };
    assert!(message.contains("verify the deliverable"));
    assert!(!message.contains("tests"));
    assert!(!message.contains("full diff"));
    assert!(!message.contains("repository"));
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

fn git_commit_all(root: &std::path::Path) {
    for arguments in [
        vec!["add", "-A"],
        vec![
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=test",
            "commit",
            "-qm",
            "init",
        ],
    ] {
        let status = Command::new("git")
            .args(arguments)
            .current_dir(root)
            .status()
            .expect("run git");
        assert!(status.success());
    }
}

fn sleep_past_duplicate_window() {
    std::thread::sleep(std::time::Duration::from_millis(1_600));
}

fn git_init(root: &std::path::Path) {
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root)
        .status()
        .expect("run git init");
    assert!(status.success());
}
