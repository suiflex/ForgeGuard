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

## FG-PARSE-001 — Structural analysis skipped

Informational finding for supported files containing syntax errors or an unavailable grammar. ForgeGuard skips structural claims for that file instead of falling back to noisy lexical guesses.

## False positives

ForgeGuard rules are evidence for review, not permission to refactor blindly. Structural scope proves code placement, not runtime cost or business intent. Configure narrow exclusions or use Lite/Guard mode while the analyzer evolves. Strict mode is intended for repositories that have reviewed their warning baseline.

A reviewed heuristic can be suppressed on its line or the preceding line with a required reason:

```text
// forgeguard: allow FG-ALG-001 -- bounded inner loop; maximum 8 items
```

Error-level findings and command failures cannot be suppressed inline.

For an existing repository, `forgeguard baseline create` records current static findings in
`.forgeguard/baseline.json`. Later gates hide matching existing findings while still reporting
additional occurrences, changed evidence, and all command failures. Commit the baseline so local
and CI gates enforce the same boundary.
