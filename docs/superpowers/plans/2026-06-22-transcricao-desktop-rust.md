# Transcricao Desktop Rust Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Windows desktop transcription app in Rust/Tauri that scans folders recursively, queues media files, transcribes them through an initial `whisper.cpp` backend, stores results in SQLite, supports review with audio playback, and exports `.txt`.

**Architecture:** The app uses Tauri for desktop integration, a TypeScript frontend for the review/work queue UI, and Rust commands for scanning, persistence, queue orchestration, transcription, and export. SQLite is the source of truth; exported `.txt` files are derived artifacts.

**Tech Stack:** Rust, Tauri 2, TypeScript, Vite, SQLite via `sqlx`, recursive scanning via `walkdir`, async runtime via `tokio`, command execution via `tokio::process`, frontend state with plain React hooks or lightweight stores.

---

## File Structure

- Create `package.json`: frontend scripts and Tauri dependencies.
- Create `src/`: TypeScript frontend.
- Create `src/App.tsx`: main app shell, navigation, state orchestration.
- Create `src/main.tsx`: frontend entry point.
- Create `src/styles.css`: desktop UI styling.
- Create `src/types.ts`: shared frontend types matching Tauri command responses.
- Create `src-tauri/Cargo.toml`: Rust package and dependencies.
- Create `src-tauri/tauri.conf.json`: Tauri app configuration.
- Create `src-tauri/build.rs`: Tauri build hook.
- Create `src-tauri/src/main.rs`: Tauri bootstrap and command registration.
- Create `src-tauri/src/db.rs`: SQLite connection, migrations, repositories.
- Create `src-tauri/src/models.rs`: Rust data models and enums.
- Create `src-tauri/src/scanner.rs`: recursive media discovery.
- Create `src-tauri/src/queue.rs`: job creation, status transitions, retry logic.
- Create `src-tauri/src/backend/mod.rs`: transcription backend trait and shared types.
- Create `src-tauri/src/backend/whisper_cpp.rs`: `whisper.cpp` command backend.
- Create `src-tauri/src/export.rs`: `.txt` export logic.
- Create `src-tauri/src/commands.rs`: Tauri commands exposed to frontend.
- Create `src-tauri/tests/scanner_tests.rs`: scanner integration tests.
- Create `src-tauri/tests/db_tests.rs`: SQLite integration tests.
- Create `src-tauri/tests/export_tests.rs`: export behavior tests.
- Create `docs/dev/manual-test.md`: manual verification checklist.

---

### Task 1: Scaffold Tauri App

**Files:**
- Create: `package.json`
- Create: `index.html`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Create: `src/styles.css`
- Create: `src/types.ts`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/src/main.rs`

- [ ] **Step 1: Create frontend package files**

Create `package.json`:

```json
{
  "name": "transcrever",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "@vitejs/plugin-react": "^5.0.0",
    "vite": "^7.0.0",
    "typescript": "^5.8.0",
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "lucide-react": "^0.468.0"
  },
  "devDependencies": {}
}
```

Create `index.html`:

```html
<!doctype html>
<html lang="pt-BR">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Transcrever</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 2: Create minimal frontend entry**

Create `src/main.tsx`:

```tsx
import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

Create `src/types.ts`:

```ts
export type JobStatus = "pending" | "processing" | "completed" | "error" | "reviewed" | "exported";

export interface MediaFile {
  id: number;
  relativePath: string;
  fileName: string;
  extension: string;
  sizeBytes: number;
}

export interface JobSummary {
  id: number;
  mediaFileId: number;
  status: JobStatus;
  progress: number;
  errorMessage?: string | null;
}
```

Create `src/App.tsx`:

```tsx
export default function App() {
  return (
    <main className="app-shell">
      <aside className="sidebar">
        <h1>Transcrever</h1>
        <button type="button">Origem</button>
        <button type="button">Destino</button>
        <button type="button">Perfil</button>
      </aside>
      <section className="workspace">
        <header className="toolbar">
          <h2>Fila</h2>
          <button type="button">Iniciar</button>
        </header>
        <div className="empty-state">Selecione uma pasta de origem para montar a fila.</div>
      </section>
    </main>
  );
}
```

Create `src/styles.css`:

```css
* {
  box-sizing: border-box;
}

