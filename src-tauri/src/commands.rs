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
use crate::search;
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
) -> Result<search::Found, String> {
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
pub fn attach_images(
    state: State<AppState>,
    note_path: String,
    paths: Vec<String>,
) -> Result<Vec<String>, String> {
    api::attach_images(&state, &note_path, paths)
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

/// The token is the caller's handle on this call: `cancel_ask` uses it to kill
/// the request rather than merely stop waiting for it.
#[tauri::command]
pub fn ask(
    state: State<AppState>,
    question: String,
    token: Option<u64>,
) -> Result<crate::ask::Answer, String> {
    api::ask(&state, &question, token)
}

#[tauri::command]
pub fn cancel_ask(token: u64) {
    crate::provider::cancel(token);
}

#[tauri::command]
pub fn ai_status(state: State<AppState>) -> api::AiStatus {
    api::ai_status(&state)
}

// Deciding what may leave this machine, and with what key, is the machine's
// business — so these three are desktop-only, like the server switches. They
// are absent from `serve::dispatch`, which is what puts them out of the phone's
// reach: on the local network the session travels as plain HTTP.

/// What the AI may read. Empty means the whole vault.
#[tauri::command]
pub fn set_excluded(state: State<AppState>, excluded: Vec<String>) -> Result<api::AiStatus, String> {
    let mut current = settings::load(&state.root);
    current.excluded = excluded;
    settings::save(&state.root, &current)?;
    Ok(api::ai_status(&state))
}

/// Writes a key, or removes it when given nothing. What comes back is the
/// status — whether a key is there and where it came from, never a value.
#[tauri::command]
pub fn set_key(
    state: State<AppState>,
    name: String,
    value: Option<String>,
) -> Result<api::AiStatus, String> {
    crate::secrets::store(&state.root, &name, value.as_deref())?;
    Ok(api::ai_status(&state))
}

/// Spends one request to hear the provider's own numbers, for a panel opened
/// before the first question of the day. Groq only: on Gemini this would cost
/// 5% of the daily quota to report the quota.
#[tauri::command]
pub fn refresh_quota(state: State<AppState>) -> Result<api::AiStatus, String> {
    let settings = settings::load(&state.root);
    crate::provider::check(&state.root, crate::provider::Provider::Groq, &settings.groq_model)?;
    Ok(api::ai_status(&state))
}

/// The daily limit Gemini refuses to state. Yours to correct when Google moves
/// it, the same as the model name.
#[tauri::command]
pub fn set_gemini_limit(state: State<AppState>, limit: u32) -> Result<api::AiStatus, String> {
    let mut current = settings::load(&state.root);
    current.gemini_daily_limit = limit;
    settings::save(&state.root, &current)?;
    Ok(api::ai_status(&state))
}

/// One real call to one provider. A key is only configured once it answers.
#[tauri::command]
pub fn test_key(state: State<AppState>, provider: crate::provider::Provider) -> Result<(), String> {
    let settings = settings::load(&state.root);
    let model = match provider {
        crate::provider::Provider::Groq => settings.groq_model,
        crate::provider::Provider::Gemini => settings.gemini_model,
    };
    crate::provider::check(&state.root, provider, &model)
}

pub const EMBED_EVENT: &str = "embed:status";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EmbedStatus {
    phase: &'static str,
    done: u32,
    total: u32,
    error: Option<String>,
}

/// A flag rather than a channel: one pass runs at a time, and the only thing
/// the button has to say is "stop after this batch".
static STOP_EMBEDDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
pub fn cancel_embedding() {
    STOP_EMBEDDING.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Read by the automatic pass in the watcher, so one cancel stops whichever of
/// the two is running.
pub fn embedding_cancelled() -> bool {
    STOP_EMBEDDING.load(std::sync::atomic::Ordering::Relaxed)
}

/// Clears the flag at the start of a pass. Cancelling stops that pass; turning
/// the feature off is the switch's job.
pub fn resume_embedding() {
    STOP_EMBEDDING.store(false, std::sync::atomic::Ordering::Relaxed);
}

/// Turning it off is a decision about someone's quota, so it is remembered.
#[tauri::command]
pub fn set_auto_embed(state: State<AppState>, on: bool) -> Result<api::AiStatus, String> {
    let mut current = settings::load(&state.root);
    current.auto_embed = on;
    settings::save(&state.root, &current)?;
    Ok(api::ai_status(&state))
}

/// Turns the vault into vectors, a hundred passages per call. Never on its own —
/// this spends someone's quota — and it picks up where it stopped, so a
/// cancelled pass is not a wasted one.
#[tauri::command]
pub fn embed_vault(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let root = state.root.clone();
    let model = settings::load(&root).embedding_model;
    resume_embedding();

    std::thread::spawn(move || {
        let emit = |phase, done, total, error| {
            let _ = app.emit(EMBED_EVENT, EmbedStatus { phase, done, total, error });
        };

        let conn = match crate::db::open(&crate::vault::db_path(&root)) {
            Ok(conn) => conn,
            Err(error) => return emit("failed", 0, 0, Some(error.to_string())),
        };
        let total = crate::embed::pending(&conn).map(|waiting| waiting.len() as u32).unwrap_or(0);
        emit("running", 0, total, None);

        match crate::embed::drain(
            &root,
            &conn,
            &model,
            |done, total| emit("running", done, total, None),
            embedding_cancelled,
        ) {
            Ok(done) => emit("done", done, total, None),
            Err(error) => emit("failed", 0, total, Some(error)),
        }
    });

    Ok(())
}

pub const MODEL_EVENT: &str = "model:status";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelStatus {
    phase: &'static str,
    /// Bytes, not passages: this is the download, not the embedding.
    done: u64,
    total: u64,
    error: Option<String>,
}

/// Downloads the embedding model, which is the one moment this app reaches the
/// network for the search to work at all. Never called on the app's own
/// initiative — the button in the panel is the whole of the consent.
///
/// Progress is polled off the disk rather than reported by the downloader,
/// which offers no callback. Bytes that landed are the one honest measure.
#[tauri::command]
pub fn download_model(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let model = settings::load(&state.root).embedding_model;

    std::thread::spawn(move || {
        let emit = |phase, done, total, error| {
            let _ = app.emit(MODEL_EVENT, ModelStatus { phase, done, total, error });
        };
        emit("downloading", crate::embed::bytes_on_disk(), crate::embed::MODEL_BYTES, None);

        let watching = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let ticker = {
            let watching = watching.clone();
            let app = app.clone();
            std::thread::spawn(move || {
                while watching.load(std::sync::atomic::Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    let _ = app.emit(
                        MODEL_EVENT,
                        ModelStatus {
                            phase: "downloading",
                            done: crate::embed::bytes_on_disk(),
                            total: crate::embed::MODEL_BYTES,
                            error: None,
                        },
                    );
                }
            })
        };

        let outcome = crate::embed::fetch_model(&model);
        watching.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = ticker.join();

        match outcome {
            Ok(()) => {
                let landed = crate::embed::bytes_on_disk();
                emit("ready", landed, landed, None)
            }
            Err(error) => emit("failed", 0, crate::embed::MODEL_BYTES, Some(error)),
        }
    });

    Ok(())
}

/// Forgets the model and deletes the weights. The vectors already made stay:
/// they are floats in the vault, and what is embedded stays searchable.
#[tauri::command]
pub fn delete_model(state: State<AppState>) -> Result<api::AiStatus, String> {
    crate::embed::remove_model()?;
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
