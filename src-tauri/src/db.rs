#![allow(dead_code)]

use crate::scanner::DiscoveredMedia;
use crate::models::JobStatus;
use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::path::Path;

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
