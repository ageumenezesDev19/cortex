# Architecture

## One API, several doors

`api.rs` holds the body of every command. Each door only translates:

| Door | File | Caller |
|---|---|---|
| Tauri commands | `commands.rs` | the desktop window |
| HTTP, `POST /api/<command>` with a session token | `serve.rs` | the phone, over the network or Tailscale |
| A shim with the same commands in TypeScript | `src/demo/` | the browser demo |

On the front end, `src/lib/api.ts` is the only file that knows which door it is behind.

## From a file to a search result

```text
watch.rs     the folder changes
  -> parse.rs   cut the note into passages (see Rules › Passages)
  -> index.rs   write notes, passages and screenshots to SQLite (FTS5)
  -> ocr.rs     screenshots: Vision reads the text, one image at a time
  -> embed.rs   vectors for passages that have none (local model)
retrieve.rs  rank by words and by meaning, fused (see Rules › Search ranking)
  -> ask.rs     a question: the shortlist goes to a language model, with citations
  -> api.rs     the search box: the same ranking, grouped by note
  -> mcp.rs     an agent's search, limited to the folders it may read
```

## The MCP server

`src/bin/cortex-mcp.rs` speaks MCP over stdio with `rmcp`. The tools live in
`mcp.rs`; everything written to the vault goes through `devvault.rs`, the only code
that writes the memory format. It reads the vault without the app running, and refuses
an index on a schema it did not write (see Decisions › 0001).

## Storage

| What | Where |
|---|---|
| Notes and screenshots | `~/Cortex/**/*.md`, images in `_assets/` beside them |
| Index (derived) | `~/Cortex/.index/cortex.db`, schema version 8 (`db.rs`) |
| Settings, session token | `~/Cortex/.server/` (a dot folder: never indexed or served) |
| Embedding model | `~/Library/Application Support/Cortex/models/`, outside the vault |
| Memory written by agents | `~/Cortex/dev/` (see Rules › Agent memory) |
