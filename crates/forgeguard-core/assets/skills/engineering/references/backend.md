# Backend Engineering

- Trace the request from transport through authorization, business rules, persistence, and external calls before choosing the edit point.
- Validate external input and authorize the target resource at the trust boundary. Return existing error shapes without leaking internals.
- Review only relevant distributed risks: idempotency, atomicity, transaction scope, pagination, timeouts, retries, partial failures, races, tenant isolation, and compatibility.
- Prove changed behavior at its boundary: request/response contract, authorization rejection, persistence effect, or idempotent retry.
