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
use crate::retrieve::{self, Candidate, Source};
use crate::search;

/// The passages that reach the answer. Eight rather than twenty: the prompt
/// drops from ~6 000 tokens to ~2 400, and Groq's ceiling is 12 000 per minute,
/// so this is the difference between one question a minute and three.
const MAX_SOURCES: usize = 8;
/// Above this the shortlist is long enough to be worth sifting.
const ENOUGH_TO_SIFT: usize = 8;
/// How much of a passage the sifting model reads. Enough to judge, not enough
/// to spend the minute's tokens on a list it is going to throw most of away.
const SIFT_PREVIEW_CHARS: usize = 200;
/// Passages enough to answer from without paying a provider to guess more
/// search terms. Gemini's free tier is 20 requests a day, so a question the
/// plain search already answered must not cost two of them.
///
/// Three because that is what one note can give: the search caps a note at
/// `CHUNKS_PER_GROUP` passages, so a higher bar would mean a single good note
/// never counted as an answer.
const ENOUGH_WITHOUT_HELP: usize = 3;

/// Which model answers what. The expansion gets its own because the free tiers
/// price them differently by a factor of twenty-five.
#[derive(Debug, Clone, Copy)]
pub struct Models<'a> {
    pub groq: &'a str,
    /// In order: each one has its own daily bucket, so an exhausted model is a
    /// reason to try the next rather than to give up on Gemini.
    pub gemini: &'a [&'a str],
    pub gemini_expand: &'a str,
    /// Only Gemini has one; without a key this is simply never reached.
    pub embedding: &'a str,
}

impl<'a> Models<'a> {
    fn chain(&self) -> provider::Chain<'a> {
        provider::Chain { groq: self.groq, gemini: self.gemini }
    }
}

/// Asking where something is and asking what it says are different questions,
/// and only one of them is answered by prose.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Locate,
    Explain,
}

/// One note the subject lives in, with how many passages of it matched. This is
/// the half of the answer that never needs the network.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    pub path: String,
    pub folder: String,
    pub title: String,
    pub passages: usize,
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
    pub kind: Kind,
    /// Where the subject lives, best first.
    pub places: Vec<Place>,
}

/// The old prompt asked for synonyms, and got them: `CSS` came back as "folhas
/// de estilo", "cascading style sheets", "estilo de página" — and the word the
/// person typed left the search. In a developer's vault an acronym is the term.
const EXPAND_SYSTEM: &str = "\
Você recebe uma pergunta sobre um acervo pessoal de notas de programação. \
Responda APENAS com um objeto JSON: {\"termos\": [...], \"tipo\": \"localizar\"} \
ou {\"termos\": [...], \"tipo\": \"explicar\"}. \
Os termos são de 1 a 4 palavras que apareceriam LITERALMENTE escritas nas notas. \
NUNCA traduza: siglas, nomes de tecnologia e termos em inglês ficam como estão \
(CSS continua CSS). Nada de sinônimos em outra língua. \
O tipo é \"localizar\" quando a pergunta é onde algo está, e \"explicar\" quando \
pede o conteúdo. Sem explicação, sem markdown. \
Exemplo: {\"termos\":[\"cache\",\"revalidatePath\"],\"tipo\":\"explicar\"}";

const ANSWER_SYSTEM: &str = "\
Você responde perguntas usando APENAS os trechos numerados fornecidos. \
Cite a fonte de cada afirmação com [n], usando o número do trecho. \
Se os trechos não contiverem a resposta, diga isso claramente em vez de \
adivinhar. Responda em português, de forma direta e curta.";

/// Asked where something is, the useful answer is a tour of the places, not a
/// lecture on the subject.
const LOCATE_SYSTEM: &str = "\
Você diz ONDE um assunto está no acervo, usando APENAS os trechos numerados. \
Para cada nota relevante, diga em uma linha o que ela cobre, citando [n]. \
Não explique o assunto em si. Se os trechos não falarem do assunto, diga isso. \
Responda em português, curto.";

/// A question read locally, before any model sees it: this is what keeps the
/// search working with the AI off, offline, or without a key.
fn local_kind(question: &str) -> Kind {
    let lowered = question.to_lowercase();
    let asks_where = ["onde", "em que pasta", "em qual pasta", "where"]
        .iter()
        .any(|mark| lowered.contains(mark));
    if asks_where {
        Kind::Locate
    } else {
        Kind::Explain
    }
}

