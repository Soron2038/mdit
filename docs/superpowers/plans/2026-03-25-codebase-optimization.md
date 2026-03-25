# Codebase Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decompose the 1,764-line `app.rs` monolith into a directory module with Coordinator structs, and clean up dead code, unused Cargo features, clippy warnings, and magic numbers.

**Architecture:** Convert `src/app.rs` → `src/app/` directory module. Extract `FindCoordinator` and `Preferences` as Coordinator structs with their own state. Split remaining `impl AppDelegate` methods into thematic sub-modules (`file_ops.rs`, `tabs.rs`, `mode.rs`). Move free functions to `helpers.rs`. All ObjC action methods stay in `mod.rs` inside the `define_class!` block (Rust/ObjC constraint).

**Tech Stack:** Rust, objc2/AppKit bindings, comrak, syntect. Tests in `tests/` (integration, no AppKit). 214 existing tests serve as regression guards.

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src/app.rs` | Delete (replaced by directory) | — |
| `src/app/mod.rs` | Create (~550 lines) | `define_class!`, `AppDelegateIvars`, ObjC actions, setup, core helpers |
| `src/app/find.rs` | Create (~200 lines) | `FindCoordinator` struct + `Direction` enum |
| `src/app/preferences.rs` | Create (~80 lines) | `Preferences` struct + UserDefaults load/save |
| `src/app/file_ops.rs` | Create (~120 lines) | `impl AppDelegate`: open, save, save panel |
| `src/app/tabs.rs` | Create (~120 lines) | `impl AppDelegate`: switch, add, close, rebuild |
| `src/app/mode.rs` | Create (~100 lines) | `impl AppDelegate`: toggle_mode |
| `src/app/helpers.rs` | Create (~180 lines) | Free functions: window, dialogs, formatting wrappers |
| `src/document.rs` | Delete | Unused NSDocument stub |
| `src/lib.rs` | Modify | Remove `pub mod document;` |
| `Cargo.toml` | Modify | Remove 5 unused features |
| `src/ui/mod.rs` | Modify | Add alignment constants |
| `src/ui/path_bar.rs` | Modify | Use alignment constants |
| `src/ui/find_bar.rs` | Modify | Use alignment constants |
| `src/ui/welcome_overlay.rs` | Modify | Use alignment constants + fix cast |
| `src/ui/appearance.rs` | Modify | Implement `FromStr` trait |

---

## Task 1: Baseline + Cleanups (non-app.rs)

**Files:**
- Delete: `src/document.rs`
- Modify: `src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `src/ui/mod.rs`
- Modify: `src/ui/path_bar.rs:70,87`
- Modify: `src/ui/find_bar.rs:183`
- Modify: `src/ui/welcome_overlay.rs:144,212,232`
- Modify: `src/ui/appearance.rs:115-131`

- [ ] **Step 1: Confirm tests pass**

```bash
cargo test 2>&1 | tail -3
```
Expected: 214 tests passed, 0 failed

- [ ] **Step 2: Delete `src/document.rs` and remove from `src/lib.rs`**

Delete the file `src/document.rs` entirely.

In `src/lib.rs`, change:
```rust
pub mod markdown;
pub mod editor;
pub mod document;
pub mod ui;
pub mod export;
pub mod menu;
```
to:
```rust
pub mod markdown;
pub mod editor;
pub mod ui;
pub mod export;
pub mod menu;
```

- [ ] **Step 3: Remove unused features from `Cargo.toml`**

In the `objc2-app-kit` features list, remove these 5 lines:
- `"NSDocument", "NSDocumentController",`
- `"NSPanel", "NSVisualEffectView",`
- `"NSButtonCell"` (from the NSButton line — keep `"NSButton"` and `"NSControl"`)

The NSButton line changes from:
```toml
    "NSButton", "NSButtonCell", "NSControl",
```
to:
```toml
    "NSButton", "NSControl",
```

- [ ] **Step 4: Add alignment constants to `src/ui/mod.rs`**

Add before the `pub mod` lines:
```rust
/// NSTextAlignmentCenter (1) — used by text fields that need centered text.
pub(crate) const NS_TEXT_ALIGNMENT_CENTER: usize = 1;
/// NSTextAlignmentRight (2) — used by text fields that need right-aligned text.
pub(crate) const NS_TEXT_ALIGNMENT_RIGHT: usize = 2;
```

- [ ] **Step 5: Replace magic alignment values**

In `src/ui/path_bar.rs`, add to imports:
```rust
use super::{NS_TEXT_ALIGNMENT_CENTER, NS_TEXT_ALIGNMENT_RIGHT};
```

Line 70 — change `2usize` to `NS_TEXT_ALIGNMENT_RIGHT`:
```rust
unsafe { let _: () = msg_send![&*word_field, setAlignment: NS_TEXT_ALIGNMENT_RIGHT]; }
```

Line 87 — change `1usize` to `NS_TEXT_ALIGNMENT_CENTER`:
```rust
unsafe { let _: () = msg_send![&*info_field, setAlignment: NS_TEXT_ALIGNMENT_CENTER]; }
```

In `src/ui/find_bar.rs`, add to imports:
```rust
use super::NS_TEXT_ALIGNMENT_CENTER;
```

Line 183 — change `1usize` to `NS_TEXT_ALIGNMENT_CENTER`:
```rust
unsafe { let _: () = msg_send![&*count_label, setAlignment: NS_TEXT_ALIGNMENT_CENTER]; }
```

In `src/ui/welcome_overlay.rs`, add to imports:
```rust
use super::NS_TEXT_ALIGNMENT_CENTER;
```

Lines 212 and 232 — change `1_isize` to `NS_TEXT_ALIGNMENT_CENTER`:
```rust
unsafe { let _: () = msg_send![&*field, setAlignment: NS_TEXT_ALIGNMENT_CENTER]; }
```

- [ ] **Step 6: Fix unnecessary `as usize` cast in welcome_overlay.rs**

Line 144, change:
```rust
let count = subviews.count() as usize;
```
to:
```rust
let count = subviews.count();
```

- [ ] **Step 7: Implement `FromStr` trait in `src/ui/appearance.rs`**

Replace lines 125-131 (the `from_str` method on `ThemePreference`):
```rust
    pub fn from_str(s: &str) -> Self {
        match s {
            "light" => Self::Light,
            "dark"  => Self::Dark,
            _       => Self::System,
        }
    }
```

with a `FromStr` trait implementation placed after the `impl ThemePreference` block:
```rust
impl std::str::FromStr for ThemePreference {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "light" => Self::Light,
            "dark"  => Self::Dark,
            _       => Self::System,
        })
    }
}
```

Then update the call site in `src/app.rs` line 1615, change:
```rust
.map(|s| ThemePreference::from_str(&s.to_string()))
```
to:
```rust
.map(|s| s.to_string().parse::<ThemePreference>().unwrap_or_default())
```

(The `unwrap_or_default` is safe because `Err` is `Infallible`, and `Default` is `System`.)

Note: Remove `from_str` from the existing `impl ThemePreference` block. Keep `as_str` and `resolve` there.

- [ ] **Step 8: Run tests and build**

```bash
cargo test && cargo build
```
Expected: 214 tests passed, zero warnings