body {
  margin: 0;
  font-family: Inter, Segoe UI, Arial, sans-serif;
  background: #f6f7f9;
  color: #20242a;
}

button {
  border: 1px solid #c8ced8;
  background: #ffffff;
  color: #20242a;
  border-radius: 6px;
  min-height: 36px;
  padding: 0 12px;
  cursor: pointer;
}

.app-shell {
  display: grid;
  grid-template-columns: 240px 1fr;
  min-height: 100vh;
}

.sidebar {
  border-right: 1px solid #dfe3ea;
  background: #ffffff;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.sidebar h1 {
  font-size: 20px;
  margin: 0 0 12px;
}

.workspace {
  padding: 20px;
  min-width: 0;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
}

.toolbar h2 {
  font-size: 18px;
  margin: 0;
}

.empty-state {
  border: 1px dashed #b9c1ce;
  border-radius: 8px;
  padding: 28px;
  background: #ffffff;
}
```

- [ ] **Step 3: Create Rust/Tauri package**

Create `src-tauri/Cargo.toml`:

```toml
[package]
name = "transcrever"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono", "migrate"] }
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
tauri-plugin-opener = "2"
thiserror = "2"
tokio = { version = "1", features = ["macros", "process", "rt-multi-thread", "sync"] }
walkdir = "2"

[dev-dependencies]
tempfile = "3"
```

Create `src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

Create `src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Transcrever",
  "version": "0.1.0",
  "identifier": "br.local.transcrever",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Transcrever",
        "width": 1280,
        "height": 800,
        "minWidth": 980,
        "minHeight": 640
      }
    ]
  },
  "bundle": {
    "active": true,
    "targets": "all"
  }
}
```

Create `src-tauri/src/main.rs`:

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
```

- [ ] **Step 4: Verify scaffold**

Run: `npm install`

Expected: dependencies install successfully.

Run: `npm run build`

Expected: TypeScript and Vite build complete successfully.

Run: `cd src-tauri; cargo test`

Expected: Rust crate compiles and reports zero tests or passing tests.

- [ ] **Step 5: Commit**

If Git has not been initialized, run:

```bash
git init
```

Then commit:

```bash
git add package.json package-lock.json index.html src src-tauri
git commit -m "chore: scaffold tauri transcription app"
```

---

### Task 2: SQLite Models And Migrations

**Files:**
- Create: `src-tauri/src/models.rs`
- Create: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/main.rs`
- Create: `src-tauri/tests/db_tests.rs`

- [ ] **Step 1: Add shared models**

Create `src-tauri/src/models.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Error,
    Reviewed,
    Exported,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Processing => "processing",
            JobStatus::Completed => "completed",
            JobStatus::Error => "error",
            JobStatus::Reviewed => "reviewed",
            JobStatus::Exported => "exported",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFile {
    pub id: i64,
    pub source_root: String,
    pub absolute_path: String,
    pub relative_path: String,
    pub file_name: String,
    pub extension: String,
    pub size_bytes: i64,
    pub modified_at: DateTime<Utc>,
    pub duration_ms: Option<i64>,
    pub discovered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionJob {
    pub id: i64,
    pub media_file_id: i64,
    pub status: JobStatus,
    pub profile_id: Option<i64>,
    pub progress: f32,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionSegment {
    pub id: i64,
    pub transcription_id: i64,
    pub segment_index: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub raw_text: String,
    pub edited_text: Option<String>,
    pub confidence: Option<f32>,
}
```

- [ ] **Step 2: Add database initialization**

Create `src-tauri/src/db.rs`:

```rust
use anyhow::Result;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::Path;

pub async fn connect(database_path: &Path) -> Result<SqlitePool> {
    let url = format!("sqlite://{}", database_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
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
    sqlx::query(
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
        );

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
        );

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
        );

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
        );

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
        );

        CREATE TABLE IF NOT EXISTS exports (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            transcription_id INTEGER NOT NULL,
            export_path TEXT NOT NULL,
            format TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(transcription_id) REFERENCES transcriptions(id)
        );
        "#
    )
    .execute(pool)
    .await?;

    Ok(())
}
```

