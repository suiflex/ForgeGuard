# ForgeGuard Engineering Policy

Use the ForgeGuard skills installed under `.codex/skills/` for every implementation task.

Before changing code:

1. Inspect the repository, related implementations, tests, schemas, and conventions.
2. Select the engineering role that matches the affected area.
3. Search for existing reusable functions, services, hooks, and components.
4. Analyze input constraints, algorithmic complexity, database access, concurrency, and failure modes.
5. Define the smallest correct design and its verification strategy.

Before declaring completion:

1. Run `forgeguard gate`.
2. Run the relevant formatter, linter, type checker, unit tests, integration tests, and build.
3. Review the complete diff for accidental or unrelated changes.
4. Report actual command results; never claim checks passed without executing them.
5. Disclose remaining risks and unverified assumptions.

Do not bypass ForgeGuard rules by disabling tests, linting, type checking, or security controls.
