# Sidebar Toggle-Formatting Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace blind insert formatting actions with state-aware toggle/switch behavior so sidebar buttons never alter visible text content.

**Architecture:** Extract pure string-transformation functions into `src/editor/formatting.rs` (testable without AppKit). The `app.rs` action methods become thin wrappers that read text from NSTextView, call the pure functions, and apply the result. Block-format functions detect and swap line prefixes; inline-format functions detect, peel, and reconstruct marker layers around the selection.

**Tech Stack:** Rust, NSTextView (via objc2), existing test infrastructure (`cargo test`)

---

### Task 1: Block-format pure functions + tests

**Files:**
- Create: `src/editor/formatting.rs`
- Modify: `src/editor/mod.rs:1-8` (add `pub mod formatting;`)
- Create: `tests/formatting_tests.rs`

**Step 1: Write the failing tests**

In `tests/formatting_tests.rs`:

```rust
use mdit::editor::formatting::{detect_block_prefix, set_block_format};

// ── detect_block_prefix ──────────────────────────────────────────────────

#[test]
fn detect_no_prefix() {
    assert_eq!(detect_block_prefix("Hello world"), None);
}

#[test]
fn detect_h1() {
    assert_eq!(detect_block_prefix("# Hello"), Some("# "));
}

#[test]
fn detect_h2() {
    assert_eq!(detect_block_prefix("## Hello"), Some("## "));
}

#[test]
fn detect_h3() {
    assert_eq!(detect_block_prefix("### Hello"), Some("### "));
}

#[test]
fn detect_blockquote() {
    assert_eq!(detect_block_prefix("> Hello"), Some("> "));
}

#[test]
fn detect_h1_with_trailing_newline() {
    assert_eq!(detect_block_prefix("# Hello\n"), Some("# "));
}

// ── set_block_format ─────────────────────────────────────────────────────

#[test]
fn plain_to_h1() {
    assert_eq!(set_block_format("Hello", "# "), "# Hello");
}

#[test]
fn h1_to_h1_toggles_off() {
    assert_eq!(set_block_format("# Hello", "# "), "Hello");
}

#[test]
fn h1_to_h2_switches() {
    assert_eq!(set_block_format("# Hello", "## "), "## Hello");
}

#[test]
fn h2_to_h3_switches() {
    assert_eq!(set_block_format("## Hello", "### "), "### Hello");
}

#[test]
fn blockquote_to_h1_switches() {
    assert_eq!(set_block_format("> Hello", "# "), "# Hello");
}

#[test]
fn h1_to_normal() {
    assert_eq!(set_block_format("# Hello", ""), "Hello");
}

#[test]
fn normal_to_normal_noop() {
    assert_eq!(set_block_format("Hello", ""), "Hello");
}

#[test]
fn blockquote_toggles_off() {
    assert_eq!(set_block_format("> Hello", "> "), "Hello");
}

#[test]
fn preserves_trailing_newline() {
    assert_eq!(set_block_format("# Hello\n", "## "), "## Hello\n");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test formatting_tests 2>&1 | head -20`
Expected: compilation error — module `formatting` does not exist.

**Step 3: Write minimal implementation**

Add to `src/editor/mod.rs`:

```rust
pub mod formatting;
```

Create `src/editor/formatting.rs`:

```rust
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
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test formatting_tests -v`
Expected: all tests PASS.

**Step 5: Commit**

```bash
git add src/editor/formatting.rs src/editor/mod.rs tests/formatting_tests.rs
git commit -m "feat: add block-format toggle pure functions with tests"
```

---

### Task 2: Wire block-format actions in app.rs

**Files:**
- Modify: `src/app.rs:314-365` (action methods)
- Modify: `src/app.rs:749-811` (replace `prepend_line` + `strip_line_prefix` with new wrapper)

**Step 1: Add the `apply_block_format` wrapper to app.rs**

Replace the `prepend_line` function (lines 752-770) and `strip_line_prefix` function (lines 774-811) with a single wrapper. Place it where `prepend_line` used to be:

```rust
/// Apply a block-level format to the line containing the caret.
///
/// Uses the pure `set_block_format()` under the hood: same prefix toggles
/// off, different prefix switches, empty prefix strips (Normal).
fn apply_block_format(tv: &NSTextView, desired_prefix: &str) {
    let caret: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else {
        return;
    };
    let ns_str = storage.string();
    let point = NSRange { location: caret.location, length: 0 };
    let line_range: NSRange = ns_str.lineRangeForRange(point);
    let line_text = ns_str.substringWithRange(line_range).to_string();

    let new_line = mdit::editor::formatting::set_block_format(&line_text, desired_prefix);
    let ns = NSString::from_str(&new_line);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: line_range] }
}
```

