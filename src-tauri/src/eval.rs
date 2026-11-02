//! Measuring the search against questions whose answer is known.
//!
//! The questions live in the vault, written and approved by hand
//! (`dev/avaliacoes/rag/gold.tsv`). A hit is a passage from the expected note that
//! carries the expected words, so the same questions still hold after the way
//! notes are cut into passages changes.
//!
//! It never reads the live index. The installed app may be on an older schema that
//! this build would migrate, and the app would then trip over the newer one; so
//! the index is copied, the copy is migrated, and the copy is measured.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{Connection, OpenFlags};

use crate::{db, embed, index, retrieve, search, settings, vault};

/// Deep enough to tell "found, but low" from "not found".
const DEPTH: usize = 10;
/// What the search box asks for.
const BOX_LIMIT: u32 = 40;
const GOLD: &str = "dev/avaliacoes/rag/gold.tsv";
const REPORTS: &str = "dev/avaliacoes/rag";

pub struct Question {
    pub id: String,
    pub group: String,
    pub text: String,
    pub note: String,
    pub evidence: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Words only: the control, and what the box falls back to without vectors.
    Words,
    /// Words fused with vectors: what the box and a question both rank by.
    Hybrid,
}

pub struct Outcome {
    /// 1-based position of the first right passage, if it is within `DEPTH`.
    pub rank: Option<usize>,
    /// Hybrid only: whether the words survive the cut to what the model is sent.
    pub visible: Option<bool>,
}

/// `eval-rag [--label name] [--gold path] [--reindex]`: measures both searches,
/// writes the report next to the gold set and returns its path.
pub fn cli(args: &[String]) -> Result<PathBuf> {
    let label = flag(args, "--label").unwrap_or("medicao");
    let reindex = args.iter().any(|arg| arg == "--reindex");
    let root = vault::root()?;
    let gold = flag(args, "--gold").map(PathBuf::from).unwrap_or_else(|| root.join(GOLD));
    let questions = load_gold(&gold)?;

    let (conn, snapshot) = snapshot(&root, reindex)?;
    let measured = measure_both(&root, &conn, &questions)
        .and_then(|both| Ok((both, census(&root, &conn, reindex)?)));
    drop(conn);
    // `--keep` leaves the measured copy behind, for when a number needs explaining.
    if args.iter().any(|arg| arg == "--keep") {
        eprintln!("cópia medida: {}", snapshot.display());
    } else {
        let _ = std::fs::remove_file(&snapshot);
    }
    let ((words, hybrid), census) = measured?;

    let report = report(label, &questions, &words, &hybrid, &census);
    print!("{}", summary(&questions, &words, &hybrid));

    let out = root.join(REPORTS).join(format!("{}-{label}.md", today()?));
    // A recorded result is never rewritten; a second run the same day takes
    // another label.
    if out.exists() {
        bail!("{} já existe; rode com outro --label", out.display());
    }
    std::fs::write(&out, report).with_context(|| format!("gravando {}", out.display()))?;
    Ok(out)
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|arg| arg == name).and_then(|at| args.get(at + 1)).map(String::as_str)
}

pub fn load_gold(path: &Path) -> Result<Vec<Question>> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("lendo {}", path.display()))?;
    let mut questions = Vec::new();
    for (number, line) in raw.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') || line.starts_with("id\t") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [id, group, text, note, evidence] = fields[..] else {
            bail!("linha {} do gabarito tem {} colunas, não 5", number + 1, fields.len());
        };
        questions.push(Question {
            id: id.into(),
            group: group.into(),
            text: text.into(),
            note: note.into(),
            evidence: evidence.into(),
        });
    }
    if questions.is_empty() {
        bail!("o gabarito {} não tem perguntas", path.display());
    }
    Ok(questions)
}

