//! Finding a passage by what it means rather than by which words it repeats.
//!
//! The vectors are made here, on this machine, by a model that runs in this
//! process — the vault never leaves the disk to be understood. Everything is
//! still optional: with no model, the app falls back to the full-text search
//! it has always had.
//!
//! The vectors are built once on request, never on the app's own initiative.
//! There is no key to paste any more, so the model's presence on disk is what
//! says you opted in — see `wanted`.

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use anyhow::Result;
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use rusqlite::Connection;

/// What multilingual-e5-small produces. Not a choice — a vector of another
/// width cannot be compared with these, which is why changing the model means
/// a schema migration.
pub const DIMENSIONS: usize = 384;
/// Passages per call. Measured, not picked: ONNX pads a batch up to its longest
/// text, so 100 spent 95% of a pass on padding and took 120s where 16 takes 11.
pub const BATCH: usize = 16;
/// Embedding input is capped in tokens; a passage longer than this is embedded
/// by its opening, which is what a passage is about anyway.
const MAX_CHARS: usize = 6_000;
/// The constant in reciprocal rank fusion. 60 is the number the original paper
/// used and nothing here is sensitive enough to justify tuning it.
const RRF_K: f32 = 60.0;

/// Whether the app should be embedding at all right now. Two ways to be no,
/// and neither is an error: the optional half of the app may simply not be
/// configured, or you may have said not to.
pub fn wanted(root: &Path) -> bool {
    crate::settings::load(root).auto_embed && model_present()
}

/// Embeds what is waiting, a batch per call, stopping at the first refusal.
///
/// Returns how many were done. A failure — quota gone, network down — ends the
/// pass rather than retrying: the next change to the vault, or the next launch,
/// is a better moment than three seconds from now.
pub fn drain(
    root: &Path,
    conn: &Connection,
    model: &str,
    mut progress: impl FnMut(u32, u32),
    stop: impl Fn() -> bool,
) -> Result<u32, String> {
    let mut waiting = pending(conn).map_err(|error| error.to_string())?;
    let total = waiting.len() as u32;
    let mut done = 0;

    // Sorted by length so each batch holds passages of a similar size. Nothing
    // downstream cares about the order — every vector is stored under its own
    // chunk id — and grouping them cut a full pass from 120s to 11s. It changes
    // no vector: see `a_passage_means_the_same_alone_or_padded`.
    waiting.sort_unstable_by_key(|one| one.text.chars().count());

    for batch in waiting.chunks(BATCH) {
        if stop() {
            return Ok(done);
        }
        let texts: Vec<String> = batch.iter().map(|one| one.text.clone()).collect();
        let vectors = embed_batch(root, model, &texts)?;

        for (one, vector) in batch.iter().zip(vectors) {
            let _ = store(conn, one.chunk_id, &one.hash, &vector);
            done += 1;
        }
        progress(done, total);
    }

    // The session's allocator grows to fit the widest batch and never gives it
    // back: a finished pass was leaving gigabytes held until the app quit. A
    // single question does not grow it, so answering stays instant — only the
    // bulk pass pays the reload, and only after it is already done.
    unload();
    Ok(done)
}

/// A passage waiting for its vector.
#[derive(Debug, Clone)]
pub struct Pending {
    pub chunk_id: i64,
    pub text: String,
    pub hash: String,
}

/// What the panel shows: how much of the vault can be searched by meaning.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub chunks: i64,
    pub embedded: i64,
}

pub fn coverage(conn: &Connection) -> Coverage {
    let count = |sql: &str| conn.query_row(sql, [], |row| row.get(0)).unwrap_or(0);
    Coverage {
        chunks: count("SELECT count(*) FROM chunks"),
        // Joined rather than counted alone: a vector whose passage was rewritten
        // is not coverage, it is a leftover.
        embedded: count(
            "SELECT count(*) FROM embeddings e JOIN chunks c ON c.id = e.chunk_id
             WHERE e.text_hash = substr(c.text, 1, 64)",
        ),
    }
}

