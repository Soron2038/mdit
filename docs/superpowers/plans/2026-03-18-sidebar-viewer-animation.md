# Sidebar Viewer-Mode Animation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hide the sidebar in Viewer mode and animate it in/out (0.35s ease-in-out) when switching between Viewer and Editor modes.

**Architecture:** Add two pure frame-helper functions (`sidebar_target_frame`, `content_target_frame`) that compute NSRect based on ViewMode, then wire them into all layout call sites. `toggle_mode()` wraps the frame changes in `NSAnimationContext::runAnimationGroup_completionHandler`; all other call sites (resize, tab switch, initial setup) set frames directly without animation. The sidebar container gets layer backing + `masksToBounds = true` so buttons clip cleanly during the slide.

**Tech Stack:** Rust, objc2 0.6.3, objc2-app-kit 0.3.2 (+ new `NSAnimation`/`NSAnimationContext` features), objc2-quartz-core 0.3.2 (new direct dep, `CAMediaTimingFunction` feature), block2 0.6 (new direct dep).

**Spec:** `docs/superpowers/specs/2026-03-18-sidebar-viewer-animation-design.md`

---

## File Map

| File | Change |
| ---- | ------ |
| `Cargo.toml` | Add `block2`, `objc2-quartz-core` deps; add `NSAnimation`, `NSAnimationContext` to `objc2-app-kit` features |
| `src/ui/sidebar.rs` | Add `wantsLayer`+`masksToBounds` on container; add `set_size_direct(width, height)` method |
| `src/app.rs` | Add `sidebar_target_frame` + `content_target_frame` free fns; update `content_frame()`, `setup_content_views()`, `toggle_mode()`, `update_text_container_inset()`, `switch_to_tab()`, `windowDidResize()`, `close_tab()` |

---

## Task 1: Add Cargo.toml dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add `block2` and `objc2-quartz-core` direct dependencies, and `NSAnimation`/`NSAnimationContext` features to `objc2-app-kit`**

In `Cargo.toml`, add two new top-level deps (at any position inside `[dependencies]`):

```toml
block2 = "0.6"
objc2-quartz-core = { version = "0.3", features = ["CAMediaTimingFunction"] }
```

Then update the `objc2-app-kit` entry — add four features (`NSAnimation`, `NSAnimationContext`, `block2`, `objc2-quartz-core`). The last two unlock `runAnimationGroup_completionHandler` and `setTimingFunction` respectively:

```toml
objc2-app-kit = { version = "0.3.2", features = [
    "NSApplication",
    "NSWindow", "NSWindowController",
    "NSGraphics",
    "NSTextView", "NSTextStorage", "NSLayoutManager", "NSTextContainer",
    "NSScrollView",
    "NSDocument", "NSDocumentController",
    "NSColor", "NSFont", "NSFontDescriptor",
    "NSPanel", "NSVisualEffectView",
    "NSButton", "NSButtonCell", "NSControl",
    "NSPrintOperation", "NSPrintInfo",
    "NSMenu", "NSMenuItem", "NSEvent",
    "NSParagraphStyle", "NSText",
    "NSResponder", "NSView",
    "NSTextAttachment",
    "NSAttributedString", "NSAppearance",
    "NSTextField", "NSOpenPanel", "NSSavePanel", "NSAlert", "NSBox",
    "NSBezierPath",
    "NSPasteboard",
    "NSImage",
    "NSAnimation",
    "NSAnimationContext",
    "block2",
    "objc2-quartz-core",
] }
```

- [ ] **Step 2: Verify the build compiles**

```bash
cd /Users/witt/Developer/mdit && cargo build 2>&1 | tail -20
```

