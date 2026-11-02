# 0002 One ranking for every search

**Status:** accepted · 2026-11-02

**Context.** The question pipeline fused words with vectors; the search box used words
alone; the browser demo fused both. The same vault was ranked three ways.

**Options.**
- Keep the box on words: fastest, but it finds nothing by meaning.
- Move the fusion into `retrieve.rs` and have the box, questions, the evaluation and
  the MCP server call it.

**Decision.** One ranking in `retrieve.rs`. Two variants were measured on the
35-question gold set and rejected:
- The demo's weight of 0.35 for a widened word list: recall@5 fell from 89% to 86%.
- Ranking only the passages a caller may see, for questions: two questions dropped one
  place (MRR 0.81 to 0.79). Questions rank everything and filter after; only the MCP
  server, whose scopes can be narrow, ranks inside its folders.

**Consequences.** The search box embeds the query, so its first search loads the
model. Gold set after the change: recall@5 89%, MRR 0.81, the same as before.
