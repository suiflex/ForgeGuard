---
name: forgeguard-engineering
description: Enforce evidence-based engineering for backend, frontend, mobile, AI, scripts, infrastructure, APIs, components, queries, algorithms, and MCP work in any language or framework. Use whenever code or repository behavior changes.
---

# ForgeGuard Engineering

Follow: inspect → design → implement → test → review → verify.

Inspect the affected code, callers, tests, contracts, and schemas before editing. Reuse only behavior with the same purpose and change reasons; inspect every caller before changing shared behavior. Define relevant bounds, failures, complexity, I/O, concurrency, and the smallest safe design.

For every code change in strict mode, and every non-trivial code change in other modes, use the ForgeGuard session id injected by the lifecycle hook to register the exact objective, verifiable todos, and optional path prefixes with `forgeguard task start`. For abstract work, also state the metric, baseline, target, guardrails, and verification so progress is hill-climbable. Keep the declared scope honest; expand it only after repository evidence shows the objective requires another path. Use `--semantic` only when the host provides a native goal evaluator.

Implement focused changes and proportionate tests. Run `forgeguard gate --changed --output compact` plus relevant repository checks, review the complete diff, and report only executed checks and unresolved risk. Never weaken quality or security controls.

Update completed todos with `forgeguard task todo`. Before stopping, run `forgeguard task ready --session <id> --confidence <0-100> --evidence <exact-check-result>` for an active task. Confidence is model-reported and advisory; it never replaces executed evidence. Never present a model-derived value, correction, or completion claim as fact: cite exact tool/check evidence or label the claim unverified.

Treat an auto-poke as a new bounded verification phase: perform the requested TODO, test, review, or contract check, then submit fresh evidence. Do not repeat an earlier completion claim.

Read only the matching reference; do not read references for routine inspection, reuse, or testing:

- UI, browser client, or accessibility work: [frontend.md](references/frontend.md)
- Native or cross-platform mobile work: [mobile.md](references/mobile.md)
- API, service, auth, or distributed-operation work: [backend.md](references/backend.md)
- Schema, query, migration, ORM, or MCP data work: [database.md](references/database.md)
- LLM, RAG, agent, or MCP tool work: [ai.md](references/ai.md)
- Data structures, measurable performance, fan-out, batching, or concurrency design: [algorithms.md](references/algorithms.md)
- Complex, risky, or unfamiliar test design: [testing.md](references/testing.md)