Expected: `Finished` with no errors. (Warnings are OK.)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add block2, objc2-quartz-core, NSAnimationContext deps"
```

---

## Task 2: Sidebar — layer backing and `set_size_direct()`

**Files:**
- Modify: `src/ui/sidebar.rs`

The sidebar container needs `wantsLayer = true` and `masksToBounds = true` so buttons clip correctly when the container width is animated from 36→0. We also add `set_size_direct(width, height)` as a non-animated way to set both dimensions from outside (used by resize and tab-switch paths).

- [ ] **Step 1: Add layer backing in `FormattingSidebar::new()`**

First, add the `CALayer` import at the top of `src/ui/sidebar.rs` (alongside the other `objc2_app_kit` / `objc2_quartz_core` imports):

```rust
use objc2_quartz_core::CALayer;
```

Then, at the end of the `FormattingSidebar::new()` body, just before the `let s = Self { ... }` line (currently line ~545), add:

```rust
// Enable layer backing so masksToBounds clips buttons during animation.
container.setWantsLayer(true);
if let Some(layer) = container.layer() {
    layer.setMasksToBounds(true);
}
```

`setMasksToBounds` is a typed method on `CALayer` (from `objc2-quartz-core`), so no raw `msg_send!` is needed here.

- [ ] **Step 2: Add `set_size_direct()` method to `FormattingSidebar`**

In `src/ui/sidebar.rs`, add this method to the `impl FormattingSidebar` block (after `set_height`, before `view()`):

```rust
/// Directly set the container width and height without animation.
///
/// Use for tab-switch and window-resize paths. Internal subview heights are
/// updated; the sidebar button view and border keep their fixed x-positions
/// and are clipped by the container's layer mask.
pub fn set_size_direct(&self, width: f64, height: f64) {
    // Container frame (origin unchanged — caller is responsible for setFrame
    // on the container from outside, or we set just the size here).
    let mut f = self.container.frame();
    f.size.width = width;
    f.size.height = height;
    self.container.setFrame(f);

    // Internal views: update height only (x/width stay at their SIDEBAR_W-based positions).
    let mut sf = self.sidebar_view.frame();
    sf.size.height = height;
    self.sidebar_view.setFrame(sf);
    self.sidebar_view.compute_button_origins(height);

    let mut bf = self.border.frame();
    bf.size.height = height;
    self.border.setFrame(bf);

    let _: () = unsafe { msg_send![&*self.sidebar_view, setNeedsDisplay: true] };
}
```

Note: `set_height()` (the existing method) remains unchanged; `windowDidResize` will be updated in Task 8 to call `set_size_direct` instead.

- [ ] **Step 3: Build to verify no compile errors**

```bash
cd /Users/witt/Developer/mdit && cargo build 2>&1 | tail -20
```

Expected: `Finished`.

- [ ] **Step 4: Commit**

```bash
git add src/ui/sidebar.rs
git commit -m "feat(sidebar): add layer backing, masksToBounds, and set_size_direct()"
```

---

## Task 3: Add frame helper functions with unit tests

**Files:**
- Modify: `src/app.rs`

Two pure functions that compute `NSRect` based on `ViewMode`. Because they only touch `NSRect`/`NSPoint`/`NSSize` (pure C structs) and the `ViewMode` enum (pure Rust), they are fully unit-testable without an AppKit runtime.

- [ ] **Step 1: Add the two helper functions to `app.rs`**

In `src/app.rs`, add these two free functions **before** the `impl AppDelegate` block (e.g., right after the layout constants block, around line 33):

```rust
/// Frame for the sidebar container for a given mode.
///
/// - Viewer → width 0 (hidden)
/// - Editor → width SIDEBAR_W (visible)
fn sidebar_target_frame(mode: ViewMode, content_h: f64) -> NSRect {
    let w = if mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
    NSRect::new(
        NSPoint::new(0.0, PATH_H),
        NSSize::new(w, content_h),
    )
}

/// Frame for the active NSScrollView for a given mode.
///
/// - Viewer → x: 0, full window width
/// - Editor → x: SIDEBAR_W, reduced width
fn content_target_frame(mode: ViewMode, win_w: f64, win_h: f64) -> NSRect {
    let sidebar_w = if mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
    NSRect::new(
        NSPoint::new(sidebar_w, PATH_H),
        NSSize::new((win_w - sidebar_w).max(0.0), (win_h - TAB_H - PATH_H).max(0.0)),
    )
}
```

- [ ] **Step 2: Write failing unit tests**

At the very bottom of `src/app.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mdit::editor::view_mode::ViewMode;

    #[test]
    fn sidebar_frame_viewer_is_zero_width() {
        let f = sidebar_target_frame(ViewMode::Viewer, 500.0);
        assert_eq!(f.size.width, 0.0);
        assert_eq!(f.size.height, 500.0);
        assert_eq!(f.origin.y, PATH_H);
    }

    #[test]
    fn sidebar_frame_editor_is_sidebar_w() {
        let f = sidebar_target_frame(ViewMode::Editor, 500.0);
        assert_eq!(f.size.width, SIDEBAR_W);
        assert_eq!(f.size.height, 500.0);
    }

    #[test]
    fn content_frame_viewer_starts_at_zero() {
        let f = content_target_frame(ViewMode::Viewer, 1000.0, 700.0);
        assert_eq!(f.origin.x, 0.0);
        assert_eq!(f.size.width, 1000.0);
    }

    #[test]
    fn content_frame_editor_offset_by_sidebar() {
        let f = content_target_frame(ViewMode::Editor, 1000.0, 700.0);
        assert_eq!(f.origin.x, SIDEBAR_W);
        assert_eq!(f.size.width, 1000.0 - SIDEBAR_W);
    }

    #[test]
    fn content_frame_height_excludes_bars() {
        let f = content_target_frame(ViewMode::Viewer, 800.0, 700.0);
        assert_eq!(f.origin.y, PATH_H);
        assert_eq!(f.size.height, 700.0 - TAB_H - PATH_H);
    }
}
```

- [ ] **Step 3: Run tests — expect FAIL (functions not yet called, but tests compile)**

```bash
cd /Users/witt/Developer/mdit && cargo test 2>&1 | tail -30
```

Expected: Tests compile and **pass** (the functions are pure and correct). If they fail, fix the frame helpers before continuing.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): add sidebar_target_frame and content_target_frame helpers with tests"
```

