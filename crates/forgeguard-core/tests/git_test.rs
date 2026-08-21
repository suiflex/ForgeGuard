use std::{fs, process::Command};

use forgeguard_core::git::{changed_files, changed_scope};
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
    let scope = changed_scope(directory.path(), None).expect("read changed scope without HEAD");
    assert_eq!(
        scope.lines[std::path::Path::new("file with spaces.ts")],
        vec![(1, usize::MAX)]
    );
}

#[test]
fn reports_only_added_lines_and_supports_a_base_revision() {
    let directory = tempdir().expect("temp directory");
    git(directory.path(), &["init", "--quiet"]);
    git(
        directory.path(),
        &["config", "user.email", "test@example.com"],
    );
    git(
        directory.path(),
        &["config", "user.name", "ForgeGuard Test"],
    );
    fs::write(directory.path().join("app.ts"), "old();\nkeep();\n").expect("write source");
    git(directory.path(), &["add", "app.ts"]);
    git(directory.path(), &["commit", "--quiet", "-m", "base"]);
    let base = git_output(directory.path(), &["rev-parse", "HEAD"]);

    fs::write(
        directory.path().join("app.ts"),
        "old();\nchanged();\nadded();\n",
    )
    .expect("change source");

    let scope = changed_scope(directory.path(), Some(base.trim())).expect("changed scope");
    assert_eq!(scope.paths, vec![std::path::PathBuf::from("app.ts")]);
    assert_eq!(scope.lines[std::path::Path::new("app.ts")], vec![(2, 3)]);
}

#[test]
fn base_revision_uses_merge_base_instead_of_base_tip() {
    let directory = tempdir().expect("temp directory");
    git(directory.path(), &["init", "--quiet"]);
    git(
        directory.path(),
        &["config", "user.email", "test@example.com"],
    );
    git(
        directory.path(),
        &["config", "user.name", "ForgeGuard Test"],
    );
    fs::write(directory.path().join("app.ts"), "const common = 1;\n").expect("write common");
    git(directory.path(), &["add", "app.ts"]);
    git(directory.path(), &["commit", "--quiet", "-m", "common"]);
    git(directory.path(), &["branch", "-M", "main"]);
    git(directory.path(), &["branch", "feature"]);
    fs::write(directory.path().join("app.ts"), "const mainOnly = 2;\n").expect("change main");
    git(
        directory.path(),
        &["commit", "--quiet", "-am", "main change"],
    );
    git(directory.path(), &["switch", "--quiet", "feature"]);
    fs::write(directory.path().join("feature.ts"), "const feature = 3;\n").expect("write feature");
    git(directory.path(), &["add", "feature.ts"]);
    git(
        directory.path(),
        &["commit", "--quiet", "-m", "feature change"],
    );

    let scope = changed_scope(directory.path(), Some("main")).expect("PR scope");

    assert_eq!(scope.paths, vec![std::path::PathBuf::from("feature.ts")]);
}

fn git(root: &std::path::Path, args: &[&str]) {
    assert!(Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .expect("run git")
        .success());
}

fn git_output(root: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("run git");
    assert!(output.status.success());
    String::from_utf8(output.stdout).expect("utf8 git output")
}
