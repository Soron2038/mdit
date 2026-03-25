# Codebase Optimization & app.rs Decomposition

## Problem

`app.rs` is a 1,764-line monolith handling 6+ distinct responsibilities: window setup, find/replace, tab management, file I/O, view mode toggling, appearance/preferences, and layout coordination. This makes it hard to navigate, review, and maintain. Additionally, there are smaller issues: an unused `document.rs` stub, unnecessary Cargo features, clippy warnings, and magic numbers.

## Goal

Decompose `app.rs` into a directory module with Coordinator structs for isolated concerns. Clean up dead code, unnecessary dependencies, and code smells. End result: a codebase ready for the GitHub release (P4 in FINISHING.md).

## Design

### 1. app.rs → app/ Directory Module

Convert `src/app.rs` to `src/app/mod.rs` with sub-modules:

```
src/app/
├── mod.rs              (~550 lines) — define_class!, AppDelegateIvars, ObjC action methods,
│                                      setup_window_and_menu, setup_content_views, core helpers
│                                      (content_frame, active_text_view, editor_text_view,
│                                      apply_scheme, apply_font_size, update_text_container_inset,
│                                      update_welcome_visibility, window_did_resize)
├── find.rs             (~200 lines) — FindCoordinator struct
├── file_ops.rs         (~120 lines) — impl AppDelegate: open_file_by_path, perform_save,
│                                      run_save_panel
├── tabs.rs             (~120 lines) — impl AppDelegate: switch_to_tab, add_empty_tab,
│                                      close_tab, rebuild_tab_bar
├── mode.rs             (~100 lines) — impl AppDelegate: toggle_mode
├── preferences.rs      (~80 lines)  — Preferences struct + UserDefaults load/save
└── helpers.rs          (~180 lines) — Free functions: create_window, detect_is_dark,
                                       show_save_alert, SaveChoice, toggle_inline_wrap,
                                       insert_link_wrap, apply_block_format, insert_code_block,
                                       find_all_ranges, add_titlebar_accessory, run()
```

**Constraint:** `define_class!` requires all `#[unsafe(method(...))]` ObjC action methods in the same macro block. These ~450 lines stay in `mod.rs`. The `unsafe impl NSApplicationDelegate`, `unsafe impl NSWindowDelegate`, and the action `impl AppDelegate` block within `define_class!` are not splittable.

### 2. FindCoordinator (Coordinator Struct)

```rust
// src/app/find.rs

pub(crate) struct FindCoordinator {
    matches: RefCell<Vec<NSRange>>,
    current: Cell<usize>,
    bar_height: Cell<f64>,
}
```

**Methods** (all take external components as parameters):
- `open(&self, fb: &FindBar, win: &NSWindow, scroll_view: &NSScrollView, content_frame: NSRect)`
- `close(&self, fb: &FindBar, tv: &NSTextView, editor_delegate: &MditEditorDelegate, scroll_view: &NSScrollView, win: &NSWindow, content_frame: NSRect)`
- `perform_search(&self, fb: &FindBar, storage: &NSTextStorage, tab_mode: ViewMode, scroll_view: &NSScrollView, content_frame_fn: impl Fn() -> NSRect)`
- `navigate(&self, direction: Direction, fb: &FindBar, tv: &NSTextView, storage: &NSTextStorage)` — replaces both `find_next_action` and `find_previous_action`
- `highlight_current_match(&self, storage: &NSTextStorage)`
- `scroll_to_current_match(&self, tv: &NSTextView)`
- `update_bar_height(&self, match_count: usize, mode: ViewMode, fb: &FindBar, win: &NSWindow, scroll_view: &NSScrollView)`
- `bar_height(&self) -> f64` — accessor for content_frame calculation
- `replace_one(&self, ...)` and `replace_all(&self, ...)`

**Replaces in AppDelegateIvars:**
- `find_matches: RefCell<Vec<NSRange>>` → moved into FindCoordinator
- `find_current: Cell<usize>` → moved into FindCoordinator
- `find_bar_height: Cell<f64>` → moved into FindCoordinator

