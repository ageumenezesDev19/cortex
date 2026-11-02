# 0004 Documentation opens from the repository

**Status:** accepted · 2026-11-02

**Context.** The documentation should open styled inside the project, with no server
and no deploy.

**Options.**
- Starlight: Astro links assets from the root (`/_astro/…`), so a page opened as a file
  loses its styles; the workaround is not supported upstream.
- mdBook: relative links and a search index loaded as a script.
- One self-contained HTML file: always opens, but no navigation or search.

**Decision.** mdBook, building into `documentation-html/`, entered through
`click-here.html`. Formulas go in code blocks: MathJax loads from a CDN and shows raw
TeX offline. Checked by opening a build in Safari from the file system.

**Consequences.** This repository is public, so `documentation-html/` stays out of git
(`.git/info/exclude`) and is built locally. Nothing is deployed unless asked.
