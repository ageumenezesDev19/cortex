# Passages

**Rule.** A note is cut into passages at each heading, and a long section is cut
again at the first block that ends past 1 200 characters. A passage is the unit the
search ranks and the answer cites.

**Cut.**

```text
new passage at every heading
new passage after the paragraph, list item, code block or table
  that ends once the passage holds >= 1 200 characters    (parse::SPLIT_AT)
```

Nothing is cut mid-block, so a sentence or a code block is never split in two. A cut
passage keeps the heading trail of its section.

| Input | Unit | Source |
|---|---|---|
| note text | characters | the `.md` file |
| heading trail | text, joined with ` › ` | `parse::HEADING_SEPARATOR` |

**What is left out.** Front matter (`---` block at the top). A heading with no text
under it.

**What the vector reads.** A text passage is embedded as `passage: <trail>\n<text>`;
a screenshot passage as its OCR text. The model reads about 512 tokens, so passages
are kept short enough to be read whole.

**Worked example.** Under `## Cache`, paragraphs of 900, 700 and 500 characters. After
the first the passage holds 900, under 1 200, so it goes on; after the second it holds
1 600, so it ends there. The third paragraph starts a second passage, still under
`Cache`.

**Edge cases.** One block longer than 1 200 characters stays whole. OCR text of a
screenshot is not cut.

**In the code:** `src-tauri/src/parse.rs`, `chunk_markdown()`; `embed.rs`, `input_of()`.
