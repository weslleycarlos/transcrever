CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    archived INTEGER NOT NULL DEFAULT 0,
    default_profile_id INTEGER REFERENCES transcription_profiles(id)
);

ALTER TABLE media_files ADD COLUMN project_id INTEGER REFERENCES projects(id);

-- Migra dados existentes: cada pasta de origem distinta vira um projeto.
INSERT INTO projects (name, created_at, archived)
SELECT DISTINCT source_root, datetime('now'), 0 FROM media_files;

UPDATE media_files
SET project_id = (SELECT id FROM projects WHERE projects.name = media_files.source_root);
