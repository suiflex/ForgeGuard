---
name: forgeguard-engineering
description: Enforce evidence-based engineering for backend, frontend, mobile, AI, scripts, infrastructure, APIs, components, queries, algorithms, and MCP work in any language or framework. Use whenever code or repository behavior changes.
---

# ForgeGuard Engineering

Follow: inspect, design, implement, test, review, verify.

1. Inspect repository conventions, callers, tests, contracts, and schemas before editing.
2. Choose role matching affected area. Search existing functions, services, hooks, and components.
3. Define inputs, bounds, failure modes, complexity, I/O, concurrency, and smallest correct design.
4. Extract repeated behavior, not merely similar-looking code. Keep same-process reuse local; create APIs only across real process boundaries.
5. Implement focused code and tests. Verify database work against actual schema and query plans.
6. Run `forgeguard gate --changed --output compact` plus relevant repository checks. Review complete diff.
7. Report only executed checks, unresolved risks, and performance evidence. Never invent results.

Correct a harmful request before coding: name issue briefly, explain impact, then implement safer equivalent.

Read references only when relevant:

- Reuse/refactor: [clean-code.md](references/clean-code.md)
- Loops, data growth, query/network fan-out, caching, concurrency, or performance: [algorithms.md](references/algorithms.md)
- Frontend: [frontend.md](references/frontend.md)
- Native or cross-platform mobile: [mobile.md](references/mobile.md)
- Backend/API: [backend.md](references/backend.md)
- Database/ORM/MCP data: [database.md](references/database.md)
- AI/LLM/RAG/MCP: [ai.md](references/ai.md)
- Complex or risky test design: [testing.md](references/testing.md)

Do not disable quality or security controls. Successful hook output stays silent; failure feedback stays concise.
