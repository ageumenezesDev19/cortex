//! The desktop door into `api.rs`.
//!
//! Every command here is a wrapper and nothing else: the moment one of them
//! grows a body, the phone stops being able to do what the Mac can.

use serde::Serialize;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, State};

use crate::api::{
    self, AppState, AssetContent, ImportPreview, NoteContent, Placement, VaultStatus,
};
use crate::import;
use crate::search::Group;
use crate::topics::Topic;
use crate::serve;
use crate::settings;
use crate::tailscale;
use crate::vault;

#[tauri::command]
pub fn search(
    state: State<AppState>,
    query: String,
    topic: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Group>, String> {
    api::search(&state, &query, topic.as_deref(), limit)
}

#[tauri::command]
pub fn get_note(state: State<AppState>, path: String) -> Result<NoteContent, String> {
    api::get_note(&state, path)
}

#[tauri::command]
pub fn vault_status(state: State<AppState>) -> Result<VaultStatus, String> {
    api::vault_status(&state)
}

#[tauri::command]
pub fn import_images(state: State<AppState>, paths: Vec<String>) -> Result<Vec<String>, String> {
    api::import_images(&state, paths)
}

#[tauri::command]
pub fn list_tree(state: State<AppState>) -> Result<Vec<vault::TreeEntry>, String> {
    api::list_tree(&state)
}

#[tauri::command]
pub fn create_note(
    state: State<AppState>,
    folder: String,
    title: String,
    topics: Option<Vec<String>>,
    embed: Option<String>,
) -> Result<String, String> {
    api::create_note(&state, &folder, &title, topics, embed)
}

#[tauri::command]
pub fn get_asset(state: State<AppState>, path: String) -> Result<AssetContent, String> {
    api::get_asset(&state, path)
}

#[tauri::command]
pub fn suggest_placement(state: State<AppState>, title: String) -> Result<Placement, String> {
    api::suggest_placement(&state, &title)
}

#[tauri::command]
pub fn write_note(state: State<AppState>, path: String, source: String) -> Result<(), String> {
    api::write_note(&state, &path, &source)
}

#[tauri::command]
pub fn create_folder(
    state: State<AppState>,
    parent: String,
    name: String,
) -> Result<String, String> {
    api::create_folder(&state, &parent, &name)
}

#[tauri::command]
pub fn rename_entry(state: State<AppState>, path: String, name: String) -> Result<String, String> {
    api::rename_entry(&state, &path, &name)
}

#[tauri::command]
pub fn delete_entry(state: State<AppState>, path: String) -> Result<(), String> {
    api::delete_entry(&state, &path)
}

#[tauri::command]
pub fn paste_image(
    state: State<AppState>,
    note_path: String,
    bytes: Vec<u8>,
    extension: String,
) -> Result<String, String> {
    api::paste_image(&state, &note_path, &bytes, &extension)
}

#[tauri::command]
pub fn list_topics(state: State<AppState>) -> Result<Vec<Topic>, String> {
    api::list_topics(&state)
}

#[tauri::command]
pub fn create_topic(
    state: State<AppState>,
    name: String,
    icon: Option<String>,
    color: Option<String>,
) -> Result<Topic, String> {
    api::create_topic(&state, &name, icon, color)
}

#[tauri::command]
pub fn update_topic(
    state: State<AppState>,
    slug: String,
    name: String,
    icon: Option<String>,
    color: Option<String>,
) -> Result<Topic, String> {
    api::update_topic(&state, &slug, &name, icon, color)
}

#[tauri::command]
pub fn delete_topic(state: State<AppState>, slug: String) -> Result<(), String> {
    api::delete_topic(&state, &slug)
}

#[tauri::command]
pub fn add_to_topic(state: State<AppState>, slug: String, path: String) -> Result<Topic, String> {
    api::add_to_topic(&state, &slug, path)
}

#[tauri::command]
pub fn remove_from_topic(
    state: State<AppState>,
    slug: String,
    path: String,
) -> Result<Topic, String> {
    api::remove_from_topic(&state, &slug, &path)
}

#[tauri::command]
pub fn import_preview(state: State<AppState>) -> Result<ImportPreview, String> {
    api::import_preview(&state)
}

#[tauri::command]
pub fn ask(state: State<AppState>, question: String) -> Result<crate::ask::Answer, String> {
    api::ask(&state, &question)
}

#[tauri::command]
pub fn ai_status(state: State<AppState>) -> api::AiStatus {
    api::ai_status(&state)
}

/// The excluded list is the only part of the answer settings the app writes;
/// the keys are the person's, in their own file.
#[tauri::command]
pub fn set_excluded(state: State<AppState>, excluded: Vec<String>) -> Result<api::AiStatus, String> {
    let mut current = settings::load(&state.root);
    current.excluded = excluded;
    settings::save(&state.root, &current)?;
    Ok(api::ai_status(&state))
}

pub const CLASSIFY_EVENT: &str = "classify:status";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClassifyStatus {
    phase: &'static str,
    done: u32,
    total: u32,
    current: String,
    marked: u32,
}

/// How many screenshots are waiting, so the confirmation can say it before
/// anything is touched.
#[tauri::command]
pub fn classify_preview(state: State<AppState>) -> usize {
    api::classify_pending(&state).len()
}

/// Classifies the screenshots already in the vault. Never runs on its own —
/// the app must not start rewriting things unasked — and reports as it goes,
/// because a pass over a thousand images with no sign of life reads as a hang.
#[tauri::command]
pub fn classify_existing(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let pending = api::classify_pending(&state);
    let root = state.root.clone();

    std::thread::spawn(move || {
        let state = AppState {
            conn: Mutex::new(match crate::db::open(&crate::vault::db_path(&root)) {
                Ok(conn) => conn,
                Err(error) => {
                    let _ = app.emit(
                        CLASSIFY_EVENT,
                        ClassifyStatus {
                            phase: "failed",
                            done: 0,
                            total: 0,
                            current: error.to_string(),
                            marked: 0,
                        },
                    );
                    return;
                }
            }),
            root: root.clone(),
        };

        let total = pending.len() as u32;
        let mut marked = 0;

        for (at, image) in pending.iter().enumerate() {
            let name = image
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let _ = app.emit(
                CLASSIFY_EVENT,
                ClassifyStatus {
                    phase: "running",
                    done: at as u32,
                    total,
                    current: name,
                    marked,
                },
            );
            if api::classify_one(&state, image).unwrap_or(false) {
                marked += 1;
            }
        }

        let _ = app.emit(
            CLASSIFY_EVENT,
            ClassifyStatus { phase: "done", done: total, total, current: String::new(), marked },
        );
    });

    Ok(())
}

// Serving the vault to the phone is the machine's business, so these are
// desktop-only: the phone is looking at a server, not running one.

#[tauri::command]
pub fn server_state(control: State<serve::Control>) -> serve::ServerState {
    control.state()
}

/// Applies the switches and answers with what became true, so the panel never
/// draws a state the machine did not reach.
#[tauri::command]
pub fn set_server(
    control: State<serve::Control>,
    serving: bool,
    local_network: bool,
    tailscale: bool,
) -> Result<serve::ServerState, String> {
    // Read from disk, not from the server's cached copy: the answer settings
    // have their own writer, and merging onto a stale cache would silently drop
    // an exclusion the moment a server switch was flipped.
    let stored = settings::load(&control.root());
    let wanted = settings::Settings { serving, local_network, tailscale, ..stored };
    control.apply(wanted)?;
    Ok(control.state())
}

/// Walks the same chain the phone walks and names the link that broke.
#[tauri::command]
pub fn test_connection(control: State<serve::Control>, local: bool) -> Vec<tailscale::Step> {
    if local {
        match serve::local_address() {
            Some(address) => vec![tailscale::reach_local(&address, control.state().port)],
            None => Vec::new(),
        }
    } else {
        tailscale::test(control.state().port)
    }
}

// The Notes.app import stays here, and only here. It reaches outside the vault,
// takes minutes, and reports as it goes — so it runs on its own thread and talks
// through events, which is also why the phone does not get it: the Notes library
// lives on this machine.

pub const IMPORT_EVENT: &str = "import:status";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportStatus {
    phase: &'static str,
    done: u32,
    total: u32,
    current: String,
    report: Option<import::Report>,
    error: Option<String>,
}

#[tauri::command]
pub fn import_notes(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let root = state.root.clone();

    std::thread::spawn(move || {
        let emit = |phase, done, total, current: &str, report, error| {
            let _ = app.emit(
                IMPORT_EVENT,
                ImportStatus { phase, done, total, current: current.to_string(), report, error },
            );
        };

        emit("running", 0, 0, "", None, None);
        match import::run(&root, |done, total, name| {
            emit("running", done, total, name, None, None);
        }) {
            Ok(report) => emit("done", report.imported, report.imported, "", Some(report), None),
            Err(error) => emit("failed", 0, 0, "", None, Some(error.to_string())),
        }
    });

    Ok(())
}
