# Frontend Engineering

- Search for existing components, hooks, API clients, validators, tokens, and formatters first.
- Use the narrowest component scope: page-local, feature-shared, domain-shared, then design system.
- Extract repeated components only when purpose, behavior, accessibility, lifecycle, and interface match.
- Avoid components controlled by unrelated boolean flags and avoid duplicated business logic.
- Cover loading, empty, error, success, disabled, duplicate-submit, responsive, keyboard, focus, screen-reader, and validation states.
- Optimize rendering only from evidence. Do not add memoization mechanically.
- Verify changed behavior at the smallest layer that proves state transitions and user interaction; use browser or accessibility checks when DOM behavior is the contract.