---

## Task 4: Update `content_frame()` and `setup_content_views()` for initial Viewer state

**Files:**
- Modify: `src/app.rs`

`content_frame()` currently hardcodes `SIDEBAR_W`. Make it mode-aware by delegating to `content_target_frame`. Then fix `setup_content_views()` so the sidebar starts at width 0 (Viewer mode default).

- [ ] **Step 1: Update `content_frame()` to be mode-aware**

In `src/app.rs`, replace the existing `content_frame()` method (lines ~401-416):

```rust
// OLD:
/// Frame for the active NSScrollView, positioned between the tab bar and path bar.
///
/// The sidebar is always visible, so the left edge is always offset by `SIDEBAR_W`.
/// Returns `NSRect::ZERO` if the window is not yet initialised.
fn content_frame(&self) -> NSRect {
    let Some(win) = self.ivars().window.get() else {
        return NSRect::ZERO;
    };
    let bounds = win.contentView().unwrap().bounds();
    let h = bounds.size.height;
    let w = bounds.size.width;
    NSRect::new(
        NSPoint::new(SIDEBAR_W, PATH_H),
        NSSize::new((w - SIDEBAR_W).max(0.0), (h - TAB_H - PATH_H).max(0.0)),
    )
}
```

Replace with:

```rust
/// Frame for the active NSScrollView, positioned between the tab bar and path bar.
///
/// Returns `NSRect::ZERO` if the window is not yet initialised.
/// In Viewer mode the sidebar is hidden, so the frame starts at x:0 and uses full width.
fn content_frame(&self) -> NSRect {
    let Some(win) = self.ivars().window.get() else {
        return NSRect::ZERO;
    };
    let bounds = win.contentView().unwrap().bounds();
    let mode = self.ivars().tab_manager.borrow()
        .active()
        .map(|t| t.mode.get())
        .unwrap_or(ViewMode::Viewer);
    content_target_frame(mode, bounds.size.width, bounds.size.height)
}
```

- [ ] **Step 2: Update `setup_content_views()` — sidebar starts hidden**

In `src/app.rs`, find the sidebar setup block (lines ~386-394):

```rust
// OLD:
let sidebar = FormattingSidebar::new(mtm, content_h, target);
sidebar.view().setFrame(NSRect::new(
    NSPoint::new(0.0, PATH_H),
    NSSize::new(SIDEBAR_W, content_h),
));
// Sidebar is always visible regardless of mode.
content.addSubview(sidebar.view());
```

Replace with:

```rust
// New tabs start in Viewer mode, so the sidebar begins hidden (width = 0).
let sidebar = FormattingSidebar::new(mtm, content_h, target);
sidebar.view().setFrame(NSRect::new(
    NSPoint::new(0.0, PATH_H),
    NSSize::new(0.0, content_h),
));
content.addSubview(sidebar.view());
```

Also remove the stale comment above this block ("// Sidebar is always visible regardless of mode.") and update the doc comment on `content_frame()` if you haven't already.

- [ ] **Step 3: Remove stale `SIDEBAR_W` from the import if it becomes unused after all tasks complete** — skip for now, will be addressed in the final task.

- [ ] **Step 4: Build to verify**

```bash
cd /Users/witt/Developer/mdit && cargo build 2>&1 | tail -20
```

