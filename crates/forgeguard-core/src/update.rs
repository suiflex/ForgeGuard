//! Optional, non-blocking update notifications.
//!
//! The hot path (the Stop hook) only ever reads a local cache, so it makes no
//! network call and adds no latency. A stale cache is refreshed by a detached
//! background process, and `doctor`/`init` refresh it in the foreground. The
//! latest release is discovered with `git ls-remote` so no HTTP dependency,
//! API token, or rate-limited endpoint is required.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

const REPOSITORY_URL: &str = "https://github.com/suiflex/ForgeGuard";
const CACHE_RELATIVE: &str = ".forgeguard/update-check.json";
#[cfg(windows)]
pub const INSTALL_COMMAND: &str =
    "irm https://raw.githubusercontent.com/suiflex/ForgeGuard/main/install.ps1 | iex";
#[cfg(not(windows))]
pub const INSTALL_COMMAND: &str =
    "curl -fsSL https://raw.githubusercontent.com/suiflex/ForgeGuard/main/install.sh | sh";
const THROTTLE_SECONDS: u64 = 86_400;

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateCache {
    checked_at: u64,
    latest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateCheck {
    pub current: String,
    pub latest: String,
    pub update_available: bool,
}

/// Query whether a newer release exists, checking the remote if the cache is stale
/// or `force` is true.
pub fn check_for_update(home: &Path, force: bool) -> Option<UpdateCheck> {
    check_for_update_for(home, force, current_version())
}

pub fn check_for_update_for(
    home: &Path,
    force: bool,
    current_version: &str,
) -> Option<UpdateCheck> {
    if force || is_stale(home) {
        if let Some(latest) = fetch_latest_version() {
            write_cache(
                home,
                &UpdateCache {
                    checked_at: now_seconds(),
                    latest,
                },
            );
        }
    }
    let cache = read_cache(home)?;
    let latest = cache.latest;
    let update_available = match (parse_version(&latest), parse_version(current_version)) {
        (Some(latest_ver), Some(current_ver)) => latest_ver > current_ver,
        _ => false,
    };
    Some(UpdateCheck {
        current: current_version.to_owned(),
        latest,
        update_available,
    })
}
/// One-line optional notice built from the cached latest version. Never touches
/// the network; safe to call on every hook invocation.
pub fn cached_notice(home: &Path) -> Option<String> {
    cached_notice_for(home, current_version())
}

pub fn cached_notice_for(home: &Path, current_version: &str) -> Option<String> {
    let cache = read_cache(home)?;
    notice(current_version, &cache.latest)
}

/// Refresh the cache when it is stale (or always when `force`), then return the
/// current notice. Performs a network lookup only when a refresh is due. Used by
/// `doctor`, `init`, and the detached `update` command.
pub fn refresh(home: &Path, force: bool) -> Option<String> {
    refresh_for(home, force, current_version())
}

pub fn refresh_for(home: &Path, force: bool, current_version: &str) -> Option<String> {
    let check = check_for_update_for(home, force, current_version)?;
    if check.update_available {
        notice(current_version, &check.latest)
    } else {
        None
    }
}

/// Launch a detached refresh when the cache is missing or older than the
/// throttle window. Fire-and-forget: the caller is never blocked and no output
/// is inherited, so the hook stays fast and silent.
pub fn spawn_refresh_if_stale(home: &Path) {
    if !is_stale(home) {
        return;
    }
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let _ = Command::new(executable)
        .arg("update")
        .arg("--check")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Run the same installer a user would invoke by hand
/// (`curl -fsSL .../install.sh | sh`), with inherited stdio so install output
/// is visible. Used only when a user has explicitly confirmed an `ask`-mode
/// update prompt; never called automatically.
#[cfg(not(windows))]
pub fn run_install_command() -> io::Result<ExitStatus> {
    Command::new("sh")
        .arg("-c")
        .arg(INSTALL_COMMAND)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
}

#[cfg(windows)]
pub fn run_install_command() -> io::Result<ExitStatus> {
    Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            INSTALL_COMMAND,
        ])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
}

fn is_stale(home: &Path) -> bool {
    match read_cache(home) {
        Some(cache) => now_seconds().saturating_sub(cache.checked_at) >= THROTTLE_SECONDS,
        None => true,
    }
}

/// Discover the highest `vX.Y.Z` release tag via `git ls-remote`.
fn fetch_latest_version() -> Option<String> {
    let output = Command::new("git")
        .args(["ls-remote", "--tags", "--refs", REPOSITORY_URL])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.rsplit("refs/tags/").next())
        .filter_map(|tag| parse_version(tag.strip_prefix('v').unwrap_or(tag)))
        .max()
        .map(|(major, minor, patch)| format!("{major}.{minor}.{patch}"))
}

fn notice(current: &str, latest: &str) -> Option<String> {
    if parse_version(latest)? > parse_version(current)? {
        Some(format!(
            "Optional: ForgeGuard {latest} is available (current {current}). Update: {INSTALL_COMMAND}"
        ))
    } else {
        None
    }
}

fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let core = value.trim().split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn cache_path(home: &Path) -> PathBuf {
    home.join(CACHE_RELATIVE)
}

fn read_cache(home: &Path) -> Option<UpdateCache> {
    let text = fs::read_to_string(cache_path(home)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_cache(home: &Path, cache: &UpdateCache) {
    let path = cache_path(home);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(cache) {
        let _ = fs::write(path, text);
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_prefixed_and_prerelease_versions() {
        assert_eq!(parse_version("0.3.0"), Some((0, 3, 0)));
        assert_eq!(parse_version("1.2"), Some((1, 2, 0)));
        assert_eq!(parse_version("2.0.1-rc.1"), Some((2, 0, 1)));
        assert_eq!(parse_version("not-a-version"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
    }

    #[test]
    fn notice_only_appears_for_a_strictly_newer_version() {
        assert!(notice("0.2.0", "0.3.0").is_some());
        assert!(notice("0.2.0", "0.2.0").is_none());
        assert!(notice("0.3.0", "0.2.9").is_none());
    }

    #[test]
    fn check_for_update_detects_newer_version() {
        let home = tempfile::tempdir().expect("temp home");
        write_cache(
            home.path(),
            &UpdateCache {
                checked_at: now_seconds(),
                latest: "1.0.0".to_owned(),
            },
        );
        let check = check_for_update_for(home.path(), false, "0.9.0").expect("check result");
        assert_eq!(check.current, "0.9.0");
        assert_eq!(check.latest, "1.0.0");
        assert!(check.update_available);

        let same = check_for_update_for(home.path(), false, "1.0.0").expect("check result");
        assert!(!same.update_available);

        let older = check_for_update_for(home.path(), false, "1.1.0").expect("check result");
        assert!(!older.update_available);
    }
}
