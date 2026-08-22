use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

use crate::{config::CommandConfig, model::CheckResult};

const MAX_OUTPUT_CHARS: usize = 8_000;
const POLL_INTERVAL: Duration = Duration::from_millis(25);
const SUPPLY_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
static CAPTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn run_checks(root: &Path, commands: &[CommandConfig]) -> Vec<CheckResult> {
    run_checks_for_changes(root, commands, None)
}

pub fn run_checks_for_changes(
    root: &Path,
    commands: &[CommandConfig],
    changed_paths: Option<&[PathBuf]>,
) -> Vec<CheckResult> {
    commands
        .iter()
        .filter(|command| command.enabled && command_applies(command, changed_paths))
        .map(|command| {
            if is_supply_chain(command) {
                run_cached_supply_check(root, command)
            } else {
                run_check(root, command)
            }
        })
        .collect()
}

fn command_applies(command: &CommandConfig, changed_paths: Option<&[PathBuf]>) -> bool {
    !is_supply_chain(command)
        || changed_paths.map_or(true, |paths| {
            paths.iter().any(|path| is_dependency_path(path))
        })
}

fn is_supply_chain(command: &CommandConfig) -> bool {
    ["dependency-audit", "supply-chain", "license", "sbom"]
        .iter()
        .any(|prefix| command.name.starts_with(prefix))
}

fn is_dependency_path(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    matches!(
        name,
        "Cargo.toml"
            | "Cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "bun.lock"
            | "bun.lockb"
            | "pyproject.toml"
            | "poetry.lock"
            | "uv.lock"
            | "go.mod"
            | "go.sum"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "gradle.lockfile"
            | "Package.swift"
            | "Package.resolved"
            | "pubspec.yaml"
            | "pubspec.lock"
            | "Gemfile"
            | "Gemfile.lock"
            | "composer.json"
            | "composer.lock"
            | "packages.lock.json"
            | "Directory.Packages.props"
            | "mix.exs"
            | "mix.lock"
            | "rebar.config"
            | "rebar.lock"
            | "deps.edn"
            | "project.clj"
    ) || (name.starts_with("requirements") && name.ends_with(".txt"))
        || name.ends_with(".csproj")
}

#[derive(Serialize, Deserialize)]
struct CachedCheck {
    fingerprint: u64,
    checked_at: u64,
    result: CheckResult,
}

fn run_cached_supply_check(root: &Path, command: &CommandConfig) -> CheckResult {
    let fingerprint = dependency_fingerprint(root, command);
    let path = supply_cache_path(root, command);
    let now = now_seconds();
    if let Ok(source) = fs::read_to_string(&path) {
        if let Ok(mut cached) = serde_json::from_str::<CachedCheck>(&source) {
            if cached.fingerprint == fingerprint
                && now.saturating_sub(cached.checked_at) <= SUPPLY_CACHE_TTL.as_secs()
                && (!command.name.starts_with("sbom") || sbom_artifacts_exist(root, command))
            {
                cached.result.cached = true;
                return cached.result;
            }
        }
    }

    let result = run_check(root, command);
    if result.success {
        let cached = CachedCheck {
            fingerprint,
            checked_at: now,
            result: result.clone(),
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(output) = serde_json::to_vec(&cached) {
            let _ = fs::write(path, output);
        }
    }
    result
}

fn dependency_fingerprint(root: &Path, command: &CommandConfig) -> u64 {
    let mut hasher = DefaultHasher::new();
    command.name.hash(&mut hasher);
    command.command.hash(&mut hasher);
    let mut paths = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let path = entry.into_path();
            is_dependency_path(&path)
                .then(|| path.strip_prefix(root).unwrap_or(&path).to_path_buf())
        })
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        path.hash(&mut hasher);
        if let Ok(bytes) = fs::read(root.join(&path)) {
            bytes.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn supply_cache_path(root: &Path, command: &CommandConfig) -> PathBuf {
    root.join(".forgeguard/cache/checks")
        .join(format!("{}.json", command_slug(command)))
}

fn command_slug(command: &CommandConfig) -> String {
    command
        .name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn sbom_artifact_path(root: &Path, command: &CommandConfig) -> PathBuf {
    root.join(".forgeguard/reports/sbom")
        .join(format!("{}.json", command_slug(command)))
}

fn sbom_artifact_directory(root: &Path, command: &CommandConfig) -> PathBuf {
    root.join(".forgeguard/reports/sbom")
        .join(command_slug(command))
}

fn sbom_artifacts_exist(root: &Path, command: &CommandConfig) -> bool {
    valid_json_file(&sbom_artifact_path(root, command))
        || WalkBuilder::new(sbom_artifact_directory(root, command))
            .hidden(false)
            .git_ignore(false)
            .build()
            .filter_map(|entry| entry.ok())
            .any(|entry| {
                entry.file_type().is_some_and(|kind| kind.is_file())
                    && valid_json_file(entry.path())
            })
}

fn valid_json_file(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .is_some_and(|source| serde_json::from_str::<serde_json::Value>(&source).is_ok())
}

fn persist_supply_output(root: &Path, command: &CommandConfig, output: &str) {
    let path = root
        .join(".forgeguard/reports/supply-chain")
        .join(format!("{}.log", command_slug(command)));
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, output);
}

fn persist_sbom(root: &Path, command: &CommandConfig, output: &str) -> bool {
    let destination = sbom_artifact_path(root, command);
    if let Some(parent) = destination.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut cargo_outputs = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.file_name().and_then(|name| name.to_str()) == Some("forgeguard-sbom.cdx.json")
        })
        .collect::<Vec<_>>();
    cargo_outputs.sort();
    for generated in cargo_outputs {
        if !valid_json_file(&generated) {
            continue;
        }
        let relative = generated.strip_prefix(root).unwrap_or(&generated);
        let relative_parent = relative.parent().unwrap_or_else(|| Path::new(""));
        let target = sbom_artifact_directory(root, command)
            .join(relative_parent)
            .join("bom.json");
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::rename(&generated, &target).is_err() && fs::copy(&generated, &target).is_ok() {
            let _ = fs::remove_file(generated);
        }
    }
    for generated in ["target/bom.json", "build/reports/bom.json"] {
        let generated = root.join(generated);
        if valid_json_file(&generated) && fs::copy(generated, &destination).is_ok() {
            return true;
        }
    }
    let trimmed = output.trim();
    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        let _ = fs::write(destination, trimmed);
    }
    sbom_artifacts_exist(root, command)
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

    let mut result = CheckResult {
        name: command_config.name.clone(),
        command: command_config.command.clone(),
        required: command_config.required,
        success: !timed_out && status.is_some_and(|status| status.success()),
        exit_code: status.and_then(|status| status.code()),
        duration_ms: started.elapsed().as_millis(),
        output: truncate_output(&output),
        cached: false,
    };
    if is_supply_chain(command_config) {
        persist_supply_output(root, command_config, &output);
    }
    if result.success
        && command_config.name.starts_with("sbom")
        && !persist_sbom(root, command_config, &output)
    {
        result.success = false;
        result.output = truncate_output(&format!(
            "{}\nSBOM command succeeded but produced no valid JSON artifact",
            result.output
        ));
    }
    result
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
        cached: false,
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