- [ ] **Step 9: Commit**

```bash
git add src/document.rs src/lib.rs Cargo.toml src/ui/mod.rs src/ui/path_bar.rs src/ui/find_bar.rs src/ui/welcome_overlay.rs src/ui/appearance.rs src/app.rs
git commit -m "chore: remove dead code, trim features, fix clippy warnings

- Remove unused document.rs NSDocument stub
- Remove 5 unused objc2-app-kit features (NSDocument, NSDocumentController,
  NSPanel, NSVisualEffectView, NSButtonCell)
- Add named constants for NSTextAlignment magic numbers
- Fix unnecessary .into() on bool, unnecessary as usize cast
- Implement FromStr trait for ThemePreference

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Create `src/app/preferences.rs`

**Files:**
- Create: `src/app/preferences.rs`

- [ ] **Step 1: Create `src/app/preferences.rs`**

```rust
//! User preferences backed by NSUserDefaults.

use std::cell::Cell;

use objc2_foundation::{NSString, NSUserDefaults};

use mdit::ui::appearance::ThemePreference;

const THEME_PREF_KEY: &str = "mditThemePreference";
const FONT_SIZE_PREF_KEY: &str = "mditFontSize";
pub(crate) const DEFAULT_FONT_SIZE: f64 = 16.0;
pub(crate) const MIN_FONT_SIZE: f64 = 12.0;
pub(crate) const MAX_FONT_SIZE: f64 = 24.0;

/// Encapsulates user preferences (theme, font size) with NSUserDefaults persistence.
pub(crate) struct Preferences {
    theme_pref: Cell<ThemePreference>,
    body_font_size: Cell<f64>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme_pref: Cell::new(ThemePreference::default()),
            body_font_size: Cell::new(DEFAULT_FONT_SIZE),
        }
    }
}

impl Preferences {
    /// Load preferences from NSUserDefaults.
    pub fn load() -> Self {
        Self {
            theme_pref: Cell::new(load_theme_pref()),
            body_font_size: Cell::new(load_font_size_pref()),
        }
    }

    pub fn theme(&self) -> ThemePreference {
        self.theme_pref.get()
    }

    pub fn set_theme(&self, pref: ThemePreference) {
        self.theme_pref.set(pref);
        save_theme_pref(pref);
    }

    pub fn font_size(&self) -> f64 {
        self.body_font_size.get()
    }

    pub fn set_font_size(&self, size: f64) {
        self.body_font_size.set(size);
        save_font_size_pref(size);
    }
}

fn save_theme_pref(pref: ThemePreference) {
    let key = NSString::from_str(THEME_PREF_KEY);
    let val = NSString::from_str(pref.as_str());
    unsafe {
        let defaults = NSUserDefaults::standardUserDefaults();
        defaults.setObject_forKey(Some(&*val), &key);
    }
}

fn load_theme_pref() -> ThemePreference {
    let key = NSString::from_str(THEME_PREF_KEY);
    let stored = NSUserDefaults::standardUserDefaults().stringForKey(&key);
    stored
        .as_deref()
        .and_then(|s| s.to_string().parse::<ThemePreference>().ok())
        .unwrap_or_default()
}

fn save_font_size_pref(size: f64) {
    let key = NSString::from_str(FONT_SIZE_PREF_KEY);
    let val = NSString::from_str(&size.to_string());
    unsafe {
        let defaults = NSUserDefaults::standardUserDefaults();
        defaults.setObject_forKey(Some(&*val), &key);
    }
}

fn load_font_size_pref() -> f64 {
    let key = NSString::from_str(FONT_SIZE_PREF_KEY);
    let stored = NSUserDefaults::standardUserDefaults().stringForKey(&key);
    stored
        .as_deref()
        .and_then(|s| s.to_string().parse::<f64>().ok())
        .unwrap_or(DEFAULT_FONT_SIZE)
}
```

- [ ] **Step 2: Verify file compiles (no module wired yet — just syntax check)**

This file will be wired in Task 7 when `mod.rs` is created. For now, verify syntax is correct by inspection.

---

## Task 3: Create `src/app/find.rs`

**Files:**
- Create: `src/app/find.rs`

- [ ] **Step 1: Create `src/app/find.rs`**

```rust
//! Find & Replace coordinator — owns all search state and operations.

use std::cell::{Cell, RefCell};

use objc2::msg_send;
use objc2_app_kit::{
    NSBackgroundColorAttributeName, NSColor, NSTextStorage, NSTextView, NSWindow,
};
use objc2_foundation::{NSRange, NSString};

use mdit::editor::view_mode::ViewMode;
use mdit::ui::find_bar::{FindBar, FIND_H_COMPACT, FIND_H_EXPANDED};

use super::helpers::find_all_ranges;

/// Search navigation direction.
pub(crate) enum Direction {
    Next,
    Previous,
}

/// Coordinates find & replace state and operations.
///
/// Owns the match list, current-match index, and bar height so these
/// concerns are decoupled from the main AppDelegate.
pub(crate) struct FindCoordinator {
    matches: RefCell<Vec<NSRange>>,
    current: Cell<usize>,
    bar_height: Cell<f64>,
}

impl Default for FindCoordinator {
    fn default() -> Self {
        Self {
            matches: RefCell::new(Vec::new()),
            current: Cell::new(0),
            bar_height: Cell::new(0.0),
        }
    }
}

impl FindCoordinator {
    /// Current find bar height (0.0 = hidden).
    pub fn bar_height(&self) -> f64 {
        self.bar_height.get()
    }

    /// Returns true if the find bar is currently visible.
    pub fn is_open(&self) -> bool {
        self.bar_height.get() > 0.0
    }

    /// Open the find bar: set height, show, resize scroll view.
    pub fn open_bar(
        &self,
        fb: &FindBar,
        w: f64,
    ) {
        let h = FIND_H_COMPACT;
        self.bar_height.set(h);
        fb.view().setFrame(objc2_foundation::NSRect::new(
            objc2_foundation::NSPoint::new(0.0, super::PATH_H),
            objc2_foundation::NSSize::new(w, h),
        ));
        fb.set_height(h);
        fb.show();
        fb.focus_search();
    }

    /// Run a search against the given text storage and update highlights + count.
    pub fn perform_search(
        &self,
        fb: &FindBar,
        storage: &NSTextStorage,
        tab_mode: ViewMode,
    ) {
        if !self.is_open() { return; }

        let query = fb.search_text();

        // Remove previous highlights
        let old_matches: Vec<NSRange> = self.matches.borrow().clone();
        for range in &old_matches {
            unsafe {
                storage.removeAttribute_range(NSBackgroundColorAttributeName, *range);
            }
        }

        if query.is_empty() {
            *self.matches.borrow_mut() = Vec::new();
            self.current.set(0);
            fb.update_count(0, 0);
            fb.set_no_match(false);
            self.update_bar_height(0, tab_mode, fb, None, None);
            return;
        }

        let ns_query = NSString::from_str(&query);
        let case_sensitive = fb.is_case_sensitive();
        let matches = find_all_ranges(&storage.string(), &ns_query, !case_sensitive);
        let count = matches.len();

        let current = self.current.get().min(count.saturating_sub(1));
        self.current.set(current);
        *self.matches.borrow_mut() = matches.clone();

        // Apply highlight attributes
        let all_match_color = NSColor::colorWithRed_green_blue_alpha(1.0, 0.97, 0.82, 1.0);
        let current_match_color = NSColor::colorWithRed_green_blue_alpha(1.0, 0.93, 0.70, 1.0);
        for (i, &range) in matches.iter().enumerate() {
            let color = if i == current { &current_match_color } else { &all_match_color };
            unsafe {
                storage.addAttribute_value_range(NSBackgroundColorAttributeName, color, range);
            }
        }

        if count > 0 {
            fb.update_count(current + 1, count);
            fb.set_no_match(false);
        } else {
            fb.update_count(0, 0);
            fb.set_no_match(true);
        }

        self.update_bar_height(count, tab_mode, fb, None, None);
    }