**Direction enum:**
```rust
pub(crate) enum Direction { Next, Previous }
```

### 3. Preferences (Coordinator Struct)

```rust
// src/app/preferences.rs

pub(crate) struct Preferences {
    theme_pref: Cell<ThemePreference>,
    body_font_size: Cell<f64>,
}
```

**Methods:**
- `load() -> Self` — reads from NSUserDefaults
- `theme(&self) -> ThemePreference`
- `set_theme(&self, pref: ThemePreference)` — sets + persists
- `font_size(&self) -> f64`
- `set_font_size(&self, size: f64)` — sets + persists

**Constants move here:**
- `THEME_PREF_KEY`, `FONT_SIZE_PREF_KEY`, `DEFAULT_FONT_SIZE`, `MIN_FONT_SIZE`, `MAX_FONT_SIZE`

**Replaces in AppDelegateIvars:**
- `theme_pref: Cell<ThemePreference>` → `prefs: Preferences`
- `body_font_size: Cell<f64>` → inside Preferences

### 4. Split impl AppDelegate Blocks

**file_ops.rs** — `impl AppDelegate`:
- `open_file_by_path(&self, path: PathBuf)`
- `perform_save(&self, index: Option<usize>)`
- `run_save_panel(&self) -> Option<PathBuf>`

**tabs.rs** — `impl AppDelegate`:
- `switch_to_tab(&self, index: usize)`
- `add_empty_tab(&self)`
- `close_tab(&self, index: usize)`
- `rebuild_tab_bar(&self)`

**mode.rs** — `impl AppDelegate`:
- `toggle_mode(&self)`

These methods retain `&self` access to all ivars via `pub(super)` field visibility on `AppDelegateIvars`.

### 5. AppDelegateIvars Changes

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
    pub(super) find: FindCoordinator,          // NEW: replaces 3 fields
    pub(super) prefs: Preferences,             // NEW: replaces 2 fields
    pub(super) welcome_overlay: OnceCell<WelcomeOverlay>,
}
```

### 6. Smaller Cleanups

**a) Remove `src/document.rs`:**
- Delete the file
- Remove `pub mod document;` from `src/lib.rs`
- Remove `NSDocument`, `NSDocumentController` from Cargo.toml features

**b) Trim Cargo.toml objc2-app-kit features:**
- Remove: `NSPanel`, `NSVisualEffectView`, `NSButtonCell`
- Remove: `NSDocument`, `NSDocumentController` (after document.rs removal)

**c) Clippy fixes:**
- `app.rs` (~line 495): remove unnecessary `.into()` on bool
- `ui/welcome_overlay.rs` (~line 144): remove unnecessary `as usize` cast
- `ui/appearance.rs`: implement `std::str::FromStr` trait instead of custom `from_str()` method

**d) NSTextAlignment constants:**
Add shared constants in `ui/mod.rs` and import in sub-modules:
```rust
// ui/mod.rs
pub(crate) const NS_TEXT_ALIGNMENT_CENTER: usize = 1;
pub(crate) const NS_TEXT_ALIGNMENT_RIGHT: usize = 2;
```
Replace magic `2usize` / `1usize` / `1_isize` in: `ui/sidebar.rs`, `ui/path_bar.rs`, `ui/welcome_overlay.rs`, `ui/find_bar.rs`

### 7. Tests

**Existing tests (mod tests in app.rs):**
- `sidebar_frame_viewer_is_zero_width`, `sidebar_frame_editor_is_sidebar_w`
- `content_frame_viewer_starts_at_zero`, `content_frame_editor_offset_by_sidebar`
- `content_frame_height_excludes_bars`, `content_frame_with_find_bar_offset`
- All move to `app/mod.rs` (they test free functions that stay in mod.rs)

**No new tests needed** — all changes are structural (no behavior changes). The 214 existing tests serve as regression guards.

## Verification

After each commit:
1. `cargo test` — all 214 tests must pass
2. `cargo build` — zero warnings
3. `cargo clippy` — zero warnings after clippy fixes
4. `cargo build --release` — final check
5. Manual smoke test: open app, switch tabs, toggle mode, find/replace, save file
