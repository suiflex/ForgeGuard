# Architecture

ForgeGuard is split into two crates:

- `forgeguard-core`: project detection, configuration, initialization, scanning, command execution, health checks, and reporting.
- `forgeguard`: the CLI interface and exit-code policy.

## Execution flow

```text
request
  → project detection
  → role and skill installation
  → source scanning
  → configured quality commands
  → mode-aware gate decision
  → human or JSON report
```

## Scanner design

The MVP scanner uses bounded, line-oriented heuristics. It deliberately reports evidence rather than pretending to prove whole-program complexity. Rules that need syntax or semantic certainty will move to tree-sitter or language-server-backed analyzers in later releases.

Source walking honors Git ignore files and excludes generated or dependency directories. Files larger than the configured limit are skipped.

## Gate policy

- Required command failures always block.
- Guard mode blocks error-level findings.
- Strict mode blocks warning- and error-level findings.
- Lite mode reports static findings without blocking.

## Extension direction

Future packs should implement a stable rule trait and produce the same `Finding` model. Planned adapters include Git hooks, Claude hooks, Codex workflows, SARIF output, tree-sitter rules, MCP schema verification, query-plan capture, and benchmark runners.
