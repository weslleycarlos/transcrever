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

            // Create default profile on first run
            let count = tauri::async_runtime::block_on(db::count_profiles(&pool))?;
            if count == 0 {
                let resource_dir = app.path().resource_dir()?;
                // The bundled model lives in resources/models/whisper/base.pt
                // In dev mode this resolves to target/debug/models/whisper/base.pt
                let model_path = resource_dir
                    .join("models")
                    .join("whisper")
                    .join("base.pt");

                // Always create a default profile pointing to the bundled model path.
                // If the model file doesn't exist (dev mode without running setup-model.ps1),
                // the user can still use the app by going to Config and pointing to their
                // own faster-whisper model directory.
                let profile_id =
                    tauri::async_runtime::block_on(db::create_default_profile(
                        &pool,
                        &model_path.to_string_lossy(),
                    ))?;
                let profiles =
                    tauri::async_runtime::block_on(db::list_profiles(&pool))?;
                if let Some(profile) = profiles.into_iter().find(|p| p.id == profile_id) {
                    let state = AppState::new(pool.clone());
                    *state.active_profile
                        .lock()
                        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "lock"))? =
                        Some(profile);
                    app.manage(state);
                    return Ok(());
                }
            }

            app.manage(AppState::new(pool));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_source_folder,
            commands::set_export_folder,
            commands::save_profile,
            commands::list_profiles,
            commands::delete_profile,
            commands::get_active_profile,
            commands::set_active_profile,
            commands::start_transcription,
            commands::list_jobs,
            commands::get_transcription,
            commands::read_audio,
            commands::search_transcriptions
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
