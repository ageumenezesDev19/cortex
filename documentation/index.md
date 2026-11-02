# Overview

Cortex is a local-first knowledge base for notes and screenshots. Every image dropped
into the vault goes through OCR, so a search finds the text inside a screenshot the
same way it finds prose. It runs as a desktop app (Tauri), over HTTP for a phone, as
an MCP server for coding agents, and as a browser demo over invented notes.

Markdown on disk is the source of truth. The SQLite index in `~/Cortex/.index/` is
derived: deleting it makes the app rebuild it.

## Run

macOS (OCR uses the Vision framework), Rust stable, Node 20+.

```bash
npm install
npm run app          # OCR helper, then Tauri in dev mode
npm run app:build    # the .app bundle and a .dmg
```

The MCP server is a second binary in the same crate:

```bash
cd src-tauri && cargo build --release --bin cortex-mcp
target/release/cortex-mcp serve      # stdio, for an MCP client
```

## Test

```bash
npm test             # markdown round-trip check, then cargo test
```

`cargo test` also runs `tests/mcp.rs`, which drives `cortex-mcp serve` through a real
MCP client over stdio. Search quality is measured with
`cortex-mcp eval-rag --reindex` against a hand-written set of questions.

## This documentation

Markdown in `documentation/`, built into a styled book by mdBook:

```bash
mdbook build         # writes documentation-html/; open click-here.html
```

The changelog page comes from `CHANGELOG.md`, generated from the commits with
`git cliff -o CHANGELOG.md`.