- [ ] **Step 3: Register modules**

Modify `src-tauri/src/main.rs`:

```rust
mod db;
mod models;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
```

- [ ] **Step 4: Add database test**

Create `src-tauri/tests/db_tests.rs`:

```rust
#[path = "../src/db.rs"]
mod db;

#[tokio::test]
async fn creates_schema_in_memory() {
    let pool = db::connect_memory().await.expect("database should initialize");

    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'media_files'"
    )
    .fetch_all(&pool)
    .await
    .expect("query should run");

    assert_eq!(tables.len(), 1);
}
```

- [ ] **Step 5: Run tests**

Run: `cd src-tauri; cargo test creates_schema_in_memory`

Expected: test passes.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src src-tauri/tests
git commit -m "feat: add sqlite schema"
```

---

### Task 3: Recursive Media Scanner

**Files:**
- Create: `src-tauri/src/scanner.rs`
- Modify: `src-tauri/src/main.rs`
- Create: `src-tauri/tests/scanner_tests.rs`

- [ ] **Step 1: Add scanner implementation**

Create `src-tauri/src/scanner.rs`:

```rust
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "wav", "m4a", "flac", "ogg", "opus", "aac", "wma",
    "mp4", "mkv", "mov", "avi", "webm",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredMedia {
    pub source_root: PathBuf,
    pub absolute_path: PathBuf,
    pub relative_path: PathBuf,
    pub file_name: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified_at: DateTime<Utc>,
}

pub fn is_supported_media(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            SUPPORTED_EXTENSIONS.contains(&ext.as_str())
        })
        .unwrap_or(false)
}

pub fn scan_media(source_root: &Path) -> Result<Vec<DiscoveredMedia>> {
    let source_root = source_root
        .canonicalize()
        .with_context(|| format!("source folder does not exist: {}", source_root.display()))?;

    let mut results = Vec::new();

    for entry in WalkDir::new(&source_root).follow_links(false) {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type().is_file() || !is_supported_media(path) {
            continue;
        }

        let metadata = entry.metadata()?;
        let modified_at: DateTime<Utc> = metadata.modified()?.into();
        let absolute_path = path.canonicalize()?;
        let relative_path = absolute_path
            .strip_prefix(&source_root)
            .unwrap_or(&absolute_path)
            .to_path_buf();

        results.push(DiscoveredMedia {
            source_root: source_root.clone(),
            absolute_path,
            relative_path,
            file_name: entry.file_name().to_string_lossy().to_string(),
            extension: path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase(),
            size_bytes: metadata.len(),
            modified_at,
        });
    }

    results.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(results)
}
```

- [ ] **Step 2: Register scanner module**

Modify `src-tauri/src/main.rs`:

```rust
mod db;
mod models;
mod scanner;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
```

- [ ] **Step 3: Add scanner tests**

Create `src-tauri/tests/scanner_tests.rs`:

```rust
#[path = "../src/scanner.rs"]
mod scanner;

use std::fs;

#[test]
fn scans_supported_media_recursively() {
    let temp = tempfile::tempdir().expect("temp dir");
    let nested = temp.path().join("a").join("b");
    fs::create_dir_all(&nested).expect("nested dirs");
    fs::write(temp.path().join("root.mp3"), b"audio").expect("root file");
    fs::write(nested.join("clip.MP4"), b"video").expect("nested file");
    fs::write(nested.join("notes.txt"), b"ignore").expect("ignored file");

    let files = scanner::scan_media(temp.path()).expect("scan should work");

    let names: Vec<String> = files.into_iter().map(|file| file.file_name).collect();
    assert_eq!(names, vec!["root.mp3".to_string(), "clip.MP4".to_string()]);
}

