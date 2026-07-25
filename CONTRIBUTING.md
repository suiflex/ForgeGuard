# Contributing

1. Create a focused branch.
2. Add or update tests for behavioral changes.
3. Run formatting, Clippy, tests, and build.
4. Keep rule evidence specific and recommendations actionable.
5. Document whether a rule is deterministic, heuristic, or evidence-based.

Avoid adding hard-blocking rules based only on broad regex matches. New strict rules must have tests for both detection and false-positive resistance.
