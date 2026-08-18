use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

use crate::ocr::{OcrBlock, OcrResult};
use crate::vault;

/// How many matching passages a single note contributes before the rest are
/// summarised as a count. Without this one long note fills the whole page.
const CHUNKS_PER_GROUP: u32 = 3;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub chunk_id: i64,
    pub kind: String,
    /// Note path for text hits, image path for OCR hits — always relative to
    /// the vault root.
    pub path: String,
    pub heading_path: String,
    pub snippet: String,
    pub start_offset: i64,
    pub end_offset: i64,
    /// Only for image hits: the boxes Vision drew, so the UI can frame the line
    /// that matched inside the screenshot.
    pub blocks: Vec<OcrBlock>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub key: String,
    pub kind: String,
    pub path: String,
    pub title: String,
    pub folder: String,
    /// Every passage that matched, not only the ones carried in `hits`.
    pub total: i64,
    pub hits: Vec<Hit>,
}

/// Ranks passages, keeps the best few per note, and orders notes by their best
/// passage. The window functions run before the limit on purpose — grouping in
/// the front end would mean the limit had already thrown away other notes.
const RANK_AND_SELECT: &str = r#"
), ranked AS (
  SELECT m.*,
         ROW_NUMBER() OVER (PARTITION BY m.group_key ORDER BY m.score, m.chunk_id) AS rank_in_group,
         ROW_NUMBER() OVER (PARTITION BY m.group_key, m.kind ORDER BY m.score, m.chunk_id) AS rank_in_kind,
         MIN(m.score) OVER (PARTITION BY m.group_key) AS group_score,
         COUNT(*)     OVER (PARTITION BY m.group_key) AS group_total
  FROM matches m
)
SELECT r.group_key, r.chunk_id, r.kind, r.heading_path, r.start_offset, r.end_offset,
       n.path, n.title, n.folder, a.path, a.ocr_json, r.snippet, r.group_total
FROM ranked r
LEFT JOIN notes  n ON n.id = r.note_id
LEFT JOIN assets a ON a.id = r.asset_id
-- The best matching screenshot always keeps its slot, even when the note's prose
-- outranks it. A print buried behind "+2 trechos" is the one thing this app
-- cannot afford to do.
WHERE r.rank_in_group <= ? OR (r.kind = 'image' AND r.rank_in_kind = 1)
ORDER BY r.group_score, r.rank_in_group
LIMIT ?
"#;

/// A screenshot groups under the note that references it, so a word found only
/// inside the image still surfaces under the note it illustrates. An orphan
/// screenshot is its own group.
const GROUP_KEY: &str =
    "CASE WHEN c.note_id IS NOT NULL THEN 'n' || c.note_id ELSE 'a' || c.asset_id END";