#[test]
fn rejects_unsupported_extension() {
    assert!(!scanner::is_supported_media(std::path::Path::new("document.pdf")));
    assert!(scanner::is_supported_media(std::path::Path::new("audio.wav")));
}
```

- [ ] **Step 4: Run scanner tests**

Run: `cd src-tauri; cargo test scanner`

Expected: scanner tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/scanner.rs src-tauri/src/main.rs src-tauri/tests/scanner_tests.rs
git commit -m "feat: add recursive media scanner"
```

---

### Task 4: Database Repositories And Queue

**Files:**
- Modify: `src-tauri/src/db.rs`
- Create: `src-tauri/src/queue.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/tests/db_tests.rs`

- [ ] **Step 1: Add repository functions**

Append to `src-tauri/src/db.rs`:

```rust
use crate::scanner::DiscoveredMedia;
use chrono::Utc;

pub async fn upsert_media_file(pool: &SqlitePool, media: &DiscoveredMedia) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    let modified_at = media.modified_at.to_rfc3339();

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO media_files
        (source_root, absolute_path, relative_path, file_name, extension, size_bytes, modified_at, discovered_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#
    )
    .bind(media.source_root.to_string_lossy().to_string())
    .bind(media.absolute_path.to_string_lossy().to_string())
    .bind(media.relative_path.to_string_lossy().to_string())
    .bind(&media.file_name)
    .bind(&media.extension)
    .bind(media.size_bytes as i64)
    .bind(&modified_at)
    .bind(&now)
    .execute(pool)
    .await?;

    let id: (i64,) = sqlx::query_as(
        r#"
        SELECT id FROM media_files
        WHERE absolute_path = ?1 AND size_bytes = ?2 AND modified_at = ?3
        "#
    )
    .bind(media.absolute_path.to_string_lossy().to_string())
    .bind(media.size_bytes as i64)
    .bind(&modified_at)
    .fetch_one(pool)
    .await?;

    Ok(id.0)
}

pub async fn create_pending_job(pool: &SqlitePool, media_file_id: i64, profile_id: Option<i64>) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        INSERT INTO transcription_jobs
        (media_file_id, status, profile_id, progress, created_at)
        VALUES (?1, 'pending', ?2, 0, ?3)
        "#
    )
    .bind(media_file_id)
    .bind(profile_id)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

pub async fn update_job_status(
    pool: &SqlitePool,
    job_id: i64,
    status: &str,
    progress: f32,
    error_message: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE transcription_jobs
        SET status = ?1, progress = ?2, error_message = ?3,
            started_at = CASE WHEN ?1 = 'processing' THEN datetime('now') ELSE started_at END,
            finished_at = CASE WHEN ?1 IN ('completed', 'error') THEN datetime('now') ELSE finished_at END
        WHERE id = ?4
        "#
    )
    .bind(status)
    .bind(progress)
    .bind(error_message)
    .bind(job_id)
    .execute(pool)
    .await?;

    Ok(())
}
```

- [ ] **Step 2: Add queue orchestration**

Create `src-tauri/src/queue.rs`:

```rust
use anyhow::Result;
use sqlx::SqlitePool;

use crate::{db, scanner::DiscoveredMedia};

pub async fn enqueue_discovered_media(
    pool: &SqlitePool,
    discovered: &[DiscoveredMedia],
    profile_id: Option<i64>,
) -> Result<Vec<i64>> {
    let mut job_ids = Vec::with_capacity(discovered.len());

    for media in discovered {
        let media_id = db::upsert_media_file(pool, media).await?;
        let job_id = db::create_pending_job(pool, media_id, profile_id).await?;
        job_ids.push(job_id);
    }

    Ok(job_ids)
}

pub async fn mark_processing(pool: &SqlitePool, job_id: i64) -> Result<()> {
    db::update_job_status(pool, job_id, "processing", 0.0, None).await
}

pub async fn mark_completed(pool: &SqlitePool, job_id: i64) -> Result<()> {
    db::update_job_status(pool, job_id, "completed", 1.0, None).await
}

pub async fn mark_error(pool: &SqlitePool, job_id: i64, message: &str) -> Result<()> {
    db::update_job_status(pool, job_id, "error", 0.0, Some(message)).await
}
```

- [ ] **Step 3: Register queue module**

Modify `src-tauri/src/main.rs`:

```rust
mod db;
mod models;
mod queue;
mod scanner;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
```

- [ ] **Step 4: Add queue test**

Append to `src-tauri/tests/db_tests.rs`:

```rust
#[path = "../src/scanner.rs"]
mod scanner;
#[path = "../src/queue.rs"]
mod queue;

use chrono::Utc;
use scanner::DiscoveredMedia;
use std::path::PathBuf;

#[tokio::test]
async fn enqueues_discovered_media() {
    let pool = db::connect_memory().await.expect("database should initialize");
    let media = DiscoveredMedia {
        source_root: PathBuf::from("C:/media"),
        absolute_path: PathBuf::from("C:/media/audio.mp3"),
        relative_path: PathBuf::from("audio.mp3"),
        file_name: "audio.mp3".to_string(),
        extension: "mp3".to_string(),
        size_bytes: 10,
        modified_at: Utc::now(),
    };

    let jobs = queue::enqueue_discovered_media(&pool, &[media], None)
        .await
        .expect("enqueue should work");

    assert_eq!(jobs.len(), 1);
}
```

- [ ] **Step 5: Run queue tests**

Run: `cd src-tauri; cargo test enqueues_discovered_media`

Expected: test passes.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src src-tauri/tests/db_tests.rs
git commit -m "feat: persist media queue"
```

---

### Task 5: Export Reviewed Text To TXT

**Files:**
- Create: `src-tauri/src/export.rs`
- Modify: `src-tauri/src/main.rs`
- Create: `src-tauri/tests/export_tests.rs`

- [ ] **Step 1: Add export selection rules**

Create `src-tauri/src/export.rs`:

```rust
use anyhow::{Context, Result};
use std::{fs, path::Path};

pub fn choose_export_text(
    edited_text: Option<&str>,
    edited_segments: &[String],
    raw_text: &str,
) -> String {
    if let Some(text) = edited_text {
        if !text.trim().is_empty() {
            return text.to_string();
        }
    }

    let segment_text = edited_segments
        .iter()
        .map(|segment| segment.trim())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if !segment_text.trim().is_empty() {
        return segment_text;
    }

    raw_text.to_string()
}

pub fn write_txt_export(destination: &Path, text: &str) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create export folder {}", parent.display()))?;
    }
    fs::write(destination, text)
        .with_context(|| format!("failed to write export {}", destination.display()))?;
    Ok(())
}
```

- [ ] **Step 2: Register export module**

Modify `src-tauri/src/main.rs`:

```rust
mod db;
mod export;
mod models;
mod queue;
mod scanner;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
```

- [ ] **Step 3: Add export tests**

Create `src-tauri/tests/export_tests.rs`:

```rust
#[path = "../src/export.rs"]
mod export;

#[test]
fn prefers_edited_text_for_export() {
    let text = export::choose_export_text(
        Some("texto revisado"),
        &["segmento revisado".to_string()],
        "texto bruto",
    );

    assert_eq!(text, "texto revisado");
}

#[test]
fn falls_back_to_segments_before_raw_text() {
    let text = export::choose_export_text(
        None,
        &["primeiro".to_string(), "segundo".to_string()],
        "texto bruto",
    );

    assert_eq!(text, "primeiro\nsegundo");
}

#[test]
fn writes_txt_export() {
    let temp = tempfile::tempdir().expect("temp dir");
    let destination = temp.path().join("out").join("audio.txt");

    export::write_txt_export(&destination, "conteudo").expect("export should write");

    let content = std::fs::read_to_string(destination).expect("read export");
    assert_eq!(content, "conteudo");
}
```

- [ ] **Step 4: Run export tests**

Run: `cd src-tauri; cargo test export`

Expected: export tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/export.rs src-tauri/src/main.rs src-tauri/tests/export_tests.rs
git commit -m "feat: add txt export rules"
```

---

### Task 6: Transcription Backend Trait And Whisper CPP Adapter

