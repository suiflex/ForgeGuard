use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use regex::Regex;
use tree_sitter::{Language, Node, Parser};

use crate::{
    config::ScanConfig,
    duplication::scan_duplicate_blocks,
    model::{Finding, Severity},
};

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// `None` scans the full project. `Some(vec![])` intentionally scans no files.
    pub paths: Option<Vec<std::path::PathBuf>>,
}

pub fn scan_project(
    root: &Path,
    config: &ScanConfig,
    options: &ScanOptions,
) -> Result<Vec<Finding>> {
    if !config.enabled {
        return Ok(Vec::new());
    }

    let mut analyzer = Analyzer::new()?;
    let mut findings = Vec::new();
    let files = collect_source_files(root, config, options)?;

    for path in &files {
        let metadata =
            fs::metadata(path).with_context(|| format!("failed to inspect {}", path.display()))?;
        if metadata.len() > config.max_file_bytes {
            continue;
        }
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let mut file_findings = Vec::new();
        analyzer.scan_file(root, path, &source, &mut file_findings);
        apply_inline_suppressions(&source, &mut file_findings);
        findings.extend(file_findings);
    }

    let focus_paths = options.paths.as_ref().map(|_| {
        files
            .iter()
            .map(|path| path.strip_prefix(root).unwrap_or(path).to_path_buf())
            .collect::<BTreeSet<_>>()
    });
    let duplication_files = if options.paths.is_some() {
        collect_source_files(root, config, &ScanOptions::default())?
    } else {
        files.clone()
    };
    findings.extend(scan_duplicate_blocks(
        root,
        &duplication_files,
        config.duplicate_block_lines,
        focus_paths.as_ref(),
    ));
    findings.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.line.cmp(&right.line))
            .then(left.rule_id.cmp(&right.rule_id))
    });
    Ok(findings)
}

fn apply_inline_suppressions(source: &str, findings: &mut Vec<Finding>) {
    let mut allowed = HashSet::new();
    for (index, line) in source.lines().enumerate() {
        let Some(marker) = line.split("forgeguard: allow ").nth(1) else {
            continue;
        };
        let Some((rule_id, reason)) = marker.split_once(" -- ") else {
            continue;
        };
        if rule_id.starts_with("FG-") && !reason.trim().is_empty() {
            allowed.insert((index + 1, rule_id.trim().to_owned()));
            allowed.insert((index + 2, rule_id.trim().to_owned()));
        }
    }
    findings.retain(|finding| {
        finding.severity == Severity::Error
            || !allowed.contains(&(finding.line, finding.rule_id.clone()))
    });
}

fn collect_source_files(
    root: &Path,
    config: &ScanConfig,
    options: &ScanOptions,
) -> Result<Vec<std::path::PathBuf>> {
    if let Some(paths) = &options.paths {
        return Ok(paths
            .iter()
            .map(|path| {
                if path.is_absolute() {
                    path.clone()
                } else {
                    root.join(path)
                }
            })
            .filter(|path| path.is_file() && is_supported_source(path))
            .collect());
    }

    let include_tests = config.include_tests;
    let extra_excludes = config.extra_excludes.clone();
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(move |entry| {
            let path = entry.path();
            !is_builtin_excluded(path)
                && !extra_excludes
                    .iter()
                    .any(|fragment| path.to_string_lossy().contains(fragment))
                && (include_tests || !is_test_path(path))
        });

    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = entry.context("failed while walking project files")?;
        if entry.file_type().is_some_and(|kind| kind.is_file()) && is_supported_source(entry.path())
        {
            files.push(entry.into_path());
        }
    }
    Ok(files)
}

fn is_builtin_excluded(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(
                ".git"
                    | ".forgeguard"
                    | "node_modules"
                    | "target"
                    | "dist"
                    | "build"
                    | "coverage"
                    | "vendor"
                    | ".next"
                    | ".turbo"
            )
        )
    })
}

fn is_test_path(path: &Path) -> bool {
    let has_test_directory = path.components().any(|component| {
        matches!(
            component
                .as_os_str()
                .to_str()
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("test" | "tests" | "__tests__")
        )
    });
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    has_test_directory
        || file_name.ends_with("_test.go")
        || file_name.ends_with("_test.rs")
        || file_name.contains(".test.")
        || file_name.contains(".spec.")
}

