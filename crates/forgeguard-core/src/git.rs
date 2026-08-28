use std::{
    collections::BTreeMap,
    fs::{self, File},
    hash::Hasher,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangedScope {
    pub paths: Vec<PathBuf>,
    pub lines: BTreeMap<PathBuf, Vec<(usize, usize)>>,
}

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

/// Return readable files changed from `base` (or `HEAD`) and the added/edited
/// line ranges in their current contents. Untracked files are wholly in scope.
pub fn changed_scope(root: &Path, base: Option<&str>) -> Result<ChangedScope> {
    let comparison = match base {
        Some(base) => merge_base(root, base)?,
        None => "HEAD".to_owned(),
    };
    let comparison_exists = revision_exists(root, &comparison)?;
    let mut paths = if base.is_some() {
        diff_paths(root, &comparison)?
    } else {
        changed_files(root)?
    };
    for path in changed_files(root)? {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths.retain(|path| root.join(path).is_file());
    paths.sort();
    paths.dedup();

    let mut lines = BTreeMap::new();
    // ponytail: one Git diff per changed file; parse one combined patch if large-change latency is measured.
    for path in &paths {
        let ranges = if comparison_exists {
            diff_line_ranges(root, &comparison, path)?
        } else {
            Vec::new()
        };
        lines.insert(
            path.clone(),
            if ranges.is_empty()
                && (!comparison_exists || !exists_at_revision(root, &comparison, path)?)
            {
                vec![(1, usize::MAX)]
            } else {
                ranges
            },
        );
    }
    Ok(ChangedScope { paths, lines })
}

fn merge_base(root: &Path, base: &str) -> Result<String> {
    if !revision_exists(root, base)? {
        bail!("Git base revision does not exist: {base}");
    }
    let output = Command::new("git")
        .args(["merge-base", "HEAD", base])
        .current_dir(root)
        .output()
        .context("failed to execute git merge-base")?;
    if !output.status.success() {
        bail!(
            "Git base revision {base} has no merge base with HEAD: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn revision_exists(root: &Path, revision: &str) -> Result<bool> {
    Ok(Command::new("git")
        .args(["rev-parse", "--verify", &format!("{revision}^{{commit}}")])
        .current_dir(root)
        .output()
        .context("failed to inspect Git revision")?
        .status
        .success())
}

fn diff_paths(root: &Path, comparison: &str) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args([
            "diff",
            "--name-only",
            "-z",
            comparison,
            "--",
            ".",
            ":(exclude).forgeguard/cache",
            ":(exclude).forgeguard/reports",
        ])
        .current_dir(root)
        .output()
        .context("failed to execute git diff")?;
    if !output.status.success() {
        bail!(
            "git diff against {comparison} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(path_from_git_bytes)
        .collect())
}

fn diff_line_ranges(root: &Path, comparison: &str, path: &Path) -> Result<Vec<(usize, usize)>> {
    let output = Command::new("git")
        .args(["diff", "--unified=0", "--no-ext-diff", comparison, "--"])
        .arg(path)
        .current_dir(root)
        .output()
        .context("failed to execute git diff")?;
    if !output.status.success() {
        bail!(
            "git diff against {comparison} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_added_range)
        .collect())
}

fn parse_added_range(line: &str) -> Option<(usize, usize)> {
    let range = line.strip_prefix("@@ ")?.split_whitespace().nth(1)?;
    let (start, count) = range
        .strip_prefix('+')?
        .split_once(',')
        .unwrap_or((range.strip_prefix('+')?, "1"));
    let start: usize = start.parse().ok()?;
    let count: usize = count.parse().ok()?;
    (count > 0).then(|| (start, start.saturating_add(count - 1)))
}

fn exists_at_revision(root: &Path, comparison: &str, path: &Path) -> Result<bool> {
    let path = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    Ok(Command::new("git")
        .args(["cat-file", "-e", &format!("{comparison}:{path}")])
        .current_dir(root)
        .output()
        .context("failed to inspect comparison revision")?
        .status
        .success())
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
