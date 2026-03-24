# Configurable Font Size Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make font size adjustable via Cmd++/Cmd+–/Cmd+0 (range 12–24pt, default 16pt), with headings scaling proportionally and the preference persisted in NSUserDefaults.

**Architecture:** Thread a `base_size: f64` parameter through the rendering pipeline. `MditEditorDelegate` stores it as an ivar; `AppDelegate` persists it via NSUserDefaults and updates all open tabs on change. Pattern is identical to the existing `ColorScheme` / `ThemePreference` system.

**Tech Stack:** Rust, objc2, objc2-app-kit, NSUserDefaults

**Spec:** `docs/superpowers/specs/2026-03-24-font-size-configurable-design.md`

---

## File Map

| File | Change |
|------|--------|
| `src/markdown/attributes.rs` | `font_size()` → `Option<f64>`; `for_heading(level, base_size)` |
| `src/editor/renderer.rs` | `compute_attribute_runs` + internal helpers get `base_size` param |
| `src/editor/apply.rs` | `apply_attribute_runs`, `apply_runs`, `apply_attr_set`, `build_font` get `base_size`; fix monospace size |
| `src/editor/text_storage.rs` | New `base_size: Cell<f64>` ivar; `set_base_size` / `base_size` accessors; thread into calls |
| `src/app.rs` | Constants, `body_font_size` ivar, load/save helpers, 3 ObjC action methods |
| `src/menu.rs` | 3 new items in View menu |
| `tests/attributes_tests.rs` | Update callers of `font_size()`, add proportional heading tests |
| `tests/renderer_tests.rs` | Update `compute_attribute_runs` call sites (new `base_size` arg) |

---

## Task 1: `attributes.rs` — `font_size()` and `for_heading()`

**Files:**
- Modify: `src/markdown/attributes.rs:41-52` (font_size), `:79-94` (for_heading)
- Test: `tests/attributes_tests.rs`

- [ ] **Step 1: Write failing tests**

  Add to `tests/attributes_tests.rs`:

  ```rust
  #[test]
  fn font_size_returns_none_when_no_size_attribute() {
      let attrs = AttributeSet::for_strong();
      assert_eq!(attrs.font_size(), None);
  }

  #[test]
  fn font_size_returns_some_for_heading() {
      let attrs = AttributeSet::for_heading(1, 16.0);
      assert!(attrs.font_size().is_some());
  }

  #[test]
  fn heading1_scales_proportionally_at_default() {
      let attrs = AttributeSet::for_heading(1, 16.0);
      // 16 * 1.375 = 22.0
      assert_eq!(attrs.font_size(), Some(22.0));
  }

  #[test]
  fn heading1_scales_proportionally_at_20pt() {
      let attrs = AttributeSet::for_heading(1, 20.0);
      // 20 * 1.375 = 27.5 → rounds to 28
      assert_eq!(attrs.font_size(), Some(28.0));
  }

  #[test]
  fn heading2_scales_proportionally_at_default() {
      let attrs = AttributeSet::for_heading(2, 16.0);
      // 16 * 1.125 = 18.0
      assert_eq!(attrs.font_size(), Some(18.0));
  }

  #[test]
  fn heading3_equals_body_size() {
      let attrs = AttributeSet::for_heading(3, 16.0);
      // H3 × 1.0 = body size
      assert_eq!(attrs.font_size(), Some(16.0));
  }

  #[test]
  fn heading3_equals_body_at_20pt() {
      let attrs = AttributeSet::for_heading(3, 20.0);
      assert_eq!(attrs.font_size(), Some(20.0));
  }

  #[test]
  fn heading2_scales_proportionally_at_20pt() {
      let attrs = AttributeSet::for_heading(2, 20.0);
      // 20 * 1.125 = 22.5 → rounds to 23 (round-half-away-from-zero)
      assert_eq!(attrs.font_size(), Some(23.0));
  }
  ```

  Also update the two existing tests that will break:

  ```rust
  // was: assert!(attrs.font_size() > 20.0);
  #[test]
  fn heading1_gets_large_size() {
      let attrs = AttributeSet::for_heading(1, 16.0);
      assert!(attrs.font_size().unwrap_or(0.0) > 20.0);
  }

  // was: assert!(attrs.font_size() >= 14.0);
  #[test]
  fn heading3_gets_medium_size() {
      let attrs = AttributeSet::for_heading(3, 16.0);
      // H3 is now body size (16pt), which is ≥ 14.0
      assert!(attrs.font_size().unwrap_or(0.0) >= 14.0);
  }
  ```

