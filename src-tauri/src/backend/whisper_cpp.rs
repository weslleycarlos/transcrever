use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context};
use serde_json::Value;

use super::{BackendSegment, BackendTranscription, TranscriptionBackend, TranscriptionProfile};

pub struct WhisperCppBackend {
    pub executable_path: PathBuf,
}

impl WhisperCppBackend {
    pub fn new(executable_path: PathBuf) -> Self {
        Self { executable_path }
    }

    pub fn build_args(
        &self,
        media_path: &Path,
        profile: &TranscriptionProfile,
        output_base: &Path,
    ) -> anyhow::Result<Vec<OsString>> {
        validate_profile(profile)?;

        let mut args = vec![
            OsString::from("-m"),
            OsString::from(&profile.model_path),
            OsString::from("-f"),
            media_path.as_os_str().to_os_string(),
            OsString::from("-t"),
            OsString::from(profile.threads.to_string()),
            OsString::from("-oj"),
            OsString::from("-of"),
            output_base.as_os_str().to_os_string(),
            OsString::from("-np"),
        ];

        if let Some(language) = profile.language.as_deref().map(str::trim) {
            if !language.is_empty() {
                args.push(OsString::from("-l"));
                args.push(OsString::from(language));
            }
        }

        if profile.task == "translate" {
            args.push(OsString::from("-tr"));
        }

        if let Some(device_index) = gpu_device_index(&profile.device)? {
            args.push(OsString::from("-dev"));
            args.push(OsString::from(device_index.to_string()));
        } else if profile.device.eq_ignore_ascii_case("cpu") {
            args.push(OsString::from("-ng"));
        }

        Ok(args)
    }
}

impl TranscriptionBackend for WhisperCppBackend {
    fn transcribe(
        &self,
        media_path: &Path,
        profile: &TranscriptionProfile,
    ) -> anyhow::Result<BackendTranscription> {
        let output_base = temporary_output_base();
        let output_json = output_base.with_extension("json");
        let args = self.build_args(media_path, profile, &output_base)?;

        let output = Command::new(&self.executable_path)
            .args(args)
            .output()
            .with_context(|| {
                format!(
                    "failed to execute whisper.cpp binary at {}",
                    self.executable_path.display()
                )
            })?;

        if !output.status.success() {
            let diagnostic = process_diagnostic(&output.stderr, &output.stdout);
            return Err(anyhow!(
                "whisper.cpp failed with status {} for media {} using model {}: {}",
                output.status,
                media_path.display(),
                profile.model_path,
                diagnostic
            ));
        }

        let json = match fs::read_to_string(&output_json) {
            Ok(json) => json,
            Err(_) => {
                // Process exited 0 but wrote no JSON: usually an unreadable or
                // empty audio (no decodable samples). Surface its own output.
                let diagnostic = process_diagnostic(&output.stderr, &output.stdout);
                return Err(anyhow!(
                    "whisper.cpp nao gerou transcricao para {} (provavel audio vazio ou ilegivel): {}",
                    media_path.display(),
                    diagnostic
                ));
            }
        };
        let _ = fs::remove_file(&output_json);

        parse_whisper_json(&json)
    }
}

fn validate_profile(profile: &TranscriptionProfile) -> anyhow::Result<()> {
    if profile.threads == 0 {
        bail!("threads must be greater than zero");
    }
    if profile.task != "transcribe" && profile.task != "translate" {
        bail!("unsupported whisper.cpp task '{}'", profile.task);
    }
    if profile.precision != "auto" && profile.precision != "model" {
        bail!(
            "precision '{}' is controlled by the whisper.cpp model file; use 'auto' or 'model'",
            profile.precision
        );
    }
    if !profile
        .advanced_json
        .as_object()
        .is_some_and(|object| object.is_empty())
    {
        bail!("advanced_json options are not supported by the whisper.cpp backend yet");
    }
    let _ = gpu_device_index(&profile.device)?;
    Ok(())
}

fn gpu_device_index(device: &str) -> anyhow::Result<Option<u32>> {
    let normalized = device.trim().to_ascii_lowercase();
    if normalized == "cpu" || normalized == "gpu" || normalized == "auto" {
        return Ok(None);
    }
    if let Some(index) = normalized.strip_prefix("gpu:") {
        return index
            .parse::<u32>()
            .map(Some)
            .with_context(|| format!("invalid GPU device selector '{device}'"));
    }
    bail!("unsupported device '{device}'; use cpu, gpu, auto, or gpu:<index>")
}

