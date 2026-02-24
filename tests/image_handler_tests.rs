use mdit::editor::image_handler::generate_image_path;
use std::path::Path;

#[test]
fn generates_path_next_to_document() {
    let doc_path = Path::new("/tmp/test.md");
    let result = generate_image_path(doc_path, "png");
    assert!(
        result.starts_with("/tmp/test-assets"),
        "image should be in a sibling assets dir: {:?}",
        result
    );
    assert_eq!(
        result.extension().and_then(|e| e.to_str()),
        Some("png"),
        "extension should match supplied type"
    );
}

#[test]
fn uses_uuid_as_filename() {
    let doc_path = Path::new("/tmp/notes.md");
    let r1 = generate_image_path(doc_path, "png");
    let r2 = generate_image_path(doc_path, "png");
    assert_ne!(r1, r2, "each call must produce a unique filename");
}

#[test]
fn assets_dir_named_after_document() {
    let doc_path = Path::new("/home/user/my-notes.md");
    let result = generate_image_path(doc_path, "jpg");
    let assets_dir = result.parent().expect("result must have a parent dir");
    assert_eq!(
        assets_dir.file_name().and_then(|n| n.to_str()),
        Some("my-notes-assets"),
        "assets dir should be named <stem>-assets"
    );
}