**Files:**
- Create: `src-tauri/src/backend/mod.rs`
- Create: `src-tauri/src/backend/whisper_cpp.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Add backend trait**

Create `src-tauri/src/backend/mod.rs`:

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod whisper_cpp;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionProfile {
    pub model_path: String,
    pub device: String,
    pub precision: String,
    pub threads: usize,
    pub language: Option<String>,
    pub task: String,
    pub advanced_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendTranscription {
    pub raw_text: String,
    pub segments: Vec<BackendSegment>,
}

pub trait TranscriptionBackend {
    fn transcribe(&self, media_path: &Path, profile: &TranscriptionProfile) -> Result<BackendTranscription>;
}
```

- [ ] **Step 2: Add `whisper.cpp` command adapter**

Create `src-tauri/src/backend/whisper_cpp.rs`:

```rust
use anyhow::{anyhow, Context, Result};
use std::{path::{Path, PathBuf}, process::Command};

use super::{BackendSegment, BackendTranscription, TranscriptionBackend, TranscriptionProfile};

pub struct WhisperCppBackend {
    executable_path: PathBuf,
}

impl WhisperCppBackend {
    pub fn new(executable_path: PathBuf) -> Self {
        Self { executable_path }
    }

    fn build_args(&self, media_path: &Path, profile: &TranscriptionProfile) -> Vec<String> {
        let mut args = vec![
            "-m".to_string(),
            profile.model_path.clone(),
            "-f".to_string(),
            media_path.to_string_lossy().to_string(),
            "-t".to_string(),
            profile.threads.to_string(),
            "-oj".to_string(),
        ];

        if let Some(language) = &profile.language {
            if !language.trim().is_empty() {
                args.push("-l".to_string());
                args.push(language.clone());
            }
        }

        if profile.task == "translate" {
            args.push("-tr".to_string());
        }

        args
    }
}

impl TranscriptionBackend for WhisperCppBackend {
    fn transcribe(&self, media_path: &Path, profile: &TranscriptionProfile) -> Result<BackendTranscription> {
        let args = self.build_args(media_path, profile);
        let output = Command::new(&self.executable_path)
            .args(&args)
            .output()
            .with_context(|| format!("failed to run {}", self.executable_path.display()))?;

        if !output.status.success() {
            return Err(anyhow!(
                "whisper.cpp failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let raw_text = stdout.trim().to_string();

        Ok(BackendTranscription {
            raw_text: raw_text.clone(),
            segments: vec![BackendSegment {
                start_ms: 0,
                end_ms: 0,
                text: raw_text,
                confidence: None,
            }],
        })
    }
}
```

- [ ] **Step 3: Register backend module**

Modify `src-tauri/src/main.rs`:

```rust
mod backend;
mod db;
mod export;
mod models;
mod queue;
mod scanner;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
```

- [ ] **Step 4: Compile backend**

Run: `cd src-tauri; cargo test`

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/backend src-tauri/src/main.rs
git commit -m "feat: add whisper cpp backend adapter"
```

---

### Task 7: Tauri Commands For Scan, Queue, Review, Export

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Add application state and command skeletons**

Create `src-tauri/src/commands.rs`:

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{path::PathBuf, sync::Arc};
use tauri::State;
use tokio::sync::Mutex;

use crate::{queue, scanner};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub selected_source: Arc<Mutex<Option<PathBuf>>>,
    pub selected_destination: Arc<Mutex<Option<PathBuf>>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResponse {
    pub discovered_count: usize,
    pub queued_count: usize,
}

#[tauri::command]
pub async fn scan_source_folder(path: String, state: State<'_, AppState>) -> Result<ScanResponse, String> {
    let source = PathBuf::from(path);
    let discovered = scanner::scan_media(&source).map_err(|error| error.to_string())?;
    let jobs = queue::enqueue_discovered_media(&state.pool, &discovered, None)
        .await
        .map_err(|error| error.to_string())?;

    *state.selected_source.lock().await = Some(source);

    Ok(ScanResponse {
        discovered_count: discovered.len(),
        queued_count: jobs.len(),
    })
}

#[tauri::command]
pub async fn set_export_folder(path: String, state: State<'_, AppState>) -> Result<(), String> {
    *state.selected_destination.lock().await = Some(PathBuf::from(path));
    Ok(())
}
```