**Step 2: Update the action methods to use `apply_block_format`**

Replace the action method bodies (lines 316-365):

```rust
#[unsafe(method(applyH1:))]
fn apply_h1(&self, _sender: &AnyObject) {
    if let Some(tv) = self.active_text_view() {
        apply_block_format(&tv, "# ");
    }
}

#[unsafe(method(applyH2:))]
fn apply_h2(&self, _sender: &AnyObject) {
    if let Some(tv) = self.active_text_view() {
        apply_block_format(&tv, "## ");
    }
}

#[unsafe(method(applyH3:))]
fn apply_h3(&self, _sender: &AnyObject) {
    if let Some(tv) = self.active_text_view() {
        apply_block_format(&tv, "### ");
    }
}

#[unsafe(method(applyNormal:))]
fn apply_normal(&self, _sender: &AnyObject) {
    if let Some(tv) = self.active_text_view() {
        apply_block_format(&tv, "");
    }
}

#[unsafe(method(applyBlockquote:))]
fn apply_blockquote(&self, _sender: &AnyObject) {
    if let Some(tv) = self.active_text_view() {
        apply_block_format(&tv, "> ");
    }
}
```

The `applyCodeBlock:` and `applyHRule:` methods stay unchanged.

**Step 3: Remove dead code**

Delete the `prepend_line` and `strip_line_prefix` functions entirely.

**Step 4: Build to verify**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles without errors or warnings.

**Step 5: Run all tests**

Run: `cargo test`
Expected: all tests pass (existing + new formatting tests).

**Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat: wire block-format toggle into sidebar actions"
```

---

### Task 3: Inline toggle pure functions + tests (single markers)

**Files:**
- Modify: `src/editor/formatting.rs`
- Modify: `tests/formatting_tests.rs`

**Step 1: Write failing tests for single-marker inline toggle**

Append to `tests/formatting_tests.rs`:

```rust
use mdit::editor::formatting::{find_surrounding_markers, toggle_marker_in_layers, wrap_with_layers};

// ── find_surrounding_markers ─────────────────────────────────────────────

#[test]
fn no_markers_around() {
    let (layers, pre, post) = find_surrounding_markers("hello ", " world", );
    assert!(layers.is_empty());
    assert_eq!(pre, 0);
    assert_eq!(post, 0);
}

#[test]
fn bold_markers_around() {
    let (layers, pre, post) = find_surrounding_markers("**", "**");
    assert_eq!(layers, vec!["**"]);
    assert_eq!(pre, 2);
    assert_eq!(post, 2);
}

#[test]
fn italic_markers_around() {
    let (layers, pre, post) = find_surrounding_markers("_", "_");
    assert_eq!(layers, vec!["_"]);
    assert_eq!(pre, 1);
    assert_eq!(post, 1);
}

#[test]
fn inline_code_markers_around() {
    let (layers, pre, post) = find_surrounding_markers("`", "`");
    assert_eq!(layers, vec!["`"]);
    assert_eq!(pre, 1);
    assert_eq!(post, 1);
}

#[test]
fn strikethrough_markers_around() {
    let (layers, pre, post) = find_surrounding_markers("~~", "~~");
    assert_eq!(layers, vec!["~~"]);
    assert_eq!(pre, 2);
    assert_eq!(post, 2);
}

#[test]
fn markers_with_preceding_text() {
    // "some text **" before selection, "** more" after
    let (layers, pre, post) = find_surrounding_markers("some text **", "** more");
    assert_eq!(layers, vec!["**"]);
    assert_eq!(pre, 2);
    assert_eq!(post, 2);
}

#[test]
fn unmatched_markers_ignored() {
    // ** on left but not on right
    let (layers, _, _) = find_surrounding_markers("**", " end");
    assert!(layers.is_empty());
}

// ── toggle_marker_in_layers ──────────────────────────────────────────────

#[test]
fn toggle_adds_missing_marker() {
    let result = toggle_marker_in_layers(&[], "**");
    assert_eq!(result, vec!["**"]);
}

#[test]
fn toggle_removes_present_marker() {
    let result = toggle_marker_in_layers(&["**"], "**");
    assert!(result.is_empty());
}

// ── wrap_with_layers ─────────────────────────────────────────────────────

#[test]
fn wrap_no_layers() {
    assert_eq!(wrap_with_layers("hello", &[]), "hello");
}

