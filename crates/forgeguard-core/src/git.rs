use std::{path::Path, process::Command};

use anyhow::{Context, Result};

pub fn changed_files(root: &Path) -> Result<Vec<std::path::PathBuf>> {
    let output = Command::new("git")
        .args([
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ])
        .current_dir(root)
        .output()
        .context("failed to execute git status")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let mut records = output.stdout.split(|byte| *byte == 0).peekable();
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
