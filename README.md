# ForgeGuard — engineering discipline for AI coding agents

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/logo-dark.svg">
    <img src="assets/brand/logo-light.svg" alt="ForgeGuard" width="360">
  </picture>
</p>

<p align="center">
  <strong>Token-efficient quality layer for AI coding agents.<br>Codex · Claude Code · Cursor · OpenCode · Antigravity — one Rust binary, no LLM calls.</strong>
</p>

ForgeGuard is a language- and framework-agnostic quality layer for Codex, Claude Code, Cursor, OpenCode, Antigravity, and agents that support `AGENTS.md` or Agent Skills. It applies to backend services, web frontends, native and cross-platform mobile apps, AI systems, data code, automation scripts, CLIs, and infrastructure code.

Its goal is not merely code that runs. It makes agents work through:

```text
inspect → design → implement → test → review → verify
```

ForgeGuard pushes generated code toward the discipline expected from a top-tier software engineering team: correct boundaries, reusable behavior, efficient algorithms and queries, explicit failure handling, focused tests, reviewed diffs, and claims backed by executed evidence.

Workflow supervision and configured repository commands work with any language. Parser, structural-rule, and semantic-pack coverage are separate capabilities: Tree-sitter supplies syntax evidence across the listed profiles, while bounded import/binding and local-wrapper provenance is currently available for JavaScript/TypeScript, Python, Rust, and Go. Run `forgeguard capabilities` for the exact matrix. ForgeGuard is an AST-assisted quality scanner, not a whole-program semantic analyzer; findings remain review evidence, not a substitute for compilers, tests, profilers, or query plans.

## Why ForgeGuard

Model price and benchmark rank do not guarantee clean engineering. AI coding agents can produce working code while still introducing:

- duplicated business logic and components;
- repeated linear lookup or accidental `O(n²)` behavior;
- database queries inside loops and N+1 patterns;
- unbounded parallel requests;
- weak test coverage or unexecuted verification;
- abstractions at the wrong scope;
- unsupported claims that code is clean, optimal, or production-ready.

ForgeGuard corrects weak implementation assumptions before coding, teaches the relevant trade-off concisely, and verifies the result with deterministic local tooling.

## Token and usage contract

ForgeGuard is designed not to drain user context or model limits:

- The hook runs a local Rust binary and repository commands; ForgeGuard itself makes no LLM or external API call.
- Always-on policy stays compact; detailed backend, frontend, mobile, database, algorithm, testing, and AI references load only when relevant.
- Passing completion hooks add no model context in Codex/Claude; objective reinjection caps objective text at 300 characters, and scope feedback appears only for an explicit out-of-scope path.
- Antigravity injects focus context only on the first model invocation of an execution.
- Blocking feedback is deduplicated and capped at 2,000 characters and five findings.
- Auto-poke is default-on and creates one host request per continuation; the generated limit is three and the hard cap is five.
- Full evidence stays local in `.forgeguard/reports/latest.json`.
- Unchanged worktrees use a local fingerprint cache instead of rerunning the gate.

A blocked gate may cause the host agent to use another turn to fix real failures. ForgeGuard spends model usage only indirectly when additional corrective work is necessary.

## MVP capabilities

- Rust single-binary CLI.
- Lightweight source detection across common backend, frontend, mobile, systems, data, script, and infrastructure languages.
- AST-backed loop and call-site analysis across the published parser capability matrix.
- Bounded database/network provenance packs for JavaScript/TypeScript, Python, Rust, and Go.
- Codex `AGENTS.md` plus project skills.
- Claude Code `CLAUDE.md` plus project skills.
- Cursor always-on rule plus shared project skills.
- OpenCode `AGENTS.md` plus shared project skills.
- Antigravity rule, shared project skill, and native `Stop` hook.
- Token-efficient `Stop` hooks: silent pass, bounded failure feedback, diff cache, and local full report.
- Session-scoped objective, todo, confidence, hill-climbability, auto-poke, resume, and scope-drift state.
- One `forgeguard-engineering` skill with conditional clean-code, algorithm, backend, frontend, mobile, database, AI, and testing references.
- Static rules for nested iteration, repeated linear lookup, sorting in loops, database I/O in loops, external requests in loops, unbounded fan-out, `SELECT *`, and potential duplicated blocks.
- Automatic formatter, linter, type-check, test, and build command discovery.
- `default`, `lite`, and `strict` operating modes with project and global persistence.
- Committed finding baselines for adopting strict gates without accepting new debt.
- Interactive mode selection during `forgeguard init` and via `forgeguard mode`.
- Human-readable, JSON, and SARIF 2.1.0 reports.

