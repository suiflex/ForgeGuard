use std::{fs, process::Command};

use forgeguard_core::git::changed_files;
use tempfile::tempdir;

#[test]
fn returns_untracked_files_with_spaces() {
    let directory = tempdir().expect("temp directory");
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(directory.path())
        .status()
        .expect("run git init");
    assert!(status.success());
    fs::write(
        directory.path().join("file with spaces.ts"),
        "export const value = 1;\n",
    )
    .expect("write source");

    let paths = changed_files(directory.path()).expect("read changed files");

    assert_eq!(paths, vec![std::path::PathBuf::from("file with spaces.ts")]);
}