fn is_supported_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some(
            "rs" | "go"
                | "py"
                | "pyi"
                | "js"
                | "jsx"
                | "mjs"
                | "cjs"
                | "ts"
                | "tsx"
                | "mts"
                | "cts"
                | "java"
                | "kt"
                | "kts"
                | "swift"
                | "dart"
                | "sql"
                | "rb"
                | "php"
                | "cs"
                | "cpp"
                | "cc"
                | "cxx"
                | "c"
                | "h"
                | "hpp"
                | "sh"
                | "bash"
                | "zsh"
                | "fish"
                | "lua"
                | "ex"
                | "exs"
                | "erl"
                | "hrl"
                | "scala"
                | "sc"
                | "r"
                | "tf"
                | "hcl"
                | "sol"
                | "zig"
                | "vue"
                | "svelte"
                | "proto"
        )
    )
}

struct Analyzer {
    select_all: Regex,
}

impl Analyzer {
    fn new() -> Result<Self> {
        Ok(Self {
            select_all: Regex::new(r"(?xi)\bSELECT\s+\*\s+FROM\b")?,
        })
    }

    fn scan_file(&mut self, root: &Path, path: &Path, source: &str, findings: &mut Vec<Finding>) {
        let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        let Some(profile) = LanguageProfile::from_path(path) else {
            if path.extension().and_then(|value| value.to_str()) == Some("sql") {
                self.scan_sql(&relative, source, findings);
            }
            return;
        };
        let mut parser = Parser::new();
        if parser.set_language(&profile.language()).is_err() {
            findings.push(parse_finding(
                &relative,
                "language grammar could not be loaded",
            ));
            return;
        }
        let Some(tree) = parser.parse(source, None) else {
            findings.push(parse_finding(&relative, "source could not be parsed"));
            return;
        };
        if tree.root_node().has_error() {
            findings.push(parse_finding(
                &relative,
                "source contains syntax errors; structural checks skipped",
            ));
            return;
        }

        let mut stack = vec![(tree.root_node(), 0usize)];
        while let Some((node, loop_depth)) = stack.pop() {
            let starts_loop = profile.is_loop(node.kind())
                || call_name(node, source).is_some_and(|callee| profile.is_iteration_call(callee));
            if starts_loop && loop_depth > 0 && !has_static_bound(node, source) {
                findings.push(finding_for_node(
                    "FG-ALG-001",
                    "Potential nested iteration",
                    Severity::Warning,
                    &relative,
                    node,
                    source,
                    "Review both input bounds. Pre-index independent collections when nested traversal can grow multiplicatively.",
                ));
            }

            if let Some(callee) = call_name(node, source) {
                let in_loop = loop_depth > 0;
                let method = terminal_name(callee);
                if in_loop && profile.is_linear_lookup(method) {
                    findings.push(finding_for_node(
                        "FG-ALG-002",
                        "Repeated linear lookup inside iteration",
                        Severity::Warning,
                        &relative,
                        node,
                        source,
                        "Pre-index the lookup collection with a map or set when its size can grow.",
                    ));
                }
                if in_loop && is_database_call(callee, method) {
                    findings.push(finding_for_node(
                        "FG-DB-001",
                        "Database operation inside iteration",
                        Severity::Error,
                        &relative,
                        node,
                        source,
                        "Replace per-item database access with a set-based query, join, eager load, prefetch, or bulk operation.",
                    ));
                }
                if in_loop && is_network_call(callee, method) {
                    findings.push(finding_for_node(
                        "FG-NET-001",
                        "External request inside iteration",
                        Severity::Warning,
                        &relative,
                        node,
                        source,
                        "Use API batching or bounded concurrency with timeouts and partial-failure handling.",
                    ));
                }
                if in_loop
                    && matches!(
                        method,
                        "sort"
                            | "Sort"
                            | "sort_by"
                            | "sort_unstable"
                            | "sort_unstable_by"
                            | "sorted"
                    )
                {
                    findings.push(finding_for_node(
                        "FG-ALG-003",
                        "Sorting inside iteration",
                        Severity::Warning,
                        &relative,
                        node,
                        source,
                        "Move sorting outside the loop or use a heap or ordered structure.",
                    ));
                }
                if is_unbounded_parallel(callee, node, source) {
                    findings.push(finding_for_node(
                        "FG-CON-001",
                        "Potential unbounded parallel execution",
                        Severity::Warning,
                        &relative,
                        node,
                        source,
                        "Use a bounded worker pool, semaphore, chunking, or an explicit concurrency limit.",
                    ));
                }
                if is_database_call(callee, method)
                    && self.select_all.is_match(node_text(node, source))
                {
                    findings.push(finding_for_node(
                        "FG-DB-005",
                        "Potential unnecessary SELECT *",
                        Severity::Warning,
                        &relative,
                        node,
                        source,
                        "Select only columns required by the use case.",
                    ));
                }
            }

            let child_loop_depth = loop_depth + usize::from(starts_loop);
            stack.extend(
                (0..node.named_child_count())
                    .rev()
                    .filter_map(|index| node.named_child(index as u32))
                    .map(|child| (child, child_loop_depth)),
            );
        }
    }

