use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::CommandConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDetection {
    pub root: PathBuf,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub package_managers: Vec<String>,
    pub database_tools: Vec<String>,
    pub test_tools: Vec<String>,
    pub suggested_commands: Vec<CommandConfig>,
}

pub fn detect_project(root: &Path) -> Result<ProjectDetection> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to resolve project root {}", root.display()))?;

    let mut languages = BTreeSet::new();
    let mut frameworks = BTreeSet::new();
    let mut package_managers = BTreeSet::new();
    let mut database_tools = BTreeSet::new();
    let mut test_tools = BTreeSet::new();
    let mut commands = BTreeMap::<String, CommandConfig>::new();

    detect_source_languages(&root, &mut languages);

    if root.join("Cargo.toml").exists() {
        languages.insert("Rust".to_owned());
        package_managers.insert("Cargo".to_owned());
        test_tools.insert("cargo test".to_owned());
        add_command(&mut commands, "format", "cargo fmt --all -- --check", true);
        add_command(
            &mut commands,
            "lint",
            "cargo clippy --workspace --all-targets --all-features -- -D warnings",
            true,
        );
        add_command(&mut commands, "test", "cargo test --workspace", true);
        add_command(&mut commands, "build", "cargo build --workspace", true);
    }

    if root.join("go.mod").exists() {
        languages.insert("Go".to_owned());
        package_managers.insert("Go modules".to_owned());
        test_tools.insert("go test".to_owned());
        add_command(
            &mut commands,
            "format",
            "gofmt -l . | awk 'NF { found=1 } END { exit found }'",
            true,
        );
        add_command(&mut commands, "test", "go test ./...", true);
        add_command(&mut commands, "build", "go build ./...", true);
    }

    if root.join("pyproject.toml").exists()
        || root.join("requirements.txt").exists()
        || root.join("setup.py").exists()
    {
        languages.insert("Python".to_owned());
        detect_python(&root, &mut package_managers, &mut test_tools, &mut commands)?;
    }

    let package_json = root.join("package.json");
    if package_json.exists() {
        languages.insert("JavaScript/TypeScript".to_owned());
        detect_node(
            &package_json,
            &root,
            &mut frameworks,
            &mut package_managers,
            &mut database_tools,
            &mut test_tools,
            &mut commands,
        )?;
    }

    if root.join("pom.xml").exists() {
        languages.insert("Java/Kotlin".to_owned());
        package_managers.insert("Maven".to_owned());
        test_tools.insert("Maven test".to_owned());
        add_command(&mut commands, "test", "mvn test", true);
        add_command(&mut commands, "build", "mvn package -DskipTests", true);
    } else if root.join("build.gradle").exists() || root.join("build.gradle.kts").exists() {
        languages.insert("Java/Kotlin".to_owned());
        package_managers.insert("Gradle".to_owned());
        let gradle = if root.join("gradlew").exists() {
            "./gradlew"
        } else {
            "gradle"
        };
        add_command(&mut commands, "test", &format!("{gradle} test"), true);
        add_command(
            &mut commands,
            "build",
            &format!("{gradle} build -x test"),
            true,
        );
    }

    if root.join("Package.swift").exists() {
        languages.insert("Swift".to_owned());
        package_managers.insert("Swift Package Manager".to_owned());
        test_tools.insert("swift test".to_owned());
        add_command(&mut commands, "test", "swift test", true);
        add_command(&mut commands, "build", "swift build", true);
    }

    let pubspec = root.join("pubspec.yaml");
    if pubspec.exists() {
        languages.insert("Dart".to_owned());
        package_managers.insert("pub".to_owned());
        let source = fs::read_to_string(&pubspec).unwrap_or_default();
        let flutter = source.lines().any(|line| line.trim() == "flutter:");
        if flutter {
            frameworks.insert("Flutter".to_owned());
            test_tools.insert("flutter test".to_owned());
        } else {
            test_tools.insert("dart test".to_owned());
        }
        add_command(
            &mut commands,
            "format",
            "dart format --output=none --set-exit-if-changed .",
            true,
        );
        add_command(&mut commands, "lint", "dart analyze", true);
        add_command(
            &mut commands,
            "test",
            if flutter { "flutter test" } else { "dart test" },
            true,
        );
    }

    if has_root_extension(&root, &["sln", "csproj"]) {
        languages.insert("C#".to_owned());
        package_managers.insert("NuGet".to_owned());
        test_tools.insert("dotnet test".to_owned());
        add_command(
            &mut commands,
            "format",
            "dotnet format --verify-no-changes",
            true,
        );
        add_command(&mut commands, "test", "dotnet test", true);
        add_command(&mut commands, "build", "dotnet build --no-restore", true);
    }

    if root.join("schema.prisma").exists() || root.join("prisma/schema.prisma").exists() {
        database_tools.insert("Prisma".to_owned());
    }
    if root.join("migrations").exists() || root.join("db/migrations").exists() {
        database_tools.insert("SQL migrations".to_owned());
    }

    Ok(ProjectDetection {
        root,
        languages: languages.into_iter().collect(),
        frameworks: frameworks.into_iter().collect(),
        package_managers: package_managers.into_iter().collect(),
        database_tools: database_tools.into_iter().collect(),
        test_tools: test_tools.into_iter().collect(),
        suggested_commands: commands.into_values().collect(),
    })
}

