use std::{
    fs::{self, File},
    hash::Hasher,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{bail, Context, Result};

pub fn repository_roots(root: &Path) -> Result<Vec<PathBuf>> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to resolve workspace root {}", root.display()))?;
    if root.join(".git").exists() {
        return Ok(vec![root]);
    }

    let mut pending = vec![root];
    let mut repositories = Vec::new();
    // ponytail: scan directories once until Git roots; add configurable bounds only if discovery becomes measurable.
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        // forgeguard: allow FG-ALG-001 -- each directory entry is visited once; repository trees are not traversed
        for entry in entries.flatten() {
            let path = entry.path();
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            if path.join(".git").exists() {
                repositories.push(path);
                continue;
            }
            if matches!(
                entry.file_name().to_str(),
                Some(
                    ".git"
                        | ".forgeguard"
                        | "target"
                        | "node_modules"
                        | "vendor"
                        | ".venv"
                        | "venv"
                        | "dist"
                        | "build"
                        | ".next"
                )
            ) {
                continue;
            }
            pending.push(path);
        }
    }
    repositories.sort();
    Ok(repositories)
}

pub fn changed_files(root: &Path) -> Result<Vec<std::path::PathBuf>> {
    let output = status_output(root)?;
    changed_paths(root, &output.stdout)
}

pub fn worktree_fingerprint(root: &Path) -> Result<Option<String>> {
    let output = status_output(root)?;
    if output.stdout.is_empty() {
        return Ok(None);
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(&output.stdout);
    for path in changed_paths(root, &output.stdout)? {
        hasher.write(path.to_string_lossy().as_bytes());
        let full_path = root.join(path);
        let Ok(mut file) = File::open(full_path) else {
            continue;
        };
        let mut buffer = [0_u8; 16 * 1024];
        // forgeguard: allow FG-ALG-001 -- streaming chunks; O(total changed bytes), O(1) memory
        loop {
            let count = file
                .read(&mut buffer)
                .context("failed to hash changed file")?;
            if count == 0 {
                break;
            }
            hasher.write(&buffer[..count]);
        }
    }
    Ok(Some(format!("{:016x}", hasher.finish())))
}

fn status_output(root: &Path) -> Result<Output> {
    let output = Command::new("git")
        .args([
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
            ":(exclude).forgeguard/cache",
            ":(exclude).forgeguard/reports",
        ])
        .current_dir(root)
        .output()
        .context("failed to execute git status")?;
    if !output.status.success() {
        bail!(
            "git status failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output)
}

/// Every path reported by `git status` paired with the subset that still exists.
/// Scanning needs readable files, but a deleted source file still changes build,
/// lint, and test results, so both views come from one `git status` call.
pub fn changed_paths_partitioned(
    root: &Path,
) -> Result<(Vec<std::path::PathBuf>, Vec<std::path::PathBuf>)> {
    let all = status_paths(&status_output(root)?.stdout);
    let existing = all
        .iter()
        .filter(|path| root.join(path).is_file())
        .cloned()
        .collect();
    Ok((all, existing))
}

fn changed_paths(root: &Path, output: &[u8]) -> Result<Vec<std::path::PathBuf>> {
    let mut paths = status_paths(output);
    paths.retain(|path| root.join(path).is_file());
    Ok(paths)
}

fn status_paths(output: &[u8]) -> Vec<std::path::PathBuf> {
    let mut records = output.split(|byte| *byte == 0).peekable();
    let mut paths = Vec::new();
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let status = &record[..2];
        paths.push(path_from_git_bytes(&record[3..]));

        if status.iter().any(|value| matches!(*value, b'R' | b'C')) {
            let _ = records.next();
        }
    }

    paths.sort();
    paths.dedup();
    paths
}

fn path_from_git_bytes(value: &[u8]) -> std::path::PathBuf {
    std::path::PathBuf::from(String::from_utf8_lossy(value).into_owned())
}
