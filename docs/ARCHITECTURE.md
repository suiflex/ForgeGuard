# Architecture

ForgeGuard is split into two crates:

- `forgeguard-core`: project detection, configuration, initialization, scanning, command execution, health checks, and reporting.
- `forgeguard`: the CLI interface and exit-code policy.

Workflow rules, skills, hooks, and configured repository commands are language-agnostic. Parser, structural-rule, and semantic-pack support are separate capability tiers exposed by `forgeguard capabilities`.

## Execution flow

```text
request
  → project detection
  → compact rule + conditional skill
  → session-scoped objective and path prefixes
  → implementation cycle
  → Stop hook
  → changed-source scanning
  → committed baseline filtering
  → configured quality commands
  → mode-aware gate decision
  → silent pass or bounded agent feedback
```

Full reports stay local under `.forgeguard/reports/`; per-session task and hook state live under `.forgeguard/cache/`. Both paths are excluded from worktree fingerprints. Cache lookup is `O(1)` after an `O(total changed bytes)` streaming fingerprint and prevents unchanged gates from rerunning.

## Token contract

- Success: empty stdout for Codex/Claude; minimal JSON for Cursor and Antigravity protocols.
- Failure: maximum 2,000 characters, five findings, and one evidence line per failed command.
- Full command output never enters hook feedback; it remains in the local JSON report.
- Always-on rules contain only the six-stage workflow. Domain references load conditionally.

## Scanner design

Tree-sitter provides syntax, call nodes, loop scope, source locations, and code/comment separation across the parser matrix. JavaScript/TypeScript, Python, Rust, and Go add bounded import/binding provenance and fixed-point summaries for uniquely named local wrappers. Dynamic imports, reflection, macros, overload/type resolution, and runtime dispatch remain unresolved. Files with syntax errors receive `FG-PARSE-001` and no structural claims.

Parser-backed function scopes also receive alpha-renamed Type-2 clone evidence; unsupported languages retain exact duplicate-block checks. Standalone SQL files receive the `SELECT *` check. ForgeGuard deliberately reports evidence rather than pretending to prove whole-program complexity or runtime cost.

Source walking honors Git ignore files and excludes generated or dependency directories. Files larger than the configured limit are skipped.

## Gate policy

- Required command failures always block.
- Default mode blocks only explicitly configured static rules.
- Strict config v2 blocks warning- and error-level findings; config v1 retains error-only compatibility until migration.
- Lite mode reports static findings without blocking.
- Per-rule `enabled`, `severity`, and `block` overrides apply before gate status.
- Each configured command has a timeout; timeout is a failed check.
- Duplicate rule/path/line findings collapse before rendering.

## Hook policy

- Hooks merge into existing JSON without replacing unrelated settings.
- Global hooks pass silently outside initialized repositories.
- Claude and Codex return `decision: block`; Cursor returns `followup_message`; Antigravity returns `decision: continue`.
- Session start/resume/compaction hooks restore the active objective where the host protocol supports context injection; Antigravity injects it on the first model invocation of an execution.
- Pre-tool hooks warn when an edit path falls outside explicitly declared repository-relative prefixes. ForgeGuard never infers scope from prompt text.
- Blocked stops use per-session retry and no-progress limits. Exhaustion stops with an explicit blocker instead of claiming completion or looping forever.
- Default-on auto-poke returns the host agent to incomplete todos, low-confidence evidence, or fixed TODO/test/review/contract/final-verification phases. The same Stop-hook protocol works in headless mode where the host executes lifecycle hooks; each continuation is a new model request, and the hard limit is five. Configuration can explicitly opt out.
- Hill-climbability is computed without an LLM from five explicit goal-contract fields: metric, baseline, target, guardrail, and verification. Missing fields trigger reframing instead of speculative prose scoring.
- Deterministic focus state uses no LLM calls. Semantic evaluation is opt-in and delegated to native host goal mode; executed checks remain authoritative.
- Agent hook trust remains controlled by each host application.
- OpenCode uses the shared `AGENTS.md` and Agent Skill standards. Its current lifecycle cannot reliably prevent the loop from stopping, so ForgeGuard does not install a misleading pseudo-blocking hook.

## Extension direction

Rule metadata has one registry and every finding carries confidence plus effective blocking state. SARIF 2.1.0 maps the same report model to code-scanning results. Future work includes compiler-backed type/data-flow packs, MCP schema verification, query-plan capture, SBOM, artifact signing, and provenance attestations.