fn detect_node(
    package_json: &Path,
    root: &Path,
    frameworks: &mut BTreeSet<String>,
    package_managers: &mut BTreeSet<String>,
    database_tools: &mut BTreeSet<String>,
    test_tools: &mut BTreeSet<String>,
    commands: &mut BTreeMap<String, CommandConfig>,
) -> Result<()> {
    let source = fs::read_to_string(package_json)
        .with_context(|| format!("failed to read {}", package_json.display()))?;
    let value: Value = serde_json::from_str(&source)
        .with_context(|| format!("failed to parse {}", package_json.display()))?;

    let dependencies = dependency_names(&value);
    for (package, label) in [
        ("next", "Next.js"),
        ("react", "React"),
        ("vue", "Vue"),
        ("svelte", "Svelte"),
        ("@angular/core", "Angular"),
        ("express", "Express"),
        ("fastify", "Fastify"),
        ("@nestjs/core", "NestJS"),
        ("react-native", "React Native"),
        ("expo", "Expo"),
        ("@capacitor/core", "Capacitor"),
        ("electron", "Electron"),
        ("@langchain/core", "LangChain"),
        ("@openai/agents", "OpenAI Agents SDK"),
        ("ai", "Vercel AI SDK"),
    ] {
        if dependencies.contains(package) {
            frameworks.insert(label.to_owned());
        }
    }

    for (package, label) in [
        ("prisma", "Prisma"),
        ("@prisma/client", "Prisma"),
        ("typeorm", "TypeORM"),
        ("sequelize", "Sequelize"),
        ("drizzle-orm", "Drizzle ORM"),
        ("mongoose", "Mongoose"),
    ] {
        if dependencies.contains(package) {
            database_tools.insert(label.to_owned());
        }
    }

    for (package, label) in [
        ("vitest", "Vitest"),
        ("jest", "Jest"),
        ("@playwright/test", "Playwright"),
        ("cypress", "Cypress"),
    ] {
        if dependencies.contains(package) {
            test_tools.insert(label.to_owned());
        }
    }

    let runner = if root.join("pnpm-lock.yaml").exists() {
        package_managers.insert("pnpm".to_owned());
        "pnpm"
    } else if root.join("yarn.lock").exists() {
        package_managers.insert("Yarn".to_owned());
        "yarn"
    } else if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
        package_managers.insert("Bun".to_owned());
        "bun run"
    } else {
        package_managers.insert("npm".to_owned());
        "npm run"
    };

    if let Some(scripts) = value.get("scripts").and_then(Value::as_object) {
        let candidates = [
            ("format:check", "format"),
            ("format", "format"),
            ("lint", "lint"),
            ("typecheck", "typecheck"),
            ("type-check", "typecheck"),
            ("test", "test"),
            ("test:unit", "test"),
            ("build", "build"),
        ];
        for (script, name) in candidates {
            if scripts.contains_key(script) && !commands.contains_key(name) {
                let command = format!("{runner} {script}");
                add_command(commands, name, &command, true);
            }
        }
    }

    Ok(())
}

