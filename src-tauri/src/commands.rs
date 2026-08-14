use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::search::{self, Group};
use crate::topics::{self, Topic};
use crate::vault;

pub struct AppState {
    pub conn: Mutex<Connection>,
    pub root: PathBuf,
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

#[tauri::command]
pub fn search(
    state: State<AppState>,
    query: String,
    topic: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Group>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    search::run(&conn, &query, topic.as_deref(), limit.unwrap_or(40))
        .map_err(|error| error.to_string())
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

// Topic commands write manifests and nothing else. The watcher sees the change
// and refreshes the database mirror, so there is exactly one path into the
// index no matter who made the edit — the app or the editor.

#[tauri::command]
pub fn list_topics(state: State<AppState>) -> Result<Vec<Topic>, String> {
    topics::load_all(&state.root).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_topic(
    state: State<AppState>,
    name: String,
    icon: Option<String>,
    color: Option<String>,
) -> Result<Topic, String> {
    let slug = topics::slugify(&name);
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
        members: Vec::new(),
    };
    topics::save(&state.root, &topic).map_err(|e| e.to_string())?;
    Ok(topic)
}

#[tauri::command]
pub fn update_topic(
    state: State<AppState>,
    slug: String,
    name: String,
    icon: Option<String>,
    color: Option<String>,
) -> Result<Topic, String> {
    let mut topic = topics::load(&state.root, &slug).map_err(|e| e.to_string())?;
    topic.name = name.trim().to_string();
    topic.icon = icon;
    topic.color = color;
    topics::save(&state.root, &topic).map_err(|e| e.to_string())?;
    Ok(topic)
}

#[tauri::command]
pub fn delete_topic(state: State<AppState>, slug: String) -> Result<(), String> {
    topics::delete(&state.root, &slug).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_to_topic(state: State<AppState>, slug: String, path: String) -> Result<Topic, String> {
    let mut topic = topics::load(&state.root, &slug).map_err(|e| e.to_string())?;
    if !topic.members.contains(&path) {
        topic.members.push(path);
        topics::save(&state.root, &topic).map_err(|e| e.to_string())?;
    }
    Ok(topic)
}

#[tauri::command]
pub fn remove_from_topic(
    state: State<AppState>,
    slug: String,
    path: String,
) -> Result<Topic, String> {
    let mut topic = topics::load(&state.root, &slug).map_err(|e| e.to_string())?;
    topic.members.retain(|member| member != &path);
    topics::save(&state.root, &topic).map_err(|e| e.to_string())?;
    Ok(topic)
}