    /// Navigate to the next or previous match.
    pub fn navigate(
        &self,
        direction: Direction,
        fb: &FindBar,
        tv: &NSTextView,
        storage: &NSTextStorage,
    ) {
        let matches = self.matches.borrow();
        let count = matches.len();
        if count == 0 { return; }
        drop(matches);

        let current = self.current.get();
        let new_idx = match direction {
            Direction::Next => (current + 1) % count,
            Direction::Previous => if current == 0 { count - 1 } else { current - 1 },
        };
        self.current.set(new_idx);
        self.highlight_current_match(storage);
        self.scroll_to_current_match(tv);
        fb.update_count(new_idx + 1, count);
    }

    /// Replace the current match with the replacement text.
    pub fn replace_one(
        &self,
        fb: &FindBar,
        storage: &NSTextStorage,
    ) {
        let replace_str = fb.replace_text();
        let range = {
            let matches = self.matches.borrow();
            matches.get(self.current.get()).copied()
        };
        let Some(range) = range else { return };
        let ns_replace = NSString::from_str(&replace_str);
        storage.replaceCharactersInRange_withString(range, &ns_replace);
    }

    /// Replace all matches with the replacement text (reverse order to preserve offsets).
    pub fn replace_all(
        &self,
        fb: &FindBar,
        storage: &NSTextStorage,
    ) {
        let replace_str = fb.replace_text();
        let ranges: Vec<NSRange> = self.matches.borrow().clone();
        if ranges.is_empty() { return; }
        let ns_replace = NSString::from_str(&replace_str);
        for range in ranges.iter().rev() {
            storage.replaceCharactersInRange_withString(*range, &ns_replace);
        }
    }

    /// Highlight the current match (update background colors for all matches).
    pub fn highlight_current_match(&self, storage: &NSTextStorage) {
        let matches = self.matches.borrow().clone();
        let current = self.current.get();
        let all_color = NSColor::colorWithRed_green_blue_alpha(1.0, 0.97, 0.82, 1.0);
        let current_color = NSColor::colorWithRed_green_blue_alpha(1.0, 0.93, 0.70, 1.0);
        for (i, &range) in matches.iter().enumerate() {
            let color = if i == current { &current_color } else { &all_color };
            unsafe {
                storage.addAttribute_value_range(NSBackgroundColorAttributeName, color, range);
            }
        }
    }

    /// Scroll the text view so the current match is visible and select it.
    pub fn scroll_to_current_match(&self, tv: &NSTextView) {
        let matches = self.matches.borrow();
        let current = self.current.get();
        let Some(&range) = matches.get(current) else { return };
        drop(matches);
        unsafe {
            let _: () = msg_send![tv, scrollRangeToVisible: range];
            let _: () = msg_send![tv, setSelectedRange: range];
        }
    }

    /// Show or hide the replace row, updating bar height.
    ///
    /// When `win` and `scroll_view` are provided, also updates the find bar frame
    /// and scroll view frame. Pass `None` when only updating internal state.
    pub fn update_bar_height(
        &self,
        match_count: usize,
        mode: ViewMode,
        fb: &FindBar,
        win: Option<&NSWindow>,
        scroll_view: Option<&objc2_app_kit::NSScrollView>,
    ) {
        let show_replace = match_count > 0 && mode == ViewMode::Editor;
        fb.show_replace_row(show_replace);

        let new_h = if show_replace { FIND_H_EXPANDED } else { FIND_H_COMPACT };
        let old_h = self.bar_height.get();
        if (new_h - old_h).abs() > 0.5 && old_h > 0.0 {
            self.bar_height.set(new_h);
            fb.set_height(new_h);
            if let Some(win) = win {
                let w = win.contentView().unwrap().bounds().size.width;
                fb.view().setFrame(objc2_foundation::NSRect::new(
                    objc2_foundation::NSPoint::new(0.0, super::PATH_H),
                    objc2_foundation::NSSize::new(w, new_h),
                ));
            }
        }
    }

    /// Close the find bar, remove highlights, and reset state.
    pub fn close(
        &self,
        fb: &FindBar,
        storage: Option<&NSTextStorage>,
        editor_delegate: Option<&mdit::editor::text_storage::MditEditorDelegate>,
    ) {
        if !self.is_open() { return; }

        // Remove all highlights
        let matches: Vec<NSRange> = self.matches.borrow().clone();
        if !matches.is_empty() {
            if let Some(storage) = storage {
                for &range in &matches {
                    unsafe {
                        storage.removeAttribute_range(NSBackgroundColorAttributeName, range);
                    }
                }
                if let Some(ed) = editor_delegate {
                    ed.reapply(storage);
                }
            }
        }

        *self.matches.borrow_mut() = Vec::new();
        self.current.set(0);
        self.bar_height.set(0.0);
        fb.hide();
        fb.show_replace_row(false);
        fb.update_count(0, 0);
        fb.set_no_match(false);
    }
}
```

- [ ] **Step 2: Verify file syntax by inspection**

This file will be wired in Task 7.

---

## Task 4: Create `src/app/helpers.rs`

**Files:**
- Create: `src/app/helpers.rs`

- [ ] **Step 1: Create `src/app/helpers.rs`**

This file contains all free functions from the bottom of `app.rs` (lines 1410–1715):

```rust
//! Free helper functions for the app module.

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{msg_send, sel};
use objc2_app_kit::{
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication,
    NSBackingStoreType, NSBezelStyle, NSButton, NSColor, NSControl,
    NSImage, NSTextView, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSArray, NSPoint, NSRange, NSRect, NSSize, NSString,
};

/// Dirty-check dialog result.
pub(super) enum SaveChoice {
    Save,
    DontSave,
    Cancel,
}

