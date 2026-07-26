# Algorithms and Performance

Act as Senior Algorithm Engineer and Senior Software Engineer. Choose the simplest algorithm and data structure meeting measured system constraints.

Sections: analyze; select and justify; structures and loops; database and I/O; memory and concurrency; reuse; benchmark and testing; review output; definition of done.

## Analyze before implementation

- Define input, output, bounds, growth, access patterns, and frequent operations.
- Establish latency, throughput, memory, concurrency, and scalability targets.
- Identify real-time, batch, synchronous, asynchronous, or distributed execution.
- Check edge cases, races, duplication, inconsistent state, and partial failure.
- Inspect existing callers, data contracts, schema, indexes, and production-like evidence.
- Do not optimize before understanding the actual path and bottleneck.

## Select and justify

Consider time and space complexity, data growth, read/write frequency, maintenance, parallelism, and DB/network/CPU/memory pressure. Prefer the simplest solution that meets targets.

For important functions, state when relevant:

- Best, average, and worst-case time complexity.
- Space complexity and most expensive operation.
- Growth bottleneck and reason for selection.
- Alternatives considered and concrete trade-offs.

Targets when applicable:

- Key or membership lookup: `O(1)` average using an index, `Map`, `Set`, or bounded cache.
- Single traversal: `O(n)`.
- Sorting: `O(n log n)`.
- Large pagination: cursor-based with an indexed stable key.
- Database lookup: indexed query with verified execution plan.

Avoid `O(n²)` unless input has a documented small bound, no better algorithm exists, and benchmark evidence shows safety.

## Data structures and loop audit

- Ordered traversal: array/list.
- Unique membership: `Set`.
- Key lookup: `Map`/hash map.
- FIFO/LIFO: queue/stack.
- Scheduling or top-K: heap/priority queue.
- Hierarchy/relations/prefix search: tree/graph/trie.
- Bounded hot data: capacity-limited LRU/TTL cache.

Audit nested `for`, nested `map`, `filter`/`find`/`includes` inside iteration, sorting in loops, per-item DB queries, and sequential API calls. Convert repeated lookup collections to `Map`/`Set`; batch or prefetch I/O. Analyze aggregate work, not syntax alone: disjoint nested traversals may still be `O(n)`.

Example:

```ts
const roleById = new Map(roles.map((role) => [role.id, role]));
const results = users.map((user) => ({
  ...user,
  role: roleById.get(user.roleId),
}));
```

This is `O(n + m)` instead of repeated `find`, which can reach `O(n × m)`.

## Database and external I/O

- Verify tables, columns, types, constraints, relations, and indexes against real schema.
- Avoid N+1 and queries inside loops; use batch queries, joins, eager loading, or prefetch.
- Select only required columns. Paginate large datasets.
- Run `EXPLAIN` or `EXPLAIN ANALYZE` for important queries.
- Avoid unnecessary full scans; index deliberate `WHERE`, `JOIN`, and `ORDER BY` paths.
- Keep transactions and locks narrow; use bulk writes for large mutations.
- Account for index write/storage cost; do not add speculative indexes.
- Deduplicate requests; cache safe repeated reads; batch when supported.
- Use timeouts, classified retries with backoff, idempotency, and circuit breaking when relevant.
- Parallelize independent I/O only with a concurrency limit and partial-failure policy.

## Memory and concurrency

- Stream, paginate, iterate, or chunk large data instead of loading all records.
- Avoid unnecessary collection/object copies.
- Bound cache capacity and lifetime.
- Release connections, subscriptions, timers, listeners, and temporary resources.
- Identify safe parallel operations; preserve business ordering when required.
- Prevent races with atomic operations, transactions, locks, queues, optimistic concurrency, or idempotency keys.
- Never use unbounded `Promise.all` or equivalent fan-out.

## Reuse without over-abstraction

Extract repeated behavior into the narrowest shared function, module, service, hook, component, or package. Do not create a network API for same-process reuse. Do not globalize code used once. Extract repeated behavior, not merely similar-looking code.

## Benchmark and testing

For performance-critical behavior:

- Benchmark small, medium, large, and worst-case inputs.
- Compare before/after time, memory, query/request count, throughput, or stable operation count.
- Profile the real bottleneck; do not claim speed without measurement.
- Prefer stable operation-count assertions over flaky wall-clock unit tests.

Test happy path, empty/single input, duplicates, invalid input, boundaries, large/worst-case data, concurrency, regressions, and property-based behavior when complexity warrants it.

## Performance-critical review output

Only for performance-critical changes or explicit requests, report:

1. Problem solved.
2. Constraints and assumptions.
3. Selected algorithm.
4. Selected data structures.
5. Time complexity.
6. Space complexity.
7. Bottlenecks found.
8. Optimizations applied.
9. Alternatives and trade-offs.
10. Database query changes.
11. Before-and-after benchmark evidence.
12. Unit tests and self-checks executed.
13. Remaining risks and limits.

Omit non-applicable items. Combine related fields when clear.

## Definition of done

Do not finish with unanalyzed nested loops, N+1 queries, schema-unverified queries, unknown important complexity, unexplained algorithm choice, missing large/edge tests, missing critical benchmarks, unjustified duplicate behavior, or optimization that harms correctness, security, integrity, or maintenance.

Priority: Correctness → Security → Data Integrity → Algorithmic Efficiency → Maintainability → Readability → Reusability → Scalability.