    fn scan_sql(&self, path: &Path, source: &str, findings: &mut Vec<Finding>) {
        let mut line = 1;
        let mut previous_start = 0;
        for matched in self.select_all.find_iter(source) {
            line += source[previous_start..matched.start()]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count();
            previous_start = matched.start();
            findings.push(finding(
                "FG-DB-005",
                "Potential unnecessary SELECT *",
                Severity::Warning,
                path,
                line,
                matched.as_str(),
                "Select only columns required by the use case.",
            ));
        }
    }
}

#[derive(Clone, Copy)]
enum LanguageProfile {
    JavaScript,
    TypeScript,
    Tsx,
    Rust,
    Go,
    Python,
    Java,
    Kotlin,
    CSharp,
    C,
    Cpp,
    Ruby,
    Php,
    Swift,
    Dart,
    Bash,
}

impl LanguageProfile {
    fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "rs" => Some(Self::Rust),
            "go" => Some(Self::Go),
            "py" | "pyi" => Some(Self::Python),
            "java" => Some(Self::Java),
            "kt" | "kts" => Some(Self::Kotlin),
            "cs" => Some(Self::CSharp),
            "c" => Some(Self::C),
            "cpp" | "cc" | "cxx" | "h" | "hpp" => Some(Self::Cpp),
            "rb" => Some(Self::Ruby),
            "php" => Some(Self::Php),
            "swift" => Some(Self::Swift),
            "dart" => Some(Self::Dart),
            "sh" | "bash" | "zsh" => Some(Self::Bash),
            _ => None,
        }
    }

    fn language(self) -> Language {
        match self {
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            Self::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Self::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            Self::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Self::Swift => tree_sitter_swift::LANGUAGE.into(),
            Self::Dart => tree_sitter_dart::LANGUAGE.into(),
            Self::Bash => tree_sitter_bash::LANGUAGE.into(),
        }
    }

    fn is_loop(self, kind: &str) -> bool {
        match self {
            Self::JavaScript | Self::TypeScript | Self::Tsx => matches!(
                kind,
                "for_statement" | "for_in_statement" | "while_statement" | "do_statement"
            ),
            Self::Rust => matches!(
                kind,
                "for_expression" | "while_expression" | "loop_expression"
            ),
            Self::Go => kind == "for_statement",
            Self::Python => matches!(
                kind,
                "for_statement"
                    | "while_statement"
                    | "list_comprehension"
                    | "set_comprehension"
                    | "dictionary_comprehension"
                    | "generator_expression"
            ),
            Self::Java => matches!(
                kind,
                "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement"
            ),
            Self::Kotlin => matches!(
                kind,
                "for_statement" | "while_statement" | "do_while_statement"
            ),
            Self::CSharp => matches!(
                kind,
                "for_statement" | "foreach_statement" | "while_statement" | "do_statement"
            ),
            Self::C | Self::Cpp | Self::Dart => {
                matches!(kind, "for_statement" | "while_statement" | "do_statement")
            }
            Self::Ruby => matches!(kind, "for" | "while" | "until"),
            Self::Php => matches!(
                kind,
                "for_statement" | "foreach_statement" | "while_statement" | "do_statement"
            ),
            Self::Swift => matches!(
                kind,
                "for_statement" | "while_statement" | "repeat_while_statement"
            ),
            Self::Bash => matches!(
                kind,
                "for_statement" | "c_style_for_statement" | "while_statement" | "until_statement"
            ),
        }
    }

    fn is_linear_lookup(self, method: &str) -> bool {
        match self {
            Self::JavaScript | Self::TypeScript | Self::Tsx => {
                matches!(method, "find" | "includes" | "indexOf")
            }
            Self::Rust => matches!(method, "find" | "position"),
            Self::Python => method == "index",
            Self::Java => method == "indexOf",
            Self::Kotlin => matches!(method, "find" | "firstOrNull" | "indexOf"),
            Self::CSharp => matches!(method, "Find" | "IndexOf"),
            Self::Ruby => matches!(method, "find" | "index"),
            Self::Php => matches!(method, "in_array" | "array_search"),
            Self::Swift => method == "firstIndex",
            Self::Dart => matches!(method, "firstWhere" | "indexOf"),
            Self::Go | Self::C | Self::Cpp | Self::Bash => false,
        }
    }

    fn is_iteration_call(self, callee: &str) -> bool {
        match self {
            Self::JavaScript | Self::TypeScript | Self::Tsx => {
                matches!(terminal_name(callee), "map" | "filter" | "forEach")
            }
            Self::Rust => terminal_name(callee) == "for_each",
            Self::Java | Self::Kotlin => {
                matches!(terminal_name(callee), "map" | "filter" | "forEach")
            }
            Self::CSharp => matches!(terminal_name(callee), "Select" | "Where" | "ForEach"),
            Self::Cpp => terminal_name(callee) == "for_each",
            Self::Ruby => matches!(terminal_name(callee), "each" | "map" | "select"),
            Self::Php => matches!(terminal_name(callee), "array_map" | "array_filter"),
            Self::Swift => matches!(terminal_name(callee), "map" | "filter" | "forEach"),
            Self::Dart => matches!(terminal_name(callee), "map" | "where" | "forEach"),
            Self::Go | Self::Python | Self::C | Self::Bash => false,
        }
    }
}

