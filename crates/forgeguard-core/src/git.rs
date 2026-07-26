use std::{
    fs::File,
    hash::Hasher,
    io::Read,
    path::Path,
    process::{Command, Output},
};

use anyhow::{bail, Context, Result};

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

fn changed_paths(root: &Path, output: &[u8]) -> Result<Vec<std::path::PathBuf>> {
    let mut records = output.split(|byte| *byte == 0).peekable();
    let mut paths = Vec::new();
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let status = &record[..2];
        let raw_path = &record[3..];
        let path = path_from_git_bytes(raw_path);
        if root.join(&path).is_file() {
            paths.push(path);
        }

        if status.iter().any(|value| matches!(*value, b'R' | b'C')) {
            let _ = records.next();
        }
    }

    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn path_from_git_bytes(value: &[u8]) -> std::path::PathBuf {
    std::path::PathBuf::from(String::from_utf8_lossy(value).into_owned())
}
