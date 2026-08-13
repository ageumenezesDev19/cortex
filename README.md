# Cortex

Local-first knowledge base for course notes and screenshots. Every image dropped
into the vault is run through OCR automatically, so searching finds the text
inside a screenshot the same way it finds prose.

Markdown on disk is the source of truth. The SQLite index under `.index/` is
derived and disposable — delete it and the app rebuilds it on the next launch.

## Requirements

macOS (text recognition uses the system Vision framework), Rust stable, Node 20+.

## Running

```bash
npm install
npm run app        # builds the OCR helper, then starts Tauri in dev mode
```

`npm run app:build` produces a bundle. `npm test` runs the Rust unit tests.

The vault lives at `~/Cortex`. Notes are `.md` files in any folder; screenshots
go in an `_assets/` subfolder beside them, and can be dropped on the window or
saved there from the Finder — the watcher handles both the same way.

## Layout

| Path | What lives there |
|---|---|
| `ocr/main.swift` | Vision CLI helper, bundled as a Tauri sidecar |
| `src-tauri/src/parse.rs` | Markdown chunking by heading, with byte offsets |
| `src-tauri/src/index.rs` | Indexer: hashing, chunks, asset ownership |
| `src-tauri/src/watch.rs` | Filesystem watcher and the serial OCR queue |
| `src-tauri/src/commands.rs` | `search`, `get_note`, `vault_status`, `import_images` |
| `src/` | React front end — search field, results, readers |

## Status

Milestones 1 and 2 of `docs/CORTEX-PLAN.md`: search over notes and screenshots.
Topics, the Notes.app importer, AI search, and the editor are still ahead.
