# ForgeGuard

**Deterministic engineering quality enforcement for AI coding agents.**

ForgeGuard installs engineering skills for Codex and Claude Code, detects a repository's toolchain, scans changed code for high-risk implementation patterns, runs the project's real quality commands, and blocks unverified completion according to a configurable mode.

ForgeGuard is general-purpose. It is not tied to trading, finance, or a specific application domain.

> Status: early MVP. The current scanner is intentionally heuristic and evidence-oriented. It complements AST analyzers, linters, tests, profilers, and database query plans rather than replacing them.

## Why ForgeGuard

AI coding agents can produce working code while still introducing:

- duplicated business logic and components;
- repeated linear lookup or accidental `O(n²)` behavior;
- database queries inside loops and N+1 patterns;
- unbounded parallel requests;
- weak test coverage or unexecuted verification;
- abstractions at the wrong scope;
- unsupported claims that code is clean, optimal, or production-ready.

ForgeGuard turns engineering guidance into installable skills, static rules, executable quality checks, and clear gate results.

## MVP capabilities

- Rust single-binary CLI.
- Project detection for Rust, JavaScript/TypeScript, Go, Python, and basic JVM projects.
- Codex `AGENTS.md` plus project skills.
- Claude Code `CLAUDE.md` plus project skills.
- Built-in packs for clean code, algorithms, backend, frontend, database, general AI engineering, and testing.
- Static rules for nested iteration, repeated linear lookup, sorting in loops, database I/O in loops, external requests in loops, unbounded fan-out, `SELECT *`, and potential duplicated blocks.
- Automatic formatter, linter, type-check, test, and build command discovery.
- `lite`, `guard`, and `strict` enforcement modes.
- Human-readable and JSON reports.

## Install from source

```bash
cargo install --path crates/forgeguard-cli
```

## Quick start

```bash
# Optional: install the engineering skills globally for Codex and Claude.
forgeguard init --global --agent all

cd your-project
forgeguard detect
forgeguard init --agent all
forgeguard doctor
forgeguard gate
```

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

## Generated project structure

```text
your-project/
├── .forgeguard/config.toml
├── AGENTS.md
├── CLAUDE.md
├── .codex/skills/forgeguard-*/SKILL.md
└── .claude/skills/forgeguard-*/SKILL.md
```

Existing files are not overwritten unless `--force` is explicitly supplied. Global installation writes only ForgeGuard-owned skill directories plus `~/.codex/AGENTS.md` and `~/.claude/CLAUDE.md`; existing policy files are skipped by default.

## Configuration

Example:

```toml
version = 1
mode = "guard"

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
```

### Modes

- `lite`: reports findings but blocks only failed required commands.
- `guard`: blocks error-level deterministic findings and failed required commands.
- `strict`: also blocks warning-level findings until fixed or explicitly configured.

## Commands

| Command | Purpose |
|---|---|
| `forgeguard init` | Install project configuration and agent skills. Use `--global` for user-level skills. |
| `forgeguard detect` | Detect languages, frameworks, database tools, tests, and commands. |
| `forgeguard doctor` | Verify configuration, Git, and required local tools. |
| `forgeguard gate` | Run static rules and configured quality commands. |
| `forgeguard review` | Scan Git-changed files without running commands. |

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

## Security

ForgeGuard executes commands declared in a repository's `.forgeguard/config.toml`. Review configuration before running ForgeGuard on an untrusted repository. See [SECURITY.md](SECURITY.md).

## License

MIT
