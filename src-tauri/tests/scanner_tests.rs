use std::fs;
use std::path::PathBuf;

use transcrever::scanner::{is_supported_media, scan_media};

#[test]
fn scans_supported_media_recursively_and_ignores_other_files() {
    let temp = tempfile::tempdir().expect("temp dir");
    let nested = temp.path().join("nested");
    fs::create_dir(&nested).expect("nested dir");

    fs::write(temp.path().join("root.mp3"), b"root audio").expect("root media");
    fs::write(nested.join("UPPER.WAV"), b"nested audio").expect("nested media");
    fs::write(temp.path().join("notes.txt"), b"not media").expect("text file");

    let discovered = scan_media(temp.path()).expect("scan should succeed");

    let names: Vec<String> = discovered
        .iter()
        .map(|media| media.file_name.clone())
        .collect();
    assert_eq!(
        names,
        vec!["UPPER.WAV".to_string(), "root.mp3".to_string()]
    );

    let relative_paths: Vec<PathBuf> = discovered
        .iter()
        .map(|media| media.relative_path.clone())
        .collect();
    assert_eq!(
        relative_paths,
        vec![
            PathBuf::from("nested").join("UPPER.WAV"),
            PathBuf::from("root.mp3"),
        ]
    );

    for media in discovered {
        assert_eq!(
            media.source_root,
            dunce::canonicalize(temp.path()).expect("canonical root")
        );
        assert!(media.absolute_path.is_absolute());
        assert!(media.size_bytes > 0);
    }
}

#[test]
fn rejects_unsupported_extension_and_accepts_supported_wav() {
    assert!(!is_supported_media(&PathBuf::from("recording.txt")));
    assert!(is_supported_media(&PathBuf::from("recording.wav")));
}
