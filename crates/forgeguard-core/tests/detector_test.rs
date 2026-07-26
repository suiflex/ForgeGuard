use std::fs;

use forgeguard_core::detect_project;
use tempfile::tempdir;

#[test]
fn detects_rust_workspace_commands() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write cargo manifest");

    let detection = detect_project(directory.path()).expect("detect project");

    assert!(detection.languages.contains(&"Rust".to_owned()));
    assert!(detection.package_managers.contains(&"Cargo".to_owned()));
    assert!(detection
        .suggested_commands
        .iter()
        .any(|command| command.command == "cargo test --workspace"));
}

#[test]
fn detects_node_frameworks_and_scripts() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("pnpm-lock.yaml"),
        "lockfileVersion: 9\n",
    )
    .expect("write lockfile");
    fs::write(
        directory.path().join("package.json"),
        r#"{
          "scripts": {
            "lint": "eslint .",
            "typecheck": "tsc --noEmit",
            "test": "vitest run",
            "build": "next build"
          },
          "dependencies": {
            "next": "15.0.0",
            "react": "19.0.0",
            "@prisma/client": "6.0.0"
          },
          "devDependencies": {
            "vitest": "2.0.0"
          }
        }"#,
    )
    .expect("write package.json");

    let detection = detect_project(directory.path()).expect("detect project");

    assert!(detection.frameworks.contains(&"Next.js".to_owned()));
    assert!(detection.frameworks.contains(&"React".to_owned()));
    assert!(detection.database_tools.contains(&"Prisma".to_owned()));
    assert!(detection.test_tools.contains(&"Vitest".to_owned()));
    assert!(detection
        .suggested_commands
        .iter()
        .any(|command| command.command == "pnpm lint"));
}

#[test]
fn detects_mobile_systems_scripts_and_infrastructure_sources() {
    let directory = tempdir().expect("temp directory");
    for (name, source) in [
        ("App.swift", "struct App {}\n"),
        ("main.dart", "void main() {}\n"),
        ("worker.cpp", "int main() {}\n"),
        ("Program.cs", "class Program {}\n"),
        ("deploy.sh", "#!/bin/sh\n"),
        ("main.tf", "terraform {}\n"),
    ] {
        fs::write(directory.path().join(name), source).expect("write source");
    }

    let detection = detect_project(directory.path()).expect("detect project");

    for language in ["Swift", "Dart", "C/C++", "C#", "Shell", "Terraform/HCL"] {
        assert!(detection.languages.contains(&language.to_owned()));
    }
}

#[test]
fn detects_flutter_and_native_quality_commands() {
    let directory = tempdir().expect("temp directory");
    fs::write(
        directory.path().join("pubspec.yaml"),
        "name: sample\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )
    .expect("write pubspec");
    fs::write(directory.path().join("main.dart"), "void main() {}\n").expect("write Dart");

    let detection = detect_project(directory.path()).expect("detect project");

    assert!(detection.frameworks.contains(&"Flutter".to_owned()));
    assert!(detection
        .suggested_commands
        .iter()
        .any(|command| command.command == "dart analyze"));
    assert!(detection
        .suggested_commands
        .iter()
        .any(|command| command.command == "flutter test"));
}
