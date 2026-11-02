# 0003 Claude Code's memory lives in the vault

**Status:** accepted · 2026-11-02

**Context.** Claude Code keeps its own memory per repository, in a folder chosen by
where the session started. A session opened in one repository saved what it learned
about another there, and corrections that apply everywhere stayed in one project.

**Options.**
- Turn the native memory off and save only through `remember`: one store, but saving
  depends on the agent remembering to call a tool.
- Keep both: two copies that drift apart.
- Point the native memory at the vault (`autoMemoryDirectory`) and have the server use
  the same format.

**Decision.** Point it at the vault. Tested on 2026-11-02 with Claude Code 2.1.316: a
repository's `.claude/settings.local.json` setting made the native memory save there.

**Consequences.** Two writers, one format, so the server reads and writes Claude Code's
memory files and keeps each folder's `MEMORY.md`. Front matter is no longer indexed as
a passage. Routing to the shared folders is done by hooks and `move_memory` (see Rules ›
Agent memory).
