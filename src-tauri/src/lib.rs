mod commands;
mod db;
mod files;
mod index;
mod ocr;
mod parse;
mod search;
mod topics;
mod vault;
mod watch;

use std::sync::Mutex;

use commands::AppState;
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
            watch::spawn(app.handle().clone(), root);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search,
            commands::get_note,
            commands::vault_status,
            commands::import_images,
            commands::list_tree,
            commands::create_note,
            commands::write_note,
            commands::create_folder,
            commands::rename_entry,
            commands::delete_entry,
            commands::paste_image,
            commands::list_topics,
            commands::create_topic,
            commands::update_topic,
            commands::delete_topic,
            commands::add_to_topic,
            commands::remove_from_topic,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
