# Rule Catalog

## FG-ALG-001 — Potential nested iteration

Heuristic warning for loop-like constructs nested inside an active loop scope. Review input bounds and whether indexing, batching, or a single traversal can reduce complexity.

## FG-ALG-002 — Repeated linear lookup inside iteration

Heuristic warning for `find`, `includes`, `indexOf`, `contains`, or equivalent lookup inside a loop. Consider pre-indexing with a map or set.

## FG-ALG-003 — Sorting inside iteration

Heuristic warning for repeated sorting. Consider one-time sorting, an ordered structure, or a heap.

## FG-DB-001 — Database operation inside iteration

Error-level rule for a provenance-confirmed database operation inside an active loop. JavaScript/TypeScript, Python, Rust, and Go packs resolve supported imports, lexical aliases, and unique local wrapper summaries.

## FG-DB-002 — Potential database operation inside iteration

Informational name-based fallback. Receiver names alone never produce the semantic-confirmed rule.

## FG-DB-005 — Potential unnecessary SELECT *

Warns when a query selects every column. Confirm whether all columns are required and whether large fields are present.

## FG-NET-001 — External request inside iteration

Warns when a provenance-confirmed HTTP-client call is executed inside a loop. Prefer batching or bounded concurrency with timeout, rate-limit handling, retry classification, and partial-failure behavior.

## FG-NET-002 — Potential external request inside iteration

Informational name-based fallback for unresolved receivers such as a generic `client`.

## FG-CON-001 — Potential unbounded parallel execution

Warns on common fan-out patterns such as `Promise.all(collection.map(...))` and `join_all`. Use a concurrency limit for collections without a hard upper bound.

## FG-DRY-001 — Potential duplicated implementation

Informational cross-file duplicate block signal. Extract only when the duplicated code represents the same responsibility and should evolve together.

## FG-DRY-002 — Potential renamed duplicated implementation

Informational same-language function clone with local identifiers alpha-normalized. Operators, literals, member names, and imported API names remain significant. This is Type-2 clone evidence, not semantic business-logic equivalence.

## FG-PARSE-001 — Structural analysis skipped

Informational finding for supported files containing syntax errors or an unavailable grammar. ForgeGuard skips structural claims for that file instead of falling back to noisy lexical guesses.

## False positives

ForgeGuard rules are evidence for review, not permission to refactor blindly. Structural scope and bounded provenance do not prove runtime cost or business intent. Configure per-rule policy, narrow exclusions, or Lite mode while reviewing a baseline. Strict v2 blocks Warning and Error findings.

A reviewed heuristic can be suppressed on its line or the preceding line with a required reason:

```text
// forgeguard: allow FG-ALG-001 -- bounded inner loop; maximum 8 items
```

Error-level findings and command failures cannot be suppressed inline.

For an existing repository, `forgeguard baseline create` records current static findings in
`.forgeguard/baseline.json`. Later gates hide matching existing findings while still reporting
additional occurrences, changed evidence, and all command failures. Commit the baseline so local
and CI gates enforce the same boundary.
