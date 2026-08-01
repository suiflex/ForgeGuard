use std::{
    collections::{BTreeSet, HashMap, HashSet},
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
    model::{EvidenceConfidence, Finding, Severity},
    rules,
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
    let project_files = if options.paths.is_some() {
        collect_source_files(root, config, &ScanOptions::default())?
    } else {
        files.clone()
    };
    let semantic = SemanticIndex::build(&project_files);

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
        analyzer.scan_file(root, path, &source, &semantic, &mut file_findings);
        apply_inline_suppressions(&source, &mut file_findings);
        findings.extend(file_findings);
    }

    let focus_paths = options.paths.as_ref().map(|_| {
        files
            .iter()
            .map(|path| path.strip_prefix(root).unwrap_or(path).to_path_buf())
            .collect::<BTreeSet<_>>()
    });
    findings.extend(scan_duplicate_blocks(
        root,
        &project_files,
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
                | "astro"
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

/// A data literal spanning this many source lines is almost always embedded
/// fixture/mock data that belongs in its own file rather than mixed into logic.
const MAX_DATA_LITERAL_LINES: usize = 80;
/// Or this many top-level elements/entries, for dense single-line-ish dumps.
const MAX_DATA_LITERAL_ELEMENTS: usize = 50;

struct Analyzer {
    select_all: Regex,
    script_block: Regex,
    ts_lang: Regex,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SinkSet(u8);

impl SinkSet {
    const DATABASE: Self = Self(1);
    const NETWORK: Self = Self(2);

    fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    fn insert(&mut self, other: Self) -> bool {
        let previous = self.0;
        self.0 |= other.0;
        self.0 != previous
    }
}

#[derive(Default)]
struct FileProvenance {
    database_symbols: HashSet<String>,
    network_symbols: HashSet<String>,
    local_symbols: HashSet<String>,
}

impl FileProvenance {
    fn from_source(profile: LanguageProfile, source: &str) -> Self {
        let mut provenance = Self::default();
        for line in source.lines() {
            let lower = line.to_ascii_lowercase();
            let trimmed = line.trim_start();
            let import_like = trimmed.starts_with("import ")
                || trimmed.starts_with("from ")
                || trimmed.starts_with("use ")
                || trimmed.contains("require(");
            if import_like {
                let declaration = line.split(['\"', '\'']).next().unwrap_or(line);
                provenance.local_symbols.extend(
                    identifiers(declaration)
                        .into_iter()
                        .filter(|word| !binding_keywords().contains(&word.as_str())),
                );
                if let Some(path) = quoted_value(line) {
                    if let Some(name) = path.rsplit('/').next() {
                        provenance
                            .local_symbols
                            .insert(name.replace(['-', '.'], "_"));
                    }
                }
            }
            // forgeguard: allow FG-ALG-001 -- semantic package catalogs are fixed at eight entries or fewer
            for (package, kind) in semantic_packages(profile) {
                if !import_like || !contains_package(&lower, package) {
                    continue;
                }
                let declaration = line.split(['\"', '\'']).next().unwrap_or(line);
                let symbols = identifiers(declaration)
                    .into_iter()
                    .filter(|word| !binding_keywords().contains(&word.as_str()));
                let target = if *kind == SinkSet::DATABASE {
                    &mut provenance.database_symbols
                } else {
                    &mut provenance.network_symbols
                };
                target.extend(symbols);
                if let Some(default) = package
                    .rsplit('/')
                    .next()
                    .and_then(|part| part.rsplit('.').next())
                {
                    target.insert(default.replace('-', "_"));
                }
            }
        }
        if matches!(
            profile,
            LanguageProfile::JavaScript | LanguageProfile::TypeScript | LanguageProfile::Tsx
        ) {
            provenance.network_symbols.insert("fetch".to_owned());
        }

        // Resolve direct assignments such as `const prisma = new PrismaClient()`
        // and `db, err := sql.Open(...)`. This stays lexical and path-insensitive.
        for _ in 0..4 {
            let mut changed = false;
            // forgeguard: allow FG-ALG-001 -- assignment propagation is capped at four passes
            for line in source.lines() {
                let Some((left, right)) = line.split_once('=') else {
                    continue;
                };
                // forgeguard: allow FG-ALG-002 -- keyword catalog is a fixed nine-entry slice
                let Some(binding) = identifiers(left)
                    .into_iter()
                    .find(|word| !binding_keywords().contains(&word.as_str()))
                else {
                    continue;
                };
                let right_symbols = identifiers(&without_strings(right));
                if right_symbols
                    .iter()
                    .any(|word| provenance.database_symbols.contains(word))
                {
                    changed |= provenance.database_symbols.insert(binding.clone());
                }
                if right_symbols
                    .iter()
                    .any(|word| provenance.network_symbols.contains(word))
                {
                    changed |= provenance.network_symbols.insert(binding);
                }
            }
            if !changed {
                break;
            }
        }
        provenance
    }

    fn classify(&self, callee: &str, method: &str) -> SinkSet {
        let parts = identifiers(callee);
        let mut sinks = SinkSet::default();
        if parts
            .iter()
            .any(|part| self.database_symbols.contains(part))
            && database_methods().contains(&normalized_method(method).as_str())
        {
            sinks.insert(SinkSet::DATABASE);
        }
        if parts.iter().any(|part| self.network_symbols.contains(part))
            && network_methods().contains(&normalized_method(method).as_str())
        {
            sinks.insert(SinkSet::NETWORK);
        }
        sinks
    }
}

#[derive(Default)]
struct SemanticIndex {
    files: HashMap<std::path::PathBuf, FileProvenance>,
    functions: HashMap<String, SinkSet>,
}

struct FunctionDraft {
    name: String,
    sinks: SinkSet,
    calls: Vec<String>,
}

fn collect_function_drafts(
    root: Node<'_>,
    source: &str,
    profile: LanguageProfile,
    provenance: &FileProvenance,
    drafts: &mut Vec<FunctionDraft>,
    counts: &mut HashMap<String, usize>,
) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if profile.is_function(node.kind()) {
            let Some(name) = node
                .child_by_field_name("name")
                .map(|name| node_text(name, source).to_owned())
            else {
                continue;
            };
            let body = node.child_by_field_name("body").unwrap_or(node);
            let mut sinks = SinkSet::default();
            let mut calls = Vec::new();
            let mut body_stack = vec![body];
            // forgeguard: allow FG-ALG-001 -- function bodies are disjoint; nested functions are skipped here
            while let Some(child) = body_stack.pop() {
                if child != body && profile.is_function(child.kind()) {
                    continue;
                }
                if let Some(callee) = call_name(child, source) {
                    let method = terminal_name(callee);
                    sinks.insert(provenance.classify(callee, method));
                    calls.push(method.to_owned());
                }
                body_stack.extend(
                    (0..child.named_child_count())
                        .filter_map(|index| child.named_child(index as u32)),
                );
            }
            *counts.entry(name.clone()).or_default() += 1;
            drafts.push(FunctionDraft { name, sinks, calls });
        }
        stack.extend(
            (0..node.named_child_count()).filter_map(|index| node.named_child(index as u32)),
        );
    }
}

fn semantic_packages(profile: LanguageProfile) -> &'static [(&'static str, SinkSet)] {
    const JS: &[(&str, SinkSet)] = &[
        ("@prisma/client", SinkSet::DATABASE),
        ("sequelize", SinkSet::DATABASE),
        ("typeorm", SinkSet::DATABASE),
        ("drizzle-orm", SinkSet::DATABASE),
        ("mongoose", SinkSet::DATABASE),
        ("axios", SinkSet::NETWORK),
        ("undici", SinkSet::NETWORK),
    ];
    const PYTHON: &[(&str, SinkSet)] = &[
        ("sqlalchemy", SinkSet::DATABASE),
        ("django.db", SinkSet::DATABASE),
        ("psycopg", SinkSet::DATABASE),
        ("asyncpg", SinkSet::DATABASE),
        ("requests", SinkSet::NETWORK),
        ("httpx", SinkSet::NETWORK),
        ("aiohttp", SinkSet::NETWORK),
        ("urllib", SinkSet::NETWORK),
    ];
    const RUST: &[(&str, SinkSet)] = &[
        ("sqlx", SinkSet::DATABASE),
        ("diesel", SinkSet::DATABASE),
        ("sea_orm", SinkSet::DATABASE),
        ("reqwest", SinkSet::NETWORK),
        ("hyper", SinkSet::NETWORK),
    ];
    const GO: &[(&str, SinkSet)] = &[
        ("database/sql", SinkSet::DATABASE),
        ("jmoiron/sqlx", SinkSet::DATABASE),
        ("gorm.io/gorm", SinkSet::DATABASE),
        ("net/http", SinkSet::NETWORK),
    ];
    match profile {
        LanguageProfile::JavaScript | LanguageProfile::TypeScript | LanguageProfile::Tsx => JS,
        LanguageProfile::Python => PYTHON,
        LanguageProfile::Rust => RUST,
        LanguageProfile::Go => GO,
        _ => &[],
    }
}

