use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use crate::{config::CommandConfig, model::CheckResult};

const MAX_OUTPUT_CHARS: usize = 8_000;
const POLL_INTERVAL: Duration = Duration::from_millis(25);
static CAPTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn run_checks(root: &Path, commands: &[CommandConfig]) -> Vec<CheckResult> {
    commands
        .iter()
        .filter(|command| command.enabled)
        .map(|command| run_check(root, command))
        .collect()
}

fn run_check(root: &Path, command_config: &CommandConfig) -> CheckResult {
    let started = Instant::now();
    let capture_path = capture_path();
    let capture = match fs::File::create(&capture_path) {
        Ok(file) => file,
        Err(error) => return failed_check(command_config, started, error.to_string()),
    };
    let stderr = match capture.try_clone() {
        Ok(file) => file,
        Err(error) => {
            let _ = fs::remove_file(&capture_path);
            return failed_check(command_config, started, error.to_string());
        }
    };
    let mut process = shell_command(&command_config.command);
    let child = process
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(capture))
        .stderr(Stdio::from(stderr))
        .spawn();

    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            let _ = fs::remove_file(&capture_path);
            return failed_check(command_config, started, error.to_string());
        }
    };
    let timeout = Duration::from_secs(command_config.timeout_seconds);
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (Some(status), false),
            Ok(None) if started.elapsed() < timeout => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                let _ = child.kill();
                break (child.wait().ok(), true);
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&capture_path);
                return failed_check(command_config, started, error.to_string());
            }
        }
    };
    let mut output = fs::read_to_string(&capture_path).unwrap_or_default();
    let _ = fs::remove_file(&capture_path);
    if timed_out {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&format!(
            "timed out after {} second(s)",
            command_config.timeout_seconds
        ));
    }

    CheckResult {
        name: command_config.name.clone(),
        command: command_config.command.clone(),
        required: command_config.required,
        success: !timed_out && status.is_some_and(|status| status.success()),
        exit_code: status.and_then(|status| status.code()),
        duration_ms: started.elapsed().as_millis(),
        output: truncate_output(&output),
    }
}

fn failed_check(command_config: &CommandConfig, started: Instant, output: String) -> CheckResult {
    CheckResult {
        name: command_config.name.clone(),
        command: command_config.command.clone(),
        required: command_config.required,
        success: false,
        exit_code: None,
        duration_ms: started.elapsed().as_millis(),
        output,
    }
}

fn capture_path() -> PathBuf {
    let sequence = CAPTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "forgeguard-command-{}-{sequence}.log",
        std::process::id()
    ))
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    let mut process = Command::new("cmd");
    process.args(["/C", command]);
    process
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> Command {
    let mut process = Command::new("sh");
    process.args(["-lc", command]);
    process
}

fn truncate_output(output: &str) -> String {
    if output.chars().count() <= MAX_OUTPUT_CHARS {
        return output.trim().to_owned();
    }
    let mut truncated: String = output.chars().take(MAX_OUTPUT_CHARS).collect();
    truncated.push_str("\n… output truncated by ForgeGuard");
    truncated
}