#[test]
fn wrap_one_layer() {
    assert_eq!(wrap_with_layers("hello", &["**"]), "**hello**");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test formatting_tests 2>&1 | head -20`
Expected: compilation error — functions not found.

**Step 3: Write implementation**

Append to `src/editor/formatting.rs`:

```rust
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

/// Wrap `content` with the given marker layers (outermost first).
pub fn wrap_with_layers(content: &str, layers: &[&str]) -> String {
    let mut result = content.to_string();
    for marker in layers.iter().rev() {
        result = format!("{}{}{}", marker, result, marker);
    }
    result
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test formatting_tests -v`
Expected: all tests PASS.

**Step 5: Commit**

```bash
git add src/editor/formatting.rs tests/formatting_tests.rs
git commit -m "feat: add inline marker detection and toggle pure functions"
```

---

### Task 4: Nested inline marker tests

**Files:**
- Modify: `tests/formatting_tests.rs`

**Step 1: Write tests for nested marker scenarios**

Append to `tests/formatting_tests.rs`:

```rust
// ── nested marker scenarios ──────────────────────────────────────────────

#[test]
fn find_bold_and_italic_nested() {
    // Text is **_hello_** — selection covers "hello"
    let (layers, pre, post) = find_surrounding_markers("**_", "_**");
    assert_eq!(layers, vec!["**", "_"]);
    assert_eq!(pre, 3);
    assert_eq!(post, 3);
}

#[test]
fn find_italic_and_bold_nested() {
    // Text is _**hello**_ — selection covers "hello"
    let (layers, pre, post) = find_surrounding_markers("_**", "**_");
    assert_eq!(layers, vec!["_", "**"]);
    assert_eq!(pre, 3);
    assert_eq!(post, 3);
}

#[test]
fn toggle_remove_inner_from_nested() {
    // layers = ["**", "_"], toggle "_" → ["**"]
    let result = toggle_marker_in_layers(&["**", "_"], "_");
    assert_eq!(result, vec!["**"]);
}

#[test]
fn toggle_remove_outer_from_nested() {
    // layers = ["**", "_"], toggle "**" → ["_"]
    let result = toggle_marker_in_layers(&["**", "_"], "**");
    assert_eq!(result, vec!["_"]);
}

#[test]
fn toggle_add_to_existing_layer() {
    // layers = ["**"], toggle "_" → ["**", "_"]
    let result = toggle_marker_in_layers(&["**"], "_");
    assert_eq!(result, vec!["**", "_"]);
}

#[test]
fn wrap_two_layers() {
    assert_eq!(wrap_with_layers("hello", &["**", "_"]), "**_hello_**");
}

#[test]
fn wrap_three_layers() {
    assert_eq!(wrap_with_layers("hello", &["**", "~~", "_"]), "**~~_hello_~~**");
}

// ── full round-trip scenarios ────────────────────────────────────────────

#[test]
fn roundtrip_add_bold_then_italic() {
    // Start: "hello"
    let text = wrap_with_layers("hello", &toggle_marker_in_layers(&[], "**"));
    assert_eq!(text, "**hello**");

    // Add italic
    let (layers, _, _) = find_surrounding_markers("**", "**");
    let new_layers = toggle_marker_in_layers(&layers, "_");
    let text2 = wrap_with_layers("hello", &new_layers);
    assert_eq!(text2, "**_hello_**");
}

#[test]
fn roundtrip_remove_italic_from_bold_italic() {
    // "**_hello_**" — remove italic
    let (layers, _, _) = find_surrounding_markers("**_", "_**");
    let new_layers = toggle_marker_in_layers(&layers, "_");
    let text = wrap_with_layers("hello", &new_layers);
    assert_eq!(text, "**hello**");
}

#[test]
fn roundtrip_remove_bold_from_bold_italic() {
    // "**_hello_**" — remove bold
    let (layers, _, _) = find_surrounding_markers("**_", "_**");
    let new_layers = toggle_marker_in_layers(&layers, "**");
    let text = wrap_with_layers("hello", &new_layers);
    assert_eq!(text, "_hello_");
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo test --test formatting_tests -v`
Expected: all tests PASS (these exercise combinations of already-implemented functions).

**Step 3: Commit**

```bash
git add tests/formatting_tests.rs
git commit -m "test: add nested inline marker toggle scenarios"
```

---

### Task 5: Wire inline toggle in app.rs

**Files:**
- Modify: `src/app.rs:168-203` (inline action methods)
- Modify: `src/app.rs:738-747` (replace `wrap_selection` with `toggle_inline_wrap`)

**Step 1: Add the `toggle_inline_wrap` wrapper to app.rs**

Replace the `wrap_selection` function (lines 738-747) with:

```rust
/// Toggle an inline marker around the current selection.
///
/// If the selection is already surrounded by `marker` (possibly among other
/// nested markers), that marker layer is removed.  Otherwise the marker is
/// added as the innermost layer.
fn toggle_inline_wrap(tv: &NSTextView, marker: &str) {
    let range: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else {
        return;
    };
    let full_str = storage.string();
    let full_len = full_str.length(); // UTF-16 length

    let selected = full_str.substringWithRange(range).to_string();

    // Grab a few characters on each side for marker detection.
    // Max combined marker width (all 4 stacked): ** ~~ ` _ = 6 UTF-16 units.
    const MAX_MARKERS: usize = 6;

    let before_start = if range.location >= MAX_MARKERS {
        range.location - MAX_MARKERS
    } else {
        0
    };
    let after_end = (range.location + range.length + MAX_MARKERS).min(full_len);

    let before_range = NSRange {
        location: before_start,
        length: range.location - before_start,
    };
    let after_range = NSRange {
        location: range.location + range.length,
        length: after_end - (range.location + range.length),
    };

    let before = full_str.substringWithRange(before_range).to_string();
    let after = full_str.substringWithRange(after_range).to_string();

    use mdit::editor::formatting::{
        find_surrounding_markers, toggle_marker_in_layers, wrap_with_layers,
    };

    let (layers, consumed_before, consumed_after) =
        find_surrounding_markers(&before, &after);

    if let Some(_) = layers.iter().position(|m| *m == marker) {
        // Marker present → remove it, keep other layers.
        let new_layers = toggle_marker_in_layers(&layers, marker);
        let new_text = wrap_with_layers(&selected, &new_layers);
        let replace_range = NSRange {
            location: range.location - consumed_before,
            length: consumed_before + range.length + consumed_after,
        };
        let ns = NSString::from_str(&new_text);
        unsafe { msg_send![tv, insertText: &*ns, replacementRange: replace_range] }
    } else {
        // Marker absent → wrap the selection.
        let new_text = format!("{}{}{}", marker, selected, marker);
        let ns = NSString::from_str(&new_text);
        unsafe { msg_send![tv, insertText: &*ns, replacementRange: range] }
    }
}
```

**Step 2: Update the inline action methods**

Change lines 170-203 to call `toggle_inline_wrap` instead of `wrap_selection`:

```rust
#[unsafe(method(applyBold:))]
fn apply_bold(&self, _sender: &AnyObject) {
    if let Some(tv) = self.active_text_view() {
        toggle_inline_wrap(&tv, "**");
    }
}

#[unsafe(method(applyItalic:))]
fn apply_italic(&self, _sender: &AnyObject) {
    if let Some(tv) = self.active_text_view() {
        toggle_inline_wrap(&tv, "_");
    }
}

#[unsafe(method(applyInlineCode:))]
fn apply_inline_code(&self, _sender: &AnyObject) {
    if let Some(tv) = self.active_text_view() {
        toggle_inline_wrap(&tv, "`");
    }
}

#[unsafe(method(applyStrikethrough:))]
fn apply_strikethrough(&self, _sender: &AnyObject) {
    if let Some(tv) = self.active_text_view() {
        toggle_inline_wrap(&tv, "~~");
    }
}
```

**Note:** `applyLink:` stays unchanged — it uses the old `wrap_selection` with asymmetric markers `[` / `]()`. Keep `wrap_selection` alive if `applyLink:` still needs it, or rename it to `insert_link_wrap` for clarity.

**Step 3: Build to verify**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles without errors.

**Step 4: Run all tests**

Run: `cargo test`
Expected: all tests pass.

**Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: wire inline toggle into sidebar actions"
```

---

### Task 6: Clean up dead code

**Files:**
- Modify: `src/app.rs`

**Step 1: Audit remaining callers**

Check that `wrap_selection` is only used by `applyLink:`. If so, rename it to `insert_link_wrap` for clarity. If it has no remaining callers, delete it entirely.

Check that `prepend_line` and `strip_line_prefix` have no remaining callers and are fully deleted (should already be done in Task 2).

**Step 2: Build + test**

Run: `cargo build && cargo test`
Expected: clean build, all tests pass.

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "refactor: remove dead wrap_selection/prepend_line helpers"
```
