# Testing and Verification

- Select the smallest test layer that proves the changed behavior; add integration, contract, or regression coverage when the boundary or risk requires it.
- Cover relevant invalid input, boundaries, empty values, errors, permissions, duplicates, and concurrency.
- Prefer one regression test for the root cause over guards repeated across callers. Verify public behavior rather than private implementation details.
- Preserve meaningful assertions; never skip or weaken them solely to make a check pass.
- When a check cannot run, inspect its output and report the exact blocker and remaining command.
