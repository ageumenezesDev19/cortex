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

## Web demo

The same front end, running entirely in the browser over a vault of invented notes.
Every command the app would send to Rust is answered by `src/demo/` instead — so
searching, writing, topics, and dropping a screenshot in all work, and what you
change persists in IndexedDB. The seed passages are chunked and embedded ahead of
time by `src-tauri/examples/gerar_demo.rs`, using the app's own chunker and the same
`multilingual-e5-small` model, so a passage ranks where it would on the desktop.

```bash
npm run demo          # regenerates the vectors (needs Rust), then builds
npm run demo:web      # build only, from the committed vectors
npm run demo:preview  # serve dist-demo/
```

Three things differ from the desktop. There is no vault on disk and no watcher —
the browser is the storage. OCR is Tesseract compiled to WebAssembly rather than
Apple Vision, loaded on first use. And the answer feature reports itself
unconfigured, because a static demo has nowhere to keep an API key.

The build is 2 MB, but embedding a question in the browser needs weights the page
does not ship: roughly 160 MB of runtime, tokenizer and quantised model, fetched
from jsDelivr and Hugging Face and cached by the browser afterwards. The download
starts when the page opens and the word search answers in the meantime, so the
first search is not held behind it.

## Layout

| Path | What lives there |
|---|---|
| `ocr/main.swift` | Vision CLI helper, bundled as a Tauri sidecar |
| `src-tauri/src/parse.rs` | Markdown chunking by heading, with byte offsets |
| `src-tauri/src/index.rs` | Indexer: hashing, chunks, asset ownership |
| `src-tauri/src/watch.rs` | Filesystem watcher and the serial OCR queue |
| `src-tauri/src/commands.rs` | `search`, `get_note`, `vault_status`, `import_images` |
| `src/` | React front end — search field, results, readers |
| `src/demo/` | Browser stand-ins for the backend: search, embedding, storage |
| `demo/` | The demo vault and its precomputed vectors |

## Status

Search across notes and screenshots, automatic OCR on anything dropped in, topics,
the editor, the Notes.app importer, and questions answered with citations that jump
to the passage.