pub(super) fn show_save_alert(filename: &str, mtm: MainThreadMarker) -> SaveChoice {
    use objc2_app_kit::NSAlert;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(&format!(
        "Do you want to save changes to \"{}\"?",
        filename
    )));
    alert.setInformativeText(&NSString::from_str(
        "Your changes will be lost if you don't save them.",
    ));
    alert.addButtonWithTitle(&NSString::from_str("Save"));
    alert.addButtonWithTitle(&NSString::from_str("Don't Save"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    let response = alert.runModal();
    match response {
        1000 => SaveChoice::Save,
        1001 => SaveChoice::DontSave,
        _ => SaveChoice::Cancel,
    }
}

/// Toggle an inline marker around the current selection.
pub(super) fn toggle_inline_wrap(tv: &NSTextView, marker: &str) {
    let range: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else { return };
    let full_str = storage.string();
    let full_len = full_str.length();
    let selected = full_str.substringWithRange(range).to_string();

    const MAX_MARKERS: usize = 6;
    let before_start = range.location.saturating_sub(MAX_MARKERS);
    let after_end = (range.location + range.length + MAX_MARKERS).min(full_len);

    let before = full_str
        .substringWithRange(NSRange { location: before_start, length: range.location - before_start })
        .to_string();
    let after = full_str
        .substringWithRange(NSRange {
            location: range.location + range.length,
            length: after_end - (range.location + range.length),
        })
        .to_string();

    let result = mdit::editor::formatting::compute_inline_toggle(&selected, &before, &after, marker);
    let replace_range = NSRange {
        location: range.location - result.consumed_before,
        length: result.consumed_before + range.length + result.consumed_after,
    };
    let ns = NSString::from_str(&result.replacement);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: replace_range] }
}

/// Replace the current NSTextView selection with `prefix + selected + suffix`.
pub(super) fn insert_link_wrap(tv: &NSTextView, prefix: &str, suffix: &str) {
    let range: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else { return };
    let selected = storage.string().substringWithRange(range).to_string();
    let text = mdit::editor::formatting::compute_link_wrap(&selected, prefix, suffix);
    let ns = NSString::from_str(&text);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: range] }
}

/// Apply a block-level format to the line containing the caret.
pub(super) fn apply_block_format(tv: &NSTextView, desired_prefix: &str) {
    let caret: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else { return };
    let ns_str = storage.string();
    let point = NSRange { location: caret.location, length: 0 };
    let line_range: NSRange = ns_str.lineRangeForRange(point);
    let line_text = ns_str.substringWithRange(line_range).to_string();
    let new_line = mdit::editor::formatting::set_block_format(&line_text, desired_prefix);
    let ns = NSString::from_str(&new_line);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: line_range] }
}

/// Wrap the current selection in a fenced code block.
pub(super) fn insert_code_block(tv: &NSTextView) {
    let range: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else { return };
    let selected = storage.string().substringWithRange(range).to_string();
    let text = mdit::editor::formatting::compute_code_block_wrap(&selected);
    let ns = NSString::from_str(&text);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: range] }
}

/// Find all occurrences of `query` in `text`, returning NSRange for each match.
pub(crate) fn find_all_ranges(text: &NSString, query: &NSString, case_insensitive: bool) -> Vec<NSRange> {
    let mut ranges = Vec::new();
    let len = text.length();
    if len == 0 || query.length() == 0 { return ranges; }
    let options: usize = if case_insensitive { 1 } else { 0 };
    let mut search_from = NSRange { location: 0, length: len };
    loop {
        let found: NSRange = unsafe {
            msg_send![text, rangeOfString: query, options: options, range: search_from]
        };
        if found.location >= usize::MAX / 2 { break; }
        ranges.push(found);
        let next_loc = found.location + found.length.max(1);
        if next_loc >= len { break; }
        search_from = NSRange { location: next_loc, length: len - next_loc };
    }
    ranges
}

/// Create the main application window.
pub(super) fn create_window(mtm: MainThreadMarker) -> Retained<NSWindow> {
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable;

    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(900.0, 700.0)),
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };

    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(ns_string!("mdit"));
    window.setContentMinSize(NSSize::new(500.0, 400.0));
    window
}

/// Return `true` when the system is currently in dark mode.
pub(super) fn detect_is_dark(app: &NSApplication) -> bool {
    let appearance = app.effectiveAppearance();
    unsafe {
        let names = NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]);
        appearance
            .bestMatchFromAppearancesWithNames(&names)
            .map(|name| name.isEqualToString(NSAppearanceNameDarkAqua))
            .unwrap_or(false)
    }
}

/// Add Eye (toggle mode) + ellipsis buttons to the title bar.
pub(super) fn add_titlebar_accessory(window: &NSWindow, mtm: MainThreadMarker, target: &AnyObject) {
    let btn_h = 20.0_f64;
    let btn_w = 26.0_f64;
    let acc_h = 28.0_f64;
    let gap = 2.0_f64;
    let total_w = btn_w * 2.0 + gap + 4.0;
    let v_off = (acc_h - btn_h) / 2.0;

    let acc_view = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(total_w, acc_h)),
    );

    let eye_btn = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(2.0, v_off), NSSize::new(btn_w, btn_h)),
    );
    eye_btn.setBezelStyle(NSBezelStyle::AccessoryBarAction);
    eye_btn.setBordered(false);
    let eye_name = NSString::from_str("eye");
    if let Some(img) = NSImage::imageWithSystemSymbolName_accessibilityDescription(&eye_name, None) {
        eye_btn.setImage(Some(&img));
    }
    eye_btn.setTitle(&NSString::from_str(""));
    unsafe {
        NSControl::setTarget(&eye_btn, Some(target));
        NSControl::setAction(&eye_btn, Some(sel!(toggleMode:)));
        let _: () = msg_send![&*eye_btn, setToolTip: &*NSString::from_str("Toggle Editor (⌘E)")];
    }
    acc_view.addSubview(&eye_btn);

    let more_btn = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(2.0 + btn_w + gap, v_off), NSSize::new(btn_w, btn_h)),
    );
    more_btn.setBezelStyle(NSBezelStyle::AccessoryBarAction);
    more_btn.setBordered(false);
    let ellipsis_name = NSString::from_str("ellipsis");
    if let Some(img) = NSImage::imageWithSystemSymbolName_accessibilityDescription(&ellipsis_name, None) {
        more_btn.setImage(Some(&img));
    }
    more_btn.setTitle(&NSString::from_str(""));
    acc_view.addSubview(&more_btn);

    let Some(vc_cls) = AnyClass::get(c"NSTitlebarAccessoryViewController") else { return };
    unsafe {
        let alloc: *mut AnyObject = msg_send![vc_cls, alloc];
        let vc: *mut AnyObject = msg_send![alloc, init];
        if vc.is_null() { return; }
        let vc_ret = Retained::retain(vc).expect("NSTitlebarAccessoryViewController");
        let _: () = msg_send![&*vc_ret, setView: &*acc_view];
        let _: () = msg_send![&*vc_ret, setLayoutAttribute: 2isize];
        let _: () = msg_send![window, addTitlebarAccessoryViewController: &*vc_ret];
    }
}
```

- [ ] **Step 2: Verify file syntax by inspection**

---

## Task 5: Create `src/app/file_ops.rs`

**Files:**
- Create: `src/app/file_ops.rs`

- [ ] **Step 1: Create `src/app/file_ops.rs`**

Move `open_file_by_path`, `perform_save`, and `run_save_panel` from `app.rs` lines 794–1172:

```rust
//! File I/O operations for AppDelegate.

use std::path::PathBuf;

use objc2_app_kit::NSTextView;
use objc2_foundation::{NSRange, NSString};

use mdit::editor::document_state::DocumentState;

use super::AppDelegate;
use super::helpers::{SaveChoice, show_save_alert};

