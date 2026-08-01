use serde::Serialize;

use crate::model::{EvidenceConfidence, Severity};

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RuleMetadata {
    pub id: &'static str,
    pub title: &'static str,
    pub default_severity: Severity,
    pub confidence: EvidenceConfidence,
}

pub const RULES: &[RuleMetadata] = &[
    rule(
        "FG-ALG-001",
        "Potential nested iteration",
        Severity::Warning,
        EvidenceConfidence::Structural,
    ),
    rule(
        "FG-ALG-002",
        "Repeated linear lookup inside iteration",
        Severity::Warning,
        EvidenceConfidence::Heuristic,
    ),
    rule(
        "FG-ALG-003",
        "Sorting inside iteration",
        Severity::Warning,
        EvidenceConfidence::Structural,
    ),
    rule(
        "FG-DB-001",
        "Database operation inside iteration",
        Severity::Error,
        EvidenceConfidence::Semantic,
    ),
    rule(
        "FG-DB-002",
        "Potential database operation inside iteration",
        Severity::Info,
        EvidenceConfidence::Heuristic,
    ),
    rule(
        "FG-DB-005",
        "Potential unnecessary SELECT *",
        Severity::Warning,
        EvidenceConfidence::Heuristic,
    ),
    rule(
        "FG-NET-001",
        "External request inside iteration",
        Severity::Warning,
        EvidenceConfidence::Semantic,
    ),
    rule(
        "FG-NET-002",
        "Potential external request inside iteration",
        Severity::Info,
        EvidenceConfidence::Heuristic,
    ),
    rule(
        "FG-CON-001",
        "Potential unbounded parallel execution",
        Severity::Warning,
        EvidenceConfidence::Structural,
    ),
    rule(
        "FG-DRY-001",
        "Potential duplicated implementation",
        Severity::Info,
        EvidenceConfidence::Structural,
    ),
    rule(
        "FG-DRY-002",
        "Potential renamed duplicated implementation",
        Severity::Info,
        EvidenceConfidence::Heuristic,
    ),
    rule(
        "FG-ARCH-001",
        "Large inline data literal",
        Severity::Warning,
        EvidenceConfidence::Structural,
    ),
    rule(
        "FG-PARSE-001",
        "Structural analysis skipped",
        Severity::Info,
        EvidenceConfidence::Deterministic,
    ),
];

const fn rule(
    id: &'static str,
    title: &'static str,
    default_severity: Severity,
    confidence: EvidenceConfidence,
) -> RuleMetadata {
    RuleMetadata {
        id,
        title,
        default_severity,
        confidence,
    }
}

pub fn metadata(rule_id: &str) -> Option<&'static RuleMetadata> {
    RULES.iter().find(|rule| rule.id == rule_id)
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct LanguageCapability {
    pub language: &'static str,
    pub parser: bool,
    pub structural_rules: bool,
    pub semantic_pack: bool,
}

pub const LANGUAGE_CAPABILITIES: &[LanguageCapability] = &[
    capability("JavaScript/TypeScript", true),
    capability("Python", true),
    capability("Rust", true),
    capability("Go", true),
    structural("Java/Kotlin"),
    structural("C#"),
    structural("C/C++"),
    structural("Ruby"),
    structural("PHP"),
    structural("Swift"),
    structural("Dart"),
    structural("Shell"),
    structural("Zig"),
    structural("Lua"),
    structural("Scala"),
    structural("Solidity"),
    structural("R"),
    parser_only("Elixir/Erlang"),
    parser_only("HCL/Terraform"),
];

const fn capability(language: &'static str, semantic_pack: bool) -> LanguageCapability {
    LanguageCapability {
        language,
        parser: true,
        structural_rules: true,
        semantic_pack,
    }
}

const fn structural(language: &'static str) -> LanguageCapability {
    capability(language, false)
}

const fn parser_only(language: &'static str) -> LanguageCapability {
    LanguageCapability {
        language,
        parser: true,
        structural_rules: false,
        semantic_pack: false,
    }
}
