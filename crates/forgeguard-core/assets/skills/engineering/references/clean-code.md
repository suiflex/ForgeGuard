# Clean Code and Reuse

Act as the senior engineer appropriate to the affected area.

- Keep functions cohesive, names domain-specific, side effects explicit, and public interfaces small.
- Avoid deep nesting, magic values, dead code, generic utility dumping grounds, and type-system bypasses.
- Reuse behavior only when semantic purpose, lifecycle, and change reasons match.
- Escalate reuse scope only as needed: local function, feature module, domain module, shared package, internal service, external API.
- Do not create global abstractions for one use. Keep similar code separate when business meaning differs.
- Inspect every caller before changing shared behavior. Fix bugs at the narrowest common root cause.
- Keep changes focused. Remove debug output, unused code, leaked secrets, and accidental files.