- [ ] **Step 2: Run tests to confirm they fail**

  ```bash
  cargo test --test attributes_tests 2>&1 | head -40
  ```

  Expected: compile errors (font_size signature mismatch, for_heading arity).

- [ ] **Step 3: Update `src/markdown/attributes.rs`**

  Change `font_size()` at line 41 to return `Option<f64>`:

  ```rust
  pub fn font_size(&self) -> Option<f64> {
      self.0.iter().find_map(|a| {
          if let TextAttribute::FontSize(s) = a {
              Some(*s as f64)
          } else {
              None
          }
      })
  }
  ```

  Change `for_heading` at line 79 to accept `base_size: f64`:

  ```rust
  pub fn for_heading(level: u8, base_size: f64) -> Self {
      let size = match level {
          1 => (base_size * 1.375).round() as u8,
          2 => (base_size * 1.125).round() as u8,
          _ => base_size as u8,
      };
      let mut attrs = vec![
          TextAttribute::FontSize(size),
          TextAttribute::ForegroundColor("heading"),
      ];
      if level <= 2 {
          attrs.push(TextAttribute::HeadingSeparator);
      }
      Self::new(attrs)
  }
  ```

- [ ] **Step 4: Run the new attribute tests**

  ```bash
  cargo test --test attributes_tests 2>&1 | tail -20
  ```

  Expected: all attributes_tests pass; other crates will have compile errors (that's OK — they'll be fixed in subsequent tasks).

- [ ] **Step 5: Commit**

  ```bash
  git add src/markdown/attributes.rs tests/attributes_tests.rs
  git commit -m "feat: font_size() returns Option<f64>, for_heading takes base_size"
  ```

---

## Task 2: `renderer.rs` — Thread `base_size` through `compute_attribute_runs`

**Files:**
- Modify: `src/editor/renderer.rs` (public function + internal helpers + `collect_heading`)
- Test: `tests/renderer_tests.rs`

- [ ] **Step 1: Find all internal functions that reach `for_heading`**

  ```bash
  grep -n "for_heading\|collect_heading\|collect_runs\|compute_attribute_runs" src/editor/renderer.rs
  ```

  You'll see `collect_heading` calls `for_heading`, `collect_runs` calls `collect_heading`, `compute_attribute_runs` calls `collect_runs`.

- [ ] **Step 2: Update the failing renderer tests**

  First, find ALL calls to `font_size()` and `compute_attribute_runs` in `renderer_tests.rs`:

  ```bash
  grep -n "font_size\|compute_attribute_runs" tests/renderer_tests.rs
  ```

  - Every `compute_attribute_runs(text, &spans, ...)` call needs `16.0` added as the last argument.
  - Any `r.attrs.font_size() > 20.0` comparisons (line ~69) must become `r.attrs.font_size().unwrap_or(0.0) > 20.0` since `font_size()` now returns `Option<f64>`.

  Example diffs:
  ```rust
  // compute_attribute_runs calls:
  // Before:
  let runs = compute_attribute_runs(text, &spans, None).runs;
  // After:
  let runs = compute_attribute_runs(text, &spans, None, 16.0).runs;

  // font_size() comparisons:
  // Before:
  assert!(r.attrs.font_size() > 20.0);
  // After:
  assert!(r.attrs.font_size().unwrap_or(0.0) > 20.0);
  ```