Expected: `Finished`. The sidebar will now start hidden when the app launches (visible after the next task).

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): content_frame() is now mode-aware; sidebar hidden on startup (Viewer mode)"
```

---

## Task 5: Animate `toggle_mode()` with NSAnimationContext

**Files:**
- Modify: `src/app.rs`

Replace the immediate `setFrame` call in `toggle_mode()` with an `NSAnimationContext` animation block that moves both the sidebar and the scroll view simultaneously.

- [ ] **Step 1: Add imports for animation APIs**

At the top of `src/app.rs`, update the `use objc2_app_kit::` block to include `NSAnimationContext`:

```rust
use objc2_app_kit::{
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSApplicationActivationPolicy,
    NSApplicationDelegate, NSBackingStoreType, NSBezelStyle, NSButton, NSColor, NSControl,
    NSImage, NSTextDelegate, NSTextView, NSTextViewDelegate, NSView, NSWindow,
    NSWindowDelegate, NSWindowStyleMask,
    NSAnimationContext,  // ← add this
};
```

Add new `use` statements for `objc2-quartz-core` and `block2`:

```rust
use objc2_quartz_core::CAMediaTimingFunction;
use block2::StackBlock;
use std::ptr::NonNull;
```

You will also need to import the timing constant. Check the exact path in the generated bindings:

```bash
grep -r "kCAMediaTimingFunctionEaseInEaseOut" ~/.cargo/registry/src/*/objc2-quartz-core-*/src/ 2>/dev/null | head -5
```

The constant is typically at `objc2_quartz_core::kCAMediaTimingFunctionEaseInEaseOut` or inside a submodule. Add the appropriate `use` line. Example:

```rust
use objc2_quartz_core::kCAMediaTimingFunctionEaseInEaseOut;
// If the above doesn't resolve, try:
// use objc2_quartz_core::generated::CAMediaTimingFunction::kCAMediaTimingFunctionEaseInEaseOut;
```

- [ ] **Step 2: Rewrite `toggle_mode()` with animated frame changes**

Replace the current `toggle_mode()` implementation (lines ~425-462) with:

```rust
/// Toggle between Viewer and Editor mode for the active tab.
fn toggle_mode(&self) {
    // ── 1. Collect state ──────────────────────────────────────────────────
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

    // ── 2. Non-visual changes (immediate) ─────────────────────────────────
    editor_delegate.set_mode(new_mode);
    text_view.setEditable(new_mode == ViewMode::Editor);
    if let Some(storage) = unsafe { text_view.textStorage() } {
        editor_delegate.reapply(&storage);
    }
    self.update_text_container_inset();

    // ── 3. Compute target frames ───────────────────────────────────────────
    let Some(win) = self.ivars().window.get() else { return };
    let bounds = win.contentView().unwrap().bounds();
    let (win_w, win_h) = (bounds.size.width, bounds.size.height);
    let content_h = (win_h - TAB_H - PATH_H).max(0.0);

    let target_sb_frame = sidebar_target_frame(new_mode, content_h);
    let target_sv_frame = content_target_frame(new_mode, win_w, win_h);

    // ── 4. Animated frame changes ──────────────────────────────────────────
    let Some(sb) = self.ivars().sidebar.get() else { return };

    // Use raw pointers for view references captured by the animation block.
    // Safety: both views are owned by long-lived structs (FormattingSidebar
    // lives in OnceCell, scroll_view is kept alive by the Retained below).
    // The changes block is called synchronously before runAnimationGroup returns.
    let sb_ptr: *const NSView = sb.view();
    let sv_ptr: *const objc2_app_kit::NSScrollView = &*scroll_view;

    // Bind to a named variable to avoid temporary lifetime issues.
    let animation_block = StackBlock::new(move |ctx: NonNull<NSAnimationContext>| {
        // Safety: ctx is a valid NSAnimationContext pointer provided by AppKit.
        let ctx = unsafe { ctx.as_ref() };
        ctx.setDuration(0.35);
        let timing = unsafe {
            CAMediaTimingFunction::functionWithName(kCAMediaTimingFunctionEaseInEaseOut)
        };
        unsafe { ctx.setTimingFunction(Some(&*timing)) };

        // Animate sidebar container via raw msg_send on the animator proxy.
        // (The animator proxy is an opaque AnyObject, not a typed NSView.)
        let sb_proxy: *const AnyObject = unsafe { msg_send![sb_ptr, animator] };
        let _: () = unsafe { msg_send![sb_proxy, setFrame: target_sb_frame] };

        // Animate scroll view
        let sv_proxy: *const AnyObject = unsafe { msg_send![sv_ptr, animator] };
        let _: () = unsafe { msg_send![sv_proxy, setFrame: target_sv_frame] };
    });
    unsafe {
        NSAnimationContext::runAnimationGroup_completionHandler(
            &animation_block,
            None::<&block2::DynBlock<dyn Fn()>>,
        );
    }
}
```

Note: `scroll_view` (the `Retained<NSScrollView>` clone) is NOT moved into the block — it stays alive in the outer scope for the duration of the synchronous block call, keeping `sv_ptr` valid.

- [ ] **Step 3: Build**

```bash
cd /Users/witt/Developer/mdit && cargo build 2>&1 | tail -30
```

Expected: `Finished`. Common issues:
- If `kCAMediaTimingFunctionEaseInEaseOut` doesn't resolve, run the grep command from Step 1 to find the correct import path.
- If `StackBlock` lifetime errors occur, try adding an explicit lifetime annotation to the closure parameter.

- [ ] **Step 4: Manual smoke test**

```bash
cd /Users/witt/Developer/mdit && cargo run
```

1. App opens in Viewer mode — sidebar is **not visible**
2. Press Cmd+E — sidebar slides in from the left, text shrinks simultaneously (0.35s)
3. Press Cmd+E again — sidebar slides out, text expands (0.35s)

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): animate sidebar in/out with NSAnimationContext on mode toggle"
```