fn identifiers(value: &str) -> Vec<String> {
    value
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|part| {
            !part.is_empty()
                && part
                    .as_bytes()
                    .first()
                    .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
        })
        .map(str::to_owned)
        .collect()
}

fn contains_package(source: &str, package: &str) -> bool {
    source.match_indices(package).any(|(start, _)| {
        let before = source[..start].chars().next_back();
        let after = source[start + package.len()..].chars().next();
        !before.is_some_and(package_identifier_character)
            && !after.is_some_and(package_identifier_character)
    })
}

fn package_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
}

fn quoted_value(line: &str) -> Option<&str> {
    for quote in ['\"', '\''] {
        if let Some((_, rest)) = line.split_once(quote) {
            if let Some((value, _)) = rest.split_once(quote) {
                return Some(value);
            }
        }
    }
    None
}

fn without_strings(value: &str) -> String {
    let mut quote = None;
    value
        .chars()
        .map(|character| match quote {
            Some(active) if character == active => {
                quote = None;
                ' '
            }
            Some(_) => ' ',
            None if matches!(character, '\"' | '\'') => {
                quote = Some(character);
                ' '
            }
            None => character,
        })
        .collect()
}

fn binding_keywords() -> &'static [&'static str] {
    &[
        "as", "const", "from", "import", "let", "mut", "require", "use", "var",
    ]
}

