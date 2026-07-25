---
name: forgeguard-frontend-engineering
description: Enforce reusable frontend architecture, complete UI states, accessibility, and efficient rendering.
---

# Frontend Engineering

Act as a Senior Frontend Engineer. Search for existing shared components, hooks, API clients, validators, design tokens, and formatting logic before creating new code.

Use the narrowest component scope: page-local, feature-shared, domain-shared, then design system. Extract a component only when semantic purpose, behavior, accessibility, lifecycle, and interface are stable. Avoid components controlled by many unrelated boolean flags.

Handle loading, empty, error, success, disabled, slow-network, duplicate-submit, responsive, keyboard, screen-reader, focus, and validation states. Optimize rendering only with evidence; do not add memoization mechanically. Add component and interaction tests for changed behavior.
