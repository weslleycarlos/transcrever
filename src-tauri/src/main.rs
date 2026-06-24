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

            // Create default profile on first run with bundled model + binary
            let count = tauri::async_runtime::block_on(db::count_profiles(&pool))?;
            if count == 0 {
                // In production (installed app), the model is in resource_dir
                // In dev mode, it's in the source tree (resource_dir points to target/debug)
                let model_path = resolve_model_path(app.path().resource_dir()?);

                let profile_id = tauri::async_runtime::block_on(
                    db::create_default_profile(&pool, &model_path.to_string_lossy()),
                )?;
                let profiles = tauri::async_runtime::block_on(db::list_profiles(&pool))?;
                if let Some(profile) = profiles.into_iter().find(|p| p.id == profile_id) {
                    let state = AppState::new(pool.clone());
                    *state.active_profile.lock().map_err(|_| {
                        std::io::Error::new(std::io::ErrorKind::Other, "lock")
                    })? = Some(profile);
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
            commands::stop_transcription,
            commands::list_jobs,
            commands::get_transcription,
            commands::read_audio,
            commands::search_transcriptions,
            commands::list_transcriptions,
            commands::update_transcription,
            commands::export_transcription
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}

/// Returns the actual path to the bundled base.pt model.
/// In production, the model is in resource_dir/models/whisper/base.pt.
/// In dev mode (tauri dev), resource_dir points to target/debug/ where
/// resources aren't copied, so we fall back to the source tree.
fn resolve_model_path(resource_dir: std::path::PathBuf) -> std::path::PathBuf {
    let bundled = resource_dir
        .join("models")
        .join("whisper")
        .join("base.pt");

    if bundled.exists() {
        return bundled;
    }

    // Dev mode fallback: look in the source tree
    let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join("whisper")
        .join("base.pt");

    if source.exists() {
        return source;
    }

    // Return the bundled path anyway (it will show a clear error if missing)
    bundled
}