- [ ] **Step 3: Update `src/editor/renderer.rs`**

  3a. List ALL internal functions that need `base_size` threaded through:

  ```bash
  grep -n "fn collect_\|fn compute_" src/editor/renderer.rs
  ```

  Every `fn collect_*` that directly or indirectly calls `collect_runs` needs `base_size: f64` added to its signature. Thread it through every call: `collect_runs`, `collect_heading`, `collect_symmetric_marker`, `collect_link`, and any other helpers listed by the grep above.

  3b. `compute_attribute_runs` (line 52): add `base_size: f64` parameter, pass to `collect_runs`:

  ```rust
  pub fn compute_attribute_runs(
      text: &str,
      spans: &[MarkdownSpan],
      cursor_pos: Option<usize>,
      base_size: f64,
  ) -> RenderOutput {
      let mut runs = Vec::new();
      let mut table_infos = Vec::new();
      for span in spans {
          collect_runs(text, span, cursor_pos, base_size, &[], &mut runs, &mut table_infos);
      }
      RenderOutput {
          runs: fill_gaps(text.len(), runs),
          table_infos,
      }
  }
  ```

  3c. `collect_heading` (line ~270): add `base_size: f64`. Change all 3 calls to `for_heading` (lines 286, 296, 308):

  ```rust
  AttributeSet::for_heading(level, base_size)  // was: for_heading(level)
  ```

- [ ] **Step 4: Run renderer tests**

  ```bash
  cargo test --test renderer_tests 2>&1 | tail -20
  ```

  Expected: all renderer_tests pass.

- [ ] **Step 5: Commit**

  ```bash
  git add src/editor/renderer.rs tests/renderer_tests.rs
  git commit -m "feat: thread base_size through compute_attribute_runs and collect_heading"
  ```

---

## Task 3: `apply.rs` — Thread `base_size`, fix body font and monospace size

**Files:**
- Modify: `src/editor/apply.rs` (4 function signatures + 3 call sites)

- [ ] **Step 1: Update `apply_attribute_runs` (line 124)**

  Add `base_size: f64` as last parameter. Change line 142 to use `base_size`:

  ```rust
  pub fn apply_attribute_runs(
      storage: &NSTextStorage,
      text: &str,
      runs: &[AttributeRun],
      table_infos: &[TableInfo],
      code_block_infos: &[CodeBlockInfo],
      scheme: &ColorScheme,
      base_size: f64,
  ) -> LayoutPositions {
      // ...
      let body_font = serif_font(base_size, false, false);  // was 16.0
      // ...
      let (heading_seps, thematic_breaks) = apply_runs(storage, text, runs, text_len_u16, scheme, base_size);
      // ...
  }
  ```

- [ ] **Step 2: Update `apply_runs` (line 194)**

  Add `base_size: f64` and pass to `apply_attr_set`:

  ```rust
  fn apply_runs(
      storage: &NSTextStorage,
      text: &str,
      runs: &[AttributeRun],
      text_len_u16: usize,
      scheme: &ColorScheme,
      base_size: f64,
  ) -> (Vec<usize>, Vec<usize>) {
      // ...
      apply_attr_set(storage, range, &run.attrs, scheme, base_size);
      // ...
  }
  ```

- [ ] **Step 3: Update `apply_attr_set` (line 433)**

  Add `base_size: f64` and pass to `build_font`. Note: line 440 has `let font = build_font(attrs);` — this must become `build_font(attrs, base_size)`:

  ```rust
  fn apply_attr_set(
      storage: &NSTextStorage,
      range: NSRange,
      attrs: &AttributeSet,
      scheme: &ColorScheme,
      base_size: f64,
  ) {
      let font = build_font(attrs, base_size);  // line 440: was build_font(attrs)
      // rest unchanged...
  }
  ```

- [ ] **Step 4: Update `build_font` (line 549)**

  Add `base_size: f64`. Use `unwrap_or(base_size)` for the size fallback. Fix the monospace size to be relative:

  ```rust
  fn build_font(attrs: &AttributeSet, base_size: f64) -> Retained<NSFont> {
      if attrs.contains(&TextAttribute::Hidden) {
          return unsafe { NSFont::systemFontOfSize_weight(0.001, NSFontWeightRegular) };
      }

      let size = attrs.font_size().unwrap_or(base_size);  // Option<f64> → f64
      let bold = attrs.contains(&TextAttribute::Bold);
      let italic = attrs.contains(&TextAttribute::Italic);
      let mono = attrs.contains(&TextAttribute::Monospace);

      if mono {
          let code_size = size - 2.0;  // was: if size == 16.0 { 14.0 } else { size }
          // rest unchanged...
      }
      // rest unchanged...
  }
  ```

