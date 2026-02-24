use mdit::ui::tab_bar::{tab_label, path_label};
use std::path::Path;

#[test]
fn tab_label_untitled_clean() {
    assert_eq!(tab_label(None, false), "Untitled");
}
#[test]
fn tab_label_untitled_dirty() {
    assert_eq!(tab_label(None, true), "• Untitled");
}
#[test]
fn tab_label_named_clean() {
    assert_eq!(tab_label(Some(Path::new("/a/notes.md")), false), "notes.md");
}
#[test]
fn tab_label_named_dirty() {
    assert_eq!(tab_label(Some(Path::new("/a/notes.md")), true), "• notes.md");
}
#[test]
fn path_label_untitled() {
    assert_eq!(path_label(None), "Untitled — not saved");
}
#[test]
fn path_label_with_url() {
    assert_eq!(
        path_label(Some(Path::new("/Users/witt/notes.md"))),
        "/Users/witt/notes.md"
    );
}
