//! What the app can do, with no Tauri in sight.
//!
//! Two callers reach these functions: the desktop commands in `commands.rs` and
//! the HTTP server in `serve.rs`. Keeping the bodies here is what stops a
//! command from existing on the Mac and being missing on the phone.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::files;
use crate::import;
use crate::search;
use crate::topics::{self, Topic};
use crate::vault;

pub struct AppState {
    pub conn: Mutex<Connection>,
    pub root: PathBuf,
}

/// Every function here answers with a message a person could read, because that
/// is what the interface shows when something goes wrong.
pub type Answer<T> = Result<T, String>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkRange {
    pub heading_path: String,
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteContent {
    pub path: String,
    pub title: String,
    pub source: String,
    pub chunks: Vec<ChunkRange>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub root: String,
    pub notes: i64,
    pub images: i64,
    pub chunks: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetContent {
    pub path: String,
    pub blocks: Vec<crate::ocr::OcrBlock>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub folder: Option<String>,
    pub topics: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub notes: u32,
    pub folders: u32,
    /// Already brought over in an earlier run; these are skipped.
    pub already: u32,
}

pub fn search(
    state: &AppState,
    query: &str,
    topic: Option<&str>,
    limit: Option<u32>,
) -> Answer<search::Found> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    search::run(&conn, query, topic, limit.unwrap_or(40)).map_err(|error| error.to_string())
}

pub fn get_note(state: &AppState, path: String) -> Answer<NoteContent> {
    let abs = vault::resolve(&state.root, &path).map_err(|e| e.to_string())?;
    let source = std::fs::read_to_string(&abs).map_err(|e| e.to_string())?;
    let stem = abs
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    // The same chunk boundaries the index was built from, so the reader can jump
    // to a passage in rendered mode where byte offsets mean nothing.
    let chunks = crate::parse::chunk_markdown(&source)
        .into_iter()
        .map(|chunk| ChunkRange {
            heading_path: chunk.heading_path,
            start: chunk.start as i64,
            end: chunk.end as i64,
        })
        .collect();

    Ok(NoteContent {
        title: crate::parse::title_of(&source, &stem),
        path,
        source,
        chunks,
    })
}

pub fn vault_status(state: &AppState) -> Answer<VaultStatus> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let count = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
    Ok(VaultStatus {
        root: state.root.to_string_lossy().to_string(),
        notes: count("SELECT COUNT(*) FROM notes"),
        images: count("SELECT COUNT(*) FROM assets WHERE ocr_json IS NOT NULL"),
        chunks: count("SELECT COUNT(*) FROM chunks"),
    })
}

/// Copies dropped screenshots into the vault. The watcher notices them there and
/// runs OCR — the same path a file saved from the Finder takes.
pub fn import_images(state: &AppState, paths: Vec<String>) -> Answer<Vec<String>> {
    let target = state.root.join("inbox").join(vault::ASSETS_DIR);
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;

    // Paths rather than a tally: the notice that follows a drop has to name the
    // screenshot it is talking about.
    let mut copied = Vec::new();
    for path in paths {
        let source = PathBuf::from(&path);
        if !vault::is_image(&source) {
            continue;
        }
        let Some(name) = source.file_name() else {
            continue;
        };
        // A macOS screenshot arrives as "Screenshot 2026-08-14 at 1.57.12 AM.png",
        // and a space in a markdown link ends the URL right there.
        let safe = vault::safe_file_name(&name.to_string_lossy());
        let destination = free_name(&target, &safe);
        if std::fs::copy(&source, &destination).is_ok() {
            if let Ok(relative) = vault::relative(&state.root, &destination) {
                copied.push(relative);
            }
        }
    }
    Ok(copied)
}

/// Screenshots dropped on an open note, written beside it. Returns the sources
/// to put in the markdown, in the order they were dropped.
///
/// Nothing here runs OCR: the watcher sees the new file and indexes it, which is
/// the same single way into the index that a paste or the Finder takes.
pub fn attach_images(state: &AppState, note_path: &str, paths: Vec<String>) -> Answer<Vec<String>> {
    let mut written = Vec::new();
    for path in paths {
        let source = PathBuf::from(&path);
        if !vault::is_image(&source) {
            continue;
        }
        written.push(attach_one(state, note_path, &source)?);
    }
    Ok(written)
}