fn detect_source_languages(root: &Path, languages: &mut BTreeSet<String>) {
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| {
            !matches!(
                entry.file_name().to_str(),
                Some(
                    ".git"
                        | "target"
                        | "node_modules"
                        | "vendor"
                        | ".venv"
                        | "venv"
                        | "dist"
                        | "build"
                        | ".next"
                )
            )
        })
        .build();
    for entry in walker.filter_map(|entry| entry.ok()).filter(|entry| {
        entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
    }) {
        let Some(extension) = entry.path().extension().and_then(|value| value.to_str()) else {
            continue;
        };
        let language = match extension.to_ascii_lowercase().as_str() {
            "rs" => "Rust",
            "go" => "Go",
            "py" | "pyi" => "Python",
            "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => "JavaScript/TypeScript",
            "java" | "kt" | "kts" => "Java/Kotlin",
            "swift" => "Swift",
            "dart" => "Dart",
            "cs" => "C#",
            "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" => "C/C++",
            "php" => "PHP",
            "rb" => "Ruby",
            "sh" | "bash" | "zsh" | "fish" => "Shell",
            "lua" => "Lua",
            "ex" | "exs" => "Elixir",
            "erl" | "hrl" => "Erlang",
            "scala" | "sc" => "Scala",
            "r" => "R",
            "sql" => "SQL",
            "tf" | "hcl" => "Terraform/HCL",
            "sol" => "Solidity",
            "zig" => "Zig",
            "vue" => "Vue SFC",
            "svelte" => "Svelte",
            "proto" => "Protocol Buffers",
            _ => continue,
        };
        languages.insert(language.to_owned());
    }
}

fn has_root_extension(root: &Path, extensions: &[&str]) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    entries.filter_map(|entry| entry.ok()).any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| {
                extensions
                    .iter()
                    .any(|candidate| extension.eq_ignore_ascii_case(candidate))
            })
    })
}

fn detect_python(
    root: &Path,
    package_managers: &mut BTreeSet<String>,
    test_tools: &mut BTreeSet<String>,
    commands: &mut BTreeMap<String, CommandConfig>,
) -> Result<()> {
    let pyproject = root.join("pyproject.toml");
    let source = if pyproject.exists() {
        fs::read_to_string(&pyproject)
            .with_context(|| format!("failed to read {}", pyproject.display()))?
    } else {
        String::new()
    };

    if root.join("poetry.lock").exists() {
        package_managers.insert("Poetry".to_owned());
    } else if root.join("uv.lock").exists() {
        package_managers.insert("uv".to_owned());
    } else {
        package_managers.insert("pip".to_owned());
    }

    if source.contains("pytest") || root.join("pytest.ini").exists() {
        test_tools.insert("pytest".to_owned());
        add_command(commands, "test", "pytest", true);
    }
    if source.contains("ruff") {
        add_command(commands, "lint", "ruff check .", true);
        add_command(commands, "format", "ruff format --check .", true);
    }
    if source.contains("mypy") {
        add_command(commands, "typecheck", "mypy .", true);
    }

    Ok(())
}

fn dependency_names(value: &Value) -> BTreeSet<&str> {
    ["dependencies", "devDependencies", "peerDependencies"]
        .into_iter()
        .filter_map(|key| value.get(key).and_then(Value::as_object))
        .flat_map(|map| map.keys().map(String::as_str))
        .collect()
}

fn add_command(
    commands: &mut BTreeMap<String, CommandConfig>,
    name: &str,
    command: &str,
    required: bool,
) {
    commands.entry(name.to_owned()).or_insert(CommandConfig {
        name: name.to_owned(),
        command: command.to_owned(),
        required,
        enabled: true,
        timeout_seconds: 600,
    });
}
