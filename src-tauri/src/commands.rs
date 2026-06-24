use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::{queue, scanner};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub selected_source: Arc<Mutex<Option<PathBuf>>>,
    pub selected_destination: Arc<Mutex<Option<PathBuf>>>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanResponse {
    pub discovered_count: usize,
    pub queued_count: usize,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            selected_source: Arc::new(Mutex::new(None)),
            selected_destination: Arc::new(Mutex::new(None)),
        }
    }

    async fn scan_source_folder_path(&self, path: String) -> Result<ScanResponse, String> {
        let (source_root, discovered) = tauri::async_runtime::spawn_blocking(move || {
            let source_root = dunce::canonicalize(PathBuf::from(path))
                .map_err(|error| format!("Unable to read source folder: {error}"))?;
            if !source_root.is_dir() {
                return Err("Source path is not a folder".to_string());
            }
            let discovered = scanner::scan_media(&source_root)
                .map_err(|error| format!("Unable to scan source folder: {error}"))?;
            Ok::<_, String>((source_root, discovered))
        })
        .await
        .map_err(|error| format!("Unable to run folder scan: {error}"))??;

        let job_ids = queue::enqueue_discovered_media(&self.pool, &discovered, None)
            .await
            .map_err(|error| format!("Unable to queue discovered media: {error}"))?;

        *self
            .selected_source
            .lock()
            .map_err(|_| "Unable to update selected source folder".to_string())? =
            Some(source_root);

        Ok(ScanResponse {
            discovered_count: discovered.len(),
            queued_count: job_ids.len(),
        })
    }

    fn set_export_folder_path(&self, path: String) -> Result<(), String> {
        let destination = dunce::canonicalize(PathBuf::from(path))
            .map_err(|error| format!("Unable to read export folder: {error}"))?;
        if !destination.is_dir() {
            return Err("Export path is not a folder".to_string());
        }

        *self
            .selected_destination
            .lock()
            .map_err(|_| "Unable to update export folder".to_string())? = Some(destination);

        Ok(())
    }
}

#[tauri::command]
pub async fn scan_source_folder(
    path: String,
    state: State<'_, AppState>,
) -> Result<ScanResponse, String> {
    state.scan_source_folder_path(path).await
}

#[tauri::command]
pub async fn set_export_folder(path: String, state: State<'_, AppState>) -> Result<(), String> {
    state.set_export_folder_path(path)
}

#[cfg(test)]
mod tests {
    use super::AppState;

    #[tokio::test]
    async fn scan_source_folder_counts_queued_media_and_remembers_source() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        std::fs::write(temp.path().join("one.mp3"), b"audio").expect("fixture should be written");
        std::fs::write(temp.path().join("two.txt"), b"text").expect("fixture should be written");
        std::fs::write(temp.path().join("three.wav"), b"audio").expect("fixture should be written");

        let pool = crate::db::connect_memory()
            .await
            .expect("memory database should initialize");
        let state = AppState::new(pool);

        let response = state
            .scan_source_folder_path(temp.path().to_string_lossy().into_owned())
            .await
            .expect("scan should succeed");

        assert_eq!(response.discovered_count, 2);
        assert_eq!(response.queued_count, 2);
        assert_eq!(
            *state
                .selected_source
                .lock()
                .expect("selected source should lock"),
            Some(dunce::canonicalize(temp.path()).expect("temp path should canonicalize"))
        );
    }

    #[tokio::test]
    async fn scan_source_folder_rejects_missing_source() {
        let pool = crate::db::connect_memory()
            .await
            .expect("memory database should initialize");
        let state = AppState::new(pool);

        let error = state
            .scan_source_folder_path("C:/definitely/missing/source".to_string())
            .await
            .expect_err("missing source should fail");

        assert!(error.contains("Unable to read source folder"));
    }

    #[tokio::test]
    async fn repeat_scan_reuses_existing_jobs() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        std::fs::write(temp.path().join("one.mp3"), b"audio").expect("fixture should be written");

        let pool = crate::db::connect_memory()
            .await
            .expect("memory database should initialize");
        let state = AppState::new(pool);

        let first = state
            .scan_source_folder_path(temp.path().to_string_lossy().into_owned())
            .await
            .expect("first scan should succeed");
        let second = state
            .scan_source_folder_path(temp.path().to_string_lossy().into_owned())
            .await
            .expect("second scan should succeed");

        assert_eq!(first.queued_count, 1);
        assert_eq!(second.queued_count, 1);

        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transcription_jobs")
            .fetch_one(&state.pool)
            .await
            .expect("count should run");
        assert_eq!(row.0, 1);
    }

    #[tokio::test]
    async fn set_export_folder_remembers_destination() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let pool = crate::db::connect_memory()
            .await
            .expect("memory database should initialize");
        let state = AppState::new(pool);

        state
            .set_export_folder_path(temp.path().to_string_lossy().into_owned())
            .expect("export folder should be stored");

        assert_eq!(
            *state
                .selected_destination
                .lock()
                .expect("selected destination should lock"),
            Some(dunce::canonicalize(temp.path()).expect("temp path should canonicalize"))
        );
    }

    #[tokio::test]
    async fn set_export_folder_rejects_file_path() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let file_path = temp.path().join("not-a-folder.txt");
        std::fs::write(&file_path, b"text").expect("fixture should be written");
        let pool = crate::db::connect_memory()
            .await
            .expect("memory database should initialize");
        let state = AppState::new(pool);

        let error = state
            .set_export_folder_path(file_path.to_string_lossy().into_owned())
            .expect_err("file destination should fail");

        assert_eq!(error, "Export path is not a folder");
    }
}