---

## Task 6: Fix `update_text_container_inset()` — mode-aware width

**Files:**
- Modify: `src/app.rs`

The current implementation subtracts `SIDEBAR_W` unconditionally, making the text column 36pt too narrow in Viewer mode.

- [ ] **Step 1: Update `update_text_container_inset()`**

Find the existing function (lines ~774-795):

```rust
// OLD line 782:
let editor_width = (win.frame().size.width - SIDEBAR_W).max(0.0);
```

Replace that single line:

```rust
let effective_sidebar_w = if self.is_editor_mode() { SIDEBAR_W } else { 0.0 };
let editor_width = (win.frame().size.width - effective_sidebar_w).max(0.0);
```

- [ ] **Step 2: Build and run**

```bash
cd /Users/witt/Developer/mdit && cargo build 2>&1 | tail -5 && cargo run
```

Verify: In Viewer mode at various window widths, the text column is properly centred (not 36pt off-centre).

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "fix(app): update_text_container_inset uses effective sidebar width per mode"
```

---

## Task 7: Fix `switch_to_tab()` — snap sidebar on tab switch

**Files:**
- Modify: `src/app.rs`

When switching tabs, the sidebar should snap immediately (no animation) to match the new tab's mode.

- [ ] **Step 1: Add sidebar frame update to `switch_to_tab()`**

In `src/app.rs`, find `switch_to_tab()` at line ~556. After the block that inserts the new scroll view (after line ~580):

```rust
// INSERT after the scroll view is added to the content view:
// Snap sidebar to the new tab's mode without animation.
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
```

Place this block right before the `// Update path bar` comment (around line 582).

- [ ] **Step 2: Build and test with multiple tabs**

```bash
cd /Users/witt/Developer/mdit && cargo build 2>&1 | tail -5 && cargo run
```

