# ForgeGuard — stop AI agents from finishing before the work is verified

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/logo-dark.svg">
    <img src="assets/brand/logo-light.svg" alt="ForgeGuard" width="360">
  </picture>
</p>

<p align="center">
  <strong>Deterministic quality gates for AI-assisted work.<br>General tasks and production code · one local Rust binary · no extra LLM calls.</strong>
</p>

<p align="center">
  <img src="assets/brand/readme-hero.png" alt="ForgeGuard turns an AI-generated code diff into a verified change through static analysis, tests, diff review, and local evidence." width="960">
</p>

An AI agent can say the task is complete while acceptance criteria are still open, evidence is missing, or a passing test suite hides an N+1 query. ForgeGuard gives the agent a bounded objective, intercepts supported completion paths, and requires local evidence before the work is accepted.

## The failure ForgeGuard stops

```text
Agent: "Done. Tests pass."
                    ↓
ForgeGuard completion gate runs locally
                    ↓
FG-DB-001: database operation inside iteration
                    ↓
Agent receives bounded evidence and fixes the code
                    ↓
Changed-code scan + configured checks pass
                    ↓
Completion is allowed
```

The same contract also catches non-code completion theater: unfinished product decisions, QA claims without artifacts, security conclusions without provenance, unsupported statistics, and work that silently drifted outside its declared scope.

## Try it in 30 seconds

Install on Linux or macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/suiflex/ForgeGuard/main/install.sh | sh
```

The one-line installer also installs the current global policies, skills, and supported hooks, so General Guard is available to supported hosts without modifying a repository. To add Code Guard, initialize the repository your agent is working in:

```bash
cd your-project
forgeguard init
forgeguard doctor
forgeguard gate --changed --output compact
```

Windows, Homebrew, Scoop, npm, and source-build instructions are under [Installation options](#installation-options). Existing installations can be updated safely; see [Updating](#updating) for the difference between refreshing the binary, global assets, and project-owned files.

## Two guards, one evidence contract

| | General Guard | Code Guard |
|---|---|---|
| Use it for | Product discovery, QA, security reviews, business analysis, database work, architecture, content, statistics, research, and custom professions | Backend, frontend, mobile, AI/ML, data, scripts, APIs, infrastructure, and other repository changes |
| Activation | Global agent integration; no project initialization required | `forgeguard init` inside a repository |
| Prevents | Auto-poke loops, vague completion, missing acceptance coverage, unsupported claims, low hill-climbability, and file or MCP/resource scope drift | Everything in General Guard plus unreviewed diffs, static findings, missing tests, and failed formatter/linter/type-check/test/build commands |
| Evidence | Objectives, metrics, baselines, targets, guardrails, todos, acceptance criteria, provenance, artifacts, confidence, and verification | The same task evidence plus Git changes, AST-assisted findings, configured commands, JSON/SARIF reports, and baselines |
| Profiles | Open-ended: built-ins include product owner, QA, security engineer, business analyst, DBA, architect, content creator, and statistician | Language- and framework-agnostic workflow with capability-specific parser and semantic packs |

General Guard does not pretend every job is software development. A QA engineer can declare a Playwright MCP resource and trace artifact; a product owner can require decision evidence; a statistician can bind a claim to a dataset and analysis artifact. Unknown profile names remain valid, so a profession does not need a ForgeGuard release before it can use the completion contract.

Code Guard adds the repository workflow:

```text
inspect → design → implement → test → review → verify
```

## What it catches

- **Premature completion:** pending todos, uncovered acceptance criteria, insufficient confidence, missing verification, and goal contracts too incomplete to evaluate.
- **Scope drift:** edits outside declared files and evidence drawn from undeclared resources such as MCP servers, URLs, datasets, or dashboards.
- **AI code slop:** duplicate behavior, repeated linear lookup, accidental `O(n²)`, sorting or database/network I/O inside loops, unbounded fan-out, swallowed failures, and oversized complexity.
- **Security and data risks:** hardcoded credentials, bounded taint flow into dangerous sinks, weak crypto/TLS, unsafe deserialization, XSS/path sinks, access-control hotspots, and broad `SELECT *` queries.
- **Unproven claims:** performance, quality, security, product, QA, content, and statistical conclusions without the declared provenance or artifact.

ForgeGuard supports Codex, Claude Code, Cursor, OpenCode, Hermes, OpenClaw, Antigravity, and agents that consume `AGENTS.md`. See [Agent support](#agent-support) for the exact policy, skill, and hook behavior of each integration.

## Installation options

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
forgeguard init
forgeguard doctor
```

