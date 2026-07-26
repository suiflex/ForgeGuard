# Architecture

ForgeGuard is split into two crates:

- `forgeguard-core`: project detection, configuration, initialization, scanning, command execution, health checks, and reporting.
- `forgeguard`: the CLI interface and exit-code policy.

Rules, skills, hooks, and configured repository commands are language-agnostic. Parser-backed findings are an additional evidence layer, not the boundary of ForgeGuard support.

## Execution flow

```text
request
  → project detection
  → compact rule + conditional skill
  → implementation cycle
  → Stop hook
  → changed-source scanning
  → configured quality commands
  → mode-aware gate decision
  → silent pass or bounded agent feedback
```

Full reports stay local under `.forgeguard/reports/`; hook cache lives under `.forgeguard/cache/`. Both paths are excluded from worktree fingerprints. Cache lookup is `O(1)` after an `O(total changed bytes)` streaming fingerprint and prevents unchanged gates from rerunning.

## Token contract

- Success: empty stdout for Codex/Claude; minimal JSON for Cursor and Antigravity protocols.
- Failure: maximum 2,000 characters, five findings, and one evidence line per failed command.
- Full command output never enters hook feedback; it remains in the local JSON report.
- Always-on rules contain only the six-stage workflow. Domain references load conditionally.

## Scanner design

Tree-sitter provides syntax and loop scope for JavaScript, TypeScript, TSX, Rust, Go, Python, Java, Kotlin, C#, C, C++, Ruby, PHP, Swift, Dart, and Shell. Call classification stays conservative and only examines actual call nodes. Files with syntax errors receive `FG-PARSE-001` and no structural claims.

Unsupported languages receive exact duplicate-block checks only; standalone SQL files also receive the `SELECT *` check. ForgeGuard deliberately reports evidence rather than pretending to prove whole-program complexity or runtime cost.

Source walking honors Git ignore files and excludes generated or dependency directories. Files larger than the configured limit are skipped.

## Gate policy

- Required command failures always block.
- Guard mode blocks error-level findings.
- Strict mode blocks warning- and error-level findings.
- Lite mode reports static findings without blocking.
- Each configured command has a timeout; timeout is a failed check.
- Duplicate rule/path/line findings collapse before rendering.

## Hook policy

- Hooks merge into existing JSON without replacing unrelated settings.
- Global hooks pass silently outside initialized repositories.
- Claude returns `decision: block`; Cursor returns `followup_message`; Codex surfaces a deterministic `systemMessage`; Antigravity returns `decision: continue`.
- Repeated blocked stops with an unchanged fingerprint pass once to prevent infinite loops.
- Agent hook trust remains controlled by each host application.
- OpenCode uses the shared `AGENTS.md` and Agent Skill standards. Its current lifecycle cannot reliably prevent the loop from stopping, so ForgeGuard does not install a misleading pseudo-blocking hook.

## Extension direction

Future packs should implement a stable rule trait and produce the same `Finding` model. Planned work includes SARIF output, deeper data-flow rules, MCP schema verification, and query-plan capture.