## Install

No Rust, Cargo, Node.js, or Python required. The installer downloads the correct GitHub Release artifact for the current OS and CPU, verifies its SHA-256 checksum, adds ForgeGuard to the user `PATH`, and installs global rules, skills, and supported hooks.

### Linux and macOS

```bash
curl -fsSL https://raw.githubusercontent.com/suiflex/ForgeGuard/main/install.sh | sh
```

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/suiflex/ForgeGuard/main/install.ps1 | iex
```

Restart the terminal after installation. Then initialize any repository:

```bash
cd your-project
forgeguard init --agent all
forgeguard doctor
```

Installers use GitHub Release binaries produced for Linux, macOS, and Windows on x86-64 and ARM64. Advanced users building unreleased source can still use:

```bash
cargo install --path crates/forgeguard-cli
```

## Updating

`forgeguard update` only *checks* whether a newer release exists and prints a one-line
notice; it never installs anything. Upgrading is re-running the installer, then refreshing the
assets ForgeGuard already wrote.

### Upgrade the binary and global assets

Re-run the one-line installer. It downloads the latest release binary for the current OS and
CPU, verifies its SHA-256 checksum, and updates the user `PATH`.

```bash
curl -fsSL https://raw.githubusercontent.com/suiflex/ForgeGuard/main/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/suiflex/ForgeGuard/main/install.ps1 | iex
```

The installer refreshes the binary but does not overwrite global rules or skills that already
exist. To re-apply the newer bundled global skills, policies, and hooks over an existing global
install, force it:

```bash
forgeguard init --global --agent all --force
```

Source builds upgrade with:

```bash
cargo install --path crates/forgeguard-cli
```

Check the installed version at any time:

```bash
forgeguard --version
```

Check whether a newer release exists (checks only; installs nothing):

```bash
forgeguard update
```

### Refresh an already-initialized project

New releases can ship updated policy files and engineering skills. A plain `forgeguard init`
never overwrites existing ForgeGuard files, so re-initialize with `--force` to pull the newer
bundle into a repository that was set up by an older version:

```bash
cd your-project
forgeguard init --agent all --force
forgeguard doctor
```

`--force` overwrites the ForgeGuard-owned policy and skill files and prunes obsolete
role-skill directories.

> **Warning:** `--force` also regenerates `.forgeguard/config.toml` from detection defaults,
> resetting any custom commands and operating mode. If you customized the config, back it up
> first, or re-apply the mode afterward with `forgeguard mode default|lite|strict`. A committed
> `.forgeguard/baseline.json` is not touched.

## Quick start

```bash
# Only needed after a source build; one-line installers already do this.
forgeguard init --global --agent all

cd your-project
forgeguard detect
forgeguard init --agent all
forgeguard doctor
forgeguard gate
```

If `forgeguard` is not found immediately after installation, restart the terminal so its updated user `PATH` is loaded. `forgeguard doctor` then explains missing project configuration, Git, hooks, or repository tools in plain command-level output.

Review only files changed in Git:

```bash
forgeguard review
```

Run all static rules but skip repository commands:

```bash
forgeguard gate --no-run
```

Produce machine-readable output:

```bash
forgeguard gate --json
forgeguard review --output sarif > forgeguard.sarif
```

Produce bounded agent-facing output:

```bash
forgeguard gate --changed --output compact
```

Record current static findings, then report only new findings:

```bash
forgeguard baseline create
git add .forgeguard/baseline.json
forgeguard gate
```

Replace a stale baseline after reviewing current findings:

```bash
forgeguard baseline create --force
```

## Generated project structure

```text
your-project/
├── .forgeguard/config.toml
├── .forgeguard/baseline.json  # after `forgeguard baseline create`
├── .forgeguard/.gitignore
├── AGENTS.md
├── CLAUDE.md
├── .codex/hooks.json
├── .claude/settings.json
├── .cursor/hooks.json
├── .cursor/rules/forgeguard.mdc
├── .agents/hooks.json
├── .agents/rules/forgeguard.md
├── .agents/skills/forgeguard-engineering/
│   ├── SKILL.md
│   └── references/
└── .claude/skills/forgeguard-engineering/
    ├── SKILL.md
    └── references/
