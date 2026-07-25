# Rule Catalog

## FG-ALG-001 — Potential nested iteration

Heuristic warning for loop-like constructs nested inside an active loop scope. Review input bounds and whether indexing, batching, or a single traversal can reduce complexity.

## FG-ALG-002 — Repeated linear lookup inside iteration

Heuristic warning for `find`, `includes`, `indexOf`, `contains`, or equivalent lookup inside a loop. Consider pre-indexing with a map or set.

## FG-ALG-003 — Sorting inside iteration

Heuristic warning for repeated sorting. Consider one-time sorting, an ordered structure, or a heap.

## FG-DB-001 — Database operation inside iteration

Error-level rule for database-looking operations inside an active loop. Prefer set-based queries, joins, eager loading, prefetch, or bulk operations.

## FG-DB-005 — Potential unnecessary SELECT *

Warns when a query selects every column. Confirm whether all columns are required and whether large fields are present.

## FG-NET-001 — External request inside iteration

Warns when common HTTP-client calls are executed inside a loop. Prefer batching or bounded concurrency with timeout, rate-limit handling, retry classification, and partial-failure behavior.

## FG-CON-001 — Potential unbounded parallel execution

Warns on common fan-out patterns such as `Promise.all(collection.map(...))` and `join_all`. Use a concurrency limit for collections without a hard upper bound.

## FG-DRY-001 — Potential duplicated implementation

Informational cross-file duplicate block signal. Extract only when the duplicated code represents the same responsibility and should evolve together.

## False positives

ForgeGuard rules are evidence for review, not permission to refactor blindly. Configure narrow exclusions or use Lite/Guard mode while the analyzer evolves. Strict mode is intended for repositories that have reviewed their warning baseline.
