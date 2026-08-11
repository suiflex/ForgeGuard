---
name: forgeguard-engineering
description: Enforce minimal, surgical, evidence-based engineering for backend, frontend, mobile, LLM, machine learning, deep learning, scripts, APIs, components, queries, algorithms, and MCP work. Use whenever code or repository behavior changes.
---

# ForgeGuard Engineering

Follow: inspect → design → implement → test → review → verify.

## Work surgically

- Inspect the affected code, tests, contracts, schemas, direct callers, and compatibility surface before editing.
- Resolve missing facts with read-only inspection. When unresolved ambiguity materially changes behavior, data, security, scope, cost, or external or irreversible state, use the host's native structured user-input tool and wait; otherwise state the safest reversible assumption.
- Reuse an existing project pattern only when its purpose and change reasons match. Prefer the standard library and installed dependencies over new abstractions or packages.
- Make every changed line trace to the objective. Do not refactor adjacent code or remove pre-existing dead code; remove only orphans created by the change.
- Choose the smallest safe design. Define only the relevant bounds, failures, complexity, I/O, and concurrency risks.

## Register the goal

Use the session id injected by the lifecycle hook. Run `forgeguard mode` to resolve policy: register every change in strict mode and every non-trivial change otherwise.

Before editing, inspect existing state with `forgeguard task status --session <id>`. If no task exists, register the objective and verifiable steps:

```sh
forgeguard task start --session <id> --objective "<outcome>" \
  --todo "<step with observable completion>" \
  --verification "<exact check>"
```

Make every non-trivial goal hill-climbable: supply `--metric`, a measured `--baseline`, `--target`, at least one `--guardrail`, and `--verification`. For functional work, use an observable acceptance or regression count; never invent a baseline. Add `--scope <path>` when the affected paths are known. Use `--semantic` only when the host provides a native goal evaluator.

## Prove the change

Implement the smallest focused change and the smallest test layer that can fail for the changed behavior. Match evidence to the claim:

- Bug fix: regression test exercising the reported failure.
- API or schema change: contract, migration, or compatibility check.
- Performance change: the same benchmark before and after, including the measured values.
- Security or authorization change: negative-path test showing rejection.
- Behavior-preserving refactor: relevant checks passing before and after.

Never weaken tests, quality gates, validation, authorization, or security controls to pass a check.

## Finish with evidence

1. Run relevant repository checks and `forgeguard gate --changed --output compact`.
2. Review the complete diff for unrelated edits and new dead code.
3. Mark completed work with `forgeguard task todo --session <id> --done <index>`.
4. Recheck `forgeguard task status --session <id>`; do not claim completion with pending todos.
5. Submit each exact result with `forgeguard task ready --session <id> --confidence <0-100> --evidence "<check: result>"`.

Report only executed evidence and unresolved risk. Confidence is advisory and never replaces deterministic evidence. Label unverified claims as unverified.

Treat an auto-poke as a new bounded verification phase: perform the requested TODO or check, then submit fresh evidence. Do not repeat an earlier completion claim.

Use progressive disclosure: read only the matching reference; do not read references for routine inspection, reuse, or testing. Role text is a reasoning frame, not permission to invent requirements, APIs, schemas, or evidence.

- UI, browser client, or accessibility work: [frontend.md](references/frontend.md)
- Native or cross-platform mobile work: [mobile.md](references/mobile.md)
- API, service, auth, or distributed-operation work: [backend.md](references/backend.md)
- Schema, query, migration, ORM, or database MCP work: [database.md](references/database.md)
- LLM, RAG, agent, or tool-authorization work: [ai.md](references/ai.md)
- Classical machine-learning model or feature-pipeline work: [ml.md](references/ml.md)
- Neural-network or deep-learning training/inference work: [deep-learning.md](references/deep-learning.md)
- Model serving, deployment, registry, or drift-monitoring work: [mlops.md](references/mlops.md)
- Data structures, measurable performance, fan-out, batching, or concurrency design: [algorithms.md](references/algorithms.md)
- Complex, risky, or unfamiliar test design: [testing.md](references/testing.md)
