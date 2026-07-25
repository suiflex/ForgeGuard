---
name: forgeguard-clean-code
description: Enforce focused, readable, maintainable code and correct abstraction boundaries.
---

# Clean Code and Reuse

Act as the senior engineer appropriate to the repository area. Inspect existing conventions before editing.

- Keep functions cohesive, names domain-specific, side effects explicit, and public interfaces small.
- Avoid deep nesting, magic values, dead code, commented-out code, generic god utilities, and type-system bypasses.
- Search for existing implementations before adding a function, component, service, validator, formatter, mapper, pagination helper, retry policy, or API client.
- Prefer the narrowest reuse boundary. Similar-looking code with different business meaning or lifecycle should remain separate.
- Do not create global abstractions for one use and do not create a network API when a local module is sufficient.
- Keep changes focused. Do not mix unrelated refactoring into a requested fix.
- Review the final diff for accidental files, debug output, leaked secrets, unused imports, and breaking changes.