`forgeguard init` installs only for the agents that directory already uses. See
[Choosing agents](#choosing-agents) for how the selection is made.

Installers use GitHub Release binaries produced for Linux, macOS, and Windows on x86-64 and ARM64. Advanced users building unreleased source can still use:

```bash
cargo install --path crates/forgeguard-cli
```

### Homebrew

```bash
brew install suiflex/tap/forgeguard
```

A formula-installed binary is upgraded with `brew upgrade forgeguard`, not by re-running the installer. Global rules, skills, and hooks are not installed by the formula, so run `forgeguard init` afterwards.

### Scoop

```powershell
scoop bucket add suiflex https://github.com/suiflex/scoop-bucket
scoop install forgeguard
```

Upgrade with `scoop update forgeguard`.

### npm

The npm package installs the matching ForgeGuard GitHub Release binary for the current OS and CPU. It requires Node.js 18 or newer:

```bash
npm install -g @suiflex/forgeguard
forgeguard --version
```

To publish manually from a tagged release:

```bash
cd npm
npm pack --dry-run
npm publish --access public
```

The version in `npm/package.json` must match the GitHub Release tag, and release assets must already exist before publishing. The package postinstall script downloads and verifies the platform-specific archive checksum.

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
forgeguard init --force
forgeguard doctor
```

`--force` overwrites the ForgeGuard-owned policy and skill files and prunes obsolete
role-skill directories. For global installs it also relocates the Antigravity skill from
the superseded `~/.gemini/config/skills/` to `~/.gemini/antigravity-cli/skills/`, the path
[Antigravity CLI documents](https://antigravity.google/docs/cli/gcli-migration). ForgeGuard
no longer writes a user-level Antigravity hook file, because none is documented; the
workspace `.agents/hooks.json` gate is unchanged.

`.forgeguard/config.toml` is created once and then left alone. The operating mode and any
command you tuned survive every later `init`, `--refresh`, and `--force`; a committed
`.forgeguard/baseline.json` is likewise untouched.

### Keeping policy and skill files current

A new release can ship an updated engineering skill or policy template, but the copies in your
repository may also carry your own edits. `init` therefore compares them and reports the
difference instead of choosing for you:

```text
┌─ 2 ForgeGuard files differ from this version ──┐
│ CLAUDE.md                                      │
│ .claude/skills/forgeguard-engineering/SKILL.md │
└────────────────────────────────────────────────┘
◇ Replace them with the bundled versions? (y/N)
```

In a terminal it asks, listing every affected file and defaulting to **no**, so a stray Enter
never costs you your edits. Without a terminal it prints the same list and the command to run,
and changes nothing:

```bash
forgeguard init --refresh    # replace the drifted files, no prompt
forgeguard init --force      # replace every ForgeGuard-owned file, drifted or not
```

Both flags preserve existing `.forgeguard/config.toml` values and append newly detected command
presets by name. They never replace existing command definitions. Neither writes through a
symlink: a repository that points `AGENTS.md` at `CLAUDE.md` keeps both as they are, because
replacing the link would edit the file at the other end.

## Quick start

```bash
# Only needed after a source build; one-line installers already do this.
forgeguard init --global --agent all

cd your-project
forgeguard detect
forgeguard init
forgeguard doctor
forgeguard gate
```

If `forgeguard` is not found immediately after installation, restart the terminal so its updated user `PATH` is loaded. `forgeguard doctor` then explains missing project configuration, Git, hooks, or repository tools in plain command-level output.

Review only files changed in Git:

```bash
forgeguard review
```

Review branch or pull-request changes against a base revision:

```bash
forgeguard review --base origin/main
forgeguard gate --changed --base origin/main
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

Only the selected agents are written. A repository that uses Claude Code alone gets:

```text
your-project/
├── .forgeguard/config.toml
├── .forgeguard/baseline.json  # after `forgeguard baseline create`
├── .forgeguard/.gitignore
├── CLAUDE.md
├── .claude/settings.json
└── .claude/skills/forgeguard-engineering/
    ├── SKILL.md
    └── references/
```

Adding a target adds only its own files: `--agent codex` contributes `AGENTS.md`,
`.codex/hooks.json`, and `.agents/skills/`; `--agent cursor` contributes
`.cursor/rules/forgeguard.mdc`, `.cursor/hooks.json`, and `.agents/skills/`;
`--agent antigravity` contributes `.agents/rules/forgeguard.md` and `.agents/hooks.json`.

Existing policy and skill files are not overwritten unless `--force` is supplied. Hook installation merges one ForgeGuard entry into existing JSON and preserves unrelated settings. Global installation writes ForgeGuard-owned skills, compact policies, and hook entries for selected agents. `--force` also removes obsolete ForgeGuard-owned role-skill directories without touching unrelated skills.

Global lifecycle hooks supervise General Guard only. Inside a repository initialized with Code Guard, they defer to the repository hook so one lifecycle event produces one ForgeGuard decision. Re-running the installer normalizes duplicate ForgeGuard entries left by older global installations while preserving unrelated hooks.

When a project `.gitignore` already exists, `forgeguard init` appends the generated directories for
the selected agents (`.codex/`, `.claude/`, `.cursor/`, and/or `.agents/`). It preserves existing
patterns, avoids duplicate entries, and does not create a root `.gitignore`. The `AGENTS.md`-only
targets add no directory, so they add no ignore entry.

## Choosing agents

`forgeguard init` decides which integrations to write in one of three ways:

1. **Explicit `--agent`** always wins and is never second-guessed. It accepts a
   comma-separated list or a repeated flag, plus the `all` shortcut:

   ```bash
   forgeguard init --agent claude
   forgeguard init --agent claude,codex
   forgeguard init --agent all
   ```

2. **A terminal with no `--agent`** opens the interactive picker. It lists what each
   target writes, pre-checks the agents already configured in the directory, and
   treats an empty selection as "install nothing" rather than "install everything".

3. **No terminal and no `--agent`** — a script, CI job, or another agent shelling out —
   installs only for the agents whose own configuration is already present.

   | Agent | Detected by |
   |---|---|
   | Codex | `.codex/` |
   | Claude Code | `.claude/` |
   | Cursor | `.cursor/`, `.cursorrules` |
   | OpenCode | `.opencode/`, `opencode.json` |
   | Hermes | `.hermes/` |
   | OpenClaw | `.openclaw/`, `openclaw.json` |
   | Antigravity | `.agents/rules/`, `.agents/hooks.json`, `.agent/rules/` |
   | Windsurf / Devin | `.windsurf/`, `.devin/`, `.windsurfrules` |
   | GitHub Copilot | `.github/copilot-instructions.md`, `.github/instructions/` |
   | Cline | `.clinerules` |
   | Roo Code | `.roo/`, `.roorules` |

   Markers are the agent's own configuration. `.agents/skills/` is deliberately not
   one: Codex, Cursor, and OpenCode share that directory, so treating it as an
   Antigravity marker would make every re-run silently add a target. Under
   `--global` the equivalent user-directory paths are used instead.

   When nothing is detected, `init` writes nothing and exits **3** with the available
   choices on stderr, so a caller re-runs with an explicit selection instead of
   receiving every integration at once. With `--json`, it prints
   `{"needs_agent_selection": true, "choices": [...]}` instead. Exit code `2` still
   means a blocked gate.

Both report formats include the resolved `agents` list, so the output always states
what was installed.

## Agent support

| Agent | `--agent` | Rules | Skill | Automatic completion gate |
|---|---|---|---|---|
| Codex | `codex` | `AGENTS.md` | `.agents/skills` | `Stop` hook |
| Claude Code | `claude` | `CLAUDE.md` | `.claude/skills` | `Stop` hook |
| Cursor | `cursor` | `.cursor/rules` | `.agents/skills` | `stop` hook |
| Antigravity | `antigravity` | `.agents/rules` | `.agents/skills` | Native `Stop` hook |
| OpenCode | `opencode` | `AGENTS.md` | `.agents/skills` | Policy-enforced gate |
| Hermes | `hermes` | `AGENTS.md` | `.agents/skills` | Policy-enforced gate |
| OpenClaw | `openclaw` | `AGENTS.md` | `.agents/skills` | `before_agent_finalize` plugin hook |
| Windsurf / Devin | `windsurf` | `AGENTS.md` | — | Policy-enforced gate |
| GitHub Copilot | `copilot` | `AGENTS.md` | — | Policy-enforced gate |
| Cline | `cline` | `AGENTS.md` | — | Policy-enforced gate |
| Roo Code | `roo` | `AGENTS.md` | — | Policy-enforced gate |

The last four read [`AGENTS.md`](https://agents.md/) natively, so ForgeGuard supports them
with that one file and writes nothing under `.windsurf/`, `.github/`, `.clinerules/`, or
`.roo/`. None of them exposes a hook API ForgeGuard can drive, and none has a documented
skill directory, so they receive the policy but not the engineering skill.

User-level rules are not shared the same way, so `--global` follows each agent's own
documented path instead: Cline reads `~/.agents/AGENTS.md`, Windsurf/Devin reads
`~/.codeium/windsurf/memories/global_rules.md`, and Roo reads `~/.roo/rules/`. Copilot
instructions are repository-scoped, so a global install writes nothing for it.
Hermes and OpenClaw receive the engineering skill in their native global directories,
`~/.hermes/skills/` and `~/.openclaw/skills/`, respectively. OpenClaw also receives an
enabled native plugin under `~/.openclaw/extensions/forgeguard/`; it restores task context
before each prompt and runs the completion gate through `before_agent_finalize`. Restart the
OpenClaw gateway after installation. Hermes' completion hooks are observers, so its global
integration remains policy-enforced rather than claiming a blocking hook.

[OpenCode officially discovers](https://opencode.ai/docs/skills) both `AGENTS.md` and `.agents/skills`. Its current plugin lifecycle exposes `session.idle` only after the agent loop stops, so ForgeGuard does not claim a reliable blocking `Stop` hook there. The compact policy requires `forgeguard gate --changed --output compact` before completion. [Antigravity provides a native blocking `Stop` protocol](https://antigravity.google/docs/hooks), so failures automatically return the agent to its execution loop.

Other agents receive the universal CLI gate immediately. Agents that understand the emerging `AGENTS.md` and Agent Skills conventions also receive ForgeGuard guidance without a dedicated adapter.

## Technical contract and capabilities

ForgeGuard is designed not to drain user context or model limits:

- The hook runs a local Rust binary and repository commands; ForgeGuard itself makes no LLM or external API call.
- Always-on policy stays compact; detailed role and engineering references load only when relevant.
- Passing completion hooks add no model context in Codex/Claude. Blocking feedback is deduplicated and capped at 2,000 characters and five findings.
- Auto-poke is default-on and creates one host request per continuation; the generated limit is three and the hard cap is five.
- Full evidence stays local in `.forgeguard/reports/latest.json`.
- Unchanged worktrees use a local fingerprint cache instead of rerunning the gate.

A blocked gate may cause the host agent to use another turn to correct real failures. ForgeGuard spends model usage only indirectly when additional work is necessary.

Core capabilities include:

- A Rust single-binary CLI with human-readable, JSON, and SARIF 2.1.0 reports.
- Open-ended General Guard profiles and role-specific review phases for product, QA, security, business analysis, database administration, architecture, content, and statistics.
- Session-scoped objectives, metrics, acceptance criteria, todos, evidence provenance, artifacts, confidence, deterministic hill-climbability, bounded auto-poke, resume, and file/resource scope state.
- Lightweight source detection and automatic formatter, linter, type-check, test, and build command discovery.
- AST-backed loop and call-site analysis across the published parser capability matrix.
- Bounded database/network provenance packs for JavaScript/TypeScript, Python, Rust, and Go.
- Clean-as-you-code review at added/edited-line scope, optional base-ref comparison, changed-line LCOV policy, and committed finding baselines.
- Repository-scoped `default`, `lite`, and `strict` Code Guard modes.
- Optional dependency-audit, license-inventory/policy, and SBOM commands, discovered but disabled by default so realtime gates make no network calls.

Workflow supervision and configured repository commands work with any language. Parser, structural-rule, and semantic-pack coverage are separate capabilities: Tree-sitter supplies syntax evidence across the listed profiles, while bounded import/binding and local-wrapper provenance is currently available for JavaScript/TypeScript, Python, Rust, and Go. Run `forgeguard capabilities` for the exact matrix.

ForgeGuard is an AST-assisted quality scanner, not a whole-program semantic analyzer. Findings are review evidence, not substitutes for compilers, tests, profilers, security review, statistical validation, or query plans.

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
coverage_report = "coverage/lcov.info"
min_changed_coverage = 80
taint_sources = ["readExternalInput"]
trusted_sanitizers = ["validateForAllSinks"]

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

Detected dependency-audit, license-inventory/policy, and SBOM commands are installed as disabled required
checks. After reviewing and enabling them, changed gates run them only when a dependency manifest
or lockfile changes. Eligible successful results are reused for 24 hours only while the complete
dependency fingerprint is unchanged; failures always rerun. Results, including configured-check
failures, are included in JSON and SARIF. Generated SBOM JSON is kept
under `.forgeguard/reports/sbom/`; complete audit and license-tool output stays under
`.forgeguard/reports/supply-chain/`. Tool-specific commands still require their native CLI to be
installed.

`taint_sources` extends the request/input source catalog. `trusted_sanitizers` is an explicit
project trust decision and applies to every sink context; prefer built-in context-specific HTML,
path, and shell sanitizers where possible.

### Modes

Code Guard supports three repository-scoped operating modes after `forgeguard init` creates
`.forgeguard/config.toml`:

- `default`: failed required commands block; static findings block only when a rule sets `block = true`.
- `lite`: report-only mode for baselining or cleanup work. Static findings do not block.
- `strict`: failed required commands plus Warning- and Error-level static findings block. Info remains evidence.

Version 1 configs preserve the old Strict behavior that blocks only Error findings. Run `forgeguard config migrate` to opt into version 2 policy; older `mode = "guard"` values still load as `strict`.

Per-rule `enabled`, `severity`, and `block` override mode policy. `Lite` always keeps static findings nonblocking. Required command failures block in every mode.

These modes do not apply to General Guard or global agent installation. For upgrade compatibility,
the legacy `forgeguard mode --global` command shape is still recognized but returns repository-mode
migration guidance. Existing `mode` fields in `~/.forgeguard/config.toml` remain readable, and global
installation or refresh does not rewrite that file; the field no longer controls guard behavior.

`forgeguard init --global` creates `~/.forgeguard/config.toml` once when it is missing. General Guard
reads only its General Guard lifecycle settings from `[focus]`: `enabled`, `auto_poke`,
`max_auto_pokes`, `min_confidence`, and `min_hill_climbability`. Repository-gate
`max_retries` and `no_progress_limit` remain Code Guard-only. Later global installs and
refreshes preserve the file exactly.

With a global hook installed, General Guard applies task state, role-aware review, acceptance coverage, file and non-file resource scope checks, declared evidence provenance, and bounded auto-poke without project initialization or repository commands. Profiles are open-ended, so `--profile content-creator`, `--profile statistician`, or a custom profession works without a new ForgeGuard release. `forgeguard init` activates Code Guard: the same focus contract plus `inspect → design → implement → test → review → verify`, changed-source scanning, configured checks, and reports. Each continuation creates a new host request and consumes model tokens, so `max_auto_pokes` defaults to three and has a hard cap of five. Pending todos, confidence below `min_confidence`, uncovered acceptance criteria, or goal-contract completeness below `min_hill_climbability` keep the task active. Hill-climbability is a deterministic 0–100 completeness score: metric, baseline, target, guardrail, and verification contribute 20 points each. ForgeGuard does not guess these fields from prose. `forgeguard task start --semantic` only asks a supported host to use its native goal evaluator.

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

Example non-code task using Playwright MCP:

```bash
forgeguard task start --session "$SESSION" --profile qa \
  --objective "Verify guest checkout on staging" \
  --metric "checkout acceptance scenarios passed" --baseline "0 of 3" --target "3 of 3" \
  --guardrail "do not create production orders" --verification "Playwright trace reviewed" \
  --resource "mcp:playwright" --resource "url:https://staging.example.com/checkout" \
  --acceptance "guest can place an order" --acceptance "declined payment shows a safe error" \
  --todo "run guest checkout scenarios"
forgeguard task todo --session "$SESSION" --done 1
forgeguard task ready --session "$SESSION" --confidence 90 \
  --source "mcp:playwright" --artifact "artifact:checkout-trace.zip" \
  --criterion 1 --criterion 2 --evidence "3 scenarios passed on staging"
```

Set repository mode:

```bash
forgeguard mode default
forgeguard mode lite
forgeguard mode strict
```

Inspect mode as JSON:

```bash
forgeguard mode --json
```

When run in a terminal without an explicit mode, `forgeguard mode` opens the same interactive mode picker used by `forgeguard init`. Non-TTY calls and `--json` never prompt, so scripts and CI do not hang.

## Commands

| Command | Purpose |
|---|---|
| `forgeguard init` | Install project configuration and agent skills for the detected agents. Use `--agent` to select explicitly, `--global` for user-level skills, `--refresh` to replace drifted files. |
| `forgeguard detect` | Detect languages, frameworks, database tools, tests, and commands. |
| `forgeguard capabilities` | Show workflow, parser, structural-rule, and semantic-pack coverage. |
| `forgeguard doctor` | Verify configuration, Git, and required local tools. |
| `forgeguard mode` | Check or change the current repository's Code Guard mode. |
| `forgeguard config migrate` | Upgrade config v1 to v2 and append newly detected command presets without resetting existing commands or focus settings. |
| `forgeguard gate` | Run static rules and configured quality commands; `--changed --base <ref>` scopes findings to new code. |
| `forgeguard review` | Scan added/edited Git lines without running commands; `--base <ref>` compares a branch or pull request. |
| `forgeguard baseline create` | Record current static findings so gates report only new findings. |
| `forgeguard task start` | Register objective, open-ended profile, goal metrics, acceptance criteria, todos, file scopes, and non-file resources. |
| `forgeguard task todo` | Add todos or mark 1-based todo indexes complete. |
| `forgeguard task ready` | Submit exact evidence, provenance, artifacts, acceptance coverage, and optional model confidence before the completion gate. |
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

## Contributing

ForgeGuard grows through concrete problems, reproducible examples, and focused changes—not star campaigns.

- Start with the [first-contribution guide](CONTRIBUTING.md).
- Browse [`good first issue`](https://github.com/suiflex/ForgeGuard/labels/good%20first%20issue) and [`help wanted`](https://github.com/suiflex/ForgeGuard/labels/help%20wanted) work.
- Propose a bounded contribution with the [contribution proposal](https://github.com/suiflex/ForgeGuard/issues/new?template=contribution_proposal.yml).
- Bring an idea, request help, publish an integration, or show what ForgeGuard caught in [Discussions](https://github.com/suiflex/ForgeGuard/discussions).
- Use the [roadmap](docs/ROADMAP.md) to find starter, intermediate, and advanced contribution lanes.

If ForgeGuard catches a real issue in your workflow, a sanitized reproduction or screenshot is more useful than a generic endorsement. It gives maintainers a case to preserve and other users a reason to try the tool.

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
