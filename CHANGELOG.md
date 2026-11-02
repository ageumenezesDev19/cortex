# Changelog

What changed, newest first. Generated from the commit history with git-cliff.


### 2026-11-02

- Cortex-mcp: the vault over MCP, with Claude Code's memory kept in it

### 2026-10-29

- Cut passages at 1 200 characters and fuse the box ranking with vectors
- Eval-rag: reindex the copy, keep it on request, leave the reports out
- Measure the search against a gold set: cortex-mcp eval-rag
- Drop screenshots into notes; show each print once

### 2026-08-18

- Describe the demo and license the project under MIT
- Split the demo build so a host without cargo can run it
- Run the whole app in a browser over a demo vault

### 2026-08-15

- The results list would not scroll on a phone
- Mark what a screenshot is when it arrives
- Ask the vault a question, with citations that jump to the passage

### 2026-08-14

- The create menu was painted over by the search field
- The iOS keyboard was eating the interface
- The interface bugs the phone showed
- The server switches live in the app, not in a terminal
- Every command called itself instead of the app
- Serve the vault over HTTP so the phone is a window, not a second app
- The import shattered 131 of 232 titles
- Import the Notes.app library as markdown (M4)
- Convert Notes.app HTML to markdown
- Screenshots with spaces broke the notes that embedded them
- Drop the square focus ring stacked on rounded fields
- Open images from the tree, and say when a screenshot lands
- Icon had an opaque white margin baked in
- Say what deleting a topic really costs, without code jargon
- Make creating a note obvious, and file it on the way in
- Edit notes in the rendered text, with a round-trip guard
- Clearing the search closes the note it opened
- Write, browse and edit inside the app; own icon (M6)
- Group results by note and add topics (M3)

### 2026-08-13

- Search over notes and screenshots (M1 + M2)