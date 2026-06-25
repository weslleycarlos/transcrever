use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;

use crate::{db, models::JobStatus, scanner::DiscoveredMedia};

pub async fn enqueue_discovered_media(
    pool: &SqlitePool,
    discovered: &[DiscoveredMedia],
    profile_id: Option<i64>,
) -> Result<Vec<i64>> {
    let mut tx = pool.begin().await?;
    let mut job_ids = Vec::with_capacity(discovered.len());

    for media in discovered {
        let size_bytes = i64::try_from(media.size_bytes)?;
        let source_root = media.source_root.to_string_lossy().into_owned();
        let absolute_path = media.absolute_path.to_string_lossy().into_owned();
        let relative_path = media.relative_path.to_string_lossy().into_owned();
        let modified_at = media.modified_at.to_rfc3339();
        let created_at = media.created_at.map(|d| d.to_rfc3339());
        let discovered_at = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO media_files
            (source_root, absolute_path, relative_path, file_name, extension, size_bytes, modified_at, created_at, discovered_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&source_root)
        .bind(&absolute_path)
        .bind(&relative_path)
        .bind(&media.file_name)
        .bind(&media.extension)
        .bind(size_bytes)
        .bind(&modified_at)
        .bind(&created_at)
        .bind(discovered_at)
        .execute(&mut *tx)
        .await?;

        let media_file_id: (i64,) = sqlx::query_as(
            r#"
            SELECT id
            FROM media_files
            WHERE absolute_path = ?1 AND size_bytes = ?2 AND modified_at = ?3
            "#,
        )
        .bind(&absolute_path)
        .bind(size_bytes)
        .bind(&modified_at)
        .fetch_one(&mut *tx)
        .await?;

        let created_at = Utc::now().to_rfc3339();
        // Idempotent: only enqueue a job when this media has NO job yet (in any
        // status). A re-scan must not duplicate files already completed/errored;
        // it should only pick up genuinely new files.
        let result = sqlx::query(
            r#"
            INSERT INTO transcription_jobs
            (media_file_id, status, profile_id, progress, created_at)
            SELECT ?1, 'pending', ?2, 0, ?3
            WHERE NOT EXISTS (
                SELECT 1 FROM transcription_jobs
                WHERE media_file_id = ?1
                  AND ((profile_id IS NULL AND ?2 IS NULL) OR profile_id = ?2)
            )
            "#,
        )
        .bind(media_file_id.0)
        .bind(profile_id)
        .bind(created_at)
        .execute(&mut *tx)
        .await?;

        // Only count newly queued jobs.
        if result.rows_affected() == 1 {
            job_ids.push(result.last_insert_rowid());
        }
    }

    tx.commit().await?;

    Ok(job_ids)
}

pub async fn mark_processing(pool: &SqlitePool, job_id: i64) -> Result<()> {
    db::update_job_status(pool, job_id, JobStatus::Processing, 0.0, None).await
}

pub async fn mark_completed(pool: &SqlitePool, job_id: i64) -> Result<()> {
    db::update_job_status(pool, job_id, JobStatus::Completed, 1.0, None).await
}

pub async fn mark_error(pool: &SqlitePool, job_id: i64, error_message: &str) -> Result<()> {
    db::update_job_status(pool, job_id, JobStatus::Error, 0.0, Some(error_message)).await
}
