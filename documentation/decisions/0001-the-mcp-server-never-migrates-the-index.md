# 0001 The MCP server never migrates the index

**Status:** accepted · 2026-11-02

**Context.** The app and `cortex-mcp` open the same `cortex.db`. `db::open` migrates any
older schema on open. An installed app one schema behind keeps running on a migrated
file and re-indexes it in the old shape, so the two would fight over the index.

**Options.**
- The server migrates like the app: simplest, safe only if the app is always updated
  first.
- The server refuses another schema and only the app migrates.

**Decision.** Refuse. `db::open_current` reads `user_version` on a read-only connection
and opens for writing only when it matches; otherwise every tool answers "open the
updated Cortex once".

**Consequences.** Rebuilding the server before the app makes it refuse until the app is
updated, with no damage. The app and the server should ship together.
