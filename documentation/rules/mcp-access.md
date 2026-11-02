# MCP access

**Rule.** An MCP client reads only the folders listed in `mcp_folders` and writes only
under `dev/`. Every passage it receives is fenced and labelled as data.

**Reading.**

```text
readable(path) = path is under one of mcp_folders
             and path is not under a folder in `excluded` (the provider exclusions)
             and path is not under dev/avaliacoes/   (evaluation reports)
```

| Scope | What `search` covers |
|---|---|
| `notes` | readable notes outside `dev/` |
| `dev` | `dev/perfil/`, `dev/processo/` and the current project's `dev/projetos/<project>/` |
| `all` (default) | both |

Another project's memories are reached only by naming it (`project`). Index files
(`MEMORY.md`) are never returned as passages.

**Writing.** Only notes under `dev/`, never under `dev/avaliacoes/` (a recorded run is
not rewritten). A memory is marked obsolete instead of deleted.

**Duplicates.** `remember` refuses a memory whose file name exists in the folder, or
whose text is closer than 0.96 (cosine) to a saved one there, and returns that path.
Distinct notes of one project measured 0.922–0.935.

**Fence.** Each passage comes as `<note path="…">…</note>`; a `</note>` inside the text
is escaped, so a note cannot close its own fence.

**Index schema.** The server opens the index read-only first and refuses any schema
version other than its own, without touching the file (see Decisions › 0001).

**Results.** At most 24 passages per search (default 8). The output says when more
matched (`truncated`) and whether vectors ran (`by_meaning`), so an empty answer is not
read as "nothing exists".

**In the code:** `src-tauri/src/mcp.rs` (`readable`, `search`, `remember`),
`devvault.rs` (`writable`), `db.rs` (`open_current`).
