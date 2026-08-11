# Mobile Engineering

- Inspect existing navigation, state, networking, persistence, design system, and platform conventions.
- Reuse components only when behavior, lifecycle, accessibility, and platform semantics match.
- Handle loading, empty, offline, retry, permission denial, backgrounding, restoration, and interrupted flows.
- Bound memory, network work, listeners, subscriptions, and background tasks; measure startup, frame, and battery impact when relevant.
- Protect secrets and local data; validate deep links, intents, push payloads, and untrusted API/model output.
- Preserve iOS/Android conventions instead of forcing identical behavior where platforms differ.
- Match evidence to the change: state tests for business transitions, device or platform checks for lifecycle and permission behavior, and accessibility checks for changed interaction.
