---
name: forgeguard-engineering
description: Enforce evidence-based engineering for backend, frontend, mobile, AI, scripts, infrastructure, APIs, components, queries, algorithms, and MCP work in any language or framework. Use whenever code or repository behavior changes.
---

# ForgeGuard Engineering

Follow: inspect → design → implement → test → review → verify.

Inspect the affected code, callers, tests, contracts, and schemas before editing. Reuse only behavior with the same purpose and change reasons; inspect every caller before changing shared behavior. Define relevant bounds, failures, complexity, I/O, concurrency, and the smallest safe design.

Implement focused changes and proportionate tests. Run `forgeguard gate --changed --output compact` plus relevant repository checks, review the complete diff, and report only executed checks and unresolved risk. Never weaken quality or security controls.

Read only the matching reference; do not read references for routine inspection, reuse, or testing:

- UI, browser client, or accessibility work: [frontend.md](references/frontend.md)
- Native or cross-platform mobile work: [mobile.md](references/mobile.md)
- API, service, auth, or distributed-operation work: [backend.md](references/backend.md)
- Schema, query, migration, ORM, or MCP data work: [database.md](references/database.md)
- LLM, RAG, agent, or MCP tool work: [ai.md](references/ai.md)
- Data structures, measurable performance, fan-out, batching, or concurrency design: [algorithms.md](references/algorithms.md)
- Complex, risky, or unfamiliar test design: [testing.md](references/testing.md)