```

Existing policy and skill files are not overwritten unless `--force` is supplied. Hook installation merges one ForgeGuard entry into existing JSON and preserves unrelated settings. Global installation writes ForgeGuard-owned skills, compact policies, and hook entries for selected agents. `--force` also removes obsolete ForgeGuard-owned role-skill directories without touching unrelated skills.

When a project `.gitignore` already exists, `forgeguard init` appends the generated directories for
the selected agents (`.codex/`, `.claude/`, `.cursor/`, and/or `.agents/`). It preserves existing
patterns, avoids duplicate entries, and does not create a root `.gitignore`.

## Agent support

| Agent | Rules | Skill | Automatic completion gate |
|---|---|---|---|
| Codex | `AGENTS.md` | `.agents/skills` | `Stop` hook |
| Claude Code | `CLAUDE.md` | `.claude/skills` | `Stop` hook |
| Cursor | `.cursor/rules` | `.agents/skills` | `stop` hook |
| Antigravity | `.agents/rules` | `.agents/skills` | Native `Stop` hook |
| OpenCode | `AGENTS.md` | `.agents/skills` | Policy-enforced gate |

[OpenCode officially discovers](https://opencode.ai/docs/skills) both `AGENTS.md` and `.agents/skills`. Its current plugin lifecycle exposes `session.idle` only after the agent loop stops, so ForgeGuard does not claim a reliable blocking `Stop` hook there. The compact policy requires `forgeguard gate --changed --output compact` before completion. [Antigravity provides a native blocking `Stop` protocol](https://antigravity.google/docs/hooks), so failures automatically return the agent to its execution loop.

Other agents receive the universal CLI gate immediately. Agents that understand the emerging `AGENTS.md` and Agent Skills conventions also receive ForgeGuard guidance without a dedicated adapter.

## Configuration

Example:

```toml
version = 2
mode = "default"

[project]
name = "example-service"

[scan]
enabled = true
max_file_bytes = 1000000
include_tests = false
extra_excludes = ["generated/"]
duplicate_block_lines = 6

[policies]
warnings_block = false

[rules.FG-NET-001]
enabled = true
severity = "warning"
block = true

[focus]
enabled = true
max_retries = 3
no_progress_limit = 2
auto_poke = true
max_auto_pokes = 3
min_confidence = 80
min_hill_climbability = 80

[[commands]]
name = "lint"
command = "pnpm lint"
required = true
enabled = true

[[commands]]
name = "test"
command = "pnpm test"
required = true
enabled = true
timeout_seconds = 600
```

### Modes

ForgeGuard supports three operating modes:

- `default`: failed required commands block; static findings block only when a rule sets `block = true`.
- `lite`: report-only mode for baselining or cleanup work. Static findings do not block.
- `strict`: failed required commands plus Warning- and Error-level static findings block. Info remains evidence.

Version 1 configs preserve the old Strict behavior that blocks only Error findings. Run `forgeguard config migrate` to opt into version 2 policy; older `mode = "guard"` values still load as `strict`.

Per-rule `enabled`, `severity`, and `block` override global policy. `Lite` always keeps static findings nonblocking. Required command failures block in every mode.

Focus state and scope checks are local and make no LLM calls. `max_retries` bounds corrective turns; `no_progress_limit` stops a session that produces no repository or task-state progress. `auto_poke` is enabled automatically by `forgeguard init`; each continuation creates a new host request and consumes model tokens, so `max_auto_pokes` defaults to three and has a hard cap of five. Set `auto_poke = false` only when manually opting out. Pending todos, confidence below `min_confidence`, or goal-contract completeness below `min_hill_climbability` keep the task active. Hill-climbability is a deterministic 0–100 completeness score: metric, baseline, target, guardrail, and verification contribute 20 points each. ForgeGuard does not guess these fields from prose. `forgeguard task start --semantic` only asks a supported host to use its native goal evaluator.

See [Focus, auto-poke, and hill-climbability](docs/FOCUS.md) for the lifecycle, headless behavior, token bounds, upgrade path, and examples.

Example measurable task:

```bash
forgeguard task start --session "$SESSION" \
  --objective "Reduce /search latency without regressions" \
  --metric "p95 latency /search" --baseline "900 ms" --target "below 300 ms" \
  --guardrail "error rate does not increase" --verification "regression tests pass" \
  --todo "measure baseline" --todo "optimize endpoint"
