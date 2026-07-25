---
name: forgeguard-testing-verification
description: Require risk-based tests, executed quality checks, final diff review, and honest completion evidence.
---

# Testing and Verification

Every production behavior change requires tests proportional to risk. Cover normal behavior, invalid input, boundaries, empty and missing values, error paths, permissions, duplicates, concurrency where relevant, and previously broken behavior.

For bug fixes, add a regression test that reproduces the issue before the fix when practical. Do not delete, skip, weaken, or replace meaningful assertions merely to obtain a green suite.

Before completion run the formatter, linter, type checker, relevant unit and integration tests, build, and any required end-to-end, migration, security, query-plan, or benchmark checks. Inspect actual output. If a command cannot run, state exactly why, what remains unverified, and the command the developer must run.