impl AppDelegate {
    /// Open a file by path — used by both the Open dialog and Finder/Dock open events.
    pub(super) fn open_file_by_path(&self, path: PathBuf) {
        // Check if already open → switch to that tab
        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(i) = tm.find_by_path(&path) {
                drop(tm);
                self.switch_to_tab(i);
                return;
            }
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("mdit: cannot read {:?}: {}", path, e);
                return;
            }
        };

        let reuse = {
            let tm = self.ivars().tab_manager.borrow();
            tm.active().is_some_and(|t| {
                !t.is_dirty.get()
                    && t.url.borrow().is_none()
                    && unsafe { t.text_view.textStorage() }
                        .is_none_or(|s| s.length() == 0)
            })
        };
        if !reuse {
            self.add_empty_tab();
        }
        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.active() {
                *t.url.borrow_mut() = Some(path.clone());
                t.is_dirty.set(false);
                unsafe {
                    if let Some(storage) = t.text_view.textStorage() {
                        let full = NSRange { location: 0, length: storage.length() };
                        storage.replaceCharactersInRange_withString(
                            full,
                            &NSString::from_str(&content),
                        );
                    }
                }
            }
        }
        if let Some(pb) = self.ivars().path_bar.get() {
            pb.update(Some(path.as_path()));
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.active() {
                if let Some(storage) = unsafe { t.text_view.textStorage() } {
                    pb.update_wordcount(&storage.string().to_string());
                }
            }
        }
        self.rebuild_tab_bar();
        self.update_welcome_visibility();
    }

    /// Save tab at `index`, or the active tab when `index` is `None`.
    pub(super) fn perform_save(&self, index: Option<usize>) {
        let idx = index.unwrap_or_else(|| self.ivars().tab_manager.borrow().active_index());

        let existing_url: Option<PathBuf> = {
            let tm = self.ivars().tab_manager.borrow();
            match tm.get(idx) {
                None => return,
                Some(t) => t.url.borrow().clone(),
            }
        };
        let path: PathBuf = match existing_url {
            Some(p) => p,
            None => match self.run_save_panel() {
                Some(p) => p,
                None => return,
            },
        };

        let content = {
            let tm = self.ivars().tab_manager.borrow();
            let tab = match tm.get(idx) {
                Some(t) => t,
                None => return,
            };
            unsafe { tab.text_view.textStorage() }
                .map(|s| s.string().to_string())
                .unwrap_or_default()
        };

        if let Err(e) = std::fs::write(&path, content.as_bytes()) {
            eprintln!("mdit: cannot save {:?}: {}", path, e);
            return;
        }

        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.get(idx) {
                *t.url.borrow_mut() = Some(path.clone());
                t.is_dirty.set(false);
            }
        }
        if let Some(pb) = self.ivars().path_bar.get() {
            if idx == self.ivars().tab_manager.borrow().active_index() {
                pb.update(Some(path.as_path()));
            }
        }
        self.rebuild_tab_bar();
    }

    pub(super) fn run_save_panel(&self) -> Option<PathBuf> {
        use objc2_app_kit::NSSavePanel;
        let panel = NSSavePanel::savePanel(self.mtm());
        panel.setNameFieldStringValue(&NSString::from_str("Untitled.md"));
        let response = panel.runModal();
        if response != 1 { return None; }
        let ns_url = panel.URL()?;
        let ns_path = ns_url.path()?;
        Some(PathBuf::from(ns_path.to_string()))
    }
}
```

---

## Task 6: Create `src/app/tabs.rs` and `src/app/mode.rs`

**Files:**
- Create: `src/app/tabs.rs`
- Create: `src/app/mode.rs`

- [ ] **Step 1: Create `src/app/tabs.rs`**

Move `switch_to_tab`, `add_empty_tab`, `close_tab`, `rebuild_tab_bar` from `app.rs`:

```rust
//! Tab management operations for AppDelegate.

use objc2::runtime::AnyObject;
use objc2_app_kit::NSColor;
use objc2_foundation::{NSRange, NSRect, NSString};

use mdit::editor::document_state::DocumentState;
use mdit::editor::tab_manager::TabCloseResult;
use mdit::editor::view_mode::ViewMode;
use mdit::ui::sidebar::SIDEBAR_W;

use super::{AppDelegate, TAB_H, PATH_H};
use super::helpers::{show_save_alert, SaveChoice};

impl AppDelegate {
    pub(super) fn rebuild_tab_bar(&self) {
        let Some(_win) = self.ivars().window.get() else { return };
        let Some(tab_bar) = self.ivars().tab_bar.get() else { return };
        let mtm = self.mtm();
        let target: &AnyObject = unsafe { &*(self as *const AppDelegate as *const AnyObject) };
        let labels = self.ivars().tab_manager.borrow().tab_labels();
        tab_bar.rebuild(mtm, &labels, target);
    }

    pub(super) fn switch_to_tab(&self, index: usize) {
        self.close_find_bar();

        let Some(win) = self.ivars().window.get() else { return };
        let content = win.contentView().unwrap();

        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.active() {
                t.scroll_view.removeFromSuperview();
            }
        }

        self.ivars().tab_manager.borrow_mut().switch_to(index);

