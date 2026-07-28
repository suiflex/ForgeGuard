# Database Engineering

- Verify actual tables, columns, types, constraints, relationships, nullability, representative data, and indexes before changing queries.
- When using MCP, inspect exposed schemas and compare them with the real database. Never invent fields, tools, or mutation permissions.
- Use parameterized, set-based, bounded queries with selected columns, stable ordering, tenant filters, and short transactions.
- Use `EXPLAIN` or `EXPLAIN ANALYZE` for important queries when safe. Never call a query optimal without schema, index, query-count, and plan evidence.
- Evaluate index selectivity, read benefit, write cost, storage, column order, and overlap before adding an index.
