use std::collections::HashSet;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use transcrever::scanner::DiscoveredMedia;

fn sample_media(file_name: &str, size_bytes: u64) -> DiscoveredMedia {
    DiscoveredMedia {
        source_root: PathBuf::from("C:/media"),
        absolute_path: PathBuf::from(format!("C:/media/{file_name}")),
        relative_path: PathBuf::from(file_name),
        file_name: file_name.to_string(),
        extension: "mp3".to_string(),
        size_bytes,
        modified_at: Utc
            .with_ymd_and_hms(2026, 6, 22, 0, 0, 0)
            .single()
            .expect("valid datetime"),
        created_at: None,
    }
}

#[tokio::test]
async fn creates_schema_in_memory() {
    let pool = transcrever::db::connect_memory()
        .await
        .expect("database should initialize");

    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE '_sqlx_%'",
    )
    .fetch_all(&pool)
    .await
    .expect("query should run");

    let table_names: HashSet<String> = tables.into_iter().map(|row| row.0).collect();
    for expected in [
        "media_files",
        "transcription_profiles",
        "transcription_jobs",
        "transcriptions",
        "transcription_segments",
        "exports",
    ] {
        assert!(table_names.contains(expected), "missing table {expected}");
    }
}

#[tokio::test]
async fn job_status_rejects_unknown_values() {
    let pool = transcrever::db::connect_memory()
        .await
        .expect("database should initialize");

    let media = sqlx::query(
        r#"
        INSERT INTO media_files
        (source_root, absolute_path, relative_path, file_name, extension, size_bytes, modified_at, discovered_at)
        VALUES ('C:/media', 'C:/media/audio.mp3', 'audio.mp3', 'audio.mp3', 'mp3', 10, '2026-06-22T00:00:00Z', '2026-06-22T00:00:00Z')
        "#,
    )
    .execute(&pool)
    .await
    .expect("valid media row should insert");

    let result = sqlx::query(
        r#"
        INSERT INTO transcription_jobs
        (media_file_id, status, progress, created_at)
        VALUES (?1, 'nonsense', 0, '2026-06-22T00:00:00Z')
        "#,
    )
    .bind(media.last_insert_rowid())
    .execute(&pool)
    .await;

    let error = result.expect_err("invalid status should violate CHECK constraint");
    let message = error.to_string();
    assert!(
        message.contains("CHECK") || message.contains("status"),
        "expected status CHECK error, got {message}"
    );
}

#[tokio::test]
async fn connects_to_file_backed_database() {
    let temp = tempfile::tempdir().expect("temp dir");
    let db_path = temp.path().join("local.sqlite");

    let pool = transcrever::db::connect(&db_path)
        .await
        .expect("file database should initialize");

    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'media_files'",
    )
    .fetch_one(&pool)
    .await
    .expect("query should run");

    assert_eq!(row.0, 1);
}

#[tokio::test]
async fn enqueues_discovered_media() {
    let pool = transcrever::db::connect_memory()
        .await
        .expect("database should initialize");

    let media = sample_media("audio.mp3", 10);

    let job_ids = transcrever::queue::enqueue_discovered_media(&pool, &[media], None)
        .await
        .expect("media should enqueue");

    assert_eq!(job_ids.len(), 1);

    let row: (String,) = sqlx::query_as("SELECT status FROM transcription_jobs WHERE id = ?1")
        .bind(job_ids[0])
        .fetch_one(&pool)
        .await
        .expect("job row should exist");

    assert_eq!(row.0, "pending");
}

#[tokio::test]
async fn enqueue_reuses_existing_active_job() {
    let pool = transcrever::db::connect_memory()
        .await
        .expect("database should initialize");
    let media = sample_media("audio.mp3", 10);

    let first = transcrever::queue::enqueue_discovered_media(&pool, &[media.clone()], None)
        .await
        .expect("first enqueue should work");
    let second = transcrever::queue::enqueue_discovered_media(&pool, &[media], None)
        .await
        .expect("second enqueue should work");

    assert_eq!(first, second);

    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transcription_jobs")
        .fetch_one(&pool)
        .await
        .expect("count should run");
    assert_eq!(row.0, 1);
}

