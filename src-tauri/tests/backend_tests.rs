use std::path::PathBuf;

use transcrever::backend::whisper_cpp::WhisperCppBackend;
use transcrever::backend::TranscriptionProfile;

fn profile() -> TranscriptionProfile {
    TranscriptionProfile {
        model_path: "models/base.bin".to_string(),
        device: "cpu".to_string(),
        precision: "auto".to_string(),
        threads: 4,
        language: None,
        task: "transcribe".to_string(),
        advanced_json: serde_json::json!({}),
    }
}

#[test]
fn whisper_cpp_args_include_required_json_output_flags() {
    let backend = WhisperCppBackend::new(PathBuf::from("whisper-cli"));
    let profile = profile();

    let args = backend
        .build_args(
            PathBuf::from("audio.wav").as_path(),
            &profile,
            PathBuf::from("out").as_path(),
        )
        .expect("args should build");
    let args: Vec<String> = args
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();

    assert_eq!(
        args,
        vec![
            "-m",
            "models/base.bin",
            "-f",
            "audio.wav",
            "-t",
            "4",
            "-oj",
            "-of",
            "out",
            "-np",
            "-ng",
        ]
    );
}

#[test]
fn whisper_cpp_args_include_language_and_translate_when_requested() {
    let backend = WhisperCppBackend::new(PathBuf::from("whisper-cli"));
    let mut profile = profile();
    profile.language = Some("pt".to_string());
    profile.task = "translate".to_string();

    let args = backend
        .build_args(
            PathBuf::from("audio.wav").as_path(),
            &profile,
            PathBuf::from("out").as_path(),
        )
        .expect("args should build");
    let args: Vec<String> = args
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();

    assert!(args.windows(2).any(|pair| pair == ["-l", "pt"]));
    assert!(args.iter().any(|arg| arg == "-tr"));
}

#[test]
fn whisper_cpp_args_reject_unsupported_precision() {
    let backend = WhisperCppBackend::new(PathBuf::from("whisper-cli"));
    let mut profile = profile();
    profile.precision = "float32".to_string();

    let error = backend
        .build_args(
            PathBuf::from("audio.wav").as_path(),
            &profile,
            PathBuf::from("out").as_path(),
        )
        .expect_err("unsupported precision should fail");

    assert!(error.to_string().contains("model file"));
}
