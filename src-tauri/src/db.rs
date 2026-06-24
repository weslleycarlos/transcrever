#![allow(dead_code)]

use crate::models::JobStatus;
use crate::scanner::DiscoveredMedia;
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRow {
    pub id: i64,
    pub name: String,
    pub backend: String,
    pub model_path: String,
    pub device: String,
    pub precision: String,
    pub threads: i64,
    pub language: Option<String>,
    pub task: String,
    pub advanced_json: String,
}

pub async fn connect(database_path: &Path) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    migrate(&pool).await?;
    Ok(pool)
}

pub async fn connect_memory() -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;

    migrate(&pool).await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

pub async fn upsert_media_file(pool: &SqlitePool, media: &DiscoveredMedia) -> Result<i64> {
    let size_bytes =
        i64::try_from(media.size_bytes).context("media file size exceeds SQLite integer range")?;
    let source_root = media.source_root.to_string_lossy().into_owned();
    let absolute_path = media.absolute_path.to_string_lossy().into_owned();
    let relative_path = media.relative_path.to_string_lossy().into_owned();
    let modified_at = media.modified_at.to_rfc3339();
    let discovered_at = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO media_files
        (source_root, absolute_path, relative_path, file_name, extension, size_bytes, modified_at, discovered_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&source_root)
    .bind(&absolute_path)
    .bind(&relative_path)
    .bind(&media.file_name)
    .bind(&media.extension)
    .bind(size_bytes)
    .bind(&modified_at)
    .bind(discovered_at)
    .execute(pool)
    .await?;

    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT id
        FROM media_files
        WHERE absolute_path = ?1 AND size_bytes = ?2 AND modified_at = ?3
        "#,
    )
    .bind(absolute_path)
    .bind(size_bytes)
    .bind(modified_at)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

pub async fn create_pending_job(
    pool: &SqlitePool,
    media_file_id: i64,
    profile_id: Option<i64>,
) -> Result<i64> {
    let created_at = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO transcription_jobs
        (media_file_id, status, profile_id, progress, created_at)
        VALUES (?1, 'pending', ?2, 0, ?3)
        "#,
    )
    .bind(media_file_id)
    .bind(profile_id)
    .bind(created_at)
    .execute(pool)
    .await?;

    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT id
        FROM transcription_jobs
        WHERE media_file_id = ?1
          AND ((profile_id IS NULL AND ?2 IS NULL) OR profile_id = ?2)
          AND status IN ('pending', 'processing')
        ORDER BY id
        LIMIT 1
        "#,
    )
    .bind(media_file_id)
    .bind(profile_id)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

pub async fn update_job_status(
    pool: &SqlitePool,
    job_id: i64,
    status: JobStatus,
    progress: f32,
    error_message: Option<&str>,
) -> Result<()> {
    anyhow::ensure!(
        (0.0..=1.0).contains(&progress),
        "job progress must be between 0.0 and 1.0"
    );
    let updated_at = Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"
        UPDATE transcription_jobs
        SET status = ?1,
            progress = ?2,
            error_message = ?3,
            started_at = CASE WHEN ?1 = 'processing' AND started_at IS NULL THEN ?4 ELSE started_at END,
            finished_at = CASE WHEN ?1 IN ('completed', 'error') THEN ?4 ELSE NULL END
        WHERE id = ?5
        "#,
    )
    .bind(status.as_str())
    .bind(progress)
    .bind(error_message)
    .bind(updated_at)
    .bind(job_id)
    .execute(pool)
    .await?;

    anyhow::ensure!(result.rows_affected() == 1, "job {job_id} not found");

    Ok(())
}

pub async fn save_profile(pool: &SqlitePool, profile: &ProfileRow) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO transcription_profiles
        (name, backend, model_path, device, precision, threads, language, task, advanced_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&profile.name)
    .bind(&profile.backend)
    .bind(&profile.model_path)
    .bind(&profile.device)
    .bind(&profile.precision)
    .bind(profile.threads)
    .bind(&profile.language)
    .bind(&profile.task)
    .bind(&profile.advanced_json)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