fn database_methods() -> &'static [&'static str] {
    &[
        "create",
        "delete",
        "execute",
        "fetch",
        "fetchall",
        "fetchone",
        "findfirst",
        "findmany",
        "findunique",
        "insert",
        "insertinto",
        "open",
        "query",
        "save",
        "update",
    ]
}

fn network_methods() -> &'static [&'static str] {
    &[
        "delete", "fetch", "get", "patch", "post", "put", "request", "send",
    ]
}

impl SemanticIndex {
    fn build(files: &[std::path::PathBuf]) -> Self {
        let mut index = Self::default();
        let mut drafts = Vec::new();
        let mut counts = HashMap::<String, usize>::new();
        for path in files {
            let Some(profile) =
                LanguageProfile::from_path(path).filter(|profile| profile.has_semantic_pack())
            else {
                continue;
            };
            let Ok(source) = fs::read_to_string(path) else {
                continue;
            };
            let mut provenance = FileProvenance::from_source(profile, &source);
            let mut parser = Parser::new();
            if parser.set_language(&profile.language()).is_err() {
                continue;
            }
            let Some(tree) = parser
                .parse(&source, None)
                .filter(|tree| !tree.root_node().has_error())
            else {
                continue;
            };
            let draft_start = drafts.len();
            collect_function_drafts(
                tree.root_node(),
                &source,
                profile,
                &provenance,
                &mut drafts,
                &mut counts,
            );
            provenance
                .local_symbols
                .extend(drafts[draft_start..].iter().map(|draft| draft.name.clone()));
            index.files.insert(path.clone(), provenance);
        }

        for draft in &drafts {
            if counts.get(&draft.name) == Some(&1) {
                index.functions.insert(draft.name.clone(), draft.sinks);
            }
        }
        for _ in 0..drafts.len().max(1) {
            let mut changed = false;
            // ponytail: project-wide fixed point is O(F × (F+E)); use SCC condensation if measured on huge projects
            // forgeguard: allow FG-ALG-001 -- bounded by finite function/call graph and exits when no summary changes
            for draft in &drafts {
                if counts.get(&draft.name) != Some(&1) {
                    continue;
                }
                let inherited = draft
                    .calls
                    .iter()
                    .fold(SinkSet::default(), |mut sinks, call| {
                        if let Some(summary) = index.functions.get(call) {
                            sinks.insert(*summary);
                        }
                        sinks
                    });
                changed |= index
                    .functions
                    .entry(draft.name.clone())
                    .or_default()
                    .insert(inherited);
            }
            if !changed {
                break;
            }
        }
        index
    }

