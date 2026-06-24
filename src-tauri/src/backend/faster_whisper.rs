use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context};
use serde::Deserialize;

use super::{BackendSegment, BackendTranscription, TranscriptionBackend, TranscriptionProfile};

pub struct FasterWhisperBackend {
    /// Path to a bundled Python helper script inside the Tauri resource directory.
    /// Set by main.rs via `Self::new`.
    script_path: PathBuf,
}

impl FasterWhisperBackend {
    pub fn new(script_path: PathBuf) -> Self {
        Self { script_path }
    }

    fn build_args(
        &self,
        media_path: &Path,
        profile: &TranscriptionProfile,
    ) -> anyhow::Result<Vec<String>> {
        validate_profile(profile)?;

        let compute_type = precision_to_compute_type(&profile.precision)?;

        let mut args = vec![
            self.script_path.to_string_lossy().to_string(),
            "--model".to_string(),
            profile.model_path.clone(),
            "--audio".to_string(),
            media_path.to_string_lossy().to_string(),
            "--device".to_string(),
            profile.device.clone(),
            "--compute-type".to_string(),
            compute_type.to_string(),
            "--threads".to_string(),
            profile.threads.to_string(),
            "--task".to_string(),
            profile.task.clone(),
        ];

        if let Some(language) = profile.language.as_deref().map(str::trim) {
            if !language.is_empty() {
                args.push("--language".to_string());
                args.push(language.to_string());
            }
        }

        Ok(args)
    }

    fn find_python() -> anyhow::Result<String> {
        for candidate in &["python", "python3", "py"] {
            if Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok()
            {
                return Ok(candidate.to_string());
            }
        }
        bail!("Python not found on PATH. Install Python 3.10+ and the faster-whisper package.");
    }
}

impl TranscriptionBackend for FasterWhisperBackend {
    fn transcribe(
        &self,
        media_path: &Path,
        profile: &TranscriptionProfile,
    ) -> anyhow::Result<BackendTranscription> {
        let python = Self::find_python()?;
        let args = self.build_args(media_path, profile)?;

        let output = Command::new(&python)
            .args(&args)
            // Avoid writing __pycache__ next to the script: it clutters the tree
            // and, under `tauri dev`, the file watcher would restart the app.
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .output()
            .with_context(|| format!("failed to run {python} with faster-whisper script"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let diagnostic = if stderr.is_empty() { stdout } else { stderr };
            return Err(anyhow!(
                "faster-whisper failed for media {} using model {}: {}",
                media_path.display(),
                profile.model_path,
                diagnostic
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        parse_faster_whisper_json(&stdout)
    }
}

fn validate_profile(profile: &TranscriptionProfile) -> anyhow::Result<()> {
    if profile.threads == 0 {
        bail!("threads must be greater than zero");
    }
    if profile.task != "transcribe" && profile.task != "translate" {
        bail!("unsupported faster-whisper task '{}'", profile.task);
    }
    precision_to_compute_type(&profile.precision)?;
    if !profile.device.eq_ignore_ascii_case("cpu")
        && !profile.device.eq_ignore_ascii_case("cuda")
        && !profile.device.eq_ignore_ascii_case("auto")
    {
        bail!(
            "unsupported device '{}'; use cpu, cuda, or auto",
            profile.device
        );
    }
    if !profile
        .advanced_json
        .as_object()
        .is_some_and(|object| object.is_empty())
    {
        bail!("advanced_json options are not supported by the faster-whisper backend yet");
    }
    Ok(())
}

fn precision_to_compute_type(precision: &str) -> anyhow::Result<&'static str> {
    match precision.to_ascii_lowercase().as_str() {
        "fp32" | "float32" => Ok("float32"),
        "fp16" | "float16" => Ok("float16"),
        "int8" => Ok("int8"),
        "int8_float16" => Ok("int8_float16"),
        "int8_bfloat16" => Ok("int8_bfloat16"),
        "auto" | "default" => Ok("auto"),
        other => bail!(
            "unsupported precision '{}' for faster-whisper; use auto, float32, float16, int8, int8_float16, or int8_bfloat16",
            other
        ),
    }
}

#[derive(Debug, Deserialize)]
struct FasterWhisperSegment {
    #[serde(default)]
    start_ms: i64,
    #[serde(default)]
    end_ms: i64,
    text: String,
    #[serde(default)]
    confidence: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct FasterWhisperOutput {
    raw_text: String,
    segments: Vec<FasterWhisperSegment>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    device_used: Option<String>,
}

fn parse_faster_whisper_json(json: &str) -> anyhow::Result<BackendTranscription> {
    let output: FasterWhisperOutput =
        serde_json::from_str(json).context("failed to parse faster-whisper JSON output")?;

    if let Some(error) = output.error {
        return Err(anyhow!("faster-whisper error: {}", error));
    }

    let segments = output
        .segments
        .into_iter()
        .map(|seg| BackendSegment {
            start_ms: seg.start_ms,
            end_ms: seg.end_ms,
            text: seg.text,
            confidence: seg.confidence,
        })
        .collect();

    Ok(BackendTranscription {
        raw_text: output.raw_text,
        segments,
        device_used: output.device_used,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_faster_whisper_json_output() {
        let json = r#"
        {
          "raw_text": "primeiro\nsegundo",
          "segments": [
            {
              "start_ms": 1000,
              "end_ms": 2500,
              "text": "primeiro",
              "confidence": -0.23
            },
            {
              "start_ms": 2500,
              "end_ms": 4000,
              "text": "segundo",
              "confidence": -0.17
            }
          ],
          "detected_language": "pt",
          "duration_ms": 4000,
          "elapsed_ms": 1200
        }
        "#;

        let result = parse_faster_whisper_json(json).expect("should parse");
        assert_eq!(result.raw_text, "primeiro\nsegundo");
        assert_eq!(result.segments.len(), 2);
        assert_eq!(result.segments[0].text, "primeiro");
        assert_eq!(result.segments[0].start_ms, 1000);
        assert_eq!(result.segments[0].end_ms, 2500);
    }

    #[test]
    fn parses_error_response() {
        let json = r#"{"error": "model not found", "segments": [], "raw_text": ""}"#;
        let err = parse_faster_whisper_json(json).unwrap_err();
        assert!(err.to_string().contains("model not found"));
    }
}