pub async fn list_profiles(pool: &SqlitePool) -> Result<Vec<ProfileRow>> {
    let rows = sqlx::query_as::<_, ProfileRow>(
        r#"
        SELECT id, name, backend, model_path, device, precision, threads, language, task, advanced_json
        FROM transcription_profiles
        ORDER BY id
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn delete_profile(pool: &SqlitePool, profile_id: i64) -> Result<()> {
    let result = sqlx::query("DELETE FROM transcription_profiles WHERE id = ?1")
        .bind(profile_id)
        .execute(pool)
        .await?;

    anyhow::ensure!(result.rows_affected() == 1, "profile {profile_id} not found");
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct JobWithMedia {
    pub job_id: i64,
    pub media_file_id: i64,
    pub file_name: String,
    pub relative_path: String,
    pub status: String,
    pub progress: f32,
    pub error_message: Option<String>,
}

pub async fn list_all_jobs(pool: &SqlitePool) -> Result<Vec<JobWithMedia>> {
    let rows = sqlx::query_as::<_, JobWithMedia>(
        r#"
        SELECT
            j.id AS job_id,
            m.id AS media_file_id,
            m.file_name,
            m.relative_path,
            j.status,
            j.progress,
            j.error_message
        FROM transcription_jobs j
        JOIN media_files m ON m.id = j.media_file_id
        ORDER BY
            CASE j.status
                WHEN 'processing' THEN 0
                WHEN 'pending' THEN 1
                WHEN 'error' THEN 2
                WHEN 'completed' THEN 3
                ELSE 4
            END,
            j.id
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn find_next_pending_job(pool: &SqlitePool) -> Result<Option<(i64, i64, String)>> {
    let row = sqlx::query_as::<_, (i64, i64, String)>(
        r#"
        SELECT j.id, m.id, m.absolute_path
        FROM transcription_jobs j
        JOIN media_files m ON m.id = j.media_file_id
        WHERE j.status = 'pending'
        ORDER BY j.id
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn save_transcription(
    pool: &SqlitePool,
    job_id: i64,
    media_file_id: i64,
    raw_text: &str,
    segments: &[(i64, i64, String, Option<f32>)],
) -> Result<i64> {
    let now = Utc::now().to_rfc3339();

    let transcription_id = sqlx::query(
        r#"
        INSERT INTO transcriptions (media_file_id, job_id, raw_text, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(media_file_id)
    .bind(job_id)
    .bind(raw_text)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?
    .last_insert_rowid();

    for (idx, (start_ms, end_ms, text, confidence)) in segments.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO transcription_segments
            (transcription_id, segment_index, start_ms, end_ms, raw_text, confidence)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(transcription_id)
        .bind(idx as i64)
        .bind(start_ms)
        .bind(end_ms)
        .bind(text)
        .bind(confidence)
        .execute(pool)
        .await?;
    }

    Ok(transcription_id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionView {
    pub transcription_id: i64,
    pub media_file_id: i64,
    pub job_id: i64,
    pub file_name: String,
    pub absolute_path: String,
    pub raw_text: String,
    pub edited_text: Option<String>,
    pub segments: Vec<SegmentView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentView {
    pub id: i64,
    pub segment_index: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub raw_text: String,
    pub edited_text: Option<String>,
    pub confidence: Option<f32>,
}

pub async fn get_transcription_by_job(pool: &SqlitePool, job_id: i64) -> Result<Option<TranscriptionView>> {
    let row = sqlx::query_as::<_, (i64, i64, i64, String, String, String, Option<String>)>(
        r#"
        SELECT t.id, t.media_file_id, t.job_id, m.file_name, m.absolute_path, t.raw_text, t.edited_text
        FROM transcriptions t
        JOIN media_files m ON m.id = t.media_file_id
        WHERE t.job_id = ?1
        "#,
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await?;

    let (transcription_id, media_file_id, jid, file_name, absolute_path, raw_text, edited_text) =
        match row {
            Some(r) => r,
            None => return Ok(None),
        };

    let segments = sqlx::query_as::<_, (i64, i64, i64, i64, String, Option<String>, Option<f32>)>(
        r#"
        SELECT id, segment_index, start_ms, end_ms, raw_text, edited_text, confidence
        FROM transcription_segments
        WHERE transcription_id = ?1
        ORDER BY segment_index
        "#,
    )
    .bind(transcription_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id, idx, start_ms, end_ms, raw, edited, conf)| SegmentView {
        id,
        segment_index: idx,
        start_ms,
        end_ms,
        raw_text: raw,
        edited_text: edited,
        confidence: conf,
    })
    .collect();

    Ok(Some(TranscriptionView {
        transcription_id,
        media_file_id,
        job_id: jid,
        file_name,
        absolute_path,
        raw_text,
        edited_text,
        segments,
    }))
}

pub async fn count_profiles(pool: &SqlitePool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transcription_profiles")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn create_default_profile(pool: &SqlitePool, model_path: &str) -> Result<i64> {
    let profile = ProfileRow {
        id: 0,
        name: "Padrao (base.pt)".to_string(),
        backend: "whisper_cpp".to_string(),
        model_path: model_path.to_string(),
        device: "cpu".to_string(),
        precision: "auto".to_string(),
        threads: 4,
        language: Some("pt".to_string()),
        task: "transcribe".to_string(),
        advanced_json: "{}".to_string(),
    };
    save_profile(pool, &profile).await
}
