CREATE TABLE media_files (
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

CREATE TABLE transcription_profiles (
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

CREATE TABLE transcription_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_file_id INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'processing', 'completed', 'error', 'reviewed', 'exported')),
    profile_id INTEGER,
    progress REAL NOT NULL DEFAULT 0,
    error_message TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    FOREIGN KEY(media_file_id) REFERENCES media_files(id),
    FOREIGN KEY(profile_id) REFERENCES transcription_profiles(id)
);

CREATE TABLE transcriptions (
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

CREATE TABLE transcription_segments (
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

CREATE TABLE exports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transcription_id INTEGER NOT NULL,
    export_path TEXT NOT NULL,
    format TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(transcription_id) REFERENCES transcriptions(id)
);
