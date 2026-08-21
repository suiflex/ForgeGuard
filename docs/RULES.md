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

## FG-CPLX-001 — High function complexity

Warns when a function exceeds the bounded structural complexity threshold. Changed-code review retains the finding when any line in that function changed.

## FG-SEC-001 — Hardcoded credential

Error-level detection for high-confidence credential formats and private-key headers in source strings and common configuration files. Evidence is always redacted.

## FG-SEC-002 — Dynamic sensitive operation

Informational security hotspot for non-literal dynamic evaluation or shell execution. Review whether the value can be attacker-controlled.

## FG-SEC-003 — Tainted function data reaches a sensitive sink

Informational hotspot when function data reaches a command, query, or network sink through direct use or bounded assignment propagation. Unique local wrapper summaries allow this evidence to cross files; recognized escaping/sanitizing calls stop propagation.

## FG-SEC-004 — Weak cryptography or TLS configuration

Warns on high-confidence MD5/SHA-1 hashing APIs, obsolete SSL/TLS versions, or disabled certificate verification. The rule does not label non-security checksums as exploitable; review the use context.

## FG-SEC-005 — Unsafe deserialization

Warns on object deserializers such as pickle, marshal, unsafe YAML loaders, `unserialize`, and generic object deserialization APIs. Prefer non-executable data formats or restricted loaders.

## FG-SEC-006 — Tainted data reaches an HTML sink

Warns when parameter-derived data reaches known raw-HTML calls or assignments such as `innerHTML`. Recognized HTML escaping/sanitizing calls stop the bounded flow.

## FG-SEC-007 — Tainted path reaches a filesystem sink

Informational hotspot when parameter-derived data reaches a filesystem API through direct use or assignments. Constrain the resolved path to an allowed root; normalization alone is not treated as sanitization.

## FG-AUTH-001 — Mutating route requires access-control review

Informational hotspot for common mutating route registrations that do not visibly declare auth, policy, permission, role, guard, or middleware checks. Inherited controls may satisfy the requirement.

## FG-ERR-001 — Exception is swallowed

Warns on structurally empty `catch`, `except`, or `rescue` handlers. Richer dead-code, unused-import, nullability, and resource-lifetime analysis remains delegated to the project's compiler and linter.

## FG-COV-001 — Changed-line coverage below policy

Deterministic finding when an explicitly configured LCOV report is missing or coverable added/edited lines fall below `scan.min_changed_coverage`. ForgeGuard imports the report; the repository's existing test tooling generates it.

## FG-DRY-001 — Potential duplicated implementation

Informational cross-file duplicate block signal. Extract only when the duplicated code represents the same responsibility and should evolve together.

## FG-DRY-002 — Potential renamed duplicated implementation

Informational same-language function clone with local identifiers alpha-normalized. Operators, literals, member names, and imported API names remain significant. This is Type-2 clone evidence, not semantic business-logic equivalence.

## FG-DRY-003 — Potential duplicated business operation

Informational same-language signal for differently structured functions that invoke the same set of at least three distinct operations. It is deliberately a review hint: only domain invariants can prove business equivalence.

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
