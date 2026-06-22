use std::collections::HashSet;

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
