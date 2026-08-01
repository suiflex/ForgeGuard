use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use crate::{
    model::{EvidenceConfidence, Finding, Severity},
    rules,
    scanner::canonical_functions,
};

#[derive(Debug, Clone)]
struct Occurrence {
    path: PathBuf,
    line: usize,
    snippet: String,
}

pub fn scan_duplicate_blocks(
    root: &Path,
    files: &[PathBuf],
    block_lines: usize,
    focus_paths: Option<&BTreeSet<PathBuf>>,
) -> Vec<Finding> {
    if block_lines < 4 {
        return Vec::new();
    }

    let mut blocks = BTreeMap::<u64, Vec<Occurrence>>::new();
    for path in files {
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let lines: Vec<(usize, String)> = source
            .lines()
            .enumerate()
            .filter_map(|(index, line)| normalize_line(line).map(|value| (index + 1, value)))
            .collect();

        // forgeguard: allow FG-ALG-001 -- bounded window per file; O(total lines × block_lines)
        for window in lines.windows(block_lines) {
            let normalized = window
                .iter()
                .map(|(_, line)| line.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if normalized.len() < 160 || distinct_line_count(window) < 3 {
                continue;
            }
            let mut hasher = DefaultHasher::new();
            normalized.hash(&mut hasher);
            blocks.entry(hasher.finish()).or_default().push(Occurrence {
                path: path.strip_prefix(root).unwrap_or(path).to_path_buf(),
                line: window[0].0,
                snippet: normalized,
            });
        }
    }

    let mut findings = Vec::new();
    let mut reported_pairs = BTreeSet::new();
    for occurrences in blocks.into_values() {
        let Some(first) = occurrences.first() else {
            continue;
        };
        // forgeguard: allow FG-ALG-001 -- disjoint hash groups; total traversal is O(occurrences)
        for duplicate in occurrences.iter().skip(1) {
            if first.path == duplicate.path || first.snippet != duplicate.snippet {
                continue;
            }
            let (target, other) = match focus_paths {
                Some(focus) if focus.contains(&duplicate.path) => (duplicate, first),
                Some(focus) if focus.contains(&first.path) => (first, duplicate),
                Some(_) => continue,
                None => (duplicate, first),
            };
            let pair = if first.path <= duplicate.path {
                (first.path.clone(), duplicate.path.clone())
            } else {
                (duplicate.path.clone(), first.path.clone())
            };
            if !reported_pairs.insert(pair) {
                continue;
            }
            findings.push(Finding {
                rule_id: "FG-DRY-001".to_owned(),
                title: "Potential duplicated implementation".to_owned(),
                severity: Severity::Info,
                confidence: rules::metadata("FG-DRY-001")
                    .map(|rule| rule.confidence)
                    .unwrap_or(EvidenceConfidence::Heuristic),
                blocking: false,
                path: target.path.clone(),
                line: target.line,
                evidence: format!(
                    "A similar {}-line block also appears at {}:{}",
                    block_lines,
                    other.path.display(),
                    other.line
                ),
                recommendation: "Verify that both blocks represent the same responsibility and lifecycle. Extract the narrowest valid shared abstraction only when they should evolve together.".to_owned(),
            });
        }
    }
    findings.extend(scan_renamed_duplicate_blocks(
        root,
        files,
        block_lines,
        focus_paths,
    ));
    findings
}

#[derive(Debug, Clone)]
struct CloneOccurrence {
    path: PathBuf,
    line: usize,
    canonical: String,
    original: String,
}

fn scan_renamed_duplicate_blocks(
    root: &Path,
    files: &[PathBuf],
    minimum_lines: usize,
    focus_paths: Option<&BTreeSet<PathBuf>>,
) -> Vec<Finding> {
    let mut groups = BTreeMap::<u64, Vec<CloneOccurrence>>::new();
    for path in files {
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        // forgeguard: allow FG-ALG-001 -- functions are disjoint per file; total traversal is O(AST nodes)
        for function in canonical_functions(path, &source, minimum_lines) {
            let mut hasher = DefaultHasher::new();
            function.profile.hash(&mut hasher);
            function.canonical.hash(&mut hasher);
            groups
                .entry(hasher.finish())
                .or_default()
                .push(CloneOccurrence {
                    path: path.strip_prefix(root).unwrap_or(path).to_path_buf(),
                    line: function.line,
                    canonical: function.canonical,
                    original: function.original,
                });
        }
    }

    let mut findings = Vec::new();
    let mut reported_pairs = BTreeSet::new();
    for occurrences in groups.into_values() {
        let Some(first) = occurrences.first() else {
            continue;
        };
        // forgeguard: allow FG-ALG-001 -- hash groups are disjoint; total traversal is O(occurrences)
        for duplicate in occurrences.iter().skip(1) {
            if first.path == duplicate.path
                || first.canonical != duplicate.canonical
                || first.original == duplicate.original
            {
                continue;
            }
            let (target, other) = match focus_paths {
                Some(focus) if focus.contains(&duplicate.path) => (duplicate, first),
                Some(focus) if focus.contains(&first.path) => (first, duplicate),
                Some(_) => continue,
                None => (duplicate, first),
            };
            let pair = if first.path <= duplicate.path {
                (first.path.clone(), duplicate.path.clone())
            } else {
                (duplicate.path.clone(), first.path.clone())
            };
            if !reported_pairs.insert(pair) {
                continue;
            }
            findings.push(Finding {
                rule_id: "FG-DRY-002".to_owned(),
                title: "Potential renamed duplicated implementation".to_owned(),
                severity: Severity::Info,
                confidence: rules::metadata("FG-DRY-002")
                    .map(|rule| rule.confidence)
                    .unwrap_or(EvidenceConfidence::Heuristic),
                blocking: false,
                path: target.path.clone(),
                line: target.line,
                evidence: format!(
                    "An alpha-renamed function also appears at {}:{}",
                    other.path.display(),
                    other.line
                ),
                recommendation: "Confirm both functions share responsibility before extracting a common implementation.".to_owned(),
            });
        }
    }
    findings
}

fn normalize_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("import ")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("package ")
    {
        return None;
    }
    Some(trimmed.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn distinct_line_count(window: &[(usize, String)]) -> usize {
    window
        .iter()
        .map(|(_, line)| line)
        .collect::<BTreeSet<_>>()
        .len()
}