/// Dropping a print that is already sitting loose in the vault moves that one
/// into the note instead of writing a second copy of the same picture. The OCR
/// it has already been through travels with it, and the search stops answering
/// twice for one screenshot.
fn attach_one(state: &AppState, note_path: &str, source: &Path) -> Answer<String> {
    let Some(loose) = loose_twin(state, source)? else {
        return files::copy_into_assets(&state.root, note_path, source).map_err(|e| e.to_string());
    };

    // The move refuses a name already taken beside the note. Another screenshot
    // happening to share a file name is no reason to lose the drop, so it falls
    // back to the copy, which numbers the name instead.
    let Ok(moved) = files::move_into_assets(&state.root, &loose, note_path) else {
        return files::copy_into_assets(&state.root, note_path, source).map_err(|e| e.to_string());
    };
    topics::retarget(&state.root, &loose, Some(&moved)).map_err(|e| e.to_string())?;

    let name = moved.rsplit('/').next().unwrap_or(&moved);
    Ok(format!("{}/{name}", vault::ASSETS_DIR))
}

/// The same bytes, already in the vault and belonging to no note.
///
/// A copy that belongs to another note is left alone: moving it would empty the
/// `![]()` in a note nobody asked about.
fn loose_twin(state: &AppState, source: &Path) -> Answer<Option<String>> {
    let Some(hash) = vault::content_hash(source) else {
        return Ok(None);
    };

    let found: Option<String> = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT path FROM assets WHERE content_hash = ?1 AND note_id IS NULL LIMIT 1",
            [&hash],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    };

    // A row can outlive its file; a path that is no longer there would turn the
    // drop into a failed move rather than a copy.
    Ok(found.filter(|rel| vault::absolute(&state.root, rel).exists()))
}

/// The first name in `folder` that is not taken, counting up from `name`.
pub fn free_name(folder: &Path, name: &str) -> PathBuf {
    let mut destination = folder.join(name);
    let mut attempt = 1;
    while destination.exists() {
        let stem = Path::new(name).file_stem().unwrap_or_default().to_string_lossy().to_string();
        let ext = Path::new(name).extension().unwrap_or_default().to_string_lossy().to_string();
        destination = folder.join(format!("{stem}-{attempt}.{ext}"));
        attempt += 1;
    }
    destination
}

// Everything below writes to disk and stops there. The watcher notices and
// reindexes, so there is exactly one path into the index no matter who made the
// edit — this app, the phone, or an editor someone happens to have open.

pub fn list_tree(state: &AppState) -> Answer<Vec<vault::TreeEntry>> {
    vault::tree(&state.root).map_err(|e| e.to_string())
}

/// Returns the path of the new note so the caller can open it straight away.
/// The note is filed on the way in: writing the file and joining its topics is
/// one call, so the tree and the topic manifests never disagree in between.
pub fn create_note(
    state: &AppState,
    folder: &str,
    title: &str,
    topics: Option<Vec<String>>,
    // A screenshot to bring along: moved next to the note and embedded at the top.
    embed: Option<String>,
) -> Answer<String> {
    let path = files::create_note(&state.root, folder, title).map_err(|e| e.to_string())?;

    if let Some(image) = embed.filter(|image| !image.is_empty()) {
        let moved =
            files::move_into_assets(&state.root, &image, &path).map_err(|e| e.to_string())?;
        topics::retarget(&state.root, &image, Some(&moved)).map_err(|e| e.to_string())?;

        let name = moved.rsplit('/').next().unwrap_or(&moved);
        let source = format!("# {title}\n\n![]({}/{name})\n\n", vault::ASSETS_DIR);
        files::write_note(&state.root, &path, &source).map_err(|e| e.to_string())?;
    }

    for slug in topics.unwrap_or_default() {
        topics::add_member(&state.root, &slug, &path).map_err(|e| e.to_string())?;
    }
    Ok(path)
}

/// An image opened from the tree, where there is no search result to carry the
/// recognised boxes.
pub fn get_asset(state: &AppState, path: String) -> Answer<AssetContent> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let ocr_json: Option<String> = conn
        .query_row("SELECT ocr_json FROM assets WHERE path = ?1", [&path], |row| row.get(0))
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();

    Ok(AssetContent {
        path,
        blocks: ocr_json
            .and_then(|raw| serde_json::from_str::<crate::ocr::OcrResult>(raw.as_str()).ok())
            .map(|ocr| ocr.blocks)
            .unwrap_or_default(),
    })
}

