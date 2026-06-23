use std::fs;

use transcrever::export::{choose_export_text, write_txt_export};

#[test]
fn prefers_edited_text_over_segments_and_raw() {
    let segments = vec!["segment one".to_string(), "segment two".to_string()];

    let chosen = choose_export_text(Some("  reviewed text  "), &segments, "raw text");

    assert_eq!(chosen, "  reviewed text  ");
}

#[test]
fn falls_back_to_segments_before_raw() {
    let segments = vec![
        " first segment ".to_string(),
        "   ".to_string(),
        "second segment".to_string(),
    ];

    let chosen = choose_export_text(Some("   "), &segments, "raw text");

    assert_eq!(chosen, "first segment\nsecond segment");
}

#[test]
fn writes_txt_export_to_nested_temp_folder() {
    let temp = tempfile::tempdir().expect("temp dir");
    let destination = temp.path().join("nested").join("exports").join("review.txt");

    write_txt_export(&destination, "reviewed text").expect("export should write");

    let content = fs::read_to_string(destination).expect("export content");
    assert_eq!(content, "reviewed text");
}

#[test]
fn falls_back_to_raw_text_when_edits_are_empty() {
    let segments = vec!["   ".to_string()];

    let chosen = choose_export_text(Some("   "), &segments, "raw text");

    assert_eq!(chosen, "raw text");
}

#[test]
fn writes_filename_only_export_in_current_directory() {
    let temp = tempfile::tempdir().expect("temp dir");
    let original = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(temp.path()).expect("change current dir");

    let result = write_txt_export(std::path::Path::new("review.txt"), "conteudo acentuado");
    let content = fs::read_to_string(temp.path().join("review.txt")).expect("export content");

    std::env::set_current_dir(original).expect("restore current dir");

    result.expect("filename-only export should write");
    assert_eq!(content, "conteudo acentuado");
}

#[test]
fn overwrites_existing_export() {
    let temp = tempfile::tempdir().expect("temp dir");
    let destination = temp.path().join("review.txt");
    fs::write(&destination, "old").expect("seed file");

    write_txt_export(&destination, "novo texto").expect("export should overwrite");

    let content = fs::read_to_string(destination).expect("export content");
    assert_eq!(content, "novo texto");
}