#[tokio::test]
async fn create_pending_job_reuses_existing_active_job() {
    let pool = transcrever::db::connect_memory()
        .await
        .expect("database should initialize");
    let media = sample_media("audio.mp3", 10);
    let media_id = transcrever::db::upsert_media_file(&pool, &media)
        .await
        .expect("media should insert");

    let first = transcrever::db::create_pending_job(&pool, media_id, None)
        .await
        .expect("first job should insert");
    let second = transcrever::db::create_pending_job(&pool, media_id, None)
        .await
        .expect("second job should reuse active");

    assert_eq!(first, second);

    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transcription_jobs")
        .fetch_one(&pool)
        .await
        .expect("count should run");
    assert_eq!(row.0, 1);
}

#[tokio::test]
async fn concurrent_enqueue_creates_one_active_job() {
    let temp = tempfile::tempdir().expect("temp dir");
    let db_path = temp.path().join("concurrent.sqlite");
    let pool = transcrever::db::connect(&db_path)
        .await
        .expect("database should initialize");
    let media = sample_media("audio.mp3", 10);
    let left_batch = vec![media.clone()];
    let right_batch = vec![media];

    let (left, right) = tokio::join!(
        transcrever::queue::enqueue_discovered_media(&pool, &left_batch, None),
        transcrever::queue::enqueue_discovered_media(&pool, &right_batch, None)
    );

    let left = left.expect("left enqueue should work");
    let right = right.expect("right enqueue should work");

    assert_eq!(left.len(), 1);
    assert_eq!(right.len(), 1);
    assert_eq!(left[0], right[0]);

    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transcription_jobs")
        .fetch_one(&pool)
        .await
        .expect("count should run");
    assert_eq!(row.0, 1);
}

#[tokio::test]
async fn enqueue_rolls_back_when_job_creation_fails() {
    let pool = transcrever::db::connect_memory()
        .await
        .expect("database should initialize");
    let media = sample_media("audio.mp3", 10);

    let result = transcrever::queue::enqueue_discovered_media(&pool, &[media], Some(999)).await;

    assert!(result.is_err(), "invalid profile FK should fail enqueue");

    let media_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media_files")
        .fetch_one(&pool)
        .await
        .expect("count media should run");
    let job_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transcription_jobs")
        .fetch_one(&pool)
        .await
        .expect("count jobs should run");

    assert_eq!(media_count.0, 0);
    assert_eq!(job_count.0, 0);
}

#[tokio::test]
async fn status_update_rejects_missing_job() {
    let pool = transcrever::db::connect_memory()
        .await
        .expect("database should initialize");

    let result = transcrever::queue::mark_processing(&pool, 123).await;

    assert!(result.is_err(), "missing job update should fail");
}

#[tokio::test]
async fn status_update_preserves_started_at_and_sets_finished_at() {
    let pool = transcrever::db::connect_memory()
        .await
        .expect("database should initialize");
    let media = sample_media("audio.mp3", 10);
    let job_id = transcrever::queue::enqueue_discovered_media(&pool, &[media], None)
        .await
        .expect("enqueue should work")[0];

    transcrever::queue::mark_processing(&pool, job_id)
        .await
        .expect("mark processing");
    let first_started: (String,) =
        sqlx::query_as("SELECT started_at FROM transcription_jobs WHERE id = ?1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .expect("started_at should exist");

    transcrever::queue::mark_processing(&pool, job_id)
        .await
        .expect("mark processing again");
    let second_started: (String,) =
        sqlx::query_as("SELECT started_at FROM transcription_jobs WHERE id = ?1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .expect("started_at should exist");

    assert_eq!(first_started, second_started);

    transcrever::queue::mark_completed(&pool, job_id)
        .await
        .expect("mark completed");
    let terminal: (String, Option<String>) =
        sqlx::query_as("SELECT status, finished_at FROM transcription_jobs WHERE id = ?1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .expect("terminal state should exist");

    assert_eq!(terminal.0, "completed");
    assert!(terminal.1.is_some());
}