        let frame = self.content_frame();
        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.get(index) {
                t.scroll_view.setFrame(frame);
                content.addSubview(&t.scroll_view);
            }
        }

        if let Some(sb) = self.ivars().sidebar.get() {
            let new_tab_mode = self.ivars().tab_manager.borrow()
                .active()
                .map(|t| t.mode.get())
                .unwrap_or(ViewMode::Viewer);
            let bounds = win.contentView().unwrap().bounds();
            let content_h = (bounds.size.height - TAB_H - PATH_H).max(0.0);
            let sidebar_w = if new_tab_mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
            sb.set_size_direct(sidebar_w, content_h);
        }

        if let Some(pb) = self.ivars().path_bar.get() {
            let (url, wordcount, is_editor) = {
                let tm = self.ivars().tab_manager.borrow();
                let url = tm.get(index).and_then(|t| t.url.borrow().clone());
                let wordcount = tm.get(index).and_then(|t| {
                    unsafe { t.text_view.textStorage() }
                        .map(|s| s.string().to_string())
                });
                let is_editor = tm
                    .get(index)
                    .map(|t| t.mode.get() == ViewMode::Editor)
                    .unwrap_or(false);
                (url, wordcount, is_editor)
            };
            pb.update(url.as_deref());
            if let Some(text) = wordcount {
                pb.update_wordcount(&text);
            }
            let win_w = win.contentView().unwrap().bounds().size.width;
            pb.set_wordcount_visible(is_editor, win_w);
        }

        self.rebuild_tab_bar();
        self.update_text_container_inset();
        self.update_welcome_visibility();
    }

    pub(super) fn add_empty_tab(&self) {
        let mtm = self.mtm();
        let scheme = self.ivars().tab_manager.borrow().first_scheme()
            .unwrap_or_else(mdit::ui::appearance::ColorScheme::light);
        let frame = self.content_frame();
        let tab = DocumentState::new_empty(mtm, scheme, frame);
        tab.text_view.setEditable(false);
        tab.text_view
            .setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(self)));
        let new_idx = self.ivars().tab_manager.borrow_mut().add(tab);
        let font_size = self.ivars().prefs.font_size();
        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(tab) = tm.get(new_idx) {
                tab.editor_delegate.set_base_size(font_size);
            }
        }
        self.switch_to_tab(new_idx);
        self.update_welcome_visibility();
    }

    pub(super) fn close_tab(&self, index: usize) {
        self.close_find_bar();

        let (is_dirty, filename) = {
            let tm = self.ivars().tab_manager.borrow();
            let tab = match tm.get(index) {
                Some(t) => t,
                None => return,
            };
            let dirty = tab.is_dirty.get();
            let name = tab.url
                .borrow()
                .as_deref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".to_string());
            (dirty, name)
        };

        if is_dirty {
            match show_save_alert(&filename, self.mtm()) {
                SaveChoice::Save => self.perform_save(Some(index)),
                SaveChoice::DontSave => {}
                SaveChoice::Cancel => return,
            }
        }

        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.get(index) {
                t.scroll_view.removeFromSuperview();
            }
        }

        let result = self.ivars().tab_manager.borrow_mut().remove(index);

        match result {
            TabCloseResult::LastTab => {
                let tm = self.ivars().tab_manager.borrow();
                if let Some(t) = tm.active() {
                    unsafe {
                        if let Some(storage) = t.text_view.textStorage() {
                            let full = NSRange { location: 0, length: storage.length() };
                            let empty = NSString::from_str("");
                            storage.replaceCharactersInRange_withString(full, &empty);
                        }
                    }
                    *t.url.borrow_mut() = None;
                    t.is_dirty.set(false);
                    let content = self.ivars().window.get().unwrap().contentView().unwrap();
                    t.scroll_view.setFrame(self.content_frame());
                    content.addSubview(&t.scroll_view);
                    let remaining_mode = t.mode.get();
                    if let (Some(sb), Some(win)) = (self.ivars().sidebar.get(), self.ivars().window.get()) {
                        let bounds = win.contentView().unwrap().bounds();
                        let content_h = (bounds.size.height - TAB_H - PATH_H).max(0.0);
                        let sidebar_w = if remaining_mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
                        sb.set_size_direct(sidebar_w, content_h);
                    }
                }
                drop(tm);
                self.rebuild_tab_bar();
                if let Some(pb) = self.ivars().path_bar.get() {
                    pb.update(None);
                }
                self.update_welcome_visibility();
            }
            TabCloseResult::Removed { new_active } => {
                self.switch_to_tab(new_active);
            }
        }
    }
}
```

- [ ] **Step 2: Create `src/app/mode.rs`**

Move `toggle_mode` from `app.rs` lines 697–787:

```rust
//! View mode toggle animation for AppDelegate.

use std::ptr::NonNull;

use block2::StackBlock;
use objc2::msg_send;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSAnimationContext, NSColor, NSView};
use objc2_foundation::NSString;
use objc2_quartz_core::{CAMediaTimingFunction, kCAMediaTimingFunctionEaseInEaseOut};

use mdit::editor::view_mode::ViewMode;
use mdit::ui::sidebar::SIDEBAR_W;

use super::{AppDelegate, TAB_H, PATH_H, sidebar_target_frame, content_target_frame};

impl AppDelegate {
    pub(super) fn toggle_mode(&self) {
        let (new_mode, text_view, editor_delegate, scroll_view) = {
            let tm = self.ivars().tab_manager.borrow();
            let tab = match tm.active() {
                Some(t) => t,
                None => return,
            };
            let new_mode = match tab.mode.get() {
                ViewMode::Viewer => ViewMode::Editor,
                ViewMode::Editor => ViewMode::Viewer,
            };
            tab.mode.set(new_mode);
            (
                new_mode,
                tab.text_view.clone(),
                tab.editor_delegate.clone(),
                tab.scroll_view.clone(),
            )
        };

        editor_delegate.set_mode(new_mode);
        text_view.setEditable(new_mode == ViewMode::Editor);
        if let Some(storage) = unsafe { text_view.textStorage() } {
            editor_delegate.reapply(&storage);
        }
        self.update_text_container_inset();

        let Some(win) = self.ivars().window.get() else { return };
        let bounds = win.contentView().unwrap().bounds();
        let (win_w, win_h) = (bounds.size.width, bounds.size.height);
        let content_h = (win_h - TAB_H - PATH_H).max(0.0);

        let target_sb_frame = sidebar_target_frame(new_mode, content_h);
        let find_offset = self.ivars().find.bar_height();
        let target_sv_frame = content_target_frame(new_mode, find_offset, win_w, win_h);

        let Some(sb) = self.ivars().sidebar.get() else { return };

        let sb_ptr: *const NSView = sb.view();
        let sv_ptr: *const objc2_app_kit::NSScrollView = &*scroll_view;

        let animation_block = StackBlock::new(move |ctx: NonNull<NSAnimationContext>| {
            let ctx = unsafe { ctx.as_ref() };
            ctx.setDuration(0.35);
            let timing = CAMediaTimingFunction::functionWithName(unsafe { kCAMediaTimingFunctionEaseInEaseOut });
            ctx.setTimingFunction(Some(&*timing));
            let sb_proxy: *const AnyObject = unsafe { msg_send![sb_ptr, animator] };
            let _: () = unsafe { msg_send![sb_proxy, setFrame: target_sb_frame] };
            let sv_proxy: *const AnyObject = unsafe { msg_send![sv_ptr, animator] };
            let _: () = unsafe { msg_send![sv_proxy, setFrame: target_sv_frame] };
        });
        NSAnimationContext::runAnimationGroup_completionHandler(
            &animation_block,
            None::<&block2::DynBlock<dyn Fn()>>,
        );

        if self.ivars().find.is_open() {
            if let Some(fb) = self.ivars().find_bar.get() {
                let tm = self.ivars().tab_manager.borrow();
                if let Some(tab) = tm.active() {
                    if let Some(storage) = unsafe { tab.text_view.textStorage() } {
                        self.ivars().find.perform_search(fb, &storage, new_mode);
                        self.ivars().find.scroll_to_current_match(&tab.text_view);
                    }
                }
            }
        }

        if let Some(pb) = self.ivars().path_bar.get() {
            pb.set_wordcount_visible(new_mode == ViewMode::Editor, win_w);
            if new_mode == ViewMode::Editor {
                if let Some(storage) = unsafe { text_view.textStorage() } {
                    pb.update_wordcount(&storage.string().to_string());
                }
            }
        }
        self.update_welcome_visibility();
    }
}
```

---

## Task 7: Create `src/app/mod.rs` (the core module)

**Files:**
- Create: `src/app/mod.rs` (replaces `src/app.rs`)

This is the largest and most critical task. The `mod.rs` contains:
1. All sub-module declarations
2. Layout constants and free functions (`sidebar_target_frame`, `content_target_frame`)
3. `AppDelegateIvars` with updated fields
4. The entire `define_class!` block (all ObjC action methods)
5. Core `impl AppDelegate` methods that remain here
6. Tests
7. `pub fn run()`

- [ ] **Step 1: Rename `src/app.rs` to `src/app/mod.rs`**

```bash
mkdir -p src/app
mv src/app.rs src/app/mod.rs
```

- [ ] **Step 2: Add sub-module declarations at the top of `src/app/mod.rs`**

Add after the existing `use` statements (around line 30):
```rust
mod find;
mod preferences;
mod file_ops;
mod tabs;
mod mode;
mod helpers;

