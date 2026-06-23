use std::path::Path;

use serde::{Deserialize, Serialize};

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
    fn transcribe(
        &self,
        media_path: &Path,
        profile: &TranscriptionProfile,
    ) -> anyhow::Result<BackendTranscription>;
}
