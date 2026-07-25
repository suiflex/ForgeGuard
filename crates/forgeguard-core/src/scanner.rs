use std::{
    collections::BTreeSet,
    fs,
    path::Path,
};

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use regex::Regex;

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

    let analyzer = Analyzer::new()?;
    let mut findings = Vec::new();
    let files = collect_source_files(root, config, options)?;

    for path in &files {
        let metadata = fs::metadata(path)
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if metadata.len() > config.max_file_bytes {
            continue;
        }
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        analyzer.scan_file(root, path, &source, &mut findings);
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
        if entry.file_type().is_some_and(|kind| kind.is_file())
            && is_supported_source(entry.path())
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
            component.as_os_str().to_str().map(str::to_ascii_lowercase).as_deref(),
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
                | "js"
                | "jsx"
                | "ts"
                | "tsx"
                | "java"
                | "kt"
                | "kts"
                | "sql"
                | "rb"
                | "php"
                | "cs"
                | "cpp"
                | "cc"
                | "c"
                | "h"
                | "hpp"
        )
    )
}

struct Analyzer {
    loop_start: Regex,
    repeated_lookup: Regex,
    python_loop_start: Regex,
    database_operation: Regex,
    network_operation: Regex,
    unbounded_parallel: Regex,
    select_all: Regex,
    repeated_sort: Regex,
}

impl Analyzer {
    fn new() -> Result<Self> {
        Ok(Self {
            loop_start: Regex::new(
                r"(?x)(?:\bfor\b|\bwhile\b|\bloop\b|\.for_each\s*\(|\.map\s*\()",
            )?,
            repeated_lookup: Regex::new(
                r"(?x)\.(?:find|includes|indexOf|contains|position)\s*\(",
            )?,
            python_loop_start: Regex::new(r"(?x)^\s*(?:async\s+)?(?:for|while)\b.*:\s*$")?,
            database_operation: Regex::new(
                r"(?xi)(?:\b(?:SELECT\s+|INSERT\s+INTO|UPDATE\s+\w+|DELETE\s+FROM)\b|\b(?:db|database|repo|repository|prisma|sequelize|typeorm)\b(?:\s*\.\s*[A-Za-z_]\w*){0,2}\s*\.\s*(?:findMany|findUnique|findFirst|query|execute|fetch|save|create|update|delete|insert)\s*\(|\b(?:sqlx|diesel)\s*::\s*(?:query|execute|insert_into|update|delete)\b)",
            )?,
            network_operation: Regex::new(
                r"(?xi)(?:\bfetch\s*\(|\baxios\s*\.\s*(?:get|post|put|patch|delete|request)\s*\(|\b(?:http|client|apiClient|httpClient)\b\s*\.\s*(?:get|post|put|patch|delete|request|send)\s*\(|\breqwest\s*::)",
            )?,
            unbounded_parallel: Regex::new(
                r"(?x)(?:Promise\.all\s*\([^\n]*\.map\s*\(|join_all\s*\()",
            )?,
            select_all: Regex::new(r"(?xi)\bSELECT\s+\*\s+FROM\b")?,
            repeated_sort: Regex::new(r"(?x)\.(?:sort|sort_by|sorted)\s*\(")?,
        })
    }

