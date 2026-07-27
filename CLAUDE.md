# ForgeGuard

Use the ForgeGuard engineering skill under `crates/forgeguard-core/assets/skills/engineering/` for every code change.

Required cycle: inspect → design → implement → test → review → verify.

Inspect related code, callers, tests, contracts, and schemas first. Reuse proven behavior; avoid speculative global APIs/components. Analyze complexity, queries, concurrency, memory, and failure modes when relevant. Test changed behavior. Before completion, run `forgeguard gate --changed --output compact`, repository checks, and review full diff. Report evidence and remaining risk briefly. Never bypass quality or security controls.

## Layout

- `crates/forgeguard-cli/` — the `forgeguard` binary (clap CLI, wizard, output rendering).
- `crates/forgeguard-core/` — detection, config, gate, hook, and init logic; also ships the
  bundled assets (`assets/skills/engineering/`, `assets/templates/`) baked in via `include_str!`.

## Commands

- `forgeguard init` — bootstrap a repo. Run in a terminal with no flags to get an interactive
  wizard (scope, which agents, and whether to add `.forgeguard/` to `.gitignore`). Non-terminal
  runs, `--json`, or explicit flags skip the wizard and default to all agents.
  Flags: `--global` (install under the user home instead of the repo), `--agent <codex|claude|cursor|opencode|antigravity|all>`, `--force` (overwrite + prune legacy skills), `--json`.
- `forgeguard detect` — report languages, frameworks, DB tooling, tests, and suggested commands.
- `forgeguard doctor` — check config and required local tools.
- `forgeguard gate [--changed] [--output full|compact|quiet] [--no-run]` — run static rules + configured quality commands.
- `forgeguard review` — static rules on changed files only (no command execution).
- `forgeguard hook stop --agent <...>` — lifecycle adapter the agent stop-hooks invoke.
- `forgeguard update` — optional release check; never required.

## Supported agents

Codex, Claude, Cursor, OpenCode, Antigravity. Each install writes a policy file, the
engineering skill, and (where supported) a Stop hook running `forgeguard hook stop --agent <name>`.