/// Where a note by this name probably belongs, decided by what the vault already
/// contains: the title goes through the normal search, and the folders and
/// topics of the notes that come back are counted.
///
/// No model and no network — the index already knows the answer, and a wrong
/// guess costs one click to change.
pub fn suggest_placement(state: &AppState, title: &str) -> Answer<Placement> {
    if title.trim().is_empty() {
        return Ok(Placement::default());
    }
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let groups = search::run(&conn, title, None, 12).map_err(|e| e.to_string())?.groups;
    if groups.is_empty() {
        return Ok(Placement::default());
    }

    let mut folders: HashMap<String, usize> = HashMap::new();
    for group in &groups {
        *folders.entry(group.folder.clone()).or_default() += 1;
    }

    let paths: Vec<String> = groups.iter().map(|group| group.path.clone()).collect();
    let placeholders = vec!["?"; paths.len()].join(", ");
    let mut statement = conn
        .prepare(&format!(
            "SELECT topic_slug, COUNT(*) FROM topic_members
             WHERE target_path IN ({placeholders})
             GROUP BY topic_slug ORDER BY COUNT(*) DESC LIMIT 3"
        ))
        .map_err(|e| e.to_string())?;

    let topics: Vec<String> = statement
        .query_map(rusqlite::params_from_iter(paths), |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|slug| slug.ok())
        .collect();

    Ok(Placement {
        // An empty string is the vault root, which is a real answer, not a miss.
        folder: folders
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(folder, _)| folder),
        topics,
    })
}

pub fn write_note(state: &AppState, path: &str, source: &str) -> Answer<()> {
    files::write_note(&state.root, path, source).map_err(|e| e.to_string())
}

pub fn create_folder(state: &AppState, parent: &str, name: &str) -> Answer<String> {
    files::create_folder(&state.root, parent, name).map_err(|e| e.to_string())
}

pub fn rename_entry(state: &AppState, path: &str, name: &str) -> Answer<String> {
    let new_path = files::rename(&state.root, path, name).map_err(|e| e.to_string())?;
    topics::retarget(&state.root, path, Some(&new_path)).map_err(|e| e.to_string())?;
    Ok(new_path)
}

pub fn delete_entry(state: &AppState, path: &str) -> Answer<()> {
    files::delete(&state.root, path).map_err(|e| e.to_string())?;
    topics::retarget(&state.root, path, None).map_err(|e| e.to_string())
}

pub fn paste_image(
    state: &AppState,
    note_path: &str,
    bytes: &[u8],
    extension: &str,
) -> Answer<String> {
    files::paste_image(&state.root, note_path, bytes, extension).map_err(|e| e.to_string())
}

// Topic functions write manifests and nothing else, for the same reason.

pub fn list_topics(state: &AppState) -> Answer<Vec<Topic>> {
    topics::load_all(&state.root).map_err(|e| e.to_string())
}

pub fn create_topic(
    state: &AppState,
    name: &str,
    icon: Option<String>,
    color: Option<String>,
) -> Answer<Topic> {
    let slug = topics::slugify(name);
    if slug.is_empty() {
        return Err("um tópico precisa de um nome".into());
    }
    if topics::load(&state.root, &slug).is_ok() {
        return Err(format!("já existe um tópico chamado {name}"));
    }

    let topic = Topic {
        slug,
        name: name.trim().to_string(),
        icon,
        color,
        // Made by hand, so it belongs in your own list.
        auto: false,
        members: Vec::new(),
    };
    topics::save(&state.root, &topic).map_err(|e| e.to_string())?;
    Ok(topic)
}

pub fn update_topic(
    state: &AppState,
    slug: &str,
    name: &str,
    icon: Option<String>,
    color: Option<String>,
) -> Answer<Topic> {
    let mut topic = topics::load(&state.root, slug).map_err(|e| e.to_string())?;
    topic.name = name.trim().to_string();
    topic.icon = icon;
    topic.color = color;
    topics::save(&state.root, &topic).map_err(|e| e.to_string())?;
    Ok(topic)
}

pub fn delete_topic(state: &AppState, slug: &str) -> Answer<()> {
    topics::delete(&state.root, slug).map_err(|e| e.to_string())
}

pub fn add_to_topic(state: &AppState, slug: &str, path: String) -> Answer<Topic> {
    let mut topic = topics::load(&state.root, slug).map_err(|e| e.to_string())?;
    if !topic.members.contains(&path) {
        topic.members.push(path);
        topics::save(&state.root, &topic).map_err(|e| e.to_string())?;
    }
    Ok(topic)
}

pub fn remove_from_topic(state: &AppState, slug: &str, path: &str) -> Answer<Topic> {
    let mut topic = topics::load(&state.root, slug).map_err(|e| e.to_string())?;
    topic.members.retain(|member| member != path);
    topics::save(&state.root, &topic).map_err(|e| e.to_string())?;
    Ok(topic)
}