use find::{FindCoordinator, Direction};
use preferences::Preferences;
use helpers::*;
```

- [ ] **Step 3: Update `AppDelegateIvars` to use Coordinator structs**

Replace the old `AppDelegateIvars` (lines 72-92) with:
```rust
#[derive(Default)]
pub(super) struct AppDelegateIvars {
    pub(super) window: OnceCell<Retained<NSWindow>>,
    pub(super) sidebar: OnceCell<FormattingSidebar>,
    pub(super) tab_bar: OnceCell<TabBar>,
    pub(super) path_bar: OnceCell<PathBar>,
    pub(super) tab_manager: RefCell<TabManager>,
    pub(super) pending_open: RefCell<Option<PathBuf>>,
    pub(super) find_bar: OnceCell<FindBar>,
    pub(super) find: FindCoordinator,
    pub(super) prefs: Preferences,
    pub(super) welcome_overlay: OnceCell<WelcomeOverlay>,
}
```

- [ ] **Step 4: Update `did_finish_launching` to use Preferences**

Replace lines 110-113:
```rust
let pref = load_theme_pref();
self.ivars().theme_pref.set(pref);
let font_size = load_font_size_pref();
self.ivars().body_font_size.set(font_size);
```
with:
```rust
let loaded_prefs = Preferences::load();
let pref = loaded_prefs.theme();
let font_size = loaded_prefs.font_size();
// Note: ivars already has Default prefs; overwrite with loaded values
self.ivars().prefs.set_theme_no_persist(pref);
self.ivars().prefs.set_font_size_no_persist(font_size);
```

Actually, simpler: since `AppDelegateIvars` uses `Default`, and preferences need to be loaded at launch time, add a method to Preferences:

In `preferences.rs`, add:
```rust
/// Set theme without persisting (used during initial load).
pub fn set_theme_no_persist(&self, pref: ThemePreference) {
    self.theme_pref.set(pref);
}

/// Set font size without persisting (used during initial load).
pub fn set_font_size_no_persist(&self, size: f64) {
    self.body_font_size.set(size);
}
```

Then replace lines 110-113 with:
```rust
let loaded = Preferences::load();
self.ivars().prefs.set_theme_no_persist(loaded.theme());
self.ivars().prefs.set_font_size_no_persist(loaded.font_size());
let pref = loaded.theme();
let font_size = loaded.font_size();
```

- [ ] **Step 5: Update theme action methods to use `self.ivars().prefs`**

Replace `self.ivars().theme_pref.set(...)` + `save_theme_pref(...)` with `self.ivars().prefs.set_theme(...)`:

Lines 249-253 (apply_light_mode):
```rust
fn apply_light_mode(&self, _sender: &AnyObject) {
    self.ivars().prefs.set_theme(ThemePreference::Light);
    self.apply_scheme(ColorScheme::light());
}
```

Lines 255-260 (apply_dark_mode):
```rust
fn apply_dark_mode(&self, _sender: &AnyObject) {
    self.ivars().prefs.set_theme(ThemePreference::Dark);
    self.apply_scheme(ColorScheme::dark());
}
```

Lines 262-269 (apply_system_mode):
```rust
fn apply_system_mode(&self, _sender: &AnyObject) {
    self.ivars().prefs.set_theme(ThemePreference::System);
    let app = NSApplication::sharedApplication(self.mtm());
    let scheme = ThemePreference::System.resolve(detect_is_dark(&app));
    self.apply_scheme(scheme);
}
```

- [ ] **Step 6: Update font size action methods to use `self.ivars().prefs`**

Lines 273-288, replace `self.ivars().body_font_size.get()` with `self.ivars().prefs.font_size()`:
```rust
fn increase_font_size_action(&self, _sender: &AnyObject) {
    let new_size = (self.ivars().prefs.font_size() + 1.0).min(preferences::MAX_FONT_SIZE);
    self.apply_font_size(new_size);
}

fn decrease_font_size_action(&self, _sender: &AnyObject) {
    let new_size = (self.ivars().prefs.font_size() - 1.0).max(preferences::MIN_FONT_SIZE);
    self.apply_font_size(new_size);
}