- [ ] **Step 2: Wire state in main**

Modify `src-tauri/src/main.rs`:

```rust
mod backend;
mod commands;
mod db;
mod export;
mod models;
mod queue;
mod scanner;

use commands::AppState;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

fn database_path() -> PathBuf {
    std::env::current_dir()
        .expect("current dir")
        .join("transcrever.sqlite")
}

fn main() {
    let pool = tauri::async_runtime::block_on(async {
        db::connect(&database_path())
            .await
            .expect("database should initialize")
    });

    tauri::Builder::default()
        .manage(AppState {
            pool,
            selected_source: Arc::new(Mutex::new(None)),
            selected_destination: Arc::new(Mutex::new(None)),
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::scan_source_folder,
            commands::set_export_folder
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
```

- [ ] **Step 3: Compile command layer**

Run: `cd src-tauri; cargo test`

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/main.rs
git commit -m "feat: expose scan and export folder commands"
```

---

### Task 8: Frontend Folder Selection And Queue Shell

**Files:**
- Modify: `src/types.ts`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Add frontend API types**

Modify `src/types.ts`:

```ts
export type JobStatus = "pending" | "processing" | "completed" | "error" | "reviewed" | "exported";

export interface MediaFile {
  id: number;
  relativePath: string;
  fileName: string;
  extension: string;
  sizeBytes: number;
}

export interface JobSummary {
  id: number;
  mediaFileId: number;
  status: JobStatus;
  progress: number;
  errorMessage?: string | null;
}

export interface ScanResponse {
  discoveredCount: number;
  queuedCount: number;
}
```

- [ ] **Step 2: Add folder actions**

Modify `src/App.tsx`:

```tsx
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { FolderOpen, Play, Download } from "lucide-react";
import { useState } from "react";
import type { ScanResponse } from "./types";

export default function App() {
  const [source, setSource] = useState<string>("");
  const [destination, setDestination] = useState<string>("");
  const [message, setMessage] = useState("Selecione uma pasta de origem para montar a fila.");

  async function chooseSource() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;
    setSource(selected);
    const response = await invoke<ScanResponse>("scan_source_folder", { path: selected });
    setMessage(`${response.discoveredCount} arquivos encontrados, ${response.queuedCount} jobs criados.`);
  }

  async function chooseDestination() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;
    setDestination(selected);
    await invoke("set_export_folder", { path: selected });
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <h1>Transcrever</h1>
        <button type="button" onClick={chooseSource}>
          <FolderOpen size={16} /> Origem
        </button>
        <button type="button" onClick={chooseDestination}>
          <Download size={16} /> Destino
        </button>
        <button type="button">
          <Play size={16} /> Perfil
        </button>
        <div className="path-box">{source || "Origem nao selecionada"}</div>
        <div className="path-box">{destination || "Destino nao selecionado"}</div>
      </aside>
      <section className="workspace">
        <header className="toolbar">
          <h2>Fila</h2>
          <button type="button">
            <Play size={16} /> Iniciar
          </button>
        </header>
        <div className="empty-state">{message}</div>
      </section>
    </main>
  );
}
```

- [ ] **Step 3: Update button styles**

Append to `src/styles.css`:

```css
button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.path-box {
  overflow-wrap: anywhere;
  border: 1px solid #e2e6ed;
  background: #f9fafc;
  border-radius: 6px;
  padding: 8px;
  font-size: 12px;
  line-height: 1.35;
  color: #4b5563;
}
```

- [ ] **Step 4: Build frontend**

Run: `npm run build`

Expected: frontend build succeeds.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "feat: add folder selection UI"
```

---

### Task 9: Review UI With Player And Text Views

**Files:**
- Modify: `src/types.ts`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Add review types**

Append to `src/types.ts`:

```ts
export interface Segment {
  id: number;
  startMs: number;
  endMs: number;
  rawText: string;
  editedText?: string | null;
}

export interface ReviewDocument {
  mediaPath: string;
  rawText: string;
  editedText?: string | null;
  segments: Segment[];
}
```

- [ ] **Step 2: Add static review layout**

