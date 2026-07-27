# ForgeGuard

**Token-efficient engineering discipline for AI coding agents.**

ForgeGuard is a language- and framework-agnostic quality layer for Codex, Claude Code, Cursor, OpenCode, Antigravity, and agents that support `AGENTS.md` or Agent Skills. It applies to backend services, web frontends, native and cross-platform mobile apps, AI systems, data code, automation scripts, CLIs, and infrastructure code.

Its goal is not merely code that runs. It makes agents work through:

```text
inspect → design → implement → test → review → verify
```

ForgeGuard pushes generated code toward the discipline expected from a top-tier software engineering team: correct boundaries, reusable behavior, efficient algorithms and queries, explicit failure handling, focused tests, reviewed diffs, and claims backed by executed evidence.

Universal rules, skills, hooks, and repository commands work with any language. Deep structural rules use Tree-sitter for JavaScript, TypeScript, TSX, Rust, Go, Python, Java, Kotlin, C#, C, C++, Ruby, PHP, Swift, Dart, and Shell. Every language still receives workflow enforcement and configured quality commands; common non-parser source types also receive exact duplicate checks. Findings remain review evidence, not a substitute for tests, profilers, or query plans.

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
- Passing hooks add no model context in Codex/Claude; Cursor and Antigravity receive only their required minimal protocol response.
- Blocking feedback is deduplicated and capped at 2,000 characters and five findings.
- Full evidence stays local in `.forgeguard/reports/latest.json`.
- Unchanged worktrees use a local fingerprint cache instead of rerunning the gate.

A blocked gate may cause the host agent to use another turn to fix real failures. ForgeGuard spends model usage only indirectly when additional corrective work is necessary.

## MVP capabilities

- Rust single-binary CLI.
- Lightweight source detection across common backend, frontend, mobile, systems, data, script, and infrastructure languages.
- AST-backed loop and call-site analysis for 16 common language profiles.
- Codex `AGENTS.md` plus project skills.
- Claude Code `CLAUDE.md` plus project skills.
- Cursor always-on rule plus shared project skills.
- OpenCode `AGENTS.md` plus shared project skills.
- Antigravity rule, shared project skill, and native `Stop` hook.
- Token-efficient `Stop` hooks: silent pass, bounded failure feedback, diff cache, and local full report.
- One `forgeguard-engineering` skill with conditional clean-code, algorithm, backend, frontend, mobile, database, AI, and testing references.
- Static rules for nested iteration, repeated linear lookup, sorting in loops, database I/O in loops, external requests in loops, unbounded fan-out, `SELECT *`, and potential duplicated blocks.
- Automatic formatter, linter, type-check, test, and build command discovery.
- `default`, `lite`, and `strict` operating modes with project and global persistence.
- Interactive mode selection during `forgeguard init` and via `forgeguard mode`.
- Human-readable and JSON reports.

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
```

Produce bounded agent-facing output:

```bash
forgeguard gate --changed --output compact
```

## Generated project structure

```text
your-project/
├── .forgeguard/config.toml
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
version = 1
mode = "default"

[project]
name = "example-service"

[scan]
enabled = true
max_file_bytes = 1000000
include_tests = false
extra_excludes = ["generated/"]
duplicate_block_lines = 6

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

- `default`: token-friendly default for new installs. Static findings are reported, but only failed required commands block.
- `lite`: report-only mode for baselining or cleanup work. Static findings do not block.
- `strict`: strong guard mode. Failed required commands and error-level deterministic findings block.

Older configs with `mode = "guard"` still load as `strict` for compatibility.

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
| `forgeguard doctor` | Verify configuration, Git, and required local tools. |
| `forgeguard mode` | Check or change project/global operating mode. |
| `forgeguard gate` | Run static rules and configured quality commands. |
| `forgeguard review` | Scan Git-changed files without running commands. |
| `forgeguard hook stop` | Internal token-efficient lifecycle adapter for supported agents. |

## Agent contract

```text
Developer
  ↓
Claude Code / Codex / Cursor / OpenCode / Antigravity
  ↓
ForgeGuard compact rules + conditional skill + Stop hook
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

1. **Deterministic:** failed tests, database I/O inside a proven loop, or failed build commands can block.
2. **Heuristic:** possible nested scans or duplicate code provide evidence and require review.
3. **Evidence-based:** performance and quality improvements require benchmarks, query plans, profiler output, or evaluation reports.

See [Rule catalog](docs/RULES.md) and [Architecture](docs/ARCHITECTURE.md).

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace
```

Maintainers publish binaries by pushing a tag matching the workspace version, for example `v0.2.0`. The release workflow verifies the tag, tests the repository and installers, builds six native platform archives, generates checksums, and publishes the GitHub Release automatically.

## Security

ForgeGuard executes commands declared in a repository's `.forgeguard/config.toml`. Review configuration and agent hooks before trusting an untrusted repository. See [SECURITY.md](SECURITY.md).

## License

MIT
