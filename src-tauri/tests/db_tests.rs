#[path = "../src/db.rs"]
mod db;

#[tokio::test]
async fn creates_schema_in_memory() {
    let pool = db::connect_memory()
        .await
        .expect("database should initialize");

    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'media_files'",
    )
    .fetch_all(&pool)
    .await
    .expect("query should run");

    assert_eq!(tables.len(), 1);
}
