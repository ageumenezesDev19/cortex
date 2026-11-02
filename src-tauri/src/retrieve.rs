//! What a question finds before any model is called.
//!
//! One road for every reader — the answer, the search box, the evaluation and
//! the MCP server — so they cannot drift into ranking the same vault two ways.

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::search;

/// The shortlist before anything ranks it.
const MAX_CANDIDATES: usize = 24;
/// What the box itself asks for, so the words side of a fusion is its ranking.
const WORDS_LIMIT: usize = 40;
/// Read past the limit so a caller that only admits some folders still gets a
/// full list. The order is the same, so admitting everything changes nothing.
const WORDS_POOL: u32 = 200;
/// Long passages are trimmed rather than dropped: half a passage still supports
/// a citation, a missing one supports nothing.
const MAX_CHARS: usize = 1_200;
/// Below this a passage is a heading or a stray line — it cannot support an
/// answer, and bm25 ranks it above real prose precisely because it is short.
pub(crate) const MIN_SOURCE_CHARS: usize = 80;
/// A memory in the dev vault may be a single line, and is still the answer.
const MIN_MEMORY_CHARS: usize = 1;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// The number the answer cites, starting at 1.
    pub index: usize,
    pub path: String,
    pub title: String,
    pub heading_path: String,
    /// Byte offset of the passage, so a citation lands on the anchor the same
    /// way a search result does.
    pub start_offset: i64,
}

/// One passage, with everything a citation needs and the id the rankings speak
/// in.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub chunk_id: i64,
    pub source: Source,
    pub text: String,
}

/// Gathers passages for every term, the ones answering most of them first.
///
/// Ordered by how many terms a passage carries, not by which term came first:
/// walking term by term gave the top places to whatever matched "arquivos",
/// and the fusion downstream reads that order as relevance.
pub(crate) fn gather(
    conn: &Connection,
    terms: &[String],
    admits: &dyn Fn(&str) -> bool,
    min_chars: usize,
) -> Result<Vec<Candidate>, String> {
    let mut found_candidates: Vec<Candidate> = Vec::new();
    let mut seen: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    // How many distinct terms each passage answered, in step with the vector
    // above so the sort can carry both.
    let mut hits_per_candidate: Vec<usize> = Vec::new();

    for term in terms {
        let found = search::run(conn, term, None, 8).map_err(|error| error.to_string())?;
        for group in found.groups {
            if !admits(&group.path) {
                continue;
            }
            for hit in group.hits {
                if let Some(&at) = seen.get(&hit.chunk_id) {
                    // Seen for an earlier term: that is the signal, not a
                    // duplicate to drop.
                    hits_per_candidate[at] += 1;
                    continue;
                }
                let Some(text) = chunk_text(conn, hit.chunk_id) else { continue };
                // A passage too short to say anything is not worth a citation,
                // and it is what the model was being handed: a heading, alone.
                if text.trim().chars().count() < min_chars {
                    continue;
                }

                seen.insert(hit.chunk_id, found_candidates.len());
                hits_per_candidate.push(1);
                found_candidates.push(Candidate {
                    chunk_id: hit.chunk_id,
                    source: Source {
                        index: 0,
                        path: group.path.clone(),
                        title: group.title.clone(),
                        heading_path: hit.heading_path,
                        start_offset: hit.start_offset,
                    },
                    text: trim(&text),
                });
            }
        }
    }

    // Stable, so passages answering the same number of terms keep the order
    // bm25 gave them inside each term's own results.
    let mut order: Vec<usize> = (0..found_candidates.len()).collect();
    order.sort_by_key(|&at| std::cmp::Reverse(hits_per_candidate[at]));

    // Capped after ranking rather than during collection: cutting at
    // MAX_CANDIDATES mid-walk threw away passages that answered every term
    // simply because a common word filled the list first.
    Ok(order
        .into_iter()
        .take(MAX_CANDIDATES)
        .map(|at| found_candidates[at].clone())
        .collect())
}

/// One passage by id, for a hit the vector found and the words did not.
fn candidate_of(
    conn: &Connection,
    chunk_id: i64,
    admits: &dyn Fn(&str) -> bool,
    min_chars: usize,
) -> Option<Candidate> {
    let row = conn
        .query_row(
            "SELECT c.text, c.heading_path, c.start_offset,
                    COALESCE(n.path, a.path, c.path), COALESCE(n.title, c.title)
             FROM chunks c
             LEFT JOIN notes n  ON n.id = c.note_id
             LEFT JOIN assets a ON a.id = c.asset_id
             WHERE c.id = ?1",
            [chunk_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .ok()?;

    let (text, heading_path, start_offset, path, title) = row;
    // The exclusion is checked here too: a vector hit never went through the
    // search, so this is the only gate on its way out.
    if !admits(&path) || text.trim().chars().count() < min_chars {
        return None;
    }
    Some(Candidate {
        chunk_id,
        source: Source { index: 0, path, title, heading_path, start_offset },
        text: trim(&text),
    })
}

/// A path under an excluded prefix never leaves the machine — not as a passage,
/// not as a title.
pub fn is_excluded(path: &str, excluded: &[String]) -> bool {
    is_under(path, excluded)
}

/// Whether `path` is one of the folders or inside one. The same rule decides
/// what is excluded and what an MCP client may read.
pub fn is_under(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| {
        let prefix = prefix.trim_end_matches('/');
        !prefix.is_empty() && (path == prefix || path.starts_with(&format!("{prefix}/")))
    })
}

fn path_of(conn: &Connection, chunk_id: i64) -> Option<String> {
    conn.query_row(
        "SELECT COALESCE(n.path, a.path, c.path) FROM chunks c
         LEFT JOIN notes n  ON n.id = c.note_id
         LEFT JOIN assets a ON a.id = c.asset_id
         WHERE c.id = ?1",
        [chunk_id],
        |row| row.get(0),
    )
    .ok()
}

fn chunk_text(conn: &Connection, chunk_id: i64) -> Option<String> {
    conn.query_row("SELECT text FROM chunks WHERE id = ?1", [chunk_id], |row| row.get(0))
        .ok()
}

fn trim(text: &str) -> String {
    if text.chars().count() <= MAX_CHARS {
        return text.to_string();
    }
    text.chars().take(MAX_CHARS).collect::<String>() + "…"
}

/// What a question finds before any model is called: the passages for its words,
/// fused with the vectors when the vault has them. The flag says whether the
/// vectors ran; `run` takes a different road without them, and an evaluation
/// that silently measured words alone would report the wrong search.
pub fn retrieve(
    root: &Path,
    conn: &Connection,
    question: &str,
    excluded: &[String],
    embedding: &str,
) -> Result<(Vec<Candidate>, bool), String> {
    // Ranked over everything and filtered after: ranking only the admitted
    // passages moved two gold questions down a place (measured 2026-11-02).
    let admits = |path: &str| !is_excluded(path, excluded);
    ranked(root, conn, question, &|_| true, &admits, MIN_SOURCE_CHARS, embedding)
}

/// `retrieve` for a caller that decides path by path what may come back, the
/// MCP server's allowed folders. Ranked only among what it admits, so a narrow
/// scope still gets a full list.
pub fn retrieve_within(
    root: &Path,
    conn: &Connection,
    question: &str,
    admits: &dyn Fn(&str) -> bool,
    embedding: &str,
) -> Result<(Vec<Candidate>, bool), String> {
    ranked(root, conn, question, admits, admits, MIN_MEMORY_CHARS, embedding)
}

fn ranked(
    root: &Path,
    conn: &Connection,
    question: &str,
    rank_among: &dyn Fn(&str) -> bool,
    admits: &dyn Fn(&str) -> bool,
    min_chars: usize,
    embedding: &str,
) -> Result<(Vec<Candidate>, bool), String> {
    let Some((order, _)) = fused(root, conn, question, None, rank_among, admits, min_chars, embedding)? else {
        return Ok((gather(conn, &search::terms_of(question), admits, min_chars)?, false));
    };
    let candidates = order
        .iter()
        .filter_map(|id| candidate_of(conn, *id, admits, min_chars))
        .take(MAX_CANDIDATES)
        .collect();
    Ok((candidates, true))
}

/// The search box: the same fusion as a question, grouped by note. Without
/// vectors, or with nothing typed inside a topic, it is the word search alone.
pub fn search_box(
    root: &Path,
    conn: &Connection,
    query: &str,
    topic: Option<&str>,
    limit: u32,
    embedding: &str,
) -> Result<search::Found, String> {
    let words_only = || search::run(conn, query, topic, limit).map_err(|error| error.to_string());
    if search::terms_of(query).is_empty() {
        return words_only();
    }
    // The box never sends anything anywhere, so the provider exclusions do not
    // apply to it.
    let Some((order, found)) = fused(root, conn, query, topic, &|_| true, &|_| true, MIN_SOURCE_CHARS, embedding)? else {
        return words_only();
    };
    let groups = search::in_order(conn, &order, &found, topic, limit).map_err(|error| error.to_string())?;
    Ok(search::Found { groups, widened: found.widened, terms: found.terms })
}

/// The words' ranking fused with the vectors', best first, with the word search
/// it came from. `None` when there are no vectors to fuse with.
fn fused(
    root: &Path,
    conn: &Connection,
    query: &str,
    topic: Option<&str>,
    rank_among: &dyn Fn(&str) -> bool,
    admits: &dyn Fn(&str) -> bool,
    min_chars: usize,
    embedding: &str,
) -> Result<Option<(Vec<i64>, search::Found)>, String> {
    // The vector knows what the words could not, so nothing else is needed: its
    // ranking and the search's are fused and that is the shortlist.
    let Some(semantic) = by_meaning(root, conn, query, rank_among, admits, min_chars, embedding) else {
        return Ok(None);
    };
    // The words side is the search box's own ranking of the whole question.
    // Gathering term by term kept three passages per note for each word, and a
    // long note cut into several passages hid the right one behind that cap.
    let found = search::run(conn, query, topic, WORDS_POOL).map_err(|error| error.to_string())?;
    let by_words: Vec<i64> = found
        .groups
        .iter()
        .filter(|group| rank_among(&group.path))
        .flat_map(|group| group.hits.iter().map(|hit| hit.chunk_id))
        .take(WORDS_LIMIT)
        .collect();
    Ok(Some((crate::embed::fuse(&by_words, &semantic), found)))
}

/// The chunk ids closest in meaning, or nothing when the vault has no vectors —
/// which is every vault until someone asks for them, and every vault with no
/// Gemini key.
fn by_meaning(
    root: &Path,
    conn: &Connection,
    question: &str,
    rank_among: &dyn Fn(&str) -> bool,
    admits: &dyn Fn(&str) -> bool,
    min_chars: usize,
    embedding: &str,
) -> Option<Vec<i64>> {
    let vectors = crate::embed::all(conn).ok().filter(|all| !all.is_empty())?;
    let asked = crate::embed::embed_one(root, embedding, question).ok()?;

    let near = crate::embed::nearest(&vectors, &asked, vectors.len());
    let allowed: Vec<i64> = near
        .into_iter()
        .filter(|id| path_of(conn, *id).is_some_and(|path| rank_among(&path)))
        .take(MAX_CANDIDATES)
        .filter(|id| candidate_of(conn, *id, admits, min_chars).is_some())
        .collect();
    (!allowed.is_empty()).then_some(allowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one rule that keeps a note off the network.
    #[test]
    fn an_excluded_prefix_covers_the_folder_and_the_note_itself() {
        let excluded = vec!["_importado/projects-dev".to_string()];
        assert!(is_excluded("_importado/projects-dev/project-komerben.md", &excluded));
        assert!(is_excluded("_importado/projects-dev", &excluded));
    }

    /// A prefix must not match a sibling that merely starts with the same
    /// letters, or excluding one folder would silently gag another.
    #[test]
    fn a_neighbour_with_a_similar_name_is_not_excluded() {
        let excluded = vec!["notes".to_string()];
        assert!(!is_excluded("notes-antigas/x.md", &excluded));
        assert!(is_excluded("notes/x.md", &excluded));
    }

    #[test]
    fn an_empty_exclusion_gags_nothing() {
        assert!(!is_excluded("qualquer/nota.md", &["".to_string(), "/".to_string()]));
    }


    #[test]
    fn a_long_passage_is_trimmed_rather_than_dropped() {
        let long = "a".repeat(MAX_CHARS + 500);
        let trimmed = trim(&long);
        assert_eq!(trimmed.chars().count(), MAX_CHARS + 1);
        assert!(trimmed.ends_with('…'));
    }
}
