use std::{
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};

use crate::{config::CommandConfig, model::CheckResult};

const MAX_OUTPUT_CHARS: usize = 8_000;

pub fn run_checks(root: &Path, commands: &[CommandConfig]) -> Vec<CheckResult> {
    commands
        .iter()
        .filter(|command| command.enabled)
        .map(|command| run_check(root, command))
        .collect()
}

fn run_check(root: &Path, command_config: &CommandConfig) -> CheckResult {
    let started = Instant::now();
    let mut process = shell_command(&command_config.command);
    let output = process
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(output) => {
            let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
            if !output.stderr.is_empty() {
                if !combined.is_empty() {
                    combined.push('\n');
                }
                combined.push_str(&String::from_utf8_lossy(&output.stderr));
            }
            CheckResult {
                name: command_config.name.clone(),
                command: command_config.command.clone(),
                required: command_config.required,
                success: output.status.success(),
                exit_code: output.status.code(),
                duration_ms: started.elapsed().as_millis(),
                output: truncate_output(&combined),
            }
        }
        Err(error) => CheckResult {
            name: command_config.name.clone(),
            command: command_config.command.clone(),
            required: command_config.required,
            success: false,
            exit_code: None,
            duration_ms: started.elapsed().as_millis(),
            output: error.to_string(),
        },
    }
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
