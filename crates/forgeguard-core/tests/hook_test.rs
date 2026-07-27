use std::{fs, process::Command};

use forgeguard_core::{
    evaluate_stop_hook, render_hook_decision, ForgeGuardConfig, HookAgent, HookDecision,
};
use tempfile::tempdir;

#[test]
fn blocked_hook_is_compact_cached_and_loop_safe() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    ForgeGuardConfig::new("sample", Vec::new())
        .save(directory.path())
        .expect("save config");
    for index in 0..40 {
        fs::write(
            directory.path().join(format!("repository-{index}.ts")),
            "for (const user of users) { await db.query('SELECT id FROM users'); }\n",
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
    assert_eq!(decision, HookDecision::Pass);
    assert!(cache_hit);

    let antigravity_repeated = format!(
        r#"{{"workspacePaths":["{}"],"executionNum":2}}"#,
        directory.path().display()
    );
    let (decision, cache_hit) = evaluate_stop_hook(directory.path(), &antigravity_repeated)
        .expect("evaluate repeated Antigravity hook");
    assert_eq!(decision, HookDecision::Pass);
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
        "Fix failing tests"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&antigravity).expect("Antigravity JSON")
            ["decision"],
        "continue"
    );
}

#[test]
fn auto_gate_runs_without_config_and_ignores_artifacts() {
    let directory = tempdir().expect("temp directory");
    git_init(directory.path());
    // A pre-existing root .gitignore must be appended to, not clobbered.
    fs::write(directory.path().join(".gitignore"), "node_modules/\n").expect("write gitignore");
    fs::write(
        directory.path().join("service.ts"),
        "for (const user of users) { await db.query('SELECT id FROM users WHERE id = 1'); }\n",
    )
    .expect("write source");

    let input = format!(
        r#"{{"cwd":"{}","executionNum":1}}"#,
        directory.path().display()
    );
    let (decision, _) = evaluate_stop_hook(directory.path(), &input).expect("evaluate hook");
    assert!(matches!(decision, HookDecision::Block(_)));

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

fn git_init(root: &std::path::Path) {
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root)
        .status()
        .expect("run git init");
    assert!(status.success());
}
