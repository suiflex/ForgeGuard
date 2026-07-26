# ForgeGuard

Use the ForgeGuard engineering skill under `crates/forgeguard-core/assets/skills/engineering/` for every code change.

Required cycle: inspect → design → implement → test → review → verify.

Inspect related code, callers, tests, contracts, and schemas first. Reuse proven behavior; avoid speculative global APIs/components. Analyze complexity, queries, concurrency, memory, and failure modes when relevant. Test changed behavior. Before completion, run `forgeguard gate --changed --output compact`, repository checks, and review full diff. Report evidence and remaining risk briefly. Never bypass quality or security controls.