fn full_text_sql() -> String {
    format!(
        r#"
WITH matches AS (
  SELECT c.id AS chunk_id, c.kind, c.heading_path, c.start_offset, c.end_offset,
         c.note_id, c.asset_id,
         {GROUP_KEY} AS group_key,
         snippet(chunks_fts, 0, '<<<', '>>>', '…', 14) AS snippet,
         -- The name of a note is a stronger signal than a word buried in it,
         -- and where it lives is worth something too.
         bm25(chunks_fts, 1.0, 4.0, 2.0) AS score
  FROM chunks_fts
  JOIN chunks c ON c.id = chunks_fts.rowid
  LEFT JOIN notes  n ON n.id = c.note_id
  LEFT JOIN assets a ON a.id = c.asset_id
  WHERE chunks_fts MATCH ?1
    AND (?2 IS NULL OR EXISTS (
      SELECT 1 FROM topic_members tm
      WHERE tm.topic_slug = ?2 AND tm.target_path IN (n.path, a.path)
    ))
{RANK_AND_SELECT}"#
    )
}

/// Browsing a topic with an empty query. Without this a topic could only filter
/// a search, never open on its own — and opening it is half the point.
fn topic_only_sql() -> String {
    format!(
        r#"
WITH matches AS (
  SELECT c.id AS chunk_id, c.kind, c.heading_path, c.start_offset, c.end_offset,
         c.note_id, c.asset_id,
         {GROUP_KEY} AS group_key,
         substr(c.text, 1, 180) AS snippet,
         c.id AS score
  FROM chunks c
  LEFT JOIN notes  n ON n.id = c.note_id
  LEFT JOIN assets a ON a.id = c.asset_id
  WHERE EXISTS (
    SELECT 1 FROM topic_members tm
    WHERE tm.topic_slug = ?1 AND tm.target_path IN (n.path, a.path)
  )
{RANK_AND_SELECT}"#
    )
}

/// What a search found, and what it had to do to find it.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub groups: Vec<Group>,
    /// True when nothing matched every word and the search fell back to any of
    /// them. The screen says so — widening in silence is how a tool starts
    /// lying about what it looked for.
    pub widened: bool,
    /// The words actually searched, once the filler was dropped.
    pub terms: Vec<String>,
}

pub fn run(conn: &Connection, query: &str, topic: Option<&str>, limit: u32) -> Result<Found> {
    let terms = terms_of(query);

    if terms.is_empty() {
        let Some(slug) = topic else { return Ok(Found::default()) };
        let mut stmt = conn.prepare_cached(&topic_only_sql())?;
        let rows = stmt.query(rusqlite::params![slug, CHUNKS_PER_GROUP, limit])?;
        return Ok(Found { groups: collect(rows)?, widened: false, terms });
    }

    let search = |expr: &str| -> Result<Vec<Group>> {
        let mut stmt = conn.prepare_cached(&full_text_sql())?;
        let rows = stmt.query(rusqlite::params![expr, topic, CHUNKS_PER_GROUP, limit])?;
        collect(rows)
    };

    let groups = search(&all_of(&terms))?;
    // One word cannot be widened, and a search that found something is not
    // improved by adding what merely mentions one of the words.
    if !groups.is_empty() || terms.len() < 2 {
        return Ok(Found { groups, widened: false, terms });
    }

    Ok(Found { groups: search(&any_of(&terms))?, widened: true, terms })
}

fn collect(mut rows: rusqlite::Rows<'_>) -> Result<Vec<Group>> {
    let mut groups: Vec<Group> = Vec::new();

    while let Some(row) = rows.next()? {
        let group_key: String = row.get(0)?;
        let kind: String = row.get(2)?;
        let note_path: Option<String> = row.get(6)?;
        let note_title: Option<String> = row.get(7)?;
        let note_folder: Option<String> = row.get(8)?;
        let asset_path: Option<String> = row.get(9)?;
        let ocr_json: Option<String> = row.get(10)?;

        let hit_path = match kind.as_str() {
            "image" => asset_path.clone(),
            _ => note_path.clone(),
        }
        .unwrap_or_default();

        let blocks = ocr_json
            .and_then(|raw| serde_json::from_str::<OcrResult>(&raw).ok())
            .map(|ocr| ocr.blocks)
            .unwrap_or_default();

        let hit = Hit {
            chunk_id: row.get(1)?,
            kind,
            path: hit_path.clone(),
            heading_path: row.get(3)?,
            snippet: row.get(11)?,
            start_offset: row.get(4)?,
            end_offset: row.get(5)?,
            blocks,
        };

        if let Some(group) = groups.last_mut().filter(|g| g.key == group_key) {
            group.hits.push(hit);
            continue;
        }

        // The group header follows the note when there is one; an orphan
        // screenshot has to speak for itself.
        let (kind, path, title) = match note_path.clone() {
            Some(path) => {
                let title = note_title.unwrap_or_else(|| file_name_of(&path));
                ("note".to_string(), path, title)
            }
            None => {
                let path = asset_path.unwrap_or_default();
                let title = file_name_of(&path);
                ("image".to_string(), path, title)
            }
        };

        groups.push(Group {
            key: group_key,
            folder: note_folder.unwrap_or_else(|| vault::folder_of(&path)),
            kind,
            path,
            title,
            total: row.get(12)?,
            hits: vec![hit],
        });
    }
    Ok(groups)
}

/// The words a question is made of but a search is not. Dropping them is what
/// lets "Onde estão os assuntos sobre CSS?" find anything at all — and it is
/// also what makes the OR fallback usable: `gestao de tempo` widened with `de`
/// in it matched 159 of 240 notes.
///
/// Both spellings of the accented ones are listed because this runs on what was
/// typed, before SQLite's tokenizer folds anything.
const STOPWORDS: &[&str] = &[
    "a", "à", "as", "às", "ao", "aos", "o", "os", "um", "uma", "uns", "umas", "de", "do", "da",
    "dos", "das", "em", "no", "na", "nos", "nas", "por", "pelo", "pela", "para", "pra", "com",
    "sem", "sobre", "entre", "e", "ou", "mas", "que", "quem", "qual", "quais", "quando", "onde",
    "como", "porque", "é", "era", "foi", "ser", "está", "esta", "estao", "estão", "estou", "tem",
    "têm", "tenho", "ter", "meu", "minha", "meus", "minhas", "seu", "sua", "isso", "isto", "aqui",
    "ali", "lá", "la", "mais", "menos", "muito", "todo", "toda", "todos", "todas", "não", "nao",
    "sim", "the", "of", "in", "on", "for", "and", "or", "what", "where", "how", "is", "are", "my",
    // Asking out loud rather than searching. "Me fale sobre as anotações sobre
    // o blender" was searching for `me` and `fale` too, and `me` alone reached
    // 106 of 240 notes.
    "me", "mim", "fale", "fala", "falar", "diga", "dizer", "conte", "contar", "mostre", "mostra",
    "mostrar", "quero", "queria", "gostaria", "saber", "sabe", "explique", "explica", "explicar",
    "resuma", "resumir", "liste", "listar", "veja", "ver", "procure", "procurar", "busque",
    "buscar", "ache", "achar", "encontre", "encontrar", "existe", "existem", "há", "ha", "tell",
    "show", "find", "about",
];

/// A wildcard on a short word is a wildcard on the language: `"me"*` reaches
/// *meu*, *meio*, *medido*, *memoizar*, *metodologias*. Four letters is where a
/// prefix starts to mean the word rather than the alphabet — and it is the rule
/// that protects against every filler word this list forgets.
const PREFIX_FROM: usize = 4;

/// One term, as FTS5 reads it.
fn quoted(term: &str) -> String {
    if term.chars().count() >= PREFIX_FROM {
        format!("\"{term}\"*")
    } else {
        format!("\"{term}\"")
    }
}

/// The words worth searching for. Punctuation goes, filler goes — but only when
/// something survives it: someone searching for the literal word "onde" has to
/// be able to.
pub fn terms_of(input: &str) -> Vec<String> {
    let words: Vec<String> = input
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
        })
        .filter(|word| !word.is_empty())
        .collect();

    let kept: Vec<String> = words
        .iter()
        .filter(|word| !STOPWORDS.contains(&word.to_lowercase().as_str()))
        .cloned()
        .collect();

    if kept.is_empty() {
        words
    } else {
        kept
    }
}

