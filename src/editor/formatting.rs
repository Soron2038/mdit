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

// ---------------------------------------------------------------------------
// Inline-format helpers
// ---------------------------------------------------------------------------

/// Known symmetric inline markers, longest first to avoid partial matches.
const KNOWN_MARKERS: &[&str] = &["**", "~~", "`", "_"];

/// Scan for matching marker layers surrounding a selection.
///
/// `before` — text immediately before the selection (a few characters suffice).
/// `after`  — text immediately after the selection.
///
/// Returns `(layers, consumed_before, consumed_after)` where `layers` lists
/// matched marker pairs from outermost to innermost, and the consumed counts
/// indicate how many characters on each side belong to the markers.
pub fn find_surrounding_markers(before: &str, after: &str) -> (Vec<&'static str>, usize, usize) {
    let mut layers = Vec::new();
    let mut consumed_before: usize = 0;
    let mut consumed_after: usize = 0;
    let mut b = before;
    let mut a = after;

    'outer: loop {
        for marker in KNOWN_MARKERS {
            if b.ends_with(marker) && a.starts_with(marker) {
                layers.push(*marker);
                b = &b[..b.len() - marker.len()];
                a = &a[marker.len()..];
                consumed_before += marker.len();
                consumed_after += marker.len();
                continue 'outer;
            }
        }
        break;
    }

    (layers, consumed_before, consumed_after)
}

/// Toggle a marker in a layer list.
///
/// If present → remove it.  If absent → append it (innermost position).
pub fn toggle_marker_in_layers<'a>(layers: &[&'a str], marker: &'a str) -> Vec<&'a str> {
    if let Some(idx) = layers.iter().position(|m| *m == marker) {
        let mut new = layers.to_vec();
        new.remove(idx);
        new
    } else {
        let mut new = layers.to_vec();
        new.push(marker);
        new
    }
}

/// Peel matching marker pairs from both ends of a string.
///
/// Works like [`find_surrounding_markers`] but operates *inside* the string
/// rather than on surrounding context.  Returns `(layers, inner_content)`.
pub fn peel_inline_markers(text: &str) -> (Vec<&'static str>, &str) {
    let mut layers = Vec::new();
    let mut remaining = text;

    'outer: loop {
        for marker in KNOWN_MARKERS {
            if remaining.len() >= marker.len() * 2
                && remaining.starts_with(marker)
                && remaining.ends_with(marker)
            {
                layers.push(*marker);
                remaining = &remaining[marker.len()..remaining.len() - marker.len()];
                continue 'outer;
            }
        }
        break;
    }

    (layers, remaining)
}

/// Wrap `content` with the given marker layers (outermost first).
pub fn wrap_with_layers(content: &str, layers: &[&str]) -> String {
    let mut result = content.to_string();
    for marker in layers.iter().rev() {
        result = format!("{}{}{}", marker, result, marker);
    }
    result
}