pub(crate) fn parse_whisper_json(json: &str) -> anyhow::Result<BackendTranscription> {
    let value: Value = serde_json::from_str(json).context("failed to parse whisper.cpp JSON")?;
    let items = value
        .get("transcription")
        .or_else(|| value.get("segments"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("whisper.cpp JSON did not contain transcription segments"))?;

    let mut segments = Vec::with_capacity(items.len());
    for item in items {
        let text = item
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let start_ms = offset_ms(item, "from")
            .or_else(|| timestamp_ms(item, "from", "start"))
            .or_else(|| timestamp_ms(item, "start", "start"))
            .unwrap_or_default();
        let end_ms = offset_ms(item, "to")
            .or_else(|| timestamp_ms(item, "to", "end"))
            .or_else(|| timestamp_ms(item, "end", "end"))
            .unwrap_or(start_ms);
        let confidence = item
            .get("confidence")
            .or_else(|| item.get("probability"))
            .and_then(Value::as_f64)
            .map(|value| value as f32);

        segments.push(BackendSegment {
            start_ms,
            end_ms,
            text,
            confidence,
        });
    }

    let raw_text = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(BackendTranscription { raw_text, segments })
}

/// whisper.cpp `-oj` output includes integer-millisecond offsets under
/// `offsets: { from, to }`, which are the most reliable timing source.
fn offset_ms(item: &Value, key: &str) -> Option<i64> {
    item.get("offsets")
        .and_then(|offsets| offsets.get(key))
        .and_then(Value::as_i64)
}

fn timestamp_ms(item: &Value, object_key: &str, scalar_key: &str) -> Option<i64> {
    if let Some(value) = item.get(scalar_key).and_then(Value::as_f64) {
        return Some((value * 1000.0).round() as i64);
    }

    item.get("timestamps")
        .and_then(|timestamps| timestamps.get(object_key))
        .and_then(Value::as_str)
        .and_then(parse_timestamp_ms)
}

fn parse_timestamp_ms(timestamp: &str) -> Option<i64> {
    let mut parts = timestamp.split(':');
    let hours = parts.next()?.parse::<i64>().ok()?;
    let minutes = parts.next()?.parse::<i64>().ok()?;
    // whisper.cpp uses a comma as the decimal separator (e.g. "02,500").
    let seconds = parts.next()?.replace(',', ".").parse::<f64>().ok()?;
    Some((((hours * 60 + minutes) * 60) as f64 * 1000.0 + seconds * 1000.0).round() as i64)
}

fn temporary_output_base() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("transcrever-whisper-{suffix}"))
}

fn process_diagnostic(stderr: &[u8], stdout: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let text = if stderr.is_empty() { stdout } else { stderr };
    if text.is_empty() {
        "process produced no diagnostic output".to_string()
    } else if text.chars().count() > 1000 {
        format!("{}...", text.chars().take(1000).collect::<String>())
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_whisper_json_transcription_segments() {
        let json = r#"
        {
          "transcription": [
            {
              "timestamps": { "from": "00:00:01.000", "to": "00:00:02.500" },
              "text": " primeiro "
            },
            {
              "timestamps": { "from": "00:00:02.500", "to": "00:00:04.000" },
              "text": "segundo",
              "confidence": 0.75
            }
          ]
        }
        "#;

        let transcription = parse_whisper_json(json).expect("json should parse");

        assert_eq!(transcription.raw_text, "primeiro\nsegundo");
        assert_eq!(transcription.segments.len(), 2);
        assert_eq!(transcription.segments[0].start_ms, 1000);
        assert_eq!(transcription.segments[0].end_ms, 2500);
        assert_eq!(transcription.segments[1].confidence, Some(0.75));
    }

    #[test]
    fn parses_offsets_and_comma_timestamps() {
        // Real whisper.cpp output: integer-ms offsets + comma-decimal timestamps.
        let json = r#"
        {
          "transcription": [
            {
              "timestamps": { "from": "00:00:00,000", "to": "00:00:02,500" },
              "offsets": { "from": 0, "to": 2500 },
              "text": " ola "
            },
            {
              "timestamps": { "from": "00:00:02,500", "to": "00:01:05,250" },
              "text": "mundo"
            }
          ]
        }
        "#;

        let transcription = parse_whisper_json(json).expect("json should parse");

        assert_eq!(transcription.segments[0].start_ms, 0);
        assert_eq!(transcription.segments[0].end_ms, 2500);
        // Second segment has no offsets, falls back to comma-decimal timestamps.
        assert_eq!(transcription.segments[1].start_ms, 2500);
        assert_eq!(transcription.segments[1].end_ms, 65_250);
    }

    #[test]
    fn process_diagnostic_falls_back_to_stdout() {
        let diagnostic = process_diagnostic(b"", b"stdout message");

        assert_eq!(diagnostic, "stdout message");
    }

    #[test]
    fn process_diagnostic_truncates_on_utf8_boundary() {
        let long = "ação ".repeat(400);
        let diagnostic = process_diagnostic(long.as_bytes(), b"");

        assert!(diagnostic.ends_with("..."));
        assert!(diagnostic.is_char_boundary(diagnostic.len()));
    }
}