/// Answers a question from the vault. Reachable from the phone too — the key
/// stays here, and only the answer travels back.
pub fn ask(state: &AppState, question: &str, token: Option<u64>) -> Answer<crate::ask::Answer> {
    let settings = crate::settings::load(&state.root);
    // Borrowed as `&[&str]` so the chain can be walked without copying it.
    let ladder: Vec<&str> = settings.gemini_ladder.iter().map(String::as_str).collect();
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    crate::ask::run(
        &state.root,
        &conn,
        question,
        &settings.excluded,
        crate::ask::Models {
            groq: &settings.groq_model,
            gemini: &ladder,
            gemini_expand: &settings.gemini_expand_model,
            embedding: &settings.embedding_model,
        },
        token,
    )
}

/// Screenshots that have OCR but were never classified. Reindexing never
/// happens on its own — the app must not start rewriting your vault out of
/// nowhere — so this only counts, for the confirmation to state.
pub fn classify_pending(state: &AppState) -> Vec<PathBuf> {
    walkdir::WalkDir::new(&state.root)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|path| path.is_file() && !vault::is_ignored(&state.root, path))
        .filter(|path| vault::is_image(path))
        .filter(|path| {
            crate::ocr::read_cached(path).is_some_and(|cached| cached.marks.is_none())
        })
        .collect()
}

/// Classifies one screenshot that already has OCR and files it under its marks.
/// Returns whether it gained any.
pub fn classify_one(state: &AppState, image: &Path) -> Answer<bool> {
    let Some(mut cached) = crate::ocr::read_cached(image) else {
        return Ok(false);
    };
    if cached.marks.is_some() {
        return Ok(false);
    }

    let settings = crate::settings::load(&state.root);
    let relative = vault::relative(&state.root, image).map_err(|e| e.to_string())?;
    let may_ask = !crate::ask::is_excluded(&relative, &settings.excluded);

    let marks = crate::classify::marks(
        &state.root,
        &cached.text,
        (&settings.groq_model, &settings.gemini_model),
        may_ask,
    );
    let gained = !marks.is_empty();

    let vocabulary = crate::marks::load(&state.root);
    for mark in marks.all() {
        let (name, icon) = vocabulary.presentation(&mark);
        let slug = crate::classify::slug_of(&mark);
        topics::add_auto_member(&state.root, &slug, &name, icon.as_deref(), &relative)
            .map_err(|e| e.to_string())?;
    }

    cached.marks = Some(marks);
    crate::ocr::save(image, &cached).map_err(|e| e.to_string())?;

    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;
    crate::index::index_asset_into(&mut conn, &state.root, image, &cached, None)
        .map_err(|e| e.to_string())?;
    Ok(gained)
}

/// Whether a key is configured — never the key. The panel needs to know if the
/// button will work, and nothing more than that.
pub fn ai_status(state: &AppState) -> AiStatus {
    let settings = crate::settings::load(&state.root);
    let usage = crate::usage::load(&state.root);

    AiStatus {
        keys: crate::secrets::presence(&state.root),
        excluded: settings.excluded,
        coverage: state
            .conn
            .lock()
            .map(|conn| crate::embed::coverage(&conn))
            .unwrap_or_default(),
        auto_embed: settings.auto_embed,
        model_installed: crate::embed::model_present(),
        quota: Quota {
            // Groq states its own totals in every reply; Gemini states none, so
            // its bar is this app's count against a number written down.
            groq: usage.groq.reported.clone(),
            groq_calls: usage.groq.calls,
            gemini_calls: usage.gemini.calls,
            gemini_daily_limit: settings.gemini_daily_limit,
            gemini_resets_in: crate::usage::seconds_to_reset(),
        },
        groq_model: settings.groq_model,
        gemini_model: settings.gemini_model,
        gemini_expand_model: settings.gemini_expand_model,
    }
}

/// What is left of the day, from whoever knows it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    pub groq: crate::usage::Limits,
    /// Counted here, and only useful before the provider has spoken today.
    pub groq_calls: u32,
    pub gemini_calls: u32,
    pub gemini_daily_limit: u32,
    /// Seconds to midnight Pacific, which is when Gemini's day turns.
    pub gemini_resets_in: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub keys: crate::secrets::Presence,
    pub excluded: Vec<String>,
    pub quota: Quota,
    /// How much of the vault can be searched by meaning, and whether the app
    /// keeps that current on its own.
    pub coverage: crate::embed::Coverage,
    pub auto_embed: bool,
    /// Whether the weights are on this machine. Nothing about meaning works
    /// without them, and only the panel's button may put them there.
    pub model_installed: bool,
    pub groq_model: String,
    pub gemini_model: String,
    pub gemini_expand_model: String,
}

