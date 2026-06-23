use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

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

    let segments: Vec<&str> = edited_segments
        .iter()
        .map(|segment| segment.trim())
        .filter(|segment| !segment.is_empty())
        .collect();

    if !segments.is_empty() {
        return segments.join("\n");
    }

    raw_text.to_string()
}

pub fn write_txt_export(destination: &Path, text: &str) -> Result<()> {
    if let Some(parent) = destination.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create export destination directory {}",
                parent.display()
            )
        })?;
    }

    let temp_path = temporary_export_path(destination);
    {
        let mut file = File::create(&temp_path).with_context(|| {
            format!(
                "failed to create temporary export file {}",
                temp_path.display()
            )
        })?;
        file.write_all(text.as_bytes()).with_context(|| {
            format!(
                "failed to write temporary export file {}",
                temp_path.display()
            )
        })?;
        file.sync_all().with_context(|| {
            format!(
                "failed to flush temporary export file {}",
                temp_path.display()
            )
        })?;
    }

    if destination.exists() {
        fs::remove_file(destination).with_context(|| {
            format!(
                "failed to replace existing export {}",
                destination.display()
            )
        })?;
    }

    fs::rename(&temp_path, destination).with_context(|| {
        let _ = fs::remove_file(&temp_path);
        format!(
            "failed to move temporary export {} to {}",
            temp_path.display(),
            destination.display()
        )
    })
}

fn temporary_export_path(destination: &Path) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("export.txt");

    destination.with_file_name(format!(".{file_name}.{suffix}.tmp"))
}
