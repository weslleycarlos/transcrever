use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
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
    pub cancel_flag: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
    pub concurrency: Arc<AtomicUsize>,
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
    pub source_root: String,
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
            source_root: j.source_root,
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
            cancel_flag: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
            concurrency: Arc::new(AtomicUsize::new(1)),
        }
    }

    async fn scan_source_folder_path(&self, path: String, project_id: Option<i64>) -> Result<ScanResponse, String> {
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

        // Resolve the target project: explicit id, or one named after the folder.
        let project_id = match project_id {
            Some(id) => id,
            None => {
                let name = source_root
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| source_root.to_string_lossy().into_owned());
                db::get_or_create_project_by_name(&self.pool, &name)
                    .await
                    .map_err(|e| e.to_string())?
            }
        };

        let job_ids = queue::enqueue_discovered_media(&self.pool, &discovered, Some(project_id))
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
    project_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<ScanResponse, String> {
    state.scan_source_folder_path(path, project_id).await
}

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<db::ProjectView>, String> {
    db::list_projects(&state.pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_project(name: String, state: State<'_, AppState>) -> Result<i64, String> {
    db::create_project(&state.pool, &name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_project(id: i64, name: String, state: State<'_, AppState>) -> Result<(), String> {
    db::rename_project(&state.pool, id, &name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_project_archived(id: i64, archived: bool, state: State<'_, AppState>) -> Result<(), String> {
    db::set_project_archived(&state.pool, id, archived).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_project_default_profile(id: i64, profile_id: Option<i64>, state: State<'_, AppState>) -> Result<(), String> {
    db::set_project_default_profile(&state.pool, id, profile_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_project(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    db::delete_project(&state.pool, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_duplicate_jobs(state: State<'_, AppState>) -> Result<u64, String> {
    db::cleanup_duplicate_jobs(&state.pool).await.map_err(|e| e.to_string())
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
    // Keep the active profile in sync only when creating the first profile or
    // editing the one that is currently active.
    let should_set = {
        let mut active = state.active_profile.lock().map_err(|e| e.to_string())?;
        let should_set = match active.as_ref() {
            None => true,
            Some(current) => current.id == saved.id,
        };
        if should_set {
            *active = Some(saved.clone());
        }
        should_set
    };
    if should_set {
        let _ = db::set_setting(&state.pool, "active_profile_id", &saved.id.to_string()).await;
    }
    Ok(saved)
}

#[tauri::command]
pub async fn list_profiles(state: State<'_, AppState>) -> Result<Vec<db::ProfileRow>, String> {
    db::list_profiles(&state.pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_profile(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    db::delete_profile(&state.pool, id).await.map_err(|e| e.to_string())?;
    let cleared = {
        let mut active = state.active_profile.lock().map_err(|e| e.to_string())?;
        if active.as_ref().map_or(false, |p| p.id == id) {
            *active = None;
            true
        } else {
            false
        }
    };
    if cleared {
        let _ = db::set_setting(&state.pool, "active_profile_id", "").await;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_active_profile(state: State<'_, AppState>) -> Result<Option<db::ProfileRow>, String> {
    Ok(state.active_profile.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
pub async fn set_active_profile(profile: db::ProfileRow, state: State<'_, AppState>) -> Result<(), String> {
    let id = profile.id;
    *state.active_profile.lock().map_err(|e| e.to_string())? = Some(profile);
    let _ = db::set_setting(&state.pool, "active_profile_id", &id.to_string()).await;
    Ok(())
}

#[tauri::command]
pub async fn start_transcription(state: State<'_, AppState>) -> Result<(), String> {
    // Global active profile is just a fallback; each job may resolve its own via
    // its project's default profile.
    let fallback = state.active_profile.lock().map_err(|e| e.to_string())?.clone();

    if state.running.swap(true, Ordering::SeqCst) {
        return Ok(()); // already running
    }

    let cancel = state.cancel_flag.clone();
    cancel.store(false, Ordering::SeqCst);
    let workers = state.concurrency.load(Ordering::SeqCst).max(1);
    let running = state.running.clone();
    let active = Arc::new(AtomicUsize::new(workers));

    for _ in 0..workers {
        let pool = state.pool.clone();
        let fallback = fallback.clone();
        let cancel = cancel.clone();
        let running = running.clone();
        let active = active.clone();

        tauri::async_runtime::spawn(async move {
            loop {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                let claimed = match db::claim_next_pending_job(&pool).await {
                    Ok(Some(job)) => job,
                    Ok(None) => {
                        // No row claimed: either the queue is empty or another
                        // worker won the race. Stop only when nothing remains.
                        match db::count_pending_jobs(&pool).await {
                            Ok(0) => break,
                            Ok(_) => continue,
                            Err(_) => break,
                        }
                    }
                    Err(e) => {
                        eprintln!("erro ao reservar proximo job: {e}");
                        break;
                    }
                };

                let (job_id, media_file_id, absolute_path) = claimed;
                let media_path = std::path::PathBuf::from(&absolute_path);

                // Resolve the profile: project default first, then global active.
                let job_profile = match db::resolve_profile_for_media(&pool, media_file_id).await {
                    Ok(Some(p)) => Some(p),
                    _ => fallback.clone(),
                };
                let job_profile = match job_profile {
                    Some(p) => p,
                    None => {
                        let _ = db::update_job_status(
                            &pool, job_id, JobStatus::Error, 0.0,
                            Some("Nenhum perfil ativo nem perfil padrao do projeto definido."),
                        ).await;
                        continue;
                    }
                };

                // Capture engine metadata before the profile is moved into the worker.
                let backend_label = if job_profile.backend == "faster_whisper" {
                    "faster-whisper"
                } else {
                    "whisper.cpp"
                };
                let device_cfg = job_profile.device.clone();

                let result = tauri::async_runtime::spawn_blocking(move || {
                    run_transcription(&media_path, &job_profile)
                })
                .await;

                match result {
                    Ok(Ok(transcription)) => {
                        let segments: Vec<(i64, i64, String, Option<f32>)> = transcription
                            .segments
                            .iter()
                            .map(|s| (s.start_ms, s.end_ms, s.text.clone(), s.confidence))
                            .collect();

                        let device = transcription
                            .device_used
                            .clone()
                            .unwrap_or_else(|| device_cfg.clone());
                        let engine = format!("{backend_label} · {device}");

                        match db::save_transcription(
                            &pool,
                            job_id,
                            media_file_id,
                            &transcription.raw_text,
                            &segments,
                            Some(&engine),
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
                    Ok(Err(e)) => {
                        let _ = db::update_job_status(
                            &pool, job_id, JobStatus::Error, 0.0, Some(&e.to_string()),
                        )
                        .await;
                    }
                    Err(e) => {
                        let _ = db::update_job_status(
                            &pool,
                            job_id,
                            JobStatus::Error,
                            0.0,
                            Some(&format!("worker falhou: {e}")),
                        )
                        .await;
                    }
                }
            }

            // Last worker out clears the running flag.
            if active.fetch_sub(1, Ordering::SeqCst) == 1 {
                running.store(false, Ordering::SeqCst);
            }
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_transcription(state: State<'_, AppState>) -> Result<(), String> {
    state.cancel_flag.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn retry_failed_jobs(project_id: Option<i64>, state: State<'_, AppState>) -> Result<u64, String> {
    db::retry_failed_jobs(&state.pool, project_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reset_job(job_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    db::reset_job(&state.pool, job_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_concurrency(state: State<'_, AppState>) -> Result<usize, String> {
    Ok(state.concurrency.load(Ordering::SeqCst).max(1))
}

#[tauri::command]
pub async fn set_concurrency(value: usize, state: State<'_, AppState>) -> Result<(), String> {
    let clamped = value.clamp(1, 16);
    state.concurrency.store(clamped, Ordering::SeqCst);
    db::set_setting(&state.pool, "concurrency", &clamped.to_string())
        .await
        .map_err(|e| e.to_string())
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

    // Reject empty files up front with a clear message instead of a cryptic
    // decoder error further down.
    if let Ok(metadata) = std::fs::metadata(media_path) {
        if metadata.len() == 0 {
            anyhow::bail!("arquivo de audio vazio (0 bytes)");
        }
    }

    match profile.backend.as_str() {
        "faster_whisper" => {
            let script_path = resolve_faster_whisper_script();
            let backend = crate::backend::faster_whisper::FasterWhisperBackend::new(script_path);
            backend.transcribe(media_path, &profile_config)
        }
        _ => {
            // Whisper.cpp's bundled decoders only handle a few formats reliably.
            // Convert anything else (opus, mpga, m4a, video containers, ...) to WAV.
            let actual_path = convert_to_wav_if_needed(media_path)?;
            let exe = resolve_whisper_exe();
            let backend = crate::backend::whisper_cpp::WhisperCppBackend::new(exe);
            backend.transcribe(&actual_path, &profile_config)
        }
    }
}

/// Formats whisper.cpp's bundled decoders read reliably. Everything else is
/// transcoded to 16 kHz mono WAV via ffmpeg first.
const WHISPER_NATIVE_FORMATS: &[&str] = &["wav", "mp3", "flac", "ogg"];

/// Converts non-native audio/video files to WAV using bundled ffmpeg, returning
/// the converted path. Returns the original path unchanged when not needed.
fn convert_to_wav_if_needed(media_path: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    let ext = media_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if WHISPER_NATIVE_FORMATS.contains(&ext.as_str()) {
        return Ok(media_path.to_path_buf());
    }

    let wav_path = media_path.with_extension(format!("{ext}.wav"));
    if wav_path.exists() {
        return Ok(wav_path);
    }

    // Try bundled ffmpeg first, then system PATH
    let ffmpeg = resolve_ffmpeg_exe();

    let mut command = std::process::Command::new(&ffmpeg);
    command
        .args([
            "-y", "-i",
            &media_path.to_string_lossy(),
            "-ar", "16000",
            "-ac", "1",
            "-sample_fmt", "s16",
            &wav_path.to_string_lossy(),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    crate::util::no_window(&mut command);
    let output = command
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

/// Locates a bundled resource by relative path. Looks next to the executable
/// first (installed app), then in the source tree (dev), then in the current
/// working directory. Returns None when not found anywhere.
fn find_bundled_resource(rel: &[&str]) -> Option<std::path::PathBuf> {
    let mut bases: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            bases.push(dir.to_path_buf());
        }
    }
    bases.push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd);
    }

    for base in bases {
        let mut candidate = base;
        for part in rel {
            candidate = candidate.join(part);
        }
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn resolve_ffmpeg_exe() -> std::path::PathBuf {
    find_bundled_resource(&["resources", "ffmpeg.exe"])
        // Fallback to system PATH.
        .unwrap_or_else(|| std::path::PathBuf::from("ffmpeg"))
}

fn resolve_whisper_exe() -> std::path::PathBuf {
    find_bundled_resource(&["binaries", "whisper-cli.exe"]).unwrap_or_else(|| {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join("whisper-cli.exe")
    })
}

fn resolve_faster_whisper_script() -> std::path::PathBuf {
    find_bundled_resource(&["scripts", "faster_whisper_transcribe.py"]).unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .join("scripts")
            .join("faster_whisper_transcribe.py")
    })
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyStatus {
    pub python: Option<String>,
    pub faster_whisper: bool,
    pub cuda: bool,
    /// Name of the first NVIDIA GPU reported by nvidia-smi, if any.
    pub gpu: Option<String>,
    /// CUDA compute capability (e.g. "5.2") of that GPU, if reported.
    pub compute_cap: Option<String>,
}

fn detect_nvidia_gpu() -> (Option<String>, Option<String>) {
    let mut gpu_cmd = std::process::Command::new("nvidia-smi");
    gpu_cmd.args(["--query-gpu=name,compute_cap", "--format=csv,noheader"]);
    crate::util::no_window(&mut gpu_cmd);
    let output = match gpu_cmd.output() {
        Ok(o) if o.status.success() => o,
        _ => return (None, None),
    };

    let line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if line.is_empty() {
        return (None, None);
    }

    let mut parts = line.split(',').map(|p| p.trim().to_string());
    let name = parts.next().filter(|s| !s.is_empty());
    let cap = parts.next().filter(|s| !s.is_empty());
    (name, cap)
}

fn detect_python() -> Option<String> {
    for candidate in ["python", "python3", "py"] {
        let mut cmd = std::process::Command::new(candidate);
        cmd.arg("--version");
        crate::util::no_window(&mut cmd);
        let ok = cmd.output().map(|o| o.status.success()).unwrap_or(false);
        if ok {
            return Some(candidate.to_string());
        }
    }
    None
}

fn python_import_ok(python: &str, code: &str) -> bool {
    let mut cmd = std::process::Command::new(python);
    cmd.args(["-c", code]);
    crate::util::no_window(&mut cmd);
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

#[tauri::command]
pub async fn check_faster_whisper_env() -> Result<DependencyStatus, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let python = detect_python();
        let (faster_whisper, cuda) = match &python {
            Some(p) => (
                python_import_ok(p, "import faster_whisper"),
                python_import_ok(p, "import nvidia.cublas, nvidia.cudnn"),
            ),
            None => (false, false),
        };
        let (gpu, compute_cap) = detect_nvidia_gpu();
        DependencyStatus { python, faster_whisper, cuda, gpu, compute_cap }
    })
    .await
    .map_err(|e| format!("Falha ao verificar ambiente: {e}"))
}

#[tauri::command]
pub async fn install_faster_whisper(gpu: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let python = detect_python()
            .ok_or_else(|| "Python nao encontrado no PATH. Instale o Python 3.10+ primeiro.".to_string())?;

        let mut args = vec!["-m", "pip", "install", "--upgrade", "faster-whisper"];
        if gpu {
            args.push("nvidia-cublas-cu12");
            args.push("nvidia-cudnn-cu12");
        }

        let mut command = std::process::Command::new(&python);
        command.args(&args);
        crate::util::no_window(&mut command);
        let output = command
            .output()
            .map_err(|e| format!("Falha ao executar pip: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}").trim().to_string();
        if output.status.success() {
            Ok(combined)
        } else {
            Err(combined)
        }
    })
    .await
    .map_err(|e| format!("Falha ao instalar dependencias: {e}"))?
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

#[tauri::command]
pub async fn update_transcription(job_id: i64, edited_text: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    let value = edited_text.filter(|t| !t.trim().is_empty());
    db::update_transcription_text(&state.pool, job_id, value.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_transcription(job_id: i64, destination: String, state: State<'_, AppState>) -> Result<(), String> {
    let view = db::get_transcription_by_job(&state.pool, job_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Transcricao nao encontrada".to_string())?;

    let edited_segments: Vec<String> = view
        .segments
        .iter()
        .map(|s| s.edited_text.clone().unwrap_or_else(|| s.raw_text.clone()))
        .collect();

    let text = crate::export::choose_export_text(
        view.edited_text.as_deref(),
        &edited_segments,
        &view.raw_text,
    );

    crate::export::write_txt_export(std::path::Path::new(&destination), &text)
        .map_err(|e| e.to_string())
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
            .scan_source_folder_path(temp.path().to_string_lossy().into_owned(), None)
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
            .scan_source_folder_path("C:/definitely/missing/source".to_string(), None)
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
            .scan_source_folder_path(temp.path().to_string_lossy().into_owned(), None)
            .await
            .expect("first scan should succeed");
        let second = state
            .scan_source_folder_path(temp.path().to_string_lossy().into_owned(), None)
            .await
            .expect("second scan should succeed");

        assert_eq!(first.queued_count, 1);
        // Re-scanning must not enqueue the same file again.
        assert_eq!(second.queued_count, 0);

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