/// Every word, which is what FTS5 does with terms side by side.
fn all_of(terms: &[String]) -> String {
    terms.iter().map(|term| quoted(term)).collect::<Vec<_>>().join(" ")
}

/// Any of them. bm25 still ranks whoever matched more of the rare ones first.
fn any_of(terms: &[String]) -> String {
    terms.iter().map(|term| quoted(term)).collect::<Vec<_>>().join(" OR ")
}

fn file_name_of(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn builds_a_prefix_query_per_term() {
        assert_eq!(all_of(&terms_of("server actions")), "\"server\"* \"actions\"*");
        assert_eq!(any_of(&terms_of("server actions")), "\"server\"* OR \"actions\"*");
    }

    /// The screen that started this: `me` reached 106 of 240 notes as a prefix.
    #[test]
    fn a_short_word_is_matched_whole_and_not_as_a_prefix() {
        assert_eq!(all_of(&["me".to_string(), "blender".to_string()]), "\"me\" \"blender\"*");
        assert_eq!(all_of(&["css".to_string()]), "\"css\"");
        assert_eq!(all_of(&["node".to_string()]), "\"node\"*");
    }

    #[test]
    fn asking_out_loud_is_not_searching() {
        assert_eq!(terms_of("Me fale sobre as anotações sobre o blender"), ["anotações", "blender"]);
        assert_eq!(terms_of("quero saber sobre hooks"), ["hooks"]);
        // And the safety valve holds: a search made only of filler keeps it.
        assert_eq!(terms_of("me fale"), ["me", "fale"]);
    }

    /// Accents survive to the tokenizer, which folds them; FTS5 syntax does
    /// not survive at all — `OR` and `*` reach SQLite as neither operator nor
    /// term.
    #[test]
    fn keeps_accents_and_drops_syntax() {
        assert_eq!(terms_of("código \"OR\""), ["código"]);
        assert_eq!(terms_of("cache* AND \"x\""), ["cache", "x"]);
        assert!(terms_of("  * ").is_empty());
    }

    /// The question that started this: it found nothing because every word had
    /// to match, filler included.
    #[test]
    fn a_question_is_stripped_down_to_what_it_is_about() {
        assert_eq!(terms_of("Onde esta os assuntos sobre CSS?"), ["assuntos", "CSS"]);
        assert_eq!(terms_of("gestao de tempo"), ["gestao", "tempo"]);
    }

    /// Searching for a filler word itself has to stay possible, or the app
    /// would refuse to look for a word that is in the notes.
    #[test]
    fn a_search_made_only_of_filler_keeps_it() {
        assert_eq!(terms_of("onde"), ["onde"]);
        assert_eq!(terms_of("de que"), ["de", "que"]);
    }

    /// One note with four matching passages plus a screenshot, and a second
    /// note — enough to exercise grouping, the cap, and the image slot.
    fn fixture() -> Connection {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            INSERT INTO notes (id, path, title, folder, mtime, content_hash) VALUES
              (1, 'next/actions.md', 'Server Actions', 'next', 0, 'a'),
              (2, 'react/hooks.md',  'Hooks',          'react', 0, 'b');
            INSERT INTO assets (id, note_id, path, ocr_json, mtime) VALUES
              (1, 1, 'next/_assets/print.png', NULL, 0);
            INSERT INTO chunks (note_id, asset_id, kind, heading_path, text, title, path) VALUES
              (1, NULL, 'text',  'Server Actions',            'cache cache cache', 'Server Actions', 'next/actions.md'),
              (1, NULL, 'text',  'Server Actions › Erros',    'cache de novo',     'Server Actions', 'next/actions.md'),
              (1, NULL, 'text',  'Server Actions › Tags',     'cache marcado',     'Server Actions', 'next/actions.md'),
              (1, NULL, 'text',  'Server Actions › Extra',    'cache extra',       'Server Actions', 'next/actions.md'),
              (1, 1,    'image', 'print.png',                 'cache no print',    'print.png',      'next/_assets/print.png'),
              (2, NULL, 'text',  'Hooks',                     'cache de hooks',    'Hooks',          'react/hooks.md');
            INSERT INTO topics (slug, name) VALUES ('react', 'React');
            INSERT INTO topic_members (topic_slug, target_path) VALUES
              ('react', 'react/hooks.md');
            "#,
        )
        .unwrap();
        conn
    }

    #[test]
    fn gathers_passages_under_one_note() {
        let conn = fixture();
        let groups = run(&conn, "cache", None, 40).unwrap().groups;

        assert_eq!(groups.len(), 2, "one group per note, not one per passage");
        let actions = groups.iter().find(|g| g.title == "Server Actions").unwrap();
        assert_eq!(actions.total, 5);
    }

    #[test]
    fn the_screenshot_keeps_its_slot_past_the_cap() {
        let conn = fixture();
        let groups = run(&conn, "cache", None, 40).unwrap().groups;
        let actions = groups.iter().find(|g| g.title == "Server Actions").unwrap();

        // Three text passages by rank, plus the print that would have been cut.
        assert_eq!(actions.hits.len(), CHUNKS_PER_GROUP as usize + 1);
        assert!(actions.hits.iter().any(|hit| hit.kind == "image"));
    }

    #[test]
    fn a_topic_narrows_the_search() {
        let conn = fixture();
        let groups = run(&conn, "cache", Some("react"), 40).unwrap().groups;

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].title, "Hooks");
    }

    #[test]
    fn a_topic_with_no_query_lists_its_members() {
        let conn = fixture();
        let groups = run(&conn, "", Some("react"), 40).unwrap().groups;

        assert_eq!(groups.len(), 1, "browsing a topic must not need a term");
        assert_eq!(groups[0].title, "Hooks");
        assert!(!groups[0].hits[0].snippet.is_empty());
    }

    #[test]
    fn an_empty_query_without_a_topic_finds_nothing() {
        let conn = fixture();
        assert!(run(&conn, "  ", None, 40).unwrap().groups.is_empty());
    }

    /// The failure that started the round: two words, nothing has both, and the
    /// app said "nada encontrado" over a vault that talks about one of them.
    #[test]
    fn a_search_nothing_matches_whole_is_widened_and_says_so() {
        let conn = fixture();

        let strict = run(&conn, "cache", None, 40).unwrap();
        assert!(!strict.widened, "one word that matches must not widen");

        let found = run(&conn, "cache jamstack", None, 40).unwrap();
        assert!(found.widened, "nothing has both words");
        assert!(!found.groups.is_empty(), "and yet the notes about cache exist");
    }

    /// A note is often named after the only word you remember about it.
    #[test]
    fn the_name_of_a_note_is_searchable() {
        let conn = fixture();
        conn.execute_batch(
            r#"
            INSERT INTO notes (id, path, title, folder, mtime, content_hash) VALUES
              (3, 'node/node-js-fastify.md', 'Fastify', 'node', 0, 'c');
            INSERT INTO chunks (note_id, kind, heading_path, text, title, path) VALUES
              (3, 'text', '', 'um servidor que responde rápido', 'Fastify', 'node/node-js-fastify.md');
            "#,
        )
        .unwrap();

        let found = run(&conn, "fastify", None, 40).unwrap();
        assert_eq!(found.groups.len(), 1);
        assert_eq!(found.groups[0].path, "node/node-js-fastify.md");
        assert!(!found.widened);
    }

    #[test]
    fn an_orphan_screenshot_is_its_own_group() {
        let conn = fixture();
        conn.execute(
            "INSERT INTO assets (id, note_id, path, ocr_json, mtime) VALUES (2, NULL, 'solto/print.png', NULL, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chunks (note_id, asset_id, kind, heading_path, text) VALUES (NULL, 2, 'image', 'print.png', ?1)",
            params!["cache sozinho"],
        )
        .unwrap();

        let groups = run(&conn, "cache", None, 40).unwrap().groups;
        let orphan = groups.iter().find(|g| g.path == "solto/print.png").unwrap();
        assert_eq!(orphan.kind, "image");
        assert_eq!(orphan.folder, "solto");
    }
}
