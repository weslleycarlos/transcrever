use transcrever::{
    commands::{self, AppState},
    db,
};

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let database_path = app_data_dir.join("transcrever.sqlite");
            let pool = tauri::async_runtime::block_on(db::connect(&database_path))?;
            app.manage(AppState::new(pool));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_source_folder,
            commands::set_export_folder
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
