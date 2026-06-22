#![allow(dead_code)]

use anyhow::Result;
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
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;

    migrate(&pool).await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    let migrations = [
        r#"
        CREATE TABLE IF NOT EXISTS media_files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_root TEXT NOT NULL,
            absolute_path TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            extension TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            modified_at TEXT NOT NULL,
            duration_ms INTEGER,
            discovered_at TEXT NOT NULL,
            UNIQUE(absolute_path, size_bytes, modified_at)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS transcription_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            backend TEXT NOT NULL,
            model_path TEXT NOT NULL,
            device TEXT NOT NULL,
            precision TEXT NOT NULL,
            threads INTEGER NOT NULL,
            language TEXT,
            task TEXT NOT NULL,
            advanced_json TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS transcription_jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            media_file_id INTEGER NOT NULL,
            status TEXT NOT NULL,
            profile_id INTEGER,
            progress REAL NOT NULL DEFAULT 0,
            error_message TEXT,
            created_at TEXT NOT NULL,
            started_at TEXT,
            finished_at TEXT,
            FOREIGN KEY(media_file_id) REFERENCES media_files(id),
            FOREIGN KEY(profile_id) REFERENCES transcription_profiles(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS transcriptions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            media_file_id INTEGER NOT NULL,
            job_id INTEGER NOT NULL,
            raw_text TEXT NOT NULL,
            edited_text TEXT,
            is_reviewed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(media_file_id) REFERENCES media_files(id),
            FOREIGN KEY(job_id) REFERENCES transcription_jobs(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS transcription_segments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            transcription_id INTEGER NOT NULL,
            segment_index INTEGER NOT NULL,
            start_ms INTEGER NOT NULL,
            end_ms INTEGER NOT NULL,
            raw_text TEXT NOT NULL,
            edited_text TEXT,
            confidence REAL,
            FOREIGN KEY(transcription_id) REFERENCES transcriptions(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS exports (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            transcription_id INTEGER NOT NULL,
            export_path TEXT NOT NULL,
            format TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(transcription_id) REFERENCES transcriptions(id)
        )
        "#,
    ];

    for migration in migrations {
        sqlx::query(migration).execute(pool).await?;
    }

    Ok(())
}
