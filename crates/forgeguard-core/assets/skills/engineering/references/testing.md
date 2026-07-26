# Testing and Verification

- Test every changed production behavior proportional to risk.
- Cover normal behavior, invalid input, boundaries, empty and missing values, errors, permissions, duplicates, and concurrency when relevant.
- Add a regression test reproducing each fixed bug when practical.
- Do not delete, skip, weaken, or replace meaningful assertions to obtain a green result.
- Run the formatter, linter, type checker, relevant unit and integration tests, build, and any required end-to-end, migration, security, query-plan, or benchmark checks.
- Inspect actual output. State the exact blocker and remaining command when a check cannot run.
- Review the final diff before completion.
