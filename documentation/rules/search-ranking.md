# Search ranking

**Rule.** A search ranks passages twice, by words and by meaning, and fuses the two
lists by position. The search box, a question to the vault, the evaluation and the
MCP server all use this one ranking (`retrieve.rs`).

**Words.** SQLite FTS5 with accents folded (`remove_diacritics 2`), ranked by bm25.
All words must match; if nothing does and there are two or more, any word may match,
and the result says it was widened. Filler words are dropped, and a word of four
letters or more also matches as a prefix.

```text
bm25 weights: text 1, note title 4, path 2
```

**Meaning.** The question is embedded locally (`multilingual-e5-small`, 384
dimensions, prefix `query: `) and every stored vector is scored by cosine similarity.

**Fusion (reciprocal rank fusion).**

```text
score(passage) = sum over both lists of 1 / (60 + position)     position starts at 1
```

| Input | Size | Source |
|---|---|---|
| words list | first 40 passages | `search::run`, `retrieve::WORDS_LIMIT` |
| meaning list | 24 nearest | `embed::nearest`, `retrieve::MAX_CANDIDATES` |
| constant | 60 | `embed::RRF_K` |

**Worked example.** Passage A is 1st by words and 3rd by meaning: 1/61 + 1/63 =
0.0323. Passage B is 1st by meaning and absent from the words list: 1/61 = 0.0164.
A ranks first: agreeing lists beat one strong list.

**After the fusion.**

- Search box: passages grouped by note, at most 3 per note; the best screenshot of a
  note keeps its place past that cap, and a loose copy of a screenshot that also
  lives in a note is hidden. A passage found only by meaning shows how it begins,
  since no word is there to highlight.
- Question: the first 24 passages of at least 80 characters, each trimmed to 1 200
  characters; 8 of them go to the model, numbered for citation.
- MCP server: ranks only among the folders the client may read (see Rules › MCP access).

**Edge cases.** No vectors yet, or the model is not installed: the box uses words
alone, and a question gathers passages term by term instead.

**In the code:** `retrieve.rs` (`retrieve`, `search_box`, `fused`, `by_meaning`),
`search.rs` (`run`, `in_order`), `embed.rs` (`fuse`, `nearest`).