- [ ] **Step 5: Build**

  ```bash
  cargo build 2>&1 | grep "^error" | head -20
  ```

  Expected: errors only in `text_storage.rs` (where `apply_attribute_runs` is called without the new arg). Fix those in Task 4.

- [ ] **Step 6: Commit**

  ```bash
  git add src/editor/apply.rs
  git commit -m "feat: thread base_size through apply pipeline, fix relative code font size"
  ```

---

## Task 4: `text_storage.rs` — Add `base_size` ivar to `MditEditorDelegate`

**Files:**
- Modify: `src/editor/text_storage.rs`

- [ ] **Step 1: Add ivar to `MditEditorDelegateIvars` (line 22)**

  ```rust
  pub struct MditEditorDelegateIvars {
      // ...existing fields...
      /// Base font size in points. Matches AppDelegate.body_font_size.
      base_size: Cell<f64>,
  }
  ```

- [ ] **Step 2: Initialize ivar in `MditEditorDelegate::new` (line 134)**

  ```rust
  pub fn new(mtm: MainThreadMarker, scheme: ColorScheme) -> Retained<Self> {
      let this = Self::alloc(mtm).set_ivars(MditEditorDelegateIvars {
          // ...existing fields...
          base_size: Cell::new(16.0),
      });
      unsafe { msg_send![super(this), init] }
  }
  ```

- [ ] **Step 3: Add public accessors (after line 156)**

  ```rust
  /// Get the current base font size.
  pub fn base_size(&self) -> f64 {
      self.ivars().base_size.get()
  }

  /// Update the base font size (call reapply after to reflect change).
  pub fn set_base_size(&self, size: f64) {
      self.ivars().base_size.set(size);
  }
  ```

- [ ] **Step 4: Thread `base_size` into the 4 `apply_attribute_runs` calls**

  There are 4 calls in `did_process_editing` (lines ~94 and ~114) and `reapply` (lines ~198 and ~214). Each needs `self.base_size()` as the new last argument.

  Also thread `base_size` into `compute_attribute_runs` calls (2 occurrences, lines ~108 and ~208):
  ```rust
  compute_attribute_runs(&text, &spans, cursor_pos, self.base_size())
  ```

  And the `apply_attribute_runs` calls:
  ```rust
  apply_attribute_runs(text_storage, &text, &runs, &empty_tables, &empty_infos, &scheme, self.base_size())
  // and
  apply_attribute_runs(text_storage, &text, &output.runs, &output.table_infos, &infos, &scheme, self.base_size())
  ```

- [ ] **Step 5: Build clean**

  ```bash
  cargo build 2>&1 | grep "^error"
  ```

  Expected: clean build.

- [ ] **Step 6: Run all tests**

  ```bash
  cargo test 2>&1 | tail -20
  ```

  Expected: all tests pass.

- [ ] **Step 7: Commit**

  ```bash
  git add src/editor/text_storage.rs
  git commit -m "feat: add base_size ivar to MditEditorDelegate, wire into render pipeline"
  ```

---

## Task 5: `app.rs` — State, persistence, action methods

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Add constants (near `THEME_PREF_KEY`, line ~1459)**

  ```rust
  const FONT_SIZE_PREF_KEY: &str = "mditFontSize";
  const DEFAULT_FONT_SIZE: f64 = 16.0;
  const MIN_FONT_SIZE: f64 = 12.0;
  const MAX_FONT_SIZE: f64 = 24.0;
  ```

- [ ] **Step 2: Add `body_font_size` to `AppDelegateIvars` (line 72)**

  ```rust
  struct AppDelegateIvars {
      // ...existing fields...
      /// The user's persisted font size (loaded from NSUserDefaults on launch).
      body_font_size: Cell<f64>,
  }
  ```

  Note: `Cell` is already imported at line 1 (`use std::cell::{Cell, ...}`).

