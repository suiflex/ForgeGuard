# Algorithms and Performance

Choose the simplest algorithm and data structure that meets measured constraints.

- Establish inputs, bounds, growth, frequent operations, failure modes, and the actual bottleneck before optimizing.
- State relevant time and space complexity, growth limit, and trade-off. Avoid `O(n²)` unless a documented small bound or benchmark makes it safe.
- Use a `Map` or `Set` for repeated lookup or membership; audit repeated `find`, `includes`, sorting, query, or request work inside iterations. Analyze aggregate work, not nesting alone.
- Batch, prefetch, paginate, stream, or chunk work instead of unbounded per-item I/O or memory use. Bound cache size, lifetime, and concurrent fan-out.
- Preserve ordering and prevent races with the narrowest appropriate transaction, atomic operation, lock, queue, optimistic check, or idempotency key.
- Benchmark or use stable operation-count checks for performance-critical changes. Report only the selected approach, complexity, measured evidence, trade-off, and remaining limit.
