pub mod api;
mod ask;
pub mod classify;
mod commands;
mod db;
pub mod embed;
mod files;
pub mod html;
pub mod import;
pub mod index;
pub mod marks;
mod ocr;
pub mod parse;
mod provider;
mod search;
mod secrets;
mod serve;
mod settings;
mod tailscale;
mod topics;
mod usage;
pub mod vault;
mod watch;

use std::sync::Mutex;

use api::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let root = vault::ensure()?;
            // The reader migrates the schema before the indexer thread opens its
            // own connection, so the two never race on a fresh database.
            let conn = db::open(&vault::db_path(&root))?;
            app.manage(AppState {
                conn: Mutex::new(conn),
                root: root.clone(),
            });
            watch::spawn(app.handle().clone(), root.clone());

            // Losing the web server must not take the window down with it: the
            // desktop works exactly the same without it.
            match serve::Control::new(root) {
                Ok(control) => {
                    if let Err(error) = control.apply(control.settings()) {
                        eprintln!("[cortex] sem servidor web: {error}");
                    }
                    app.manage(control);
                }
                Err(error) => eprintln!("[cortex] sem servidor web: {error:#}"),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search,
            commands::get_note,
            commands::vault_status,
            commands::import_images,
            commands::list_tree,
            commands::create_note,
            commands::suggest_placement,
            commands::get_asset,
            commands::import_preview,
            commands::import_notes,
            commands::server_state,
            commands::set_server,
            commands::test_connection,
            commands::ask,
            commands::cancel_ask,
            commands::ai_status,
            commands::set_excluded,
            commands::set_key,
            commands::test_key,
            commands::refresh_quota,
            commands::download_model,
            commands::delete_model,
            commands::embed_vault,
            commands::cancel_embedding,
            commands::set_auto_embed,
            commands::set_gemini_limit,
            commands::classify_preview,
            commands::classify_existing,
            commands::write_note,
            commands::create_folder,
            commands::rename_entry,
            commands::delete_entry,
            commands::paste_image,
            commands::attach_images,
            commands::list_topics,
            commands::create_topic,
            commands::update_topic,
            commands::delete_topic,
            commands::add_to_topic,
            commands::remove_from_topic,
        ])
        .on_window_event(|window, event| {
            // The tailnet mapping points at this process. Leaving it up after
            // the process is gone means an address that answers with nothing.
            if matches!(event, tauri::WindowEvent::Destroyed) {
                if let Some(control) = window.try_state::<serve::Control>() {
                    control.shut_everything_down();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