1. Open a new tab (Cmd+N) — starts in Viewer (no sidebar)
2. Toggle to Editor on tab 1 (Cmd+E) — sidebar appears
3. Switch to tab 2 — sidebar instantly hides (no animation)
4. Switch back to tab 1 — sidebar instantly shows

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "fix(app): snap sidebar frame on tab switch without animation"
```

---

## Task 8: Fix `windowDidResize()` and `close_tab()` — mode-aware sidebar

**Files:**
- Modify: `src/app.rs`

`windowDidResize` only updates the sidebar height via `set_height`. Replace with `set_size_direct` using the current mode's width. Also fix the `close_tab` LastTab path which re-adds the scroll view but never updates the sidebar frame.

- [ ] **Step 1: Update `windowDidResize()`**

In `src/app.rs`, find the sidebar update inside `windowDidResize:` (lines ~124-127):

```rust
// OLD:
if let Some(sb) = self.ivars().sidebar.get() {
    let content_h = (h - TAB_H - PATH_H).max(0.0);
    sb.set_height(content_h);
}
```

Replace with:

```rust
if let Some(sb) = self.ivars().sidebar.get() {
    let content_h = (h - TAB_H - PATH_H).max(0.0);
    let mode = self.ivars().tab_manager.borrow()
        .active()
        .map(|t| t.mode.get())
        .unwrap_or(ViewMode::Viewer);
    let sidebar_w = if mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
    sb.set_size_direct(sidebar_w, content_h);
}
```

- [ ] **Step 2: Fix `close_tab()` LastTab path**

In `src/app.rs`, find the `TabCloseResult::LastTab` arm inside `close_tab()` (around line 673). After the scroll view is re-added to the content view (line ~689):

```rust
t.scroll_view.setFrame(self.content_frame());
content.addSubview(&t.scroll_view);
```

Add sidebar frame update immediately after. **Important:** do not use `return` inside the `tm` borrow scope — read what you need without an early return. `self.ivars().window.get()` is in a separate `OnceCell` field and can be accessed while `tm` is borrowed.

```rust
// Snap sidebar to the remaining tab's mode.
// Read the mode while t/tm is still in scope (no early return here).
let remaining_mode = t.mode.get();
if let (Some(sb), Some(win)) = (self.ivars().sidebar.get(), self.ivars().window.get()) {
    let bounds = win.contentView().unwrap().bounds();
    let content_h = (bounds.size.height - TAB_H - PATH_H).max(0.0);
    let sidebar_w = if remaining_mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
    sb.set_size_direct(sidebar_w, content_h);
}
```

Place this block immediately after `content.addSubview(&t.scroll_view);`, still inside the `if let Some(t) = tm.active()` block.

- [ ] **Step 3: Build**

```bash
cd /Users/witt/Developer/mdit && cargo build 2>&1 | tail -10
```

Expected: `Finished`.

- [ ] **Step 4: Run tests to confirm nothing regressed**

```bash
cd /Users/witt/Developer/mdit && cargo test 2>&1 | tail -15
```

Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "fix(app): windowDidResize and close_tab respect sidebar visibility per mode"
```

---

## Task 9: End-to-end verification and cleanup

**Files:**
- Modify: `src/app.rs` (minor: remove any stale comments referencing always-visible sidebar)

- [ ] **Step 1: Full build and test run**

```bash
cd /Users/witt/Developer/mdit && cargo build && cargo test 2>&1 | tail -20
```

Expected: `Finished` + all tests pass.

- [ ] **Step 2: End-to-end manual checklist**

Launch the app and verify each scenario:

```bash
cargo run
```

Checklist:
1. ☐ App starts → sidebar is **not visible** (Viewer mode, full-width text)
2. ☐ Cmd+E → sidebar slides in from the left, text area contracts simultaneously, ~0.35s
3. ☐ Cmd+E again → sidebar slides out to the left, text area expands simultaneously, ~0.35s
4. ☐ In Viewer mode: text column is correctly centred (not 36pt off-centre) at various window widths
5. ☐ Resize window in Viewer mode → layout correct, no sidebar visible
6. ☐ Resize window in Editor mode → sidebar remains visible and correctly sized
7. ☐ Open two tabs (Cmd+N); toggle tab 1 to Editor mode; switch to tab 2 (Viewer) → sidebar snaps hidden instantly
8. ☐ Switch back to tab 1 (Editor) → sidebar snaps visible instantly
9. ☐ Close tab to last tab → sidebar state correct for remaining tab
10. ☐ Rapid Cmd+E 5× quickly → no animation artefacts, final state is correct

- [ ] **Step 3: Remove stale comment about sidebar always-visible**

In `src/app.rs` check for any remaining comment saying "Sidebar is always visible" and remove it. Also check `src/ui/sidebar.rs` module doc at line 1 which says "Permanent left-margin formatting sidebar" — update if desired (optional).

- [ ] **Step 4: Run clippy to catch any warnings**

```bash
cd /Users/witt/Developer/mdit && cargo clippy 2>&1 | grep -E "^error|^warning.*src/"
```

Fix any `error`-level clippy lints. `SIDEBAR_W` remains in use throughout `app.rs` (Tasks 7 and 8 inline code), so its import should stay.

- [ ] **Step 5: Final commit**

```bash
git add src/app.rs src/ui/sidebar.rs
git commit -m "chore: cleanup stale comments after sidebar viewer-mode animation"
```

---

## Quick Reference

| When | How |
| ---- | --- |
| Mode toggle (Cmd+E) | `NSAnimationContext` 0.35s ease-in-out |
| Tab switch | `sb.set_size_direct(w, h)` — immediate |
| Window resize | `sb.set_size_direct(w, h)` — immediate |
| App start / new tab | Frame set directly in `setup_content_views` — immediate |
| Close last tab | `sb.set_size_direct(w, h)` — immediate |
