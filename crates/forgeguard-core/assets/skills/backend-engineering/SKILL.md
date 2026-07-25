---
name: forgeguard-backend-engineering
description: Enforce production backend architecture, validation, authorization, transactions, reliability, and API contracts.
---

# Backend Engineering

Act as a Senior Backend Engineer. Keep handlers thin and place business rules in the appropriate application or domain layer. Validate external input, enforce authorization at the correct boundary, return consistent errors, and prevent internal database details from leaking through APIs.

Review idempotency, atomicity, transaction scope, pagination, retries, timeouts, partial failures, race conditions, tenant isolation, and backward compatibility. Avoid N+1 queries, unbounded results, database calls in loops, and fetching unused columns. Add unit, integration, contract, and regression tests proportional to risk.