- [ ] **Step 3: Add load/save helpers (near `save_theme_pref`, line ~1459)**

  Use `setObject_forKey` with `NSNumber` (confirmed available, mirrors how other apps use NSUserDefaults in this codebase). Use `stringForKey` + parse for reading (consistent with `load_theme_pref` pattern), or use `doubleForKey` if available:

  ```rust
  /// Persist the user's font size to `NSUserDefaults`.
  fn save_font_size_pref(size: f64) {
      let key = NSString::from_str(FONT_SIZE_PREF_KEY);
      let val = NSString::from_str(&size.to_string());
      unsafe {
          let defaults = NSUserDefaults::standardUserDefaults();
          defaults.setObject_forKey(Some(&*val), &key);
      }
  }

  /// Load the user's font size from `NSUserDefaults`.
  /// Falls back to `DEFAULT_FONT_SIZE` when no value is stored.
  fn load_font_size_pref() -> f64 {
      let key = NSString::from_str(FONT_SIZE_PREF_KEY);
      let stored = unsafe { NSUserDefaults::standardUserDefaults().stringForKey(&key) };
      stored
          .as_deref()
          .and_then(|s| s.to_string().parse::<f64>().ok())
          .unwrap_or(DEFAULT_FONT_SIZE)
  }
  ```

  > **Note:** This stores the value as a String (same pattern as `save_theme_pref`). If the objc2-foundation bindings expose `setDouble:forKey:` / `doubleForKey:` on `NSUserDefaults`, prefer those for cleanliness — but the String approach is safe and consistent.

- [ ] **Step 4: Load font size on startup in `did_finish_launching` (line ~106)**

  After the existing `let pref = load_theme_pref();` line, add:

  ```rust
  let font_size = load_font_size_pref();
  self.ivars().body_font_size.set(font_size);
  ```

- [ ] **Step 5: Add action methods (after `apply_system_mode`, line ~260)**

  ```rust
  // ── Font size ──────────────────────────────────────────────────────────

  #[unsafe(method(increaseFontSize:))]
  fn increase_font_size_action(&self, _sender: &AnyObject) {
      let new_size = (self.ivars().body_font_size.get() + 1.0).min(MAX_FONT_SIZE);
      self.apply_font_size(new_size);
  }

  #[unsafe(method(decreaseFontSize:))]
  fn decrease_font_size_action(&self, _sender: &AnyObject) {
      let new_size = (self.ivars().body_font_size.get() - 1.0).max(MIN_FONT_SIZE);
      self.apply_font_size(new_size);
  }

  #[unsafe(method(resetFontSize:))]
  fn reset_font_size_action(&self, _sender: &AnyObject) {
      self.apply_font_size(DEFAULT_FONT_SIZE);
  }
  ```

- [ ] **Step 6: Add `apply_font_size` helper (near `apply_scheme`, line ~865)**

  ```rust
  /// Apply a new base font size to all open tabs and persist it.
  fn apply_font_size(&self, size: f64) {
      self.ivars().body_font_size.set(size);
      save_font_size_pref(size);

      let tm = self.ivars().tab_manager.borrow();
      for tab in tm.iter() {
          tab.editor_delegate.set_base_size(size);
          if let Some(storage) = unsafe { tab.text_view.textStorage() } {
              tab.editor_delegate.reapply(&storage);
          }
      }
  }
  ```

  > **Note:** Unlike `apply_scheme`, `apply_font_size` calls `reapply` on **all** tabs (not just the active one). Font size affects layout dimensions, so invisible tabs that render at the old size will look wrong when the user switches to them.