/// What an import would do, so the confirmation can say it out loud.
pub fn import_preview(state: &AppState) -> Answer<ImportPreview> {
    let notes = import::list().map_err(|e| e.to_string())?;
    let manifest = import::read_manifest(&state.root);

    let mut folders: Vec<&str> = notes.iter().map(|note| note.folder.as_str()).collect();
    folders.sort_unstable();
    folders.dedup();

    Ok(ImportPreview {
        notes: notes.len() as u32,
        folders: folders.iter().filter(|folder| !folder.is_empty()).count() as u32,
        already: manifest.len() as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn vault() -> (TempDir, AppState) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("react")).unwrap();
        std::fs::create_dir_all(root.join("inbox").join(vault::ASSETS_DIR)).unwrap();
        std::fs::write(root.join("react").join("hooks.md"), "# Hooks\n\n").unwrap();

        let state = AppState {
            conn: Mutex::new(crate::db::open_in_memory().unwrap()),
            root,
        };
        (dir, state)
    }

    /// The print was dropped into the app on its own first, and is now being
    /// dropped onto a note. One picture, so one file: the loose one moves in.
    #[test]
    fn a_print_already_loose_in_the_vault_moves_into_the_note() {
        let (dir, state) = vault();
        let desktop = dir.path().join("print.png");
        std::fs::write(&desktop, b"os mesmos bytes").unwrap();

        let loose = state.root.join("inbox").join(vault::ASSETS_DIR).join("print.png");
        std::fs::write(&loose, b"os mesmos bytes").unwrap();
        state
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO assets (path, ocr_json, mtime, content_hash)
                 VALUES ('inbox/_assets/print.png', NULL, 0, ?1)",
                [vault::content_hash(&loose).unwrap()],
            )
            .unwrap();

        let written = attach_images(
            &state,
            "react/hooks.md",
            vec![desktop.to_string_lossy().to_string()],
        )
        .unwrap();

        assert_eq!(written, ["_assets/print.png"]);
        assert!(state.root.join("react/_assets/print.png").exists());
        assert!(!loose.exists(), "a cópia solta ficou para trás");
    }

    /// Nothing in the vault matches, so the drop is a plain copy and the file on
    /// the desktop stays where it is.
    #[test]
    fn a_print_from_outside_is_copied_in() {
        let (dir, state) = vault();
        let desktop = dir.path().join("print.png");
        std::fs::write(&desktop, b"bytes novos").unwrap();

        let written = attach_images(
            &state,
            "react/hooks.md",
            vec![desktop.to_string_lossy().to_string()],
        )
        .unwrap();

        assert_eq!(written, ["_assets/print.png"]);
        assert!(state.root.join("react/_assets/print.png").exists());
        assert!(desktop.exists(), "o arquivo era da Mesa");
    }

    /// A copy that already belongs to another note is not up for grabs: moving
    /// it would empty an `![]()` in a note nobody touched.
    #[test]
    fn a_print_that_belongs_to_another_note_is_left_where_it_is() {
        let (dir, state) = vault();
        let desktop = dir.path().join("print.png");
        std::fs::write(&desktop, b"os mesmos bytes").unwrap();

        std::fs::create_dir_all(state.root.join("outra").join(vault::ASSETS_DIR)).unwrap();
        let taken = state.root.join("outra").join(vault::ASSETS_DIR).join("print.png");
        std::fs::write(&taken, b"os mesmos bytes").unwrap();
        {
            let conn = state.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO notes (id, path, title, folder, mtime, content_hash)
                 VALUES (1, 'outra/nota.md', 'Outra', 'outra', 0, 'x')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO assets (path, ocr_json, mtime, note_id, content_hash)
                 VALUES ('outra/_assets/print.png', NULL, 0, 1, ?1)",
                [vault::content_hash(&taken).unwrap()],
            )
            .unwrap();
        }

        attach_images(
            &state,
            "react/hooks.md",
            vec![desktop.to_string_lossy().to_string()],
        )
        .unwrap();

        assert!(taken.exists(), "a imagem da outra nota foi levada embora");
        assert!(state.root.join("react/_assets/print.png").exists());
    }
}
