use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::ocr::{OcrBlock, OcrResult};
use crate::vault;

pub struct AppState {
    pub conn: Mutex<Connection>,
    pub root: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub chunk_id: i64,
    pub kind: String,
    /// Note path for text hits, image path for OCR hits — always relative to
    /// the vault root.
    pub path: String,
    pub title: String,
    pub note_path: Option<String>,
    pub note_title: Option<String>,
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
pub struct NoteContent {
    pub path: String,
    pub title: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub root: String,
    pub notes: i64,
    pub images: i64,
    pub chunks: i64,
}

const SEARCH_SQL: &str = r#"
SELECT c.id, c.kind, c.heading_path, c.start_offset, c.end_offset,
       n.path, n.title, a.path, a.ocr_json,
       snippet(chunks_fts, 0, '<<<', '>>>', '…', 14)
FROM chunks_fts
JOIN chunks c ON c.id = chunks_fts.rowid
LEFT JOIN notes  n ON n.id = c.note_id
LEFT JOIN assets a ON a.id = c.asset_id
WHERE chunks_fts MATCH ?1
ORDER BY bm25(chunks_fts)
LIMIT ?2
"#;

#[tauri::command]
pub fn search(state: State<AppState>, query: String, limit: Option<u32>) -> Result<Vec<Hit>, String> {
    let Some(match_expr) = fts_query(&query) else {
        return Ok(Vec::new());
    };
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare_cached(SEARCH_SQL).map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(
            rusqlite::params![match_expr, limit.unwrap_or(40)],
            |row| {
                let note_path: Option<String> = row.get(5)?;
                let note_title: Option<String> = row.get(6)?;
                let asset_path: Option<String> = row.get(7)?;
                let ocr_json: Option<String> = row.get(8)?;
                let kind: String = row.get(1)?;

                let blocks = ocr_json
                    .and_then(|raw| serde_json::from_str::<OcrResult>(&raw).ok())
                    .map(|ocr| ocr.blocks)
                    .unwrap_or_default();

                let path = asset_path
                    .clone()
                    .or_else(|| note_path.clone())
                    .unwrap_or_default();
                let title = match kind.as_str() {
                    "image" => file_name_of(&path),
                    _ => note_title.clone().unwrap_or_else(|| file_name_of(&path)),
                };

                Ok(Hit {
                    chunk_id: row.get(0)?,
                    kind,
                    path,
                    title,
                    note_path,
                    note_title,
                    heading_path: row.get(2)?,
                    snippet: row.get(9)?,
                    start_offset: row.get(3)?,
                    end_offset: row.get(4)?,
                    blocks,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_note(state: State<AppState>, path: String) -> Result<NoteContent, String> {
    let abs = vault::absolute(&state.root, &path);
    if !abs.starts_with(&state.root) {
        return Err("path outside the vault".into());
    }
    let source = std::fs::read_to_string(&abs).map_err(|e| e.to_string())?;
    let stem = abs
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    Ok(NoteContent {
        title: crate::parse::title_of(&source, &stem),
        path,
        source,
    })
}

#[tauri::command]
pub fn vault_status(state: State<AppState>) -> Result<VaultStatus, String> {
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
#[tauri::command]
pub fn import_images(state: State<AppState>, paths: Vec<String>) -> Result<u32, String> {
    let target = state.root.join("inbox").join(vault::ASSETS_DIR);
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;

    let mut copied = 0;
    for path in paths {
        let source = PathBuf::from(&path);
        if !vault::is_image(&source) {
            continue;
        }
        let Some(name) = source.file_name() else {
            continue;
        };
        let mut destination = target.join(name);
        let mut attempt = 1;
        while destination.exists() {
            let stem = source.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let ext = source.extension().unwrap_or_default().to_string_lossy().to_string();
            destination = target.join(format!("{stem}-{attempt}.{ext}"));
            attempt += 1;
        }
        if std::fs::copy(&source, &destination).is_ok() {
            copied += 1;
        }
    }
    Ok(copied)
}

/// Turns typed words into an FTS5 prefix query. Stripping everything that is not
/// alphanumeric keeps the user's quotes and asterisks from being read as syntax.
fn fts_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split_whitespace()
        .map(|term| {
            term.chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
        })
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{term}\"*"))
        .collect();

    (!terms.is_empty()).then(|| terms.join(" "))
}

fn file_name_of(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

#[cfg(test)]
mod tests {
    use super::fts_query;

    #[test]
    fn builds_a_prefix_query_per_term() {
        assert_eq!(fts_query("server actions").unwrap(), "\"server\"* \"actions\"*");
    }

    #[test]
    fn keeps_accents_and_drops_syntax() {
        assert_eq!(fts_query("código \"OR\"").unwrap(), "\"código\"* \"OR\"*");
        assert!(fts_query("  * ").is_none());
    }
}
