use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "wav", "m4a", "flac", "ogg", "opus", "aac", "wma", "mp4", "mkv", "mov", "avi", "webm",
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
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
        .unwrap_or(false)
}

pub fn scan_media(source_root: &Path) -> Result<Vec<DiscoveredMedia>> {
    let source_root = dunce::canonicalize(source_root)
        .with_context(|| format!("source root does not exist: {}", source_root.display()))?;

    let mut discovered = Vec::new();

    for entry in WalkDir::new(&source_root).follow_links(false) {
        let Ok(entry) = entry else {
            continue;
        };

        if !entry.file_type().is_file() || !is_supported_media(entry.path()) {
            continue;
        }

        let Ok(absolute_path) = dunce::canonicalize(entry.path()) else {
            continue;
        };
        let relative_path = absolute_path
            .strip_prefix(&source_root)
            .with_context(|| {
                format!(
                    "failed to build relative path for {} from {}",
                    absolute_path.display(),
                    source_root.display()
                )
            })?
            .to_path_buf();
        let Ok(metadata) = absolute_path.metadata() else {
            continue;
        };
        let Ok(modified_at) = metadata.modified() else {
            continue;
        };

        discovered.push(DiscoveredMedia {
            source_root: source_root.clone(),
            absolute_path,
            relative_path,
            file_name: entry.file_name().to_string_lossy().into_owned(),
            extension: entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase(),
            size_bytes: metadata.len(),
            modified_at: DateTime::<Utc>::from(modified_at),
        });
    }

    discovered.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    Ok(discovered)
}
