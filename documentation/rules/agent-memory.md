# Agent memory

**Rule.** What coding agents learn lives in `dev/` of the vault, in Claude Code's own
memory format, one folder per project plus two shared folders. Claude Code's native
memory writes there directly; `cortex-mcp` writes the same format.

| Folder | Holds | `type` |
|---|---|---|
| `dev/perfil/` | who the user is | `user` |
| `dev/processo/` | corrections that apply to every project | `feedback` |
| `dev/projetos/<project>/` | facts, decisions, pitfalls of one project | `project` and others |

**Format.** Front matter with `name`, `description` and a `metadata` block (`type`,
plus `source`, `created`, `status` when present), then the body. Each folder has a
`MEMORY.md` index, one line per active memory, which Claude Code loads at session start.

**Which project.** The repository of the session (the nearest folder with `.git`) is
looked up in `dev/projetos/_mapa.md`. A repository not in the map is added under its
own folder name. A folder outside git is found only through the map. Parts of one
project (`desk/api`, `desk/web`) share the project's folder.

**Pointing the native memory.** Mapping a repository, and every session start, writes
`autoMemoryDirectory` = `<vault>/dev/projetos/<project>` into the repository's
`.claude/settings.local.json`, and adds that file to `.git/info/exclude`.

**Routing.**

```text
session start:      memories with type user inside dev/projetos/ move to dev/perfil/
after a write:      a feedback memory saved in a project -> the agent is asked whether
                    it applies to every project; if so it calls move_memory -> processo
move_memory:        moves the file and its index line between folders
```

Both triggers are Claude Code hooks configured on the user's machine: a `SessionStart`
hook runs `cortex-mcp project-context`, and a `PostToolUse` hook on `Write|Edit` runs
`cortex-mcp memory-written <file>`.

**Edge cases.** A memory is never moved mid-write (an index would point at a file that
just left). If the agent does not move a correction, it stays in the project and keeps
loading there.

**In the code:** `src-tauri/src/devvault.rs` (`resolve_project`, `point_memory`,
`index_put`, `index_drop`, `Note`), `mcp.rs` (`move_memory`, `sweep_profile`,
`written_memory`), `src/bin/cortex-mcp.rs` (`project-context`, `memory-written`).