fn reset_font_size_action(&self, _sender: &AnyObject) {
    self.apply_font_size(preferences::DEFAULT_FONT_SIZE);
}
```

- [ ] **Step 7: Update find action methods to use FindCoordinator**

Replace find_next_action (lines 391-410):
```rust
fn find_next_action(&self, _sender: &AnyObject) {
    if !self.ivars().find.is_open() {
        self.open_find_bar();
        return;
    }
    let Some(fb) = self.ivars().find_bar.get() else { return };
    let tm = self.ivars().tab_manager.borrow();
    let Some(tab) = tm.active() else { return };
    let Some(storage) = (unsafe { tab.text_view.textStorage() }) else { return };
    self.ivars().find.navigate(Direction::Next, fb, &tab.text_view, &storage);
}
```

Replace find_previous_action (lines 412-426):
```rust
fn find_previous_action(&self, _sender: &AnyObject) {
    let Some(fb) = self.ivars().find_bar.get() else { return };
    let tm = self.ivars().tab_manager.borrow();
    let Some(tab) = tm.active() else { return };
    let Some(storage) = (unsafe { tab.text_view.textStorage() }) else { return };
    self.ivars().find.navigate(Direction::Previous, fb, &tab.text_view, &storage);
}
```

Replace replace_one_action (lines 436-454):
```rust
fn replace_one_action(&self, _sender: &AnyObject) {
    let Some(fb) = self.ivars().find_bar.get() else { return };
    let tm = self.ivars().tab_manager.borrow();
    let Some(tab) = tm.active() else { return };
    if let Some(storage) = unsafe { tab.text_view.textStorage() } {
        self.ivars().find.replace_one(fb, &storage);
    }
    drop(tm);
    self.perform_search();
}
```

Replace replace_all_action (lines 456-473):
```rust
fn replace_all_action(&self, _sender: &AnyObject) {
    let Some(fb) = self.ivars().find_bar.get() else { return };
    let tm = self.ivars().tab_manager.borrow();
    let Some(tab) = tm.active() else { return };
    if let Some(storage) = unsafe { tab.text_view.textStorage() } {
        self.ivars().find.replace_all(fb, &storage);
    }
    drop(tm);
    self.perform_search();
}
```

- [ ] **Step 8: Fix the `.into()` clippy warning**

Lines 493-495, change:
```rust
return true.into();
```
to `return true;` and `false.into()` to `false`.

- [ ] **Step 9: Update `apply_font_size` to use Preferences**

In the `apply_font_size` method (lines 1010-1021), replace:
```rust
self.ivars().body_font_size.set(size);
save_font_size_pref(size);
```
with:
```rust
self.ivars().prefs.set_font_size(size);
```

- [ ] **Step 10: Update `open_find_bar` and `close_find_bar` to use FindCoordinator**

Replace `open_find_bar` (lines 1200-1220) to delegate to FindCoordinator:
```rust
fn open_find_bar(&self) {
    let Some(fb) = self.ivars().find_bar.get() else { return };
    let Some(win) = self.ivars().window.get() else { return };
    let w = win.contentView().unwrap().bounds().size.width;
    self.ivars().find.open_bar(fb, w);
    let frame = self.content_frame();
    let tm = self.ivars().tab_manager.borrow();
    if let Some(t) = tm.active() {
        t.scroll_view.setFrame(frame);
    }
    drop(tm);
    fb.focus_search();
    self.perform_search();
}
```

Replace `perform_search` (lines 1222-1298):
```rust
fn perform_search(&self) {
    let Some(fb) = self.ivars().find_bar.get() else { return };
    if !self.ivars().find.is_open() { return; }
    let (storage, tab_mode, tv) = {
        let tm = self.ivars().tab_manager.borrow();
        let Some(tab) = tm.active() else { return };
        let storage = unsafe { tab.text_view.textStorage() };
        (storage, tab.mode.get(), tab.text_view.clone())
    };
    let Some(storage) = storage else { return };
    self.ivars().find.perform_search(fb, &storage, tab_mode);
    // Scroll to first match if there are matches
    self.ivars().find.scroll_to_current_match(&tv);
    // Update scroll view frame if bar height changed
    let frame = self.content_frame();
    let tm = self.ivars().tab_manager.borrow();
    if let Some(t) = tm.active() {
        t.scroll_view.setFrame(frame);
    }
}
```

Replace `close_find_bar` (lines 1362-1404):
```rust
fn close_find_bar(&self) {
    let Some(fb) = self.ivars().find_bar.get() else { return };
    if !self.ivars().find.is_open() { return; }
    let (storage, editor_delegate) = {
        let tm = self.ivars().tab_manager.borrow();
        match tm.active() {
            Some(tab) => (unsafe { tab.text_view.textStorage() }, Some(tab.editor_delegate.clone())),
            None => (None, None),
        }
    };
    self.ivars().find.close(fb, storage.as_deref(), editor_delegate.as_deref());
    let frame = self.content_frame();
    let tm = self.ivars().tab_manager.borrow();
    if let Some(t) = tm.active() {
        t.scroll_view.setFrame(frame);
        if let Some(win) = self.ivars().window.get() {
            unsafe { let _: () = msg_send![&**win, makeFirstResponder: &*t.text_view]; }
        }
    }
}
```

- [ ] **Step 11: Update `content_frame` to use FindCoordinator**

Line 657, replace `self.ivars().find_bar_height.get()` with `self.ivars().find.bar_height()`.

- [ ] **Step 12: Update `window_did_resize` to use FindCoordinator**

Line 175, replace `self.ivars().find_bar_height.get()` with `self.ivars().find.bar_height()`.

- [ ] **Step 13: Update `add_empty_tab` to use Preferences**

In `add_empty_tab` (now in `tabs.rs`), replace `self.ivars().body_font_size.get()` with `self.ivars().prefs.font_size()`.

- [ ] **Step 14: Remove all methods and functions that have been moved to sub-modules**

Delete from `mod.rs`:
- `open_file_by_path` (moved to `file_ops.rs`)
- `perform_save` (moved to `file_ops.rs`)
- `run_save_panel` (moved to `file_ops.rs`)
- `switch_to_tab` (moved to `tabs.rs`)
- `add_empty_tab` (moved to `tabs.rs`)
- `close_tab` (moved to `tabs.rs`)
- `rebuild_tab_bar` (moved to `tabs.rs`)
- `toggle_mode` (moved to `mode.rs`)
- `highlight_current_match` (moved to `find.rs`)
- `scroll_to_current_match` (moved to `find.rs`)
- `update_find_bar_height_for_matches` (moved to `find.rs`)
- `SaveChoice` enum + `show_save_alert` (moved to `helpers.rs`)
- `toggle_inline_wrap` (moved to `helpers.rs`)
- `insert_link_wrap` (moved to `helpers.rs`)
- `apply_block_format` (moved to `helpers.rs`)
- `insert_code_block` (moved to `helpers.rs`)
- `find_all_ranges` (moved to `helpers.rs`)
- `create_window` (moved to `helpers.rs`)
- `detect_is_dark` (moved to `helpers.rs`)
- `add_titlebar_accessory` (moved to `helpers.rs`)
- `save_theme_pref`, `load_theme_pref`, `save_font_size_pref`, `load_font_size_pref` (moved to `preferences.rs`)
- Constants: `THEME_PREF_KEY`, `FONT_SIZE_PREF_KEY`, `DEFAULT_FONT_SIZE`, `MIN_FONT_SIZE`, `MAX_FONT_SIZE` (moved to `preferences.rs`)

- [ ] **Step 15: Remove old imports that are no longer needed in mod.rs**

Remove imports only used by moved code. Keep imports used by the remaining code.

- [ ] **Step 16: Ensure `run()` is still public and calls `AppDelegate::new()`**

The `pub fn run()` stays in `mod.rs` (or can be in `helpers.rs` — but it's cleaner in `mod.rs` since it creates the AppDelegate).

- [ ] **Step 17: Run tests and build**

```bash
cargo test && cargo build 2>&1 | head -20
```
Expected: 214 tests passed, zero errors

- [ ] **Step 18: Commit**

```bash
git add -A
git commit -m "refactor: decompose app.rs into directory module with coordinators

Split the 1,764-line monolith into 7 focused files:
- mod.rs: define_class!, AppDelegateIvars, ObjC actions, core helpers
- find.rs: FindCoordinator struct (owns search state + operations)
- preferences.rs: Preferences struct (theme + font size + UserDefaults)
- file_ops.rs: open/save file operations
- tabs.rs: tab management (switch, add, close, rebuild)
- mode.rs: Viewer/Editor toggle with animation
- helpers.rs: free functions (window, dialogs, formatting wrappers)

No behavior changes. All 214 tests pass.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Final verification

**Files:**
- Modify: `FINISHING.md` (mark task complete)

- [ ] **Step 1: Run full test suite**

```bash
cargo test
```
Expected: 214 passed

- [ ] **Step 2: Run clippy**

```bash
cargo clippy 2>&1 | tail -10
```
Expected: zero warnings

- [ ] **Step 3: Release build**

```bash
cargo build --release
```
Expected: successful build, zero warnings

- [ ] **Step 4: Verify file structure**

```bash
ls -la src/app/
```
Expected: `mod.rs`, `find.rs`, `preferences.rs`, `file_ops.rs`, `tabs.rs`, `mode.rs`, `helpers.rs`

```bash
wc -l src/app/*.rs
```
Expected: mod.rs ~550, find.rs ~200, preferences.rs ~80, file_ops.rs ~120, tabs.rs ~120, mode.rs ~100, helpers.rs ~180

- [ ] **Step 5: Manual smoke test**

```bash
cargo run
```
Verify: open app → switch tabs → toggle mode → Cmd+F → find/replace → save file

- [ ] **Step 6: Commit FINISHING.md update**

In `FINISHING.md`, the refactoring task was already marked complete in the previous session. No additional FINISHING.md update needed unless a new task entry was added for this optimization pass.

```bash
git add FINISHING.md
git commit -m "docs: mark codebase optimization complete in FINISHING.md

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```