/// A consistent copy of the index, migrated to this build's schema.
fn snapshot(root: &Path, reindex: bool) -> Result<(Connection, PathBuf)> {
    let live = Connection::open_with_flags(vault::db_path(root), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .context("abrindo o índice só para leitura")?;
    let copy = std::env::temp_dir().join(format!("cortex-eval-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&copy);
    live.execute("VACUUM INTO ?1", [copy.to_string_lossy()]).context("copiando o índice")?;
    let conn = db::open(&copy)?;
    if reindex {
        rebuild(root, &copy, &conn)?;
    }
    // The reports quote every question word for word: left in, they are the best
    // match for each one and the search ends up measuring itself.
    conn.execute_batch(
        "DELETE FROM embeddings WHERE chunk_id IN (SELECT id FROM chunks WHERE path LIKE 'dev/avaliacoes/%');
         DELETE FROM chunks WHERE path LIKE 'dev/avaliacoes/%';
         DELETE FROM notes WHERE path LIKE 'dev/avaliacoes/%';",
    )?;
    Ok((conn, copy))
}

/// Recuts every note with this build's parser and re-embeds what changed, so a
/// change to how notes are cut is measured on the cut it makes. Screenshots keep
/// the OCR they already have: this is about passages, not reading pictures again.
fn rebuild(root: &Path, copy: &Path, conn: &Connection) -> Result<()> {
    conn.execute("UPDATE notes SET content_hash = ''", [])?;
    index::Indexer::new(root.to_path_buf(), db::open(copy)?).scan()?;
    let model = settings::load(root).embedding_model;
    embed::drain(root, conn, &model, |_, _| {}, || false).map_err(|error| anyhow!(error))?;
    Ok(())
}

fn measure_both(
    root: &Path,
    conn: &Connection,
    questions: &[Question],
) -> Result<(Vec<Outcome>, Vec<Outcome>)> {
    // Without vectors the hybrid column would quietly measure words twice.
    if !embed::model_present() || embed::all(conn)?.is_empty() {
        bail!(
            "sem modelo de vetores ou sem vetores na cópia, e a busca híbrida não roda. \
             Se a migração deste build esvaziou os vetores, rode com --reindex"
        );
    }
    Ok((measure(root, conn, questions, Mode::Words)?, measure(root, conn, questions, Mode::Hybrid)?))
}

pub fn measure(root: &Path, conn: &Connection, questions: &[Question], mode: Mode) -> Result<Vec<Outcome>> {
    let settings = settings::load(root);
    let mut outcomes = Vec::new();
    for question in questions {
        let outcome = match mode {
            Mode::Words => {
                let found = search::run(conn, &question.text, None, BOX_LIMIT)?;
                let ids: Vec<i64> =
                    found.groups.iter().flat_map(|group| group.hits.iter().map(|hit| hit.chunk_id)).collect();
                Outcome { rank: first_right(conn, &ids, question)?, visible: None }
            }
            Mode::Hybrid => {
                let (candidates, by_vector) =
                    retrieve::retrieve(root, conn, &question.text, &settings.excluded, &settings.embedding_model)
                        .map_err(|error| anyhow!(error))?;
                if !by_vector {
                    bail!("{}: os vetores não rodaram", question.id);
                }
                let ids: Vec<i64> = candidates.iter().map(|one| one.chunk_id).collect();
                let rank = first_right(conn, &ids, question)?;
                let visible = rank.map(|at| contains(&candidates[at - 1].text, &question.evidence));
                Outcome { rank, visible }
            }
        };
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

/// The first passage within `DEPTH` that comes from the expected note and carries
/// the expected words. The full passage is read from the index, not the trimmed
/// copy a candidate holds: this is about what the search found.
pub fn first_right(conn: &Connection, ids: &[i64], question: &Question) -> Result<Option<usize>> {
    let mut stmt = conn.prepare_cached(
        "SELECT COALESCE(n.path, a.path, c.path), c.text
           FROM chunks c
           LEFT JOIN notes n  ON n.id = c.note_id
           LEFT JOIN assets a ON a.id = c.asset_id
          WHERE c.id = ?1",
    )?;
    for (at, id) in ids.iter().take(DEPTH).enumerate() {
        let (path, text): (String, String) = stmt.query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        if path == question.note && contains(&text, &question.evidence) {
            return Ok(Some(at + 1));
        }
    }
    Ok(None)
}

fn contains(text: &str, evidence: &str) -> bool {
    folded(text).contains(&folded(evidence))
}

fn folded(text: &str) -> String {
    text.to_lowercase().chars().map(vault::fold).collect()
}

struct Score {
    at1: f64,
    at5: f64,
    at10: f64,
    mrr: f64,
}

fn score(outcomes: &[&Outcome]) -> Score {
    let total = outcomes.len().max(1) as f64;
    let within = |k: usize| outcomes.iter().filter(|one| one.rank.is_some_and(|r| r <= k)).count() as f64 / total;
    let mrr = outcomes.iter().filter_map(|one| one.rank).map(|r| 1.0 / r as f64).sum::<f64>() / total;
    Score { at1: within(1), at5: within(5), at10: within(DEPTH), mrr }
}

fn percent(value: f64) -> String {
    format!("{:.0}%", value * 100.0)
}

fn rank_cell(rank: Option<usize>) -> String {
    rank.map_or_else(|| "—".into(), |r| r.to_string())
}

/// What the measured copy held, so two reports can be compared.
fn census(root: &Path, conn: &Connection, reindexed: bool) -> Result<String> {
    let count = |sql: &str| -> Result<i64> { Ok(conn.query_row(sql, [], |row| row.get(0))?) };
    Ok(format!(
        "{} notas, {} trechos ({} de imagem), {} vetores, {}; modelo `{}`",
        count("SELECT count(*) FROM notes")?,
        count("SELECT count(*) FROM chunks")?,
        count("SELECT count(*) FROM chunks WHERE kind = 'image'")?,
        count("SELECT count(*) FROM embeddings")?,
        if reindexed { "notas recortadas por este build" } else { "trechos como estão no índice real" },
        settings::load(root).embedding_model,
    ))
}

fn summary(questions: &[Question], words: &[Outcome], hybrid: &[Outcome]) -> String {
    let all = |list: &[Outcome]| score(&list.iter().collect::<Vec<_>>());
    let (w, h) = (all(words), all(hybrid));
    format!(
        "{} perguntas\npalavras: recall@5 {} · MRR {:.2}\nhíbrida:  recall@5 {} · MRR {:.2}\n",
        questions.len(),
        percent(w.at5),
        w.mrr,
        percent(h.at5),
        h.mrr
    )
}

fn report(label: &str, questions: &[Question], words: &[Outcome], hybrid: &[Outcome], census: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Busca do Cortex · {label}\n");
    let _ = writeln!(out, "- Índice: {census}");
    let _ = writeln!(out, "- Perguntas: {} (`{GOLD}`), busca sobre o índice inteiro", questions.len());
    let _ = writeln!(out, "- Acerto: trecho da nota esperada com o texto esperado entre os {DEPTH} primeiros\n");

    let _ = writeln!(out, "| Busca | recall@1 | recall@5 | recall@10 | MRR@10 |\n|---|---|---|---|---|");
    for (name, list) in [("palavras", words), ("híbrida (caixa e pergunta)", hybrid)] {
        let s = score(&list.iter().collect::<Vec<_>>());
        let _ = writeln!(out, "| {name} | {} | {} | {} | {:.2} |", percent(s.at1), percent(s.at5), percent(s.at10), s.mrr);
    }
    let shown = hybrid.iter().filter(|one| one.rank.is_some_and(|r| r <= 5) && one.visible == Some(true)).count();
    let _ = writeln!(
        out,
        "\nHíbrida, texto esperado dentro do que o modelo recebe (top 5, corte de 1.200 caracteres): {}\n",
        percent(shown as f64 / questions.len().max(1) as f64)
    );

    let _ = writeln!(out, "## Por grupo (recall@5)\n\n| Grupo | Perguntas | palavras | híbrida |\n|---|---|---|---|");
    let mut groups: Vec<&str> = questions.iter().map(|q| q.group.as_str()).collect();
    groups.sort_unstable();
    groups.dedup();
    for group in groups {
        let pick = |list: &[Outcome]| -> Score {
            score(&questions.iter().zip(list).filter(|(q, _)| q.group == group).map(|(_, o)| o).collect::<Vec<_>>())
        };
        let n = questions.iter().filter(|q| q.group == group).count();
        let _ = writeln!(out, "| {group} | {n} | {} | {} |", percent(pick(words).at5), percent(pick(hybrid).at5));
    }

    let _ = writeln!(out, "\n## Perguntas\n\n| id | grupo | palavras | híbrida | no texto enviado | pergunta |\n|---|---|---|---|---|---|");
    for ((question, w), h) in questions.iter().zip(words).zip(hybrid) {
        let visible = match h.visible {
            Some(true) => "sim",
            Some(false) => "**não**",
            None => "—",
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {visible} | {} |",
            question.id,
            question.group,
            rank_cell(w.rank),
            rank_cell(h.rank),
            question.text
        );
    }
    out
}

/// The local date, from the system the same way `curl` is borrowed elsewhere:
/// the standard library knows no time zones, and a UTC name after 21h here would
/// file tonight's run under tomorrow.
fn today() -> Result<String> {
    let output = std::process::Command::new("/bin/date").arg("+%F").output().context("rodando date")?;
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(note: &str, evidence: &str) -> Question {
        Question { id: "q".into(), group: "g".into(), text: "?".into(), note: note.into(), evidence: evidence.into() }
    }

    #[test]
    fn the_gold_file_skips_comments_and_header_and_refuses_a_short_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gold.tsv");
        std::fs::write(&path, "# nota\nid\tgrupo\tpergunta\tnota\tevidencia\nq1\tlexical\tqual?\ta.md\tcerto\n").unwrap();
        assert_eq!(load_gold(&path).unwrap().len(), 1);

        std::fs::write(&path, "q1\tlexical\tqual?\ta.md\n").unwrap();
        assert!(load_gold(&path).is_err());
    }

    /// The right note is not enough, and the wrong note with the right words is
    /// not either; accents and case do not count.
    #[test]
    fn a_hit_needs_the_note_and_the_words_and_ignores_accents() {
        let conn = db::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            INSERT INTO notes (id, path, title, folder, mtime, content_hash) VALUES
              (1, 'a.md', 'A', '', 0, 'x'), (2, 'b.md', 'B', '', 0, 'y');
            INSERT INTO chunks (id, note_id, kind, heading_path, text, title, path) VALUES
              (10, 2, 'text', '', 'Árvore binária', 'B', 'b.md'),
              (11, 1, 'text', '', 'nada aqui', 'A', 'a.md'),
              (12, 1, 'text', '', 'cai ÁRVORE BINÁRIA no teste', 'A', 'a.md');
            "#,
        )
        .unwrap();

        let rank = first_right(&conn, &[10, 11, 12], &question("a.md", "arvore binaria")).unwrap();
        assert_eq!(rank, Some(3));
        assert_eq!(first_right(&conn, &[10, 11], &question("a.md", "arvore binaria")).unwrap(), None);
    }
}