/// Adds to the local terms, never replaces them. The model earns its call by
/// suggesting words that might also be in the notes — not by deciding which of
/// the person's own words survive.
fn expand(
    root: &Path,
    question: &str,
    models: Models<'_>,
    local: &[String],
    token: Option<u64>,
) -> (Vec<String>, Kind) {
    let mut terms: Vec<String> = local.to_vec();
    let mut kind = local_kind(question);

    // The lighter model on purpose: pulling keywords is not writing an answer,
    // and on Gemini's free tier this one has 500 requests a day against 20.
    let chain = provider::Chain { groq: models.groq, gemini: &[models.gemini_expand] };
    if let Ok(answer) = provider::ask(root, EXPAND_SYSTEM, question, chain, token) {
        let (suggested, said) = parse_expansion(&answer.text);
        if let Some(said) = said {
            kind = said;
        }
        for term in suggested {
            let known = terms.iter().any(|have| have.eq_ignore_ascii_case(&term));
            if !known && terms.len() < 6 {
                terms.push(term);
            }
        }
    }

    if terms.is_empty() {
        terms.push(question.to_string());
    }
    (terms, kind)
}

/// The object shape asked for, falling back to a bare array — models answer
/// with the older shape often enough that refusing it would cost a call.
fn parse_expansion(raw: &str) -> (Vec<String>, Option<Kind>) {
    let object = raw
        .find('{')
        .zip(raw.rfind('}'))
        .filter(|(open, close)| close > open)
        .and_then(|(open, close)| serde_json::from_str::<serde_json::Value>(&raw[open..=close]).ok());

    let Some(object) = object else { return (parse_terms(raw), None) };

    let terms = object
        .get("termos")
        .map(|value| serde_json::from_value::<Vec<String>>(value.clone()).unwrap_or_default())
        .unwrap_or_default()
        .into_iter()
        .map(|term| term.trim().to_string())
        .filter(|term| !term.is_empty())
        .take(5)
        .collect();

    let kind = match object.get("tipo").and_then(|value| value.as_str()) {
        Some("localizar") => Some(Kind::Locate),
        Some("explicar") => Some(Kind::Explain),
        _ => None,
    };
    (terms, kind)
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

const SIFT_SYSTEM: &str = "\
Você recebe uma pergunta e uma lista numerada de trechos de notas. \
Responda APENAS com os números dos trechos que realmente falam do que a pergunta \
pede, do mais relevante para o menos, separados por vírgula. \
Se nenhum servir, responda 'nenhum'. Sem explicação. Exemplo: 8, 3, 1";

/// The model reads the shortlist and says which passages are about the
/// question. bm25 cannot tell that "Dev (Carreira):" has nothing to do with
/// Blender; a model reading both can.
///
/// Any failure — no key, quota gone, an answer that makes no sense — leaves the
/// bm25 order untouched. A step that adds precision must never subtract an
/// answer.
fn sift(
    root: &Path,
    question: &str,
    candidates: Vec<Candidate>,
    models: Models<'_>,
    token: Option<u64>,
) -> Vec<Candidate> {
    let mut listing = String::new();
    for (at, one) in candidates.iter().enumerate() {
        let preview: String = one.text.chars().take(SIFT_PREVIEW_CHARS).collect();
        listing.push_str(&format!("[{}] {} — {}\n", at + 1, one.source.title, preview));
    }
    listing.push_str(&format!("\nPergunta: {question}"));

    let Ok(answer) = provider::ask(root, SIFT_SYSTEM, &listing, models.chain(), token) else {
        return candidates;
    };

    let chosen = parse_choice(&answer.text, candidates.len());
    if chosen.is_empty() {
        return candidates;
    }
    chosen.into_iter().map(|at| candidates[at].clone()).collect()
}

/// The numbers the model answered with, as indices. A number it invented is
/// dropped rather than trusted — the list is the app's, not the model's.
fn parse_choice(reply: &str, total: usize) -> Vec<usize> {
    let mut seen = Vec::new();
    for piece in reply.split(|c: char| !c.is_ascii_digit()) {
        let Ok(number) = piece.parse::<usize>() else { continue };
        if number >= 1 && number <= total && !seen.contains(&(number - 1)) {
            seen.push(number - 1);
        }
    }
    seen
}

/// Whether any gathered passage actually carries one of the words that were
/// typed — in its text, its title or its path.
fn mentions_any(sources: &[(Source, String)], terms: &[String]) -> bool {
    if terms.is_empty() {
        return true;
    }
    sources.iter().any(|(source, text)| {
        let haystack = format!("{} {} {}", text, source.title, source.path).to_lowercase();
        terms.iter().any(|term| haystack.contains(&term.to_lowercase()))
    })
}

/// The notes the passages came from, most passages first. This is the half of
/// the answer that comes out of the search alone.
fn places_of(sources: &[(Source, String)]) -> Vec<Place> {
    let mut places: Vec<Place> = Vec::new();
    for (source, _) in sources {
        if let Some(place) = places.iter_mut().find(|place| place.path == source.path) {
            place.passages += 1;
            continue;
        }
        places.push(Place {
            folder: crate::vault::folder_of(&source.path),
            path: source.path.clone(),
            title: source.title.clone(),
            passages: 1,
        });
    }
    places.sort_by(|a, b| b.passages.cmp(&a.passages));
    places
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
    models: Models<'_>,
    token: Option<u64>,
) -> Result<Answer, String> {
    let question = question.trim();
    if question.is_empty() {
        return Err("sem pergunta".into());
    }

    let local = search::terms_of(question);
    let mut terms = local.clone();
    let mut kind = local_kind(question);

    let (mut candidates, by_vector) = retrieve::retrieve(root, conn, question, excluded, models.embedding)?;

    // Three roads out of here, and a question only ever walks one of them, so a
    // question costs two calls at most: the retrieval one and the answer. With
    // vectors, `retrieve` already walked the first road.
    if !by_vector {
        // No vectors and too many candidates: the model reads the shortlist and
        // says which ones are about the question. This is what kills the
        // "Meu Nicho:" citation.
        if candidates.len() > ENOUGH_TO_SIFT {
            candidates = sift(root, question, candidates, models, token);
        // No vectors and barely anything: more words are what is missing, not
        // fewer, so the model is asked for terms instead.
        } else if candidates.len() < ENOUGH_WITHOUT_HELP {
            let (wider, said) = expand(root, question, models, &local, token);
            terms = wider;
            kind = said;
            let admits = |path: &str| !retrieve::is_excluded(path, excluded);
            candidates = retrieve::gather(conn, &terms, &admits, retrieve::MIN_SOURCE_CHARS)?;
        }
    }

    candidates.truncate(MAX_SOURCES);
    let sources = numbered(candidates);

    if sources.is_empty() {
        return Err("a busca não achou nada para responder".into());
    }
    // The passages have to be about what was asked. Without this the model was
    // handed whatever bm25 coughed up and paid to answer "there is no mention
    // of CSS in the passages" — a second call to be told the first one failed.
    if !mentions_any(&sources, &local) {
        return Err(format!(
            "as notas não têm nada sobre {}",
            local.join(", ")
        ));
    }

    let places = places_of(&sources);
    let system = if kind == Kind::Locate { LOCATE_SYSTEM } else { ANSWER_SYSTEM };
    let answer = provider::ask(root, system, &prompt(question, &sources), models.chain(), token)?;
    Ok(Answer {
        kind,
        places,
        text: answer.text,
        sources: sources.into_iter().map(|(source, _)| source).collect(),
        provider: answer.provider,
        terms,
    })
}

/// Numbering happens last, because the number is what the answer cites and the
/// order was still moving until now.
fn numbered(candidates: Vec<Candidate>) -> Vec<(Source, String)> {
    candidates
        .into_iter()
        .enumerate()
        .map(|(at, mut one)| {
            one.source.index = at + 1;
            (one.source, one.text)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape asked for now, and the shape models keep answering with.
    #[test]
    fn the_expansion_is_read_as_an_object_or_as_a_bare_list() {
        let (terms, kind) = parse_expansion(r#"{"termos":["cache","tags"],"tipo":"localizar"}"#);
        assert_eq!(terms, ["cache", "tags"]);
        assert_eq!(kind, Some(Kind::Locate));

        let (terms, kind) = parse_expansion(r#"["cache"]"#);
        assert_eq!(terms, ["cache"]);
        assert_eq!(kind, None, "sem tipo declarado, quem decide é a leitura local");
    }

    /// What produced the screenshot that started this: the model answered with
    /// "folhas de estilo" and `CSS` left the search. Now it is an addition.
    #[test]
    fn a_translated_term_is_added_and_never_replaces_what_was_typed() {
        let local = crate::search::terms_of("Onde esta os assuntos sobre CSS?");
        assert_eq!(local, ["assuntos", "CSS"]);

        let (suggested, _) = parse_expansion(r#"{"termos":["folhas de estilo"],"tipo":"localizar"}"#);
        let mut terms = local.clone();
        for term in suggested {
            if !terms.iter().any(|have| have.eq_ignore_ascii_case(&term)) {
                terms.push(term);
            }
        }
        assert!(terms.contains(&"CSS".to_string()));
        assert_eq!(terms.len(), 3);
    }

    /// The cheap road: enough passages from the words as typed means the model
    /// is never asked to invent more of them — two of the twenty saved.
    #[test]
    fn a_search_that_already_answers_does_not_pay_for_an_expansion() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO notes (id, path, title, folder, mtime, content_hash)
             VALUES (1, 'css/grid.md', 'CSS Grid', 'css', 0, 'a');",
        )
        .unwrap();
        for at in 0..6 {
            conn.execute(
                "INSERT INTO chunks (note_id, kind, heading_path, text, title, path)
                 VALUES (1, 'text', ?1, ?2, 'CSS Grid', 'css/grid.md')",
                rusqlite::params![
                    format!("Seção {at}"),
                    format!("Um parágrafo sobre css grid com texto suficiente para valer como fonte, de número {at}."),
                ],
            )
            .unwrap();
        }

        let local = search::terms_of("Onde esta os assuntos sobre CSS?");
        let found = retrieve::gather(&conn, &local, &|_| true, retrieve::MIN_SOURCE_CHARS).unwrap();
        assert!(found.len() >= ENOUGH_WITHOUT_HELP, "achou {}", found.len());

        // And a question the vault knows nothing about stays on the road that
        // asks for help, which is where the extra call earns itself.
        let nothing = retrieve::gather(&conn, &["kubernetes".to_string()], &|_| true, retrieve::MIN_SOURCE_CHARS).unwrap();
        assert!(nothing.len() < ENOUGH_WITHOUT_HELP);
    }

    /// The step that kills the "Meu Nicho:" citation: the model answers with
    /// numbers, and those numbers are read as positions in *our* list.
    #[test]
    fn the_numbers_the_model_chose_become_that_order() {
        assert_eq!(parse_choice("8, 3, 1", 10), [7, 2, 0]);
        assert_eq!(parse_choice("Os relevantes são [8] e [3].", 10), [7, 2]);
        assert_eq!(parse_choice("3, 3, 3", 10), [2], "sem repetir");
    }

    /// A model inventing a number must not reach into the list, and a model
    /// answering nonsense must leave the bm25 order alone.
    #[test]
    fn an_invented_number_is_dropped_and_nonsense_changes_nothing() {
        assert_eq!(parse_choice("99, 4", 5), [3]);
        assert!(parse_choice("nenhum", 5).is_empty());
        assert!(parse_choice("não sei responder", 5).is_empty());
    }

    #[test]
    fn a_question_about_where_is_read_as_one_without_asking_a_model() {
        assert_eq!(local_kind("Onde esta os assuntos sobre CSS?"), Kind::Locate);
        assert_eq!(local_kind("Em qual pasta está o fastify"), Kind::Locate);
        assert_eq!(local_kind("O que é revalidatePath?"), Kind::Explain);
    }

    /// The passages have to be about the question. Three headings that mention
    /// nothing are not an answer, and paying a model to say so is worse.
    #[test]
    fn passages_that_mention_nothing_asked_are_refused() {
        let source = |title: &str, path: &str| Source {
            index: 1,
            path: path.into(),
            title: title.into(),
            heading_path: String::new(),
            start_offset: 0,
        };
        let junk = vec![
            (source("Cultivation", "notes/cultivation.md"), "Cultivation:".to_string()),
            (source("IA", "ia/ia.md"), "IA".to_string()),
        ];
        let terms = vec!["CSS".to_string()];
        assert!(!mentions_any(&junk, &terms));

        let good = vec![(source("Estilo", "css/estilo.md"), "usar css grid aqui".to_string())];
        assert!(mentions_any(&good, &terms));
    }

    /// A hit only in the title still counts — the name of a note is where a
    /// word often lives.
    #[test]
    fn a_mention_in_the_title_is_a_mention() {
        let sources = vec![(
            Source {
                index: 1,
                path: "css/grid.md".into(),
                title: "CSS Grid".into(),
                heading_path: String::new(),
                start_offset: 0,
            },
            "nada aqui sobre o assunto em si".to_string(),
        )];
        assert!(mentions_any(&sources, &["css".to_string()]));
    }

    #[test]
    fn places_count_the_passages_per_note_and_put_the_richest_first() {
        let source = |path: &str| Source {
            index: 1,
            path: path.into(),
            title: path.into(),
            heading_path: String::new(),
            start_offset: 0,
        };
        let sources = vec![
            (source("a/um.md"), String::new()),
            (source("b/dois.md"), String::new()),
            (source("b/dois.md"), String::new()),
        ];
        let places = places_of(&sources);
        assert_eq!(places[0].path, "b/dois.md");
        assert_eq!(places[0].passages, 2);
        assert_eq!(places[0].folder, "b");
        assert_eq!(places.len(), 2);
    }

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

}
