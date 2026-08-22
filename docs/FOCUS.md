# Focus, auto-poke, and hill-climbability

When a global ForgeGuard hook is installed, General Guard provides objective, profile, acceptance criteria, TODO, evidence provenance, file/resource scope, and bounded auto-poke supervision in any working directory. It supports non-code work without project initialization.

`forgeguard init` separately activates Code Guard through `.forgeguard/config.toml`. Code Guard preserves `inspect → design → implement → test → review → verify` and adds source scanning, configured commands, and local reports. General Guard never executes repository commands or writes project configuration/reports.

## Lifecycle hooks

ForgeGuard uses three hooks where the host supports them:

- `SessionStart` restores the session objective and active continuation after startup, resume, or compaction.
- `PreToolUse` warns when an explicit edit path or recognized tool resource falls outside declared scope.
- `Stop` always checks registered task state, evidence, and continuation limits; Code Guard additionally runs repository gates.

Auto-poke itself is implemented by the `Stop` hook. A block response makes the host submit a new model request with the next instruction. Headless operation uses the same path when the host executes lifecycle hooks in headless mode.

OpenCode receives the shared policy and skill, but ForgeGuard does not claim automatic continuation there because its lifecycle does not expose a reliable blocking Stop hook.

## Session state

Each registered host conversation gets a separate task file under `.forgeguard/cache/tasks/`, even without project initialization. State contains:

- exact objective, open-ended profile, repository-relative path prefixes, and typed non-file resource prefixes;
- metric, baseline, target, guardrails, and verification contract;
- acceptance criteria and their submitted evidence coverage;
- ordered todos and completion status;
- advisory model-confidence history;
- evidence summaries, declared provenance, artifact references, current status, auto-poke count, and blocker.

Task and Stop caches are excluded from worktree fingerprints and version control.

## Continuation flow

```text
model ends turn
  → Stop hook reads session task
  → abstract goal: request measurable contract
  → incomplete todos: continue next todo
  → todos complete: require evidence and confidence
  → acceptance criteria: require explicit evidence mapping
  → Code Guard only: gate failure continues from exact failure
  → General or Code Guard: mode-appropriate evidence/review/final verification poke
  → limits reached: stop with blocker
  → requirements satisfied: allow completion
```

Every continuation is a new host request. Generated configuration allows three auto-pokes. ForgeGuard clamps any configured value to a hard maximum of five. Retry and no-progress budgets separately bound unchanged failures.

Both budgets count attempts against unchanged repository and task state only; a turn that advances either one starts a fresh budget. A blocked task keeps its auto-poke budget while the objective stays the same, and releases it when the session registers a different objective, so a stopped session can start new work instead of being stopped before its first turn.

## Hill-climbability contract

ForgeGuard does not infer whether prose sounds measurable. It scores explicit contract completeness:

- metric: 20 points;
- baseline: 20 points;
- target: 20 points;
- at least one guardrail: 20 points;
- at least one verification method: 20 points.

The score measures contract completeness, not semantic quality. A 100/100 contract can still contain a bad metric; executed evidence and repository gates remain authoritative.

For work without a natural performance metric, use an observable completion measure rather than inventing precision. A product brief can measure approved acceptance criteria, content can measure verified claims and editorial checks, and an investigation can measure hypotheses resolved with sources. Baseline is the known current state; target is the desired verified state.

Bad objective:

```text
Improve application performance.
```

Hill-climbable objective:

```text
Reduce p95 latency for /search from 900 ms to below 300 ms,
without increasing error rate, with all regression tests passing.
```

Register it:

```bash
forgeguard task start --session "$SESSION" \
  --objective "Reduce /search latency without regressions" \
  --metric "p95 latency /search" \
  --baseline "900 ms" \
  --target "below 300 ms" \
  --guardrail "error rate does not increase" \
  --verification "regression tests pass" \
  --todo "measure baseline" \
  --todo "optimize endpoint"
```

Update and finish:

```bash
forgeguard task todo --session "$SESSION" --done 1
forgeguard task ready --session "$SESSION" \
  --confidence 90 \
  --evidence "benchmark: p95 284 ms; error rate unchanged"
```

Confidence is model-reported and advisory. It never replaces tool output, tests, benchmarks, or the ForgeGuard gate.

## Profiles, acceptance, and resource scope

`--profile` accepts any lowercase name. Product, QA, security, business-analysis, database, architecture, content-creator, and statistics aliases select specialized review phases; every other name uses the general evidence/review/final-verification phases. This keeps profiles extensible without turning every profession into a compiled enum.

Non-general profiles require at least one `--acceptance` criterion. `task ready` must map submitted evidence to every 1-based criterion with `--criterion`. It also requires declared provenance through `--source <kind:value>`; optional `--artifact <kind:value>` values retain references to traces, reports, screenshots, plans, or source notes. ForgeGuard validates and records this metadata but does not claim that a user-provided source string proves tool execution.

`--scope` remains a repository-relative file prefix. `--resource` adds non-file boundaries such as `mcp:playwright`, `url:https://staging.example.com`, `database:production/analytics`, or `table:orders`. Pre-tool hooks recognize tool names and common URL, database, table, schema, environment, server, host, and project arguments. They warn on drift; host authorization and destructive-action confirmation remain authoritative.

Example:

```bash
forgeguard task start --session "$SESSION" --profile content-creator \
  --objective "Publish a sourced launch article" \
  --metric "verified factual claims" --baseline "0 verified" --target "all claims verified" \
  --guardrail "no unlicensed media" --verification "editorial source review" \
  --resource "url:https://docs.example.com" \
  --acceptance "every factual claim has a source" \
  --acceptance "headline and call to action match the brief" \
  --todo "draft and verify the article"
forgeguard task todo --session "$SESSION" --done 1
forgeguard task ready --session "$SESSION" --confidence 90 \
  --source "human:editorial-review" --artifact "artifact:launch-article.md" \
  --criterion 1 --criterion 2 --evidence "editor approved the sourced final draft"
```

## Configuration

Code Guard initialization writes:

```toml
[focus]
enabled = true
max_retries = 3
no_progress_limit = 2
auto_poke = true
max_auto_pokes = 3
min_confidence = 80
min_hill_climbability = 80
```

No edit is required. Set `auto_poke = false` only to opt out, or lower `max_auto_pokes` when request cost matters more than persistence.

## Upgrading an initialized repository

Upgrade the binary, then refresh bundled policies, skills, and hook entries:

```bash
forgeguard init --force
forgeguard doctor
```

`--force` regenerates `.forgeguard/config.toml`. Back up custom commands or modes first, then reapply them after refresh. Existing committed baselines are preserved.
