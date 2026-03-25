//! Pure string-transformation helpers for sidebar formatting actions.
//!
//! All functions are free of AppKit dependencies and operate on plain `&str`,
//! making them easy to unit-test.

// ---------------------------------------------------------------------------
// Block-format helpers
// ---------------------------------------------------------------------------

/// Known block-level prefixes, longest first so `### ` is matched before `# `.
const BLOCK_PREFIXES: &[&str] = &["- [ ] ", "- [x] ", "### ", "## ", "# ", "1. ", "> ", "- "];

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
const KNOWN_MARKERS: &[&str] = &["**", "__", "~~", "==", "`", "_", "~", "^"];

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

// ---------------------------------------------------------------------------
// Inline toggle (pure computation)
// ---------------------------------------------------------------------------

/// Result of computing an inline-marker toggle.
///
/// The caller is responsible for applying this to the text view:
/// - Replace the range `[selection_start - consumed_before,
///   selection_start - consumed_before + replace_length]` with `replacement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineToggleResult {
    /// The text that should replace the affected range.
    pub replacement: String,
    /// Number of characters consumed *before* the selection start
    /// (the surrounding markers that were stripped).
    pub consumed_before: usize,
    /// Number of characters consumed *after* the selection end.
    pub consumed_after: usize,
}

/// Compute the result of toggling `marker` around a selection.
///
/// `selected` — the currently selected text.
/// `before`   — a few characters immediately before the selection.
/// `after`    — a few characters immediately after the selection.
/// `marker`   — the symmetric marker to toggle (`"**"`, `"_"`, `` "`" ``, `"~~"`).
///
/// Three cases:
/// 1. Marker found in surrounding context → remove it (may keep other layers).
/// 2. Marker found inside the selection (hidden-marker case) → remove it.
/// 3. Marker absent → wrap the selection.
pub fn compute_inline_toggle(
    selected: &str,
    before: &str,
    after: &str,
    marker: &str,
) -> InlineToggleResult {
    let (layers, consumed_before, consumed_after) =
        find_surrounding_markers(before, after);

    if layers.contains(&marker) {
        // Case 1: marker surrounds the selection — remove it, keep other layers.
        let new_layers = toggle_marker_in_layers(&layers, marker);
        let replacement = wrap_with_layers(selected, &new_layers);
        InlineToggleResult { replacement, consumed_before, consumed_after }
    } else {
        // Check if markers are INSIDE the selection (hidden-marker case).
        let (inner_layers, inner_content) = peel_inline_markers(selected);
        if inner_layers.contains(&marker) {
            // Case 2: marker inside selection — remove it.
            let new_layers = toggle_marker_in_layers(&inner_layers, marker);
            let replacement = wrap_with_layers(inner_content, &new_layers);
            InlineToggleResult { replacement, consumed_before: 0, consumed_after: 0 }
        } else {
            // Case 3: marker absent — wrap the selection.
            // Trim whitespace so markers are adjacent to content
            // (CommonMark requires no space between marker and text).
            let content = selected.trim();
            let replacement = if content.is_empty() {
                format!("{}{}{}", marker, selected, marker)
            } else {
                let leading = &selected[..selected.len() - selected.trim_start().len()];
                let trailing = &selected[selected.trim_end().len()..];
                format!("{}{}{}{}{}", leading, marker, content, marker, trailing)
            };
            InlineToggleResult { replacement, consumed_before: 0, consumed_after: 0 }
        }
    }
}

/// Compute the text for a link wrap: `prefix + selected + suffix`.
pub fn compute_link_wrap(selected: &str, prefix: &str, suffix: &str) -> String {
    format!("{}{}{}", prefix, selected, suffix)
}

/// Compute the text for a fenced code block wrap.
///
/// If `selected` is empty, produces an empty fence with a blank line.
/// Otherwise wraps the selection in triple-backtick fences.
pub fn compute_code_block_wrap(selected: &str) -> String {
    let fence = "```";
    if selected.is_empty() {
        format!("{}\n\n{}", fence, fence)
    } else {
        format!("{}\n{}\n{}", fence, selected, fence)
    }
}