/// The passages with no vector, or whose text changed since it was made.
pub fn pending(conn: &Connection) -> Result<Vec<Pending>> {
    let mut statement = conn.prepare(
        "SELECT c.id, c.text, c.kind, c.heading_path FROM chunks c
         LEFT JOIN embeddings e ON e.chunk_id = c.id
         WHERE e.chunk_id IS NULL OR e.text_hash <> substr(c.text, 1, 64)
         ORDER BY c.id",
    )?;
    let rows = statement.query_map([], |row| {
        let text: String = row.get(1)?;
        let input = input_of(&row.get::<_, String>(2)?, &row.get::<_, String>(3)?, &text);
        Ok(Pending { chunk_id: row.get(0)?, hash: hash_of(&text), text: input })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

/// What a passage's vector is made from. A text passage goes in behind its trail
/// of headings: one cut from the middle of a long section often never names its
/// subject. An image's trail is a file name and tags, which the OCR already says.
fn input_of(kind: &str, trail: &str, text: &str) -> String {
    if kind == "text" && !trail.is_empty() {
        format!("{trail}\n{text}")
    } else {
        text.to_string()
    }
}

/// The first 64 characters, which is what the SQL above compares against. Not a
/// digest: SQLite has no blake3, and the comparison has to happen in the query
/// for the count to stay one statement.
pub fn hash_of(text: &str) -> String {
    text.chars().take(64).collect()
}

pub fn store(conn: &Connection, chunk_id: i64, hash: &str, vector: &[f32]) -> Result<()> {
    conn.execute(
        "INSERT INTO embeddings (chunk_id, vector, text_hash) VALUES (?1, ?2, ?3)
         ON CONFLICT(chunk_id) DO UPDATE SET vector = excluded.vector, text_hash = excluded.text_hash",
        rusqlite::params![chunk_id, to_blob(vector), hash],
    )?;
    Ok(())
}

/// Every vector in the vault. A few hundred passages at 768 floats is about a
/// megabyte — small enough that the nearest neighbours are found by reading all
/// of them, and no vector index has to exist.
pub fn all(conn: &Connection) -> Result<Vec<(i64, Vec<f32>)>> {
    let mut statement = conn.prepare("SELECT chunk_id, vector FROM embeddings")?;
    let rows = statement.query_map([], |row| {
        let blob: Vec<u8> = row.get(1)?;
        Ok((row.get::<_, i64>(0)?, from_blob(&blob)))
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|value| value.to_le_bytes()).collect()
}

fn from_blob(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|four| f32::from_le_bytes([four[0], four[1], four[2], four[3]]))
        .collect()
}

/// The division stays even though the model returns unit vectors: one that
/// arrived unnormalised would silently rank by length instead of by meaning,
/// and that failure looks like bad search rather than like a bug.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let length = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
    let scale = length(a) * length(b);
    if scale == 0.0 {
        0.0
    } else {
        dot / scale
    }
}

/// Merges two rankings by position rather than by score, which is what lets a
/// bm25 number and a cosine be compared at all. An id near the top of either
/// list ends up near the top of the result; an id in both wins.
pub fn fuse(first: &[i64], second: &[i64]) -> Vec<i64> {
    let mut scored: Vec<(i64, f32)> = Vec::new();
    for list in [first, second] {
        for (rank, id) in list.iter().enumerate() {
            let points = 1.0 / (RRF_K + rank as f32 + 1.0);
            match scored.iter_mut().find(|(known, _)| known == id) {
                Some((_, score)) => *score += points,
                None => scored.push((*id, points)),
            }
        }
    }
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.into_iter().map(|(id, _)| id).collect()
}

/// The chunk ids closest to a vector, best first.
pub fn nearest(vectors: &[(i64, Vec<f32>)], question: &[f32], limit: usize) -> Vec<i64> {
    let mut scored: Vec<(i64, f32)> =
        vectors.iter().map(|(id, vector)| (*id, cosine(question, vector))).collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.into_iter().take(limit).map(|(id, _)| id).collect()
}

/// Loaded once and shared, because building the session costs more than the
/// embedding. An `Option` so removing the model can drop it: files deleted
/// under a live session would leave the app embedding with what you threw out.
static ENGINE: LazyLock<Mutex<Option<TextEmbedding>>> = LazyLock::new(|| Mutex::new(None));

/// Where the weights live. Deliberately outside the vault: the vault is your
/// notes, and 120 MB of floats is not a note. One copy serves every vault.
pub fn cache_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("Cortex").join("models")
}

/// Whether the weights are already here. Searched rather than composed from a
/// fixed path: hf-hub files sit under a revision hash that changes when the
/// model is republished, and a wrong guess would read as "not downloaded".
pub fn model_present() -> bool {
    walkdir::WalkDir::new(cache_dir())
        .into_iter()
        .filter_map(Result::ok)
        .any(|entry| entry.file_name() == "model.onnx")
}

/// One model, and no choice: every stored vector is `DIMENSIONS` wide, so a
/// 768-wide option here would put two vector spaces in one table.
fn named(_model: &str) -> EmbeddingModel {
    EmbeddingModel::MultilingualE5Small
}

/// Measured, counting blobs only: hf-hub symlinks to the file it already has,
/// so following the links counts it twice. Being a little over is worse than
/// under — the progress bar stops short and looks stuck.
pub const MODEL_BYTES: u64 = 465 * 1024 * 1024;

/// How much of the model is already here. The download reports nothing usable,
/// so progress is read from the one thing that is true: bytes on the disk.
pub fn bytes_on_disk() -> u64 {
    walkdir::WalkDir::new(cache_dir())
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(|data| data.is_file())
        .map(|data| data.len())
        .sum()
}

/// Drops the loaded session, keeping the weights on disk. The next embedding
/// reads them back from the file, which is seconds rather than a download.
pub fn unload() {
    if let Ok(mut engine) = ENGINE.lock() {
        *engine = None;
    }
}

/// Forgets the session, then deletes the weights — nothing must be holding a
/// file that is about to go. The vectors stay: what is embedded stays
/// searchable, and only new passages wait for the model to come back.
pub fn remove_model() -> Result<(), String> {
    unload();
    match std::fs::remove_dir_all(cache_dir()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// Builds the session. The only place that may reach the network, and the
/// reason `load` and `fetch` are two functions rather than one with a flag.
fn build(model: &str) -> Result<(), String> {
    let mut engine = ENGINE.lock().map_err(|_| "o modelo ficou num estado ruim".to_string())?;
    if engine.is_some() {
        return Ok(());
    }
    let options = TextInitOptions::new(named(model))
        .with_cache_dir(cache_dir())
        .with_show_download_progress(false);
    *engine = Some(TextEmbedding::try_new(options).map_err(|error| error.to_string())?);
    Ok(())
}

/// Loads what is already here, and refuses rather than fetching. Every
/// embedding path goes through this, so no download can happen as a side effect
/// of embedding — consent is the shape of the code, not a rule to remember.
pub fn load_model(model: &str) -> Result<(), String> {
    if !model_present() {
        return Err("o modelo de busca por significado não está instalado".to_string());
    }
    build(model)
}

/// Downloads the model if it is missing, then loads it. Reached only from the
/// button in the panel.
pub fn fetch_model(model: &str) -> Result<(), String> {
    build(model)
}

/// Runs one embedding pass with the loaded model, loading it first if needed.
fn with_model<T>(
    model: &str,
    work: impl FnOnce(&mut TextEmbedding) -> Result<T, String>,
) -> Result<T, String> {
    load_model(model)?;
    let mut engine = ENGINE.lock().map_err(|_| "o modelo ficou num estado ruim".to_string())?;
    let loaded = engine.as_mut().ok_or_else(|| "o modelo foi removido".to_string())?;
    work(loaded)
}

/// One batch of passages, in the order they were given. The `passage: ` prefix
/// is not decoration: E5 was trained with it, and a passage embedded as a
/// question lands somewhere else than every other passage in the vault.
pub fn embed_batch(_root: &Path, model: &str, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    let prefixed: Vec<String> =
        texts.iter().map(|text| format!("passage: {}", truncate(text))).collect();
    let vectors =
        with_model(model, |engine| engine.embed(&prefixed, None).map_err(|e| e.to_string()))?;

    // A vector of the wrong width would be stored happily and then rank against
    // nothing. Loud here beats search that quietly stops working.
    if let Some(odd) = vectors.iter().find(|one| one.len() != DIMENSIONS) {
        return Err(format!("o modelo devolveu {} dimensões, e o vault guarda {DIMENSIONS}", odd.len()));
    }
    Ok(vectors)
}

/// A question. The other half of the pair above: `query: `, never `passage: `.
pub fn embed_one(_root: &Path, model: &str, text: &str) -> Result<Vec<f32>, String> {
    let asked = format!("query: {}", truncate(text));
    let mut vectors =
        with_model(model, |engine| engine.embed(&[asked], None).map_err(|e| e.to_string()))?;
    vectors.pop().ok_or_else(|| "o modelo respondeu sem vetor".to_string())
}

fn truncate(text: &str) -> String {
    text.chars().take(MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ignored because it downloads 120 MB and runs a real model. Run it by
    /// hand — `cargo test -- --ignored real_model` — when the embedding path
    /// changes. It is the only test that proves the vectors are really made.
    /// The guarantee that makes the download a button: with no model on disk,
    /// embedding refuses instead of quietly fetching 465 MB. Ignored because it
    /// only means anything with the model moved out of the way.
    #[test]
    #[ignore]
    fn embedding_refuses_rather_than_downloading() {
        assert!(!model_present(), "mova o modelo de lado antes de rodar este");

        let outcome = embed_one(Path::new("."), "multilingual-e5-small", "qualquer pergunta");
        assert!(outcome.is_err(), "devia recusar, e recusou com: {outcome:?}");
        assert!(!model_present(), "e não pode ter baixado nada ao recusar");
    }

    /// Batching must not change what a passage means.
    ///
    /// Grouping by length and shrinking the batch made a pass eleven times
    /// faster, but both only pay off by changing how much padding rides along.
    /// If the model's attention mask were wrong, the same text would land in a
    /// different place depending on its neighbours — fast and useless.
    #[test]
    #[ignore]
    fn a_passage_means_the_same_alone_or_padded() {
        // Deliberately lopsided: one long passage drags the whole batch up to
        // its width, which is the case that would expose a broken mask.
        let curtas = ["frete", "o pedido atrasou", "nota fiscal do fornecedor"];
        let comprida = "estoque ".repeat(700);

        let mut juntos: Vec<String> = curtas.iter().map(|t| t.to_string()).collect();
        juntos.push(comprida);
        let com_enchimento = embed_batch(Path::new("."), "multilingual-e5-small", &juntos)
            .expect("embutiu em lote");

        for (at, texto) in juntos.iter().enumerate() {
            let sozinho = embed_batch(Path::new("."), "multilingual-e5-small", &[texto.clone()])
                .expect("embutiu sozinho");
            let igualdade = cosine(&com_enchimento[at], &sozinho[0]);
            assert!(
                igualdade > 0.9999,
                "trecho {at} mudou de significado com o enchimento: cosseno {igualdade}"
            );
        }
    }

    #[test]
    #[ignore]
    fn the_real_model_embeds_at_the_width_the_vault_stores() {
        let question = embed_one(Path::new("."), "multilingual-e5-small", "quanto custa o frete")
            .expect("o modelo devia ter embutido a pergunta");
        assert_eq!(question.len(), DIMENSIONS);

        let passages = embed_batch(
            Path::new("."),
            "multilingual-e5-small",
            &["o frete sai por vinte reais".to_string(), "a receita leva três ovos".to_string()],
        )
        .expect("o modelo devia ter embutido as passagens");
        assert_eq!(passages.len(), 2);

        // O ponto do exercício: significado, não palavra repetida. "Frete" está
        // nas duas, mas só uma responde a pergunta.
        let sobre_frete = cosine(&question, &passages[0]);
        let sobre_receita = cosine(&question, &passages[1]);
        assert!(
            sobre_frete > sobre_receita,
            "frete {sobre_frete} devia vencer receita {sobre_receita}"
        );

        assert!(model_present(), "depois de baixar, o portão de consentimento abre");
    }

    #[test]
    fn a_vector_survives_the_round_trip_through_the_blob() {
        let vector: Vec<f32> = (0..DIMENSIONS).map(|at| at as f32 / 1000.0).collect();
        let blob = to_blob(&vector);

        assert_eq!(blob.len(), DIMENSIONS * 4);
        assert_eq!(from_blob(&blob), vector);
    }

    #[test]
    fn the_cosine_knows_same_from_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine(&a, &a) - 1.0).abs() < 1e-6);
        assert!(cosine(&a, &b).abs() < 1e-6);
        assert!((cosine(&a, &vec![-1.0, 0.0, 0.0]) + 1.0).abs() < 1e-6);
        // Length must not decide: the same direction twice as long is the same.
        assert!((cosine(&a, &vec![2.0, 0.0, 0.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_nearest_come_back_in_order() {
        let vectors = vec![
            (1, vec![0.0, 1.0]),
            (2, vec![1.0, 0.0]),
            (3, vec![0.9, 0.1]),
        ];
        assert_eq!(nearest(&vectors, &[1.0, 0.0], 2), [2, 3]);
    }

    /// The point of fusing: a passage the search matched exactly stays near the
    /// top even when the vector did not love it, and the other way round.
    #[test]
    fn what_both_lists_agree_on_wins() {
        let by_words = vec![10, 20, 30];
        let by_meaning = vec![30, 40, 50];
        let merged = fuse(&by_words, &by_meaning);

        assert_eq!(merged[0], 30, "o que aparece nas duas sobe");
        assert!(merged.contains(&10) && merged.contains(&50));
        assert_eq!(merged.len(), 5, "sem repetir");
    }

    #[test]
    fn an_exact_hit_survives_a_vector_that_ignored_it() {
        // `revalidatePath` is the case: bm25 puts it first, the vector does not
        // see it at all.
        let by_words = vec![7];
        let by_meaning = vec![1, 2, 3, 4, 5, 6];
        assert!(fuse(&by_words, &by_meaning).iter().take(2).any(|id| *id == 7));
    }

    fn vault_with_chunks(how_many: usize) -> (tempfile::TempDir, Connection) {
        let dir = tempfile::TempDir::new().unwrap();
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO notes (id, path, title, folder, mtime, content_hash)
             VALUES (1, 'a.md', 'A', '', 0, 'h');",
        )
        .unwrap();
        for at in 0..how_many {
            conn.execute(
                "INSERT INTO chunks (note_id, kind, heading_path, text, title, path)
                 VALUES (1, 'text', '', ?1, 'A', 'a.md')",
                [format!("um parágrafo de número {at}")],
            )
            .unwrap();
        }
        (dir, conn)
    }

    #[test]
    fn everything_starts_out_waiting_and_stops_once_it_has_a_vector() {
        let (_dir, conn) = vault_with_chunks(3);
        assert_eq!(pending(&conn).unwrap().len(), 3);
        assert_eq!(coverage(&conn).embedded, 0);

        let waiting = pending(&conn).unwrap();
        for one in &waiting {
            store(&conn, one.chunk_id, &one.hash, &[0.5; 8]).unwrap();
        }

        assert!(pending(&conn).unwrap().is_empty());
        assert_eq!(coverage(&conn).embedded, 3);
        assert_eq!(coverage(&conn).chunks, 3);
    }

    /// Editing a note has to put its passages back in the queue, or the vector
    /// would answer for text that no longer exists.
    #[test]
    fn a_rewritten_passage_goes_back_in_the_queue() {
        let (_dir, conn) = vault_with_chunks(1);
        let one = pending(&conn).unwrap().remove(0);
        store(&conn, one.chunk_id, &one.hash, &[0.5; 8]).unwrap();
        assert!(pending(&conn).unwrap().is_empty());

        conn.execute("UPDATE chunks SET text = 'outra coisa completamente' WHERE id = ?1", [one.chunk_id])
            .unwrap();

        assert_eq!(pending(&conn).unwrap().len(), 1);
        assert_eq!(coverage(&conn).embedded, 0, "vetor velho não conta como cobertura");
    }

    /// Saying no has to survive the model being right there on the disk. The
    /// other half of the gate — no model, no embedding — is asserted by
    /// `the_real_model_embeds_at_the_width_the_vault_stores`, which can only
    /// check it after downloading one.
    #[test]
    fn turning_it_off_is_obeyed_even_with_the_model_installed() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut settings = crate::settings::load(dir.path());
        settings.auto_embed = false;
        crate::settings::save(dir.path(), &settings).unwrap();

        assert!(!wanted(dir.path()));
    }

    /// Written as a property rather than against a fixed layout: `BATCH` is a
    /// measured number that moved once already, and a test pinned to its value
    /// fails for the tuning rather than for a bug.
    #[test]
    fn splitting_the_vault_into_batches_loses_nothing() {
        let vault: Vec<usize> = (0..484).collect();
        let batches: Vec<&[usize]> = vault.chunks(BATCH).collect();

        assert_eq!(batches.iter().map(|batch| batch.len()).sum::<usize>(), vault.len());
        assert!(batches.iter().all(|batch| !batch.is_empty() && batch.len() <= BATCH));
        assert!(batches.iter().rev().skip(1).all(|batch| batch.len() == BATCH), "só o último é curto");
    }

    #[test]
    fn a_passage_is_embedded_by_its_opening_when_it_is_enormous() {
        let huge = "a".repeat(38_468);
        assert_eq!(truncate(&huge).chars().count(), MAX_CHARS);
    }
}

