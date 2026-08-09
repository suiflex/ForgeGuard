# Focus, auto-poke, and hill-climbability

ForgeGuard installs focus enforcement with `forgeguard init`. No extra configuration is required for new repositories. Auto-poke is enabled by default and remains bounded.

Enforcement is opt-in: every hook passes silently until `.forgeguard/config.toml` exists in the repository or workspace root, so a repository you never initialized is never gated, never blocked, and never written to.

## Lifecycle hooks

ForgeGuard uses three hooks where the host supports them:

- `SessionStart` restores the session objective and active continuation after startup, resume, or compaction.
- `PreToolUse` warns when an explicit edit path falls outside declared scope.
- `Stop` checks task state, repository gates, evidence, and continuation limits.

Auto-poke itself is implemented by the `Stop` hook. A block response makes the host submit a new model request with the next instruction. Headless operation uses the same path when the host executes lifecycle hooks in headless mode.

OpenCode receives the shared policy and skill, but ForgeGuard does not claim automatic continuation there because its lifecycle does not expose a reliable blocking Stop hook.

## Session state

Each host conversation gets a separate task file under `.forgeguard/cache/tasks/`. State contains:

- exact objective and repository-relative scope prefixes;
- metric, baseline, target, guardrails, and verification contract;
- ordered todos and completion status;
- advisory model-confidence history;
- evidence, current status, auto-poke count, and blocker.

Task and Stop caches are excluded from worktree fingerprints and version control.

## Continuation flow

```text
model ends turn
  → Stop hook reads session task
  → abstract goal: request measurable contract
  → incomplete todos: continue next todo
  → todos complete: require evidence and confidence
  → gate failure: continue from exact failure
  → gate pass: TODO/test/review/contract/final verification poke
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

## Configuration

Fresh initialization writes:

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
forgeguard init --agent all --force
forgeguard doctor
```

`--force` regenerates `.forgeguard/config.toml`. Back up custom commands or modes first, then reapply them after refresh. Existing committed baselines are preserved.
