use std::{
    fs,
    io::Write,
    process::{self, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

fn temporary_directory(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "forgeguard-cli-{label}-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should follow the Unix epoch")
            .as_nanos()
    ))
}

#[test]
fn json_init_reports_mcp_registration_failure_on_stderr() {
    let root = temporary_directory("invalid-mcp");
    fs::create_dir_all(&root).expect("temporary project should be created");
    fs::write(root.join(".mcp.json"), "not json").expect("invalid MCP config should be created");

    let output = Command::new(env!("CARGO_BIN_EXE_forgeguard"))
        .args([
            "--root",
            root.to_str().expect("temporary path should be UTF-8"),
            "init",
            "--agent",
            "claude",
            "--mcp",
            "--no-index",
            "--json",
        ])
        .output()
        .expect("init command should run");

    assert!(output.status.success());
    serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("init stdout should remain valid JSON");
    assert!(String::from_utf8_lossy(&output.stderr).contains("MCP registration skipped"));
    fs::remove_dir_all(root).expect("temporary project should be removed");
}

#[test]
fn opencode_stop_hook_accepts_agent_and_stays_silent_on_pass() {
    let root = temporary_directory("opencode-hook");
    fs::create_dir_all(&root).expect("temporary project should be created");
    let mut child = Command::new(env!("CARGO_BIN_EXE_forgeguard"))
        .args([
            "--root",
            root.to_str().expect("temporary path should be UTF-8"),
            "hook",
            "stop",
            "--agent",
            "opencode",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("OpenCode hook command should start");
    child
        .stdin
        .take()
        .expect("hook stdin should be available")
        .write_all(
            format!(
                "{{\"cwd\":\"{}\",\"sessionId\":\"opencode-session\"}}",
                root.display()
            )
            .as_bytes(),
        )
        .expect("hook input should be written");
    let output = child
        .wait_with_output()
        .expect("hook command should finish");

    assert!(output.status.success());
    assert!(output.stdout.is_empty(), "a passing gate must stay silent");
    fs::remove_dir_all(root).expect("temporary project should be removed");
}