    fn classify(&self, path: &Path, callee: &str, method: &str) -> SinkSet {
        let mut sinks = self
            .files
            .get(path)
            .map(|provenance| provenance.classify(callee, method))
            .unwrap_or_default();
        let terminal = terminal_name(callee);
        if self
            .files
            .get(path)
            .is_some_and(|provenance| provenance.local_symbols.contains(terminal))
        {
            if let Some(summary) = self.functions.get(terminal) {
                sinks.insert(*summary);
            }
        }
        sinks
    }
}

impl Analyzer {
    fn new() -> Result<Self> {
        Ok(Self {
            select_all: Regex::new(r"(?xi)\bSELECT\s+\*\s+FROM\b")?,
            script_block: Regex::new(r"(?is)<script\b[^>]*>(.*?)</script>")?,
            ts_lang: Regex::new(r#"(?i)lang\s*=\s*["']?ts"#)?,
        })
    }

    /// Svelte/Vue/Astro single-file components keep their imperative logic in
    /// `<script>` blocks (and Astro's leading `---` frontmatter). Mask every
    /// other byte with blanks — preserving line and column positions — and scan
    /// the result as JavaScript or TypeScript so all existing rules apply to the
    /// real logic instead of skipping the file.
    fn extract_component_script(
        &self,
        path: &Path,
        source: &str,
    ) -> Option<(LanguageProfile, String)> {
        let ext = path.extension().and_then(|value| value.to_str())?;
        if !matches!(ext, "svelte" | "vue" | "astro") {
            return None;
        }
        let mut regions: Vec<(usize, usize)> = Vec::new();
        let mut typescript = ext == "astro";
        if ext == "astro" {
            if let Some(rest) = source.strip_prefix("---") {
                if let Some(end) = rest.find("\n---") {
                    regions.push((3, 3 + end));
                }
            }
        }
        for caps in self.script_block.captures_iter(source) {
            let tag = caps.get(0)?;
            let body = caps.get(1)?;
            if self.ts_lang.is_match(&source[tag.start()..body.start()]) {
                typescript = true;
            }
            regions.push((body.start(), body.end()));
        }
        if regions.is_empty() {
            return None;
        }
        let profile = if typescript {
            LanguageProfile::TypeScript
        } else {
            LanguageProfile::JavaScript
        };
        Some((profile, mask_outside(source, &regions)))
    }

    fn scan_file(
        &mut self,
        root: &Path,
        path: &Path,
        source: &str,
        semantic: &SemanticIndex,
        findings: &mut Vec<Finding>,
    ) {
        let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        let component = self.extract_component_script(path, source);
        let (profile, source, emit_parse_errors) = match component.as_ref() {
            Some((profile, masked)) => (*profile, masked.as_str(), false),
            None => match LanguageProfile::from_path(path) {
                Some(profile) => (profile, source, true),
                None => {
                    if path.extension().and_then(|value| value.to_str()) == Some("sql") {
                        self.scan_sql(&relative, source, findings);
                    }
                    return;
                }
            },
        };
        let mut parser = Parser::new();
        if parser.set_language(&profile.language()).is_err() {
            if emit_parse_errors {
                findings.push(parse_finding(
                    &relative,
                    "language grammar could not be loaded",
                ));
            }
            return;
        }
        let Some(tree) = parser.parse(source, None) else {
            if emit_parse_errors {
                findings.push(parse_finding(&relative, "source could not be parsed"));
            }
            return;
        };
        if tree.root_node().has_error() {
            if emit_parse_errors {
                findings.push(parse_finding(
                    &relative,
                    "source contains syntax errors; structural checks skipped",
                ));
            }
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
                let semantic_sinks = semantic.classify(path, callee, method);
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
                if in_loop && semantic_sinks.contains(SinkSet::DATABASE) {
                    findings.push(finding_for_node(
                        "FG-DB-001",
                        "Database operation inside iteration",
                        Severity::Error,
                        &relative,
                        node,
                        source,
                        "Replace per-item database access with a set-based query, join, eager load, prefetch, or bulk operation.",
                    ));
                } else if in_loop && is_database_call(callee, method) {
                    findings.push(finding_for_node(
                        "FG-DB-002",
                        "Potential database operation inside iteration",
                        Severity::Info,
                        &relative,
                        node,
                        source,
                        "Confirm receiver provenance. Batch the operation if it performs database I/O.",
                    ));
                }
                if in_loop && semantic_sinks.contains(SinkSet::NETWORK) {
                    findings.push(finding_for_node(
                        "FG-NET-001",
                        "External request inside iteration",
                        Severity::Warning,
                        &relative,
                        node,
                        source,
                        "Use API batching or bounded concurrency with timeouts and partial-failure handling.",
                    ));
                } else if in_loop && is_network_call(callee, method) {
                    findings.push(finding_for_node(
                        "FG-NET-002",
                        "Potential external request inside iteration",
                        Severity::Info,
                        &relative,
                        node,
                        source,
                        "Confirm receiver provenance. Batch or bound concurrency if this performs network I/O.",
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

            if profile.is_data_literal(node.kind())
                && !node
                    .parent()
                    .is_some_and(|parent| profile.is_data_literal(parent.kind()))
            {
                let span = node
                    .end_position()
                    .row
                    .saturating_sub(node.start_position().row);
                if span >= MAX_DATA_LITERAL_LINES
                    || node.named_child_count() >= MAX_DATA_LITERAL_ELEMENTS
                {
                    findings.push(finding_for_node(
                        "FG-ARCH-001",
                        "Large inline data literal",
                        Severity::Warning,
                        &relative,
                        node,
                        source,
                        "Move large fixed datasets or mock data into a dedicated data/fixture file, JSON, or loader; keep modules and components focused on logic.",
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
    Zig,
    Lua,
    Scala,
    Solidity,
    Elixir,
    Erlang,
    RLang,
    Hcl,
}

impl LanguageProfile {
    fn family(self) -> &'static str {
        match self {
            Self::JavaScript | Self::TypeScript | Self::Tsx => "javascript",
            Self::Rust => "rust",
            Self::Go => "go",
            Self::Python => "python",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::CSharp => "csharp",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Ruby => "ruby",
            Self::Php => "php",
            Self::Swift => "swift",
            Self::Dart => "dart",
            Self::Bash => "shell",
            Self::Zig => "zig",
            Self::Lua => "lua",
            Self::Scala => "scala",
            Self::Solidity => "solidity",
            Self::Elixir => "elixir",
            Self::Erlang => "erlang",
            Self::RLang => "r",
            Self::Hcl => "hcl",
        }
    }

    fn has_semantic_pack(self) -> bool {
        matches!(
            self,
            Self::JavaScript | Self::TypeScript | Self::Tsx | Self::Python | Self::Rust | Self::Go
        )
    }

    fn is_function(self, kind: &str) -> bool {
        match self {
            Self::JavaScript | Self::TypeScript | Self::Tsx => {
                matches!(kind, "function_declaration" | "method_definition")
            }
            Self::Python => kind == "function_definition",
            Self::Rust => kind == "function_item",
            Self::Go => matches!(kind, "function_declaration" | "method_declaration"),
            _ => false,
        }
    }

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
            "zig" => Some(Self::Zig),
            "lua" => Some(Self::Lua),
            "scala" | "sc" => Some(Self::Scala),
            "sol" => Some(Self::Solidity),
            "ex" | "exs" => Some(Self::Elixir),
            "erl" | "hrl" => Some(Self::Erlang),
            "r" => Some(Self::RLang),
            "tf" | "hcl" => Some(Self::Hcl),
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
            Self::Zig => tree_sitter_zig::LANGUAGE.into(),
            Self::Lua => tree_sitter_lua::LANGUAGE.into(),
            Self::Scala => tree_sitter_scala::LANGUAGE.into(),
            Self::Solidity => tree_sitter_solidity::LANGUAGE.into(),
            Self::Elixir => tree_sitter_elixir::LANGUAGE.into(),
            Self::Erlang => tree_sitter_erlang::LANGUAGE.into(),
            Self::RLang => tree_sitter_r::LANGUAGE.into(),
            Self::Hcl => tree_sitter_hcl::LANGUAGE.into(),
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
            Self::Zig => matches!(kind, "for_statement" | "while_statement"),
            Self::Lua => matches!(
                kind,
                "for_statement"
                    | "for_numeric_statement"
                    | "for_generic_statement"
                    | "while_statement"
                    | "repeat_statement"
            ),
            Self::Scala => matches!(kind, "for_expression" | "while_expression"),
            Self::Solidity => matches!(
                kind,
                "for_statement" | "while_statement" | "do_while_statement"
            ),
            Self::RLang => matches!(kind, "for" | "while_statement" | "for_statement" | "while"),
            // Functional/declarative grammars: no imperative loop nodes.
            Self::Elixir | Self::Erlang | Self::Hcl => false,
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
            Self::Zig
            | Self::Lua
            | Self::Scala
            | Self::Solidity
            | Self::Elixir
            | Self::Erlang
            | Self::RLang
            | Self::Hcl => false,
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
            Self::Zig
            | Self::Lua
            | Self::Scala
            | Self::Solidity
            | Self::Elixir
            | Self::Erlang
            | Self::RLang
            | Self::Hcl => false,
        }
    }

    /// Array/object/collection literal kinds that commonly hold embedded
    /// fixture or mock datasets. Scoped to the languages where huge inline data
    /// dumps actually happen; other languages return false.
    fn is_data_literal(self, kind: &str) -> bool {
        match self {
            Self::JavaScript | Self::TypeScript | Self::Tsx => {
                matches!(kind, "array" | "object")
            }
            Self::Python => matches!(kind, "list" | "dictionary" | "set" | "tuple"),
            _ => false,
        }
    }
}

pub(crate) struct CanonicalFunction {
    pub profile: &'static str,
    pub line: usize,
    pub canonical: String,
    pub original: String,
}

pub(crate) fn canonical_functions(
    path: &Path,
    source: &str,
    minimum_lines: usize,
) -> Vec<CanonicalFunction> {
    let Some(profile) = LanguageProfile::from_path(path) else {
        return Vec::new();
    };
    let mut parser = Parser::new();
    if parser.set_language(&profile.language()).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser
        .parse(source, None)
        .filter(|tree| !tree.root_node().has_error())
    else {
        return Vec::new();
    };
    let mut functions = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if is_clone_scope(node.kind())
            && node.end_position().row + 1 >= node.start_position().row + minimum_lines
        {
            let mut identifiers = HashMap::new();
            let mut canonical = String::new();
            canonicalize_node(node, source, &mut identifiers, &mut canonical);
            functions.push(CanonicalFunction {
                profile: profile.family(),
                line: node.start_position().row + 1,
                canonical,
                original: node_text(node, source)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            });
        } else {
            stack.extend(
                (0..node.named_child_count()).filter_map(|index| node.named_child(index as u32)),
            );
        }
    }
    functions
}

fn is_clone_scope(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "function_definition"
            | "function_item"
            | "method"
            | "method_declaration"
            | "method_definition"
            | "constructor_declaration"
    )
}

fn canonicalize_node(
    node: Node<'_>,
    source: &str,
    identifiers: &mut HashMap<String, usize>,
    output: &mut String,
) {
    if node.kind().contains("comment") {
        return;
    }
    output.push('(');
    output.push_str(node.kind());
    if node.child_count() == 0 {
        output.push(':');
        let text = node_text(node, source);
        if is_local_identifier(node) {
            let next = identifiers.len();
            let id = *identifiers.entry(text.to_owned()).or_insert(next);
            output.push('v');
            output.push_str(&id.to_string());
        } else {
            output.push_str(text);
        }
    } else {
        for index in 0..node.child_count() {
            if let Some(child) = node.child(index as u32) {
                canonicalize_node(child, source, identifiers, output);
            }
        }
    }
    output.push(')');
}

fn is_local_identifier(node: Node<'_>) -> bool {
    if node.kind() != "identifier" && node.kind() != "simple_identifier" {
        return false;
    }
    let Some(parent) = node.parent() else {
        return true;
    };
    for field in ["field", "function", "method", "name", "property"] {
        if parent.child_by_field_name(field) == Some(node)
            && matches!(field, "field" | "function" | "method" | "property")
        {
            return false;
        }
    }
    true
}

/// Blank every byte outside `regions` (non-newline → space, newline kept) so
/// the returned source parses as the embedded language while row and column
/// positions still map back to the original file.
fn mask_outside(source: &str, regions: &[(usize, usize)]) -> String {
    let bytes = source.as_bytes();
    let mut out: Vec<u8> = bytes
        .iter()
        .map(|&byte| if byte == b'\n' { b'\n' } else { b' ' })
        .collect();
    for &(start, end) in regions {
        if let (Some(dst), Some(src)) = (out.get_mut(start..end), bytes.get(start..end)) {
            dst.copy_from_slice(src);
        }
    }
    String::from_utf8(out).unwrap_or_default()
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
        confidence: rules::metadata(rule_id)
            .map(|rule| rule.confidence)
            .unwrap_or(EvidenceConfidence::Heuristic),
        blocking: false,
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