forgeguard task todo --session "$SESSION" --done 1
forgeguard task ready --session "$SESSION" --confidence 90 --evidence "benchmark: p95 284 ms"
```

Set project mode:

```bash
forgeguard mode default
forgeguard mode lite
forgeguard mode strict
```

Set user-level/global mode:

```bash
forgeguard mode strict --global
```

Inspect mode as JSON:

```bash
forgeguard mode --json
forgeguard mode --global --json
```

When run in a terminal without an explicit mode, `forgeguard mode` opens the same interactive mode picker used by `forgeguard init`. Non-TTY calls and `--json` never prompt, so scripts and CI do not hang.

## Commands

| Command | Purpose |
|---|---|
| `forgeguard init` | Install project configuration and agent skills. Use `--global` for user-level skills. |
| `forgeguard detect` | Detect languages, frameworks, database tools, tests, and commands. |
| `forgeguard capabilities` | Show workflow, parser, structural-rule, and semantic-pack coverage. |
| `forgeguard doctor` | Verify configuration, Git, and required local tools. |
| `forgeguard mode` | Check or change project/global operating mode. |
| `forgeguard config migrate` | Upgrade config v1 to v2 without resetting commands or focus settings. |
| `forgeguard gate` | Run static rules and configured quality commands. |
| `forgeguard review` | Scan Git-changed files without running commands. |
| `forgeguard baseline create` | Record current static findings so gates report only new findings. |
| `forgeguard task start` | Register objective, goal metrics, todos, and optional scope prefixes. |
| `forgeguard task todo` | Add todos or mark 1-based todo indexes complete. |
| `forgeguard task ready` | Submit exact evidence and optional model confidence before the completion gate. |
| `forgeguard task status` | Inspect session-scoped objective state. |
| `forgeguard hook stop/context/scope` | Internal lifecycle adapters for completion, objective restoration, and scope warnings. |

## Agent contract

```text
Developer
  ↓
Claude Code / Codex / Cursor / OpenCode / Antigravity
  ↓
ForgeGuard compact rules + conditional skill + Stop hook
  ↓
session objective + goal metric + todo state + confidence history + executed evidence
  ↓
Repository tools
  ↓
Code changes
  ↓
ForgeGuard changed-file quality gate
```

Agents must follow `inspect → design → implement → test → review → verify`. Detailed algorithm guidance loads only for relevant data paths. Full 13-point performance evidence is reserved for performance-critical work or explicit requests.

## Rule philosophy

ForgeGuard separates rules into:

1. **Deterministic/semantic:** failed commands and provenance-confirmed database/network calls can block.
2. **Structural/heuristic:** possible nested scans, receiver-name matches, and duplicate code provide review evidence with explicit confidence.
3. **Evidence-based:** performance and quality improvements require benchmarks, query plans, profiler output, or evaluation reports.

See [Rule catalog](docs/RULES.md) and [Architecture](docs/ARCHITECTURE.md).

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace
cargo deny check
cargo audit --deny warnings
```

Maintainers publish binaries by pushing a tag matching the workspace version, for example `v0.2.0`. The release workflow verifies the tag, tests the repository and installers, builds six native platform archives, generates checksums, and publishes the GitHub Release automatically.

## Security

ForgeGuard executes commands declared in a repository's `.forgeguard/config.toml`. Review configuration and agent hooks before trusting an untrusted repository. CI runs `cargo-deny` and `cargo-audit`; GitHub Actions are commit-SHA pinned and updated through Dependabot. See [SECURITY.md](SECURITY.md).

## License

MIT