fn call_name<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    let callee = match node.kind() {
        "call_expression" => node
            .child_by_field_name("function")
            .or_else(|| node.named_child(0))?,
        "call" => {
            if let Some(function) = node.child_by_field_name("function") {
                function
            } else {
                return qualified_call_name(node, source, "receiver", "method");
            }
        }
        "method_call_expression" => {
            return qualified_call_name(node, source, "receiver", "method");
        }
        "method_invocation" => return qualified_call_name(node, source, "object", "name"),
        "invocation_expression" | "function_call_expression" => {
            node.child_by_field_name("function")?
        }
        "member_call_expression" | "nullsafe_member_call_expression" => {
            return qualified_call_name(node, source, "object", "name");
        }
        "scoped_call_expression" => {
            return qualified_call_name(node, source, "scope", "name");
        }
        "macro_invocation" => node.child_by_field_name("macro")?,
        _ => return None,
    };
    Some(node_text(callee, source))
}

fn qualified_call_name<'a>(
    node: Node<'_>,
    source: &'a str,
    receiver_field: &str,
    method_field: &str,
) -> Option<&'a str> {
    let method = node.child_by_field_name(method_field)?;
    let Some(receiver) = node.child_by_field_name(receiver_field) else {
        return Some(node_text(method, source));
    };
    source.get(receiver.start_byte()..method.end_byte())
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    source.get(node.byte_range()).unwrap_or_default()
}

fn terminal_name(callee: &str) -> &str {
    callee
        .rsplit(['.', ':', '>'])
        .find(|part| !part.is_empty())
        .unwrap_or(callee)
}

fn is_database_call(callee: &str, method: &str) -> bool {
    let normalized = callee.to_ascii_lowercase();
    let method = normalized_method(method);
    let database_receiver = has_receiver(
        &normalized,
        &[
            "db",
            "database",
            "repo",
            "repository",
            "prisma",
            "sequelize",
            "typeorm",
            "sqlx",
            "diesel",
        ],
    );
    database_receiver
        && matches!(
            method.as_str(),
            "findmany"
                | "findunique"
                | "findfirst"
                | "query"
                | "execute"
                | "fetch"
                | "fetchall"
                | "fetchone"
                | "save"
                | "create"
                | "update"
                | "delete"
                | "insert"
                | "insertinto"
        )
}

