use std::path::{Path, PathBuf};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generates a unique file path for a new image asset.
///
/// The file is placed in a `<document-stem>-assets/` directory that lives
/// next to the document.  The directory is **not** created by this function —
/// callers are responsible for `fs::create_dir_all` before writing.
///
/// # Example
/// ```
/// // doc: /tmp/notes.md, ext: "png"
/// // → /tmp/notes-assets/<uuid>.png
/// ```
pub fn generate_image_path(doc_path: &Path, extension: &str) -> PathBuf {
    let stem = doc_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let dir = doc_path.parent().unwrap_or(Path::new("."));
    let assets_dir = dir.join(format!("{}-assets", stem));
    assets_dir.join(format!("{}.{}", Uuid::new_v4(), extension))
}

/// Reads an image from the system clipboard and saves it to the assets
/// directory next to `doc_path`.
///
/// Returns the **relative** Markdown image path on success, e.g.
/// `notes-assets/<uuid>.png`, ready to embed as `![](notes-assets/…)`.
///
/// # TODO
/// Implementation requires `NSPasteboard` (AppKit, main-thread only).
/// Wire up via the `paste:` action override in `text_view.rs`.
#[allow(dead_code)]
pub fn save_image_from_clipboard(doc_path: &Path) -> Option<String> {
    let _ = doc_path;
    todo!("NSPasteboard integration — see text_view.rs paste: override")
}