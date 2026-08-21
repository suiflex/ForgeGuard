# Roadmap

## 0.1 — MVP

- Rust CLI and core library.
- Codex, Claude Code, Cursor, OpenCode, and Antigravity skills/rules.
- Project detection and quality command execution.
- Initial algorithm, database, concurrency, and duplication rules.
- JSON reporting and CI.

## 0.2 — Structural analysis

- Tree-sitter parsers for JavaScript, TypeScript, TSX, Rust, Go, Python, Java, Kotlin, C#, C, C++, Ruby, PHP, Swift, Dart, and Shell.
- Structural loop and call-site context.
- Conservative N+1 and query-in-loop detection.
- Consolidated cross-agent engineering skill and current Codex skill layout.

## 0.3–0.7 — Agent enforcement and persistence

- Claude Code, Codex, Cursor, and Antigravity Stop hooks.
- Silent-success and bounded-failure token protocol.
- Changed-worktree cache and local full reports.
- Command timeout and hook installation diagnostics.
- Session-scoped objectives, todo/confidence state, deterministic hill-climbability, bounded retry/no-progress handling, context restoration, declared-scope warnings, and default-on auto-poke verification phases.
- Cross-platform release binaries and checksum-verifying installers.

## 0.8 — Policy, semantic evidence, and security hardening

- Config v2 per-rule enablement, severity, and blocking policy with v1 compatibility.
- Bounded database/network provenance packs for JavaScript/TypeScript, Python, Rust, and Go.
- Published capability matrix, labeled precision/recall gate, Type-2 clone evidence, and SARIF 2.1.0.
- `cargo-deny`, `cargo-audit`, SHA-pinned actions, and Dependabot.

## 0.9–0.13 — New-code and supply-chain quality

- Changed-function complexity, changed-line coverage, secret detection, access-control hotspots, sink-aware taint flow, weak crypto/TLS, unsafe deserialization, XSS/path sinks, and swallowed-exception evidence.
- Conservative same-operation duplicate hints with shared-domain evidence.
- Reconciled ecosystem command presets for existing configs, dependency-change gating, content-fingerprint caching, persisted SBOM JSON, and configured-check SARIF results.

## Next — Database and AI packs

- Compiler-backed type/data-flow and complexity analysis.
- MCP schema verification.
- Query-plan evidence collection.
- AI tool schema, agent-loop, RAG, token, and evaluation gates.
- Benchmark runner and before/after reports.
- Git pre-commit/pre-push adapters.
- Changed-function test mapping.
