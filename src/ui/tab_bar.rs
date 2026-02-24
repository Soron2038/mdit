//! Tab bar for the mdit editor window.
//!
//! Pure-Rust helpers at the top; AppKit view types further below.

use std::path::Path;

// ---------------------------------------------------------------------------
// Pure-Rust helpers (unit-testable, no AppKit)
// ---------------------------------------------------------------------------

/// Label text for a tab button.
/// Prefixes "• " when `is_dirty` is true.
pub fn tab_label(url: Option<&Path>, is_dirty: bool) -> String {
    let name = url
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string());
    if is_dirty { format!("• {}", name) } else { name }
}

/// Full path string for the path bar.
pub fn path_label(url: Option<&Path>) -> String {
    url.map(|p| p.display().to_string())
        .unwrap_or_else(|| "Untitled — not saved".to_string())
}
