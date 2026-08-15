//! Asking a question of the vault.
//!
//! Two passes over the same model. The first turns a question into search terms
//! — which is the job embeddings would have done, done with the index that is
//! already here. The second answers over the passages the search found, citing
//! them by number so a citation can jump to the passage the same way a search
//! result does.

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::provider::{self, Provider};
use crate::search;

/// Enough passages to answer from, few enough that a free tier will take them.
const MAX_SOURCES: usize = 20;
/// Long passages are trimmed rather than dropped: half a passage still supports
/// a citation, a missing one supports nothing.
const MAX_CHARS: usize = 1_200;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub text: String,
    pub sources: Vec<Source>,
    /// Which provider served it, so the panel can say so.
    pub provider: Provider,
    /// The terms the question was expanded into, shown so the retrieval is not
    /// a black box.
    pub terms: Vec<String>,
}

const EXPAND_SYSTEM: &str = "\
Você recebe uma pergunta sobre um acervo pessoal de notas em português. \
Responda APENAS com uma lista JSON de 2 a 5 termos de busca que encontrariam a \
resposta, incluindo sinônimos e termos técnicos relacionados. \
Sem explicação, sem markdown, só a lista. Exemplo: [\"cache\",\"revalidatePath\"]";

const ANSWER_SYSTEM: &str = "\
Você responde perguntas usando APENAS os trechos numerados fornecidos. \
Cite a fonte de cada afirmação com [n], usando o número do trecho. \
Se os trechos não contiverem a resposta, diga isso claramente em vez de \
adivinhar. Responda em português, de forma direta e curta.";

/// Turns the question into search terms. A failure here is not fatal — the
/// question itself is a serviceable term, and the answer pass still runs.
fn expand(root: &Path, question: &str, models: (&str, &str)) -> Vec<String> {
    let Ok(answer) = provider::ask(root, EXPAND_SYSTEM, question, models) else {
        return vec![question.to_string()];
    };
    let terms = parse_terms(&answer.text);
    if terms.is_empty() {
        vec![question.to_string()]
    } else {
        terms
    }
}

/// Models wrap JSON in prose or fences often enough that the brackets are found
/// rather than assumed.
fn parse_terms(raw: &str) -> Vec<String> {
    let (open, close) = match (raw.find('['), raw.rfind(']')) {
        (Some(open), Some(close)) if close > open => (open, close),
        _ => return Vec::new(),
    };
    serde_json::from_str::<Vec<String>>(&raw[open..=close])
        .unwrap_or_default()
        .into_iter()
        .map(|term| term.trim().to_string())
        .filter(|term| !term.is_empty())
        .take(5)
        .collect()
}

/// Gathers passages for every term, best first, without repeating a passage.
fn gather(
    conn: &Connection,
    terms: &[String],
    excluded: &[String],
) -> Result<Vec<(Source, String)>, String> {
    let mut sources = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for term in terms {
        let groups = search::run(conn, term, None, 8).map_err(|error| error.to_string())?;
        for group in groups {
            if is_excluded(&group.path, excluded) {
                continue;
            }
            for hit in group.hits {
                if sources.len() >= MAX_SOURCES {
                    return Ok(sources);
                }
                if !seen.insert(hit.chunk_id) {
                    continue;
                }
                let Some(text) = chunk_text(conn, hit.chunk_id) else { continue };

                sources.push((
                    Source {
                        index: sources.len() + 1,
                        path: group.path.clone(),
                        title: group.title.clone(),
                        heading_path: hit.heading_path,
                        start_offset: hit.start_offset,
                    },
                    trim(&text),
                ));
            }
        }
    }
    Ok(sources)
}

/// A path under an excluded prefix never leaves the machine — not as a passage,
/// not as a title.
pub fn is_excluded(path: &str, excluded: &[String]) -> bool {
    excluded.iter().any(|prefix| {
        let prefix = prefix.trim_end_matches('/');
        !prefix.is_empty() && (path == prefix || path.starts_with(&format!("{prefix}/")))
    })
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

/// The passages, numbered the way the answer must cite them.
fn prompt(question: &str, sources: &[(Source, String)]) -> String {
    let mut out = String::from("Trechos:\n\n");
    for (source, text) in sources {
        out.push_str(&format!(
            "[{}] {} › {}\n{}\n\n",
            source.index,
            source.title,
            if source.heading_path.is_empty() { "—" } else { &source.heading_path },
            text
        ));
    }
    out.push_str(&format!("Pergunta: {question}"));
    out
}

pub fn run(
    root: &Path,
    conn: &Connection,
    question: &str,
    excluded: &[String],
    models: (&str, &str),
) -> Result<Answer, String> {
    let question = question.trim();
    if question.is_empty() {
        return Err("sem pergunta".into());
    }

    let terms = expand(root, question, models);
    let sources = gather(conn, &terms, excluded)?;
    if sources.is_empty() {
        return Err("a busca não achou nada para responder".into());
    }

    let answer = provider::ask(root, ANSWER_SYSTEM, &prompt(question, &sources), models)?;
    Ok(Answer {
        text: answer.text,
        sources: sources.into_iter().map(|(source, _)| source).collect(),
        provider: answer.provider,
        terms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_are_read_out_of_prose_and_fences() {
        assert_eq!(parse_terms(r#"["cache","revalidatePath"]"#), ["cache", "revalidatePath"]);
        assert_eq!(parse_terms("Claro!\n```json\n[\"a\", \"b\"]\n```"), ["a", "b"]);
        assert!(parse_terms("não consegui").is_empty());
    }

    #[test]
    fn at_most_five_terms_survive() {
        let many = r#"["a","b","c","d","e","f","g"]"#;
        assert_eq!(parse_terms(many).len(), 5);
    }

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
    fn the_prompt_numbers_the_passages_it_asks_to_be_cited() {
        let sources = vec![
            (
                Source {
                    index: 1,
                    path: "a.md".into(),
                    title: "Server Actions".into(),
                    heading_path: "Cache".into(),
                    start_offset: 0,
                },
                "revalidatePath descarta a rota".into(),
            ),
            (
                Source {
                    index: 2,
                    path: "b.md".into(),
                    title: "Outra".into(),
                    heading_path: String::new(),
                    start_offset: 10,
                },
                "outro texto".into(),
            ),
        ];
        let built = prompt("como invalidar?", &sources);
        assert!(built.contains("[1] Server Actions › Cache"));
        assert!(built.contains("[2] Outra › —"));
        assert!(built.ends_with("Pergunta: como invalidar?"));
    }

    #[test]
    fn a_long_passage_is_trimmed_rather_than_dropped() {
        let long = "a".repeat(MAX_CHARS + 500);
        let trimmed = trim(&long);
        assert_eq!(trimmed.chars().count(), MAX_CHARS + 1);
        assert!(trimmed.ends_with('…'));
    }
}
