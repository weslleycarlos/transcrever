use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::backend::{TranscriptionBackend, TranscriptionProfile};
use crate::{db, models::JobStatus, queue, scanner};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub selected_source: Arc<Mutex<Option<PathBuf>>>,
    pub selected_destination: Arc<Mutex<Option<PathBuf>>>,
    pub active_profile: Arc<Mutex<Option<db::ProfileRow>>>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanResponse {
    pub discovered_count: usize,
    pub queued_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRow {
    pub job_id: i64,
    pub media_file_id: i64,
    pub file_name: String,
    pub relative_path: String,
    pub status: String,
    pub progress: f32,
    pub error_message: Option<String>,
}

impl From<db::JobWithMedia> for JobRow {
    fn from(j: db::JobWithMedia) -> Self {
        Self {
            job_id: j.job_id,
            media_file_id: j.media_file_id,
            file_name: j.file_name,
            relative_path: j.relative_path,
            status: j.status,
            progress: j.progress,
            error_message: j.error_message,
        }
    }
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            selected_source: Arc::new(Mutex::new(None)),
            selected_destination: Arc::new(Mutex::new(None)),
            active_profile: Arc::new(Mutex::new(None)),
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

#[tauri::command]
pub async fn save_profile(profile: db::ProfileRow, state: State<'_, AppState>) -> Result<db::ProfileRow, String> {
    let id = db::save_profile(&state.pool, &profile)
        .await
        .map_err(|e| e.to_string())?;

    let saved = db::ProfileRow { id, ..profile };
    *state.active_profile.lock().map_err(|e| e.to_string())? = Some(saved.clone());
    Ok(saved)
}

#[tauri::command]
pub async fn list_profiles(state: State<'_, AppState>) -> Result<Vec<db::ProfileRow>, String> {
    db::list_profiles(&state.pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_profile(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    db::delete_profile(&state.pool, id).await.map_err(|e| e.to_string())?;
    let mut active = state.active_profile.lock().map_err(|e| e.to_string())?;
    if active.as_ref().map_or(false, |p| p.id == id) {
        *active = None;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_active_profile(state: State<'_, AppState>) -> Result<Option<db::ProfileRow>, String> {
    Ok(state.active_profile.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
pub async fn set_active_profile(profile: db::ProfileRow, state: State<'_, AppState>) -> Result<(), String> {
    *state.active_profile.lock().map_err(|e| e.to_string())? = Some(profile);
    Ok(())
}

#[tauri::command]
pub async fn start_transcription(state: State<'_, AppState>) -> Result<(), String> {
    let profile = state
        .active_profile
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "Nenhum perfil de transcricao ativo. Configure um perfil primeiro.".to_string())?;

    let pool = state.pool.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            let next = match db::find_next_pending_job(&pool).await {
                Ok(Some(job)) => job,
                Ok(None) => break,
                Err(e) => {
                    eprintln!("erro ao buscar proximo job: {e}");
                    break;
                }
            };

            let (job_id, media_file_id, absolute_path) = next;
            let media_path = std::path::PathBuf::from(&absolute_path);

            let _ = db::update_job_status(&pool, job_id, JobStatus::Processing, 0.0, None).await;

            let result = run_transcription(&media_path, &profile);
            match result {
                Ok(transcription) => {
                    let segments: Vec<(i64, i64, String, Option<f32>)> = transcription
                        .segments
                        .iter()
                        .map(|s| (s.start_ms, s.end_ms, s.text.clone(), s.confidence))
                        .collect();

                    match db::save_transcription(
                        &pool,
                        job_id,
                        media_file_id,
                        &transcription.raw_text,
                        &segments,
                    )
                    .await
                    {
                        Ok(_) => {
                            let _ = db::update_job_status(
                                &pool, job_id, JobStatus::Completed, 1.0, None,
                            )
                            .await;
                        }
                        Err(e) => {
                            let _ = db::update_job_status(
                                &pool,
                                job_id,
                                JobStatus::Error,
                                0.0,
                                Some(&format!("erro ao salvar transcricao: {e}")),
                            )
                            .await;
                        }
                    }
                }
                Err(e) => {
                    let _ = db::update_job_status(
                        &pool,
                        job_id,
                        JobStatus::Error,
                        0.0,
                        Some(&e.to_string()),
                    )
                    .await;
                }
            }
        }
    });

    Ok(())
}

fn run_transcription(
    media_path: &std::path::Path,
    profile: &db::ProfileRow,
) -> anyhow::Result<crate::backend::BackendTranscription> {
    let model_path = resolve_model_path(&profile.model_path, &profile.backend);

    let profile_config = TranscriptionProfile {
        model_path,
        device: profile.device.clone(),
        precision: profile.precision.clone(),
        threads: profile.threads as usize,
        language: profile.language.clone(),
        task: profile.task.clone(),
        advanced_json: serde_json::from_str(&profile.advanced_json).unwrap_or_default(),
    };

    match profile.backend.as_str() {
        "faster_whisper" => {
            let script_path = std::env::current_dir()
                .unwrap_or_default()
                .join("scripts")
                .join("faster_whisper_transcribe.py");
            let backend = crate::backend::faster_whisper::FasterWhisperBackend::new(script_path);
            backend.transcribe(media_path, &profile_config)
        }
        _ => {
            // Whisper.cpp via miniaudio may not support opus. Convert to WAV first if needed.
            let actual_path = convert_to_wav_if_needed(media_path)?;
            let exe = resolve_whisper_exe();
            let backend = crate::backend::whisper_cpp::WhisperCppBackend::new(exe);
            backend.transcribe(&actual_path, &profile_config)
        }
    }
}

/// Converts opus files to WAV using bundled ffmpeg, returns the converted path.
/// Returns the original path unchanged if conversion is not needed.
fn convert_to_wav_if_needed(media_path: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    let ext = media_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    // Whisper.cpp handles mp3, wav, flac natively; opus needs conversion
    if ext != "opus" {
        return Ok(media_path.to_path_buf());
    }

    let wav_path = media_path.with_extension("opus.wav");
    if wav_path.exists() {
        return Ok(wav_path);
    }

    // Try bundled ffmpeg first, then system PATH
    let ffmpeg = resolve_ffmpeg_exe();

    let output = std::process::Command::new(&ffmpeg)
        .args([
            "-y", "-i",
            &media_path.to_string_lossy(),
            "-ar", "16000",
            "-ac", "1",
            "-sample_fmt", "s16",
            &wav_path.to_string_lossy(),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .with_context(|| format!("ffmpeg not found at {}", ffmpeg.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "ffmpeg conversion failed for {}: {}",
            media_path.display(),
            stderr.trim()
        );
    }

    Ok(wav_path)
}

fn resolve_ffmpeg_exe() -> std::path::PathBuf {
    // Bundled in CARGO_MANIFEST_DIR (src-tauri/) during dev
    let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("ffmpeg.exe");
    if source.exists() { return source; }

    // Bundled in resource_dir during production
    let bundled = std::env::current_dir()
        .unwrap_or_default()
        .join("resources")
        .join("ffmpeg.exe");
    if bundled.exists() { return bundled; }

    // Fallback to system PATH
    std::path::PathBuf::from("ffmpeg")
}

fn resolve_whisper_exe() -> std::path::PathBuf {
    let exe = std::env::current_dir()
        .unwrap_or_default()
        .join("binaries")
        .join("whisper-cli.exe");
    if exe.exists() { exe } else {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join("whisper-cli.exe")
    }
}

/// Resolves the model path for the given backend.
///
/// whisper.cpp expects a `.bin` file path.
/// faster-whisper expects a **directory** containing CTranslate2 model files.
/// If the user accidentally points to `model.bin` for faster-whisper, we use the parent directory.
fn resolve_model_path(raw: &str, backend: &str) -> String {
    if backend != "faster_whisper" {
        return raw.to_string();
    }

    let path = std::path::Path::new(raw);

    // If it points to a file (likely model.bin), use the parent directory
    if path.is_file() {
        if let Some(parent) = path.parent() {
            return parent.to_string_lossy().to_string();
        }
    }

    // If it's already a directory, use as-is
    raw.to_string()
}

#[tauri::command]
pub async fn list_jobs(state: State<'_, AppState>) -> Result<Vec<JobRow>, String> {
    let jobs = db::list_all_jobs(&state.pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(jobs.into_iter().map(JobRow::from).collect())
}

#[tauri::command]
pub async fn get_transcription(job_id: i64, state: State<'_, AppState>) -> Result<Option<db::TranscriptionView>, String> {
    db::get_transcription_by_job(&state.pool, job_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn read_audio(path: String) -> Result<String, String> {
    let data = std::fs::read(&path).map_err(|e| format!("Cannot read audio file: {e}"))?;
    let mime = mime_type(&path);
    let b64 = base64_encode(&data);
    Ok(format!("data:{mime};base64,{b64}"))
}

#[tauri::command]
pub async fn search_transcriptions(query: String, state: State<'_, AppState>) -> Result<Vec<db::TranscriptionView>, String> {
    db::search_transcriptions(&state.pool, &query).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_transcriptions(state: State<'_, AppState>) -> Result<Vec<db::TranscriptionView>, String> {
    db::list_transcriptions(&state.pool).await.map_err(|e| e.to_string())
}

fn mime_type(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "mp3" | "mpga" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "opus" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" | "aac" => "audio/mp4",
        "wma" => "audio/x-ms-wma",
        "webm" => "audio/webm",
        _ => "audio/mpeg",
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len() * 4 / 3 + 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((n >> 18) & 63) as usize] as char);
        out.push(CHARS[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 { out.push(CHARS[((n >> 6) & 63) as usize] as char); } else { out.push('='); }
        if chunk.len() > 2 { out.push(CHARS[(n & 63) as usize] as char); } else { out.push('='); }
    }
    out
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