fn is_network_call(callee: &str, method: &str) -> bool {
    if callee == "fetch" || callee.starts_with("reqwest::") {
        return true;
    }
    let normalized = callee.to_ascii_lowercase();
    let method = normalized_method(method);
    let network_receiver = has_receiver(
        &normalized,
        &[
            "axios",
            "http",
            "client",
            "apiclient",
            "httpclient",
            "requests",
            "httpx",
            "urllib",
            "session",
        ],
    );
    network_receiver
        && matches!(
            method.as_str(),
            "get" | "post" | "put" | "patch" | "delete" | "request" | "send"
        )
}

fn normalized_method(method: &str) -> String {
    let mut normalized = method.to_ascii_lowercase().replace('_', "");
    if normalized.ends_with("async") {
        normalized.truncate(normalized.len() - "async".len());
    }
    normalized
}

fn has_receiver(callee: &str, names: &[&str]) -> bool {
    callee
        .split(['.', ':', '-', '>', '$'])
        .any(|part| names.contains(&part))
}

fn is_unbounded_parallel(callee: &str, node: Node<'_>, source: &str) -> bool {
    terminal_name(callee) == "join_all"
        || (callee == "Promise.all" && has_iteration_descendant(node, source))
}

fn has_iteration_descendant(node: Node<'_>, source: &str) -> bool {
    let mut stack = (0..node.named_child_count())
        .filter_map(|index| node.named_child(index as u32))
        .collect::<Vec<_>>();
    while let Some(child) = stack.pop() {
        if call_name(child, source).is_some_and(|callee| terminal_name(callee) == "map") {
            return true;
        }
        stack.extend(
            (0..child.named_child_count()).filter_map(|index| child.named_child(index as u32)),
        );
    }
    false
}

fn has_static_bound(node: Node<'_>, source: &str) -> bool {
    has_static_take_bound(node, source) || has_literal_range_iterable(node, source)
}

/// True when a direct child of the loop is `range(<int literals>)`, i.e. the loop
/// bound is a compile-time constant and cannot grow multiplicatively.
fn has_literal_range_iterable(node: Node<'_>, source: &str) -> bool {
    (0..node.named_child_count())
        .filter_map(|index| node.named_child(index as u32))
        .any(|child| {
            call_name(child, source).is_some_and(|callee| terminal_name(callee) == "range")
                && all_integer_arguments(child)
        })
}

fn all_integer_arguments(call: Node<'_>) -> bool {
    let Some(arguments) = call.child_by_field_name("arguments") else {
        return false;
    };
    let mut count = 0;
    for index in 0..arguments.named_child_count() {
        let Some(argument) = arguments.named_child(index as u32) else {
            continue;
        };
        if argument.kind() != "integer" {
            return false;
        }
        count += 1;
    }
    count > 0
}

fn has_static_take_bound(node: Node<'_>, source: &str) -> bool {
    let text = node_text(node, source);
    let Some((_, after_take)) = text.split_once(".take(") else {
        return false;
    };
    let Some((argument, _)) = after_take.split_once(')') else {
        return false;
    };
    !argument.is_empty() && argument.chars().all(|character| character.is_ascii_digit())
}

fn finding_for_node(
    rule_id: &str,
    title: &str,
    severity: Severity,
    path: &Path,
    node: Node<'_>,
    source: &str,
    recommendation: &str,
) -> Finding {
    finding(
        rule_id,
        title,
        severity,
        path,
        node.start_position().row + 1,
        node_text(node, source),
        recommendation,
    )
}

fn parse_finding(path: &Path, evidence: &str) -> Finding {
    finding(
        "FG-PARSE-001",
        "Structural analysis skipped",
        Severity::Info,
        path,
        1,
        evidence,
        "Fix syntax errors or use a supported parser before relying on structural findings.",
    )
}

fn finding(
    rule_id: &str,
    title: &str,
    severity: Severity,
    path: &Path,
    line: usize,
    evidence: &str,
    recommendation: &str,
) -> Finding {
    Finding {
        rule_id: rule_id.to_owned(),
        title: title.to_owned(),
        severity,
        path: path.to_path_buf(),
        line,
        evidence: truncate(evidence, 240),
        recommendation: recommendation.to_owned(),
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut output: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        output.push('…');
    }
    output
}