- [ ] **Step 7: Apply font size to ALL new tabs at creation time**

  The cleanest fix is to apply the persisted font size inside `add_empty_tab` (line ~855 in `app.rs`), right after the tab's `DocumentState` is created. Find the `add_empty_tab` method and add a `set_base_size` call on the new tab's delegate:

  ```rust
  fn add_empty_tab(&self) {
      // ...existing tab creation logic...
      // After the new DocumentState is pushed into the tab manager:
      let font_size = self.ivars().body_font_size.get();
      // The just-added tab is the last one:
      let tm = self.ivars().tab_manager.borrow();
      if let Some(tab) = tm.get(tm.len() - 1) {
          tab.editor_delegate.set_base_size(font_size);
      }
      // ...rest of existing logic (switching to new tab, etc.)...
  }
  ```

  This ensures every tab — opened via Cmd+N, drag-and-drop, startup file, or `open_file_by_path` — inherits the current `body_font_size` immediately.

  > **Note:** The `body_font_size` ivar defaults to 0.0 until `did_finish_launching` sets it. The `add_empty_tab` call in `did_finish_launching` happens *after* `self.ivars().body_font_size.set(font_size)` (Step 4), so the order is safe.

  Also remove the startup-only workaround from the previous plan version (it is superseded by this per-creation approach).

- [ ] **Step 8: Build**

  ```bash
  cargo build 2>&1 | grep "^error"
  ```

  Expected: clean build.

- [ ] **Step 9: Commit**

  ```bash
  git add src/app.rs
  git commit -m "feat: add font size state, persistence, and action methods to AppDelegate"
  ```

---

## Task 6: `menu.rs` — Add font size menu items

**Files:**
- Modify: `src/menu.rs:159` (end of `view_menu` function)

- [ ] **Step 1: Add items before `wrap_in_top_item` (line 159)**

  ```rust
  fn view_menu(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
      // ...existing items...
      menu.addItem(&appearance_item);

      // Font size
      menu.addItem(&NSMenuItem::separatorItem(mtm));
      menu.addItem(&with_cmd(item("Increase Font Size", Some(sel!(increaseFontSize:)), "+", mtm)));
      menu.addItem(&with_cmd(item("Decrease Font Size", Some(sel!(decreaseFontSize:)), "-", mtm)));
      menu.addItem(&with_cmd(item("Default Font Size", Some(sel!(resetFontSize:)), "0", mtm)));

      wrap_in_top_item("View", menu, mtm)
  }
  ```

  > **Note:** The key equivalent `"+"` maps to Cmd++ on macOS (the `+` character). The `"-"` maps to Cmd+–. Both are standard for font size in macOS apps.

- [ ] **Step 2: Build**

  ```bash
  cargo build 2>&1 | grep "^error"
  ```

  Expected: clean build.

- [ ] **Step 3: Run all tests**

  ```bash
  cargo test 2>&1 | tail -10
  ```

  Expected: all tests pass.

- [ ] **Step 4: Commit**

  ```bash
  git add src/menu.rs
  git commit -m "feat: add font size menu items to View menu (Cmd++/Cmd+-/Cmd+0)"
  ```

---

## Task 7: Manual Verification

- [ ] **Step 1: Run the app**

  ```bash
  cargo run
  ```

- [ ] **Step 2: Editor mode — font size change**

  Press Cmd+E to enter Editor mode. Press Cmd++ several times. Confirm monospace body text gets larger.

- [ ] **Step 3: Viewer mode — proportional headings**

  Open a markdown file with # H1 / ## H2 / ### H3 headings. Press Cmd+E to go to Viewer mode. Press Cmd++ twice. Confirm:
  - Body text is 18pt
  - H1 is larger, H2 is medium, H3 matches body (differs only in color)
  - Inline code is 2pt smaller than body (16pt)

- [ ] **Step 4: Persistence**

  Change font to 20pt. Quit (Cmd+Q). Relaunch. Confirm font is still 20pt.

- [ ] **Step 5: Reset**

  Press Cmd+0. Confirm font returns to 16pt.

- [ ] **Step 6: Boundary clamp**

  From 16pt, press Cmd+– eleven times. Confirm font stops at 12pt (no crash, no change below 12pt).

  From 12pt, press Cmd++ thirteen times. Confirm font stops at 24pt.

- [ ] **Step 7: Update FINISHING.md**

  Mark the "Schriftgröße konfigurierbar" checkbox as done:

  ```markdown
  - [x] **Schriftgröße konfigurierbar (Cmd++ / Cmd+–)**
  ```

  ```bash
  git add FINISHING.md
  git commit -m "docs: mark font size feature complete in FINISHING.md"
  ```
