//! Pure string-transformation helpers for sidebar formatting actions.
//!
//! All functions are free of AppKit dependencies and operate on plain `&str`,
//! making them easy to unit-test.

// ---------------------------------------------------------------------------
// Block-format helpers
// ---------------------------------------------------------------------------

/// Known block-level prefixes, longest first so `### ` is matched before `# `.
const BLOCK_PREFIXES: &[&str] = &["### ", "## ", "# ", "> "];

/// Detect which block-level prefix (if any) a line starts with.
pub fn detect_block_prefix(line: &str) -> Option<&'static str> {
    BLOCK_PREFIXES.iter().copied().find(|p| line.starts_with(p))
}

/// Set the block format of a line.
///
/// * Same prefix as current → **toggle off** (back to plain text).
/// * Different prefix → **switch** (strip old, apply new).
/// * No prefix and `desired` is non-empty → **apply**.
/// * `desired` is `""` → strip any prefix (Normal button).
pub fn set_block_format(line: &str, desired: &str) -> String {
    let current = detect_block_prefix(line);
    let content = match current {
        Some(p) => &line[p.len()..],
        None => line,
    };

    // Toggle off: line already has the desired prefix.
    if let Some(cur) = current {
        if cur == desired {
            return content.to_string();
        }
    }

    // Apply new prefix (empty = Normal → just return content).
    if desired.is_empty() {
        content.to_string()
    } else {
        format!("{}{}", desired, content)
    }
}