Replace the `workspace` section in `src/App.tsx` with:

```tsx
<section className="workspace">
  <header className="toolbar">
    <h2>Fila</h2>
    <button type="button">
      <Play size={16} /> Iniciar
    </button>
  </header>
  <div className="queue-summary">{message}</div>
  <section className="review-layout">
    <div className="segments-panel">
      <h3>Segmentos</h3>
      <button type="button" className="segment-row">
        <span>00:00 - 00:10</span>
        <strong>Exemplo de trecho transcrito.</strong>
      </button>
    </div>
    <div className="text-panel">
      <h3>Texto continuo</h3>
      <textarea defaultValue="Exemplo de texto continuo para revisao." />
      <audio controls />
    </div>
  </section>
</section>
```

- [ ] **Step 3: Add review styles**

Append to `src/styles.css`:

```css
.queue-summary {
  border: 1px solid #dfe3ea;
  background: #ffffff;
  border-radius: 8px;
  padding: 12px;
  margin-bottom: 16px;
}

.review-layout {
  display: grid;
  grid-template-columns: minmax(280px, 420px) 1fr;
  gap: 16px;
  min-height: 520px;
}

.segments-panel,
.text-panel {
  border: 1px solid #dfe3ea;
  background: #ffffff;
  border-radius: 8px;
  padding: 14px;
  min-width: 0;
}

.segments-panel h3,
.text-panel h3 {
  margin: 0 0 12px;
  font-size: 15px;
}

.segment-row {
  width: 100%;
  align-items: flex-start;
  justify-content: flex-start;
  flex-direction: column;
  text-align: left;
  height: auto;
  padding: 10px;
}

.segment-row span {
  font-size: 12px;
  color: #667085;
}

.segment-row strong {
  font-size: 14px;
  font-weight: 500;
}

.text-panel {
  display: grid;
  grid-template-rows: auto 1fr auto;
  gap: 12px;
}

.text-panel textarea {
  width: 100%;
  min-height: 360px;
  resize: vertical;
  border: 1px solid #c8ced8;
  border-radius: 6px;
  padding: 12px;
  font: inherit;
  line-height: 1.5;
}

.text-panel audio {
  width: 100%;
}
```

- [ ] **Step 4: Build frontend**

Run: `npm run build`

Expected: frontend build succeeds.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "feat: add transcription review layout"
```

---

### Task 10: Manual Verification Document

**Files:**
- Create: `docs/dev/manual-test.md`

- [ ] **Step 1: Add manual checklist**

Create `docs/dev/manual-test.md`:

```markdown
# Manual Test Checklist

## Setup

- Install frontend dependencies with `npm install`.
- Confirm Rust toolchain is installed with `cargo --version`.
- Place a short `.mp3` or `.wav` file in a sample folder.
- Configure a local `whisper.cpp` executable and compatible model before testing transcription.

## Smoke Test

1. Run `npm run tauri dev`.
2. Select the sample folder as origin.
3. Confirm the app reports discovered files.
4. Select an export folder.
5. Start one transcription job.
6. Confirm the job reaches completed or shows a clear error.
7. Open the review area.
8. Play audio.
9. Edit a segment.
10. Edit continuous text.
11. Export `.txt`.
12. Close and reopen the app.
13. Confirm queue and transcription state remain available.

## Failure Test

1. Configure an invalid model path.
2. Start a transcription.
3. Confirm the job enters error state.
4. Confirm the rest of the queue remains usable.
```

- [ ] **Step 2: Commit**

```bash
git add docs/dev/manual-test.md
git commit -m "docs: add manual verification checklist"
```

---

## Self-Review

- Spec coverage: origin/destination selection is covered in Tasks 7 and 8; recursive scan in Task 3; SQLite in Tasks 2 and 4; backend abstraction in Task 6; review UI in Task 9; `.txt` export in Task 5; manual verification in Task 10.
- Placeholder scan: no `TBD`, `TODO`, or undefined future-only implementation steps remain.
- Type consistency: frontend `ScanResponse` uses camelCase and matches Tauri serialization; Rust `JobStatus` values match planned database strings; export precedence matches the design.