    fn scan_file(&self, root: &Path, path: &Path, source: &str, findings: &mut Vec<Finding>) {
        let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        let is_python = path.extension().and_then(|value| value.to_str()) == Some("py");
        let mut brace_depth = 0usize;
        let mut loop_scopes: Vec<usize> = Vec::new();
        let mut python_loop_indents: Vec<usize> = Vec::new();

        for (index, raw_line) in source.lines().enumerate() {
            let line_number = index + 1;
            let line = strip_inline_comment(raw_line).trim();
            if line.is_empty() {
                if !is_python {
                    update_scope_depth(raw_line, &mut brace_depth, &mut loop_scopes);
                }
                continue;
            }

            let indentation = indentation_width(raw_line);
            if is_python {
                while python_loop_indents
                    .last()
                    .is_some_and(|loop_indent| indentation <= *loop_indent)
                {
                    python_loop_indents.pop();
                }
            }

            let in_loop = if is_python {
                !python_loop_indents.is_empty()
            } else {
                !loop_scopes.is_empty()
            };
            let loop_count = if is_python {
                usize::from(self.python_loop_start.is_match(raw_line))
            } else {
                self.loop_start.find_iter(line).count()
            };
            let starts_loop = loop_count > 0;

            if (starts_loop && in_loop) || loop_count > 1 {
                findings.push(finding(
                    "FG-ALG-001",
                    "Potential nested iteration",
                    Severity::Warning,
                    &relative,
                    line_number,
                    line,
                    "Review the input bound and complexity. Prefer indexing, batching, or a single traversal when the nested scan is not inherently required.",
                ));
            }

            if (in_loop || starts_loop) && self.repeated_lookup.is_match(line) {
                findings.push(finding(
                    "FG-ALG-002",
                    "Repeated linear lookup inside iteration",
                    Severity::Warning,
                    &relative,
                    line_number,
                    line,
                    "Pre-index the lookup collection with a Map, Set, or hash map when the collection can grow.",
                ));
            }

            if (in_loop || starts_loop) && self.database_operation.is_match(line) {
                findings.push(finding(
                    "FG-DB-001",
                    "Database operation inside iteration",
                    Severity::Error,
                    &relative,
                    line_number,
                    line,
                    "Replace per-item database access with a set-based query, join, eager load, prefetch, or bulk operation.",
                ));
            }

            if (in_loop || starts_loop) && self.network_operation.is_match(line) {
                findings.push(finding(
                    "FG-NET-001",
                    "External request inside iteration",
                    Severity::Warning,
                    &relative,
                    line_number,
                    line,
                    "Use API batching when available or bounded concurrency with timeout, rate-limit handling, retry classification, and partial-failure handling.",
                ));
            }

            if (in_loop || starts_loop) && self.repeated_sort.is_match(line) {
                findings.push(finding(
                    "FG-ALG-003",
                    "Sorting inside iteration",
                    Severity::Warning,
                    &relative,
                    line_number,
                    line,
                    "Move sorting outside the loop or consider a heap, ordered structure, or one-time preprocessing.",
                ));
            }

            if self.unbounded_parallel.is_match(line) {
                findings.push(finding(
                    "FG-CON-001",
                    "Potential unbounded parallel execution",
                    Severity::Warning,
                    &relative,
                    line_number,
                    line,
                    "Use a bounded worker pool, semaphore, chunking, or an explicit concurrency limit for collections without a hard upper bound.",
                ));
            }

            if self.select_all.is_match(line) {
                findings.push(finding(
                    "FG-DB-005",
                    "Potential unnecessary SELECT *",
                    Severity::Warning,
                    &relative,
                    line_number,
                    line,
                    "Select only the columns required by the use case, especially on hot paths or tables with large fields.",
                ));
            }

            if is_python {
                if starts_loop {
                    python_loop_indents.push(indentation);
                }
            } else {
                let opening_braces = raw_line.chars().filter(|character| *character == '{').count();
                if starts_loop && opening_braces > 0 {
                    loop_scopes.push(brace_depth + opening_braces);
                }
                update_scope_depth(raw_line, &mut brace_depth, &mut loop_scopes);
            }
        }
    }
}

fn indentation_width(line: &str) -> usize {
    line.chars()
        .take_while(|character| character.is_whitespace())
        .map(|character| if character == '\t' { 4 } else { 1 })
        .sum()
}

fn update_scope_depth(line: &str, brace_depth: &mut usize, loop_scopes: &mut Vec<usize>) {
    let opening = line.chars().filter(|character| *character == '{').count();
    let closing = line.chars().filter(|character| *character == '}').count();
    *brace_depth = (*brace_depth)
        .saturating_add(opening)
        .saturating_sub(closing);
    while loop_scopes.last().is_some_and(|scope| *brace_depth < *scope) {
        loop_scopes.pop();
    }
}

fn strip_inline_comment(line: &str) -> &str {
    let slash = line.find("//");
    let hash = line.find('#');
    match (slash, hash) {
        (Some(left), Some(right)) => &line[..left.min(right)],
        (Some(index), None) | (None, Some(index)) => &line[..index],
        (None, None) => line,
    }
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
