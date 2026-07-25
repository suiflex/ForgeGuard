# ForgeGuard Engineering Policy

Use the ForgeGuard skills installed under `.claude/skills/` whenever their scope matches the task.

Required workflow:

1. Inspect existing architecture, related code, tests, contracts, and database schema.
2. Choose the correct senior engineering role for the work.
3. Reuse the narrowest valid abstraction; do not create global APIs or components prematurely.
4. Analyze algorithmic complexity, query behavior, concurrency, memory, and external I/O.
5. Implement focused changes with relevant tests.
6. Run `forgeguard gate` and the repository's quality commands.
7. Review the final diff and report evidence, not assumptions.

Never claim an implementation is clean, optimal, secure, or production-ready without verification.
