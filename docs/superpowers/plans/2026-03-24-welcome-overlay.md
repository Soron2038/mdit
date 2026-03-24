# Welcome Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a centered welcome screen in empty documents that makes Cmd+E and other key shortcuts discoverable.

**Architecture:** New `WelcomeOverlay` UI component (NSView with NSTextField labels) added as a subview above the scroll view. Visibility controlled by a helper method that checks `textStorage.length == 0`, called from all text-change and tab-change code paths. Mouse events pass through so clicks reach the text view underneath.

**Tech Stack:** Rust, objc2/objc2-app-kit, AppKit (NSView, NSTextField, NSFont, NSColor)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/ui/welcome_overlay.rs` | Create | WelcomeOverlay struct: container NSView with labels, visibility, frame, mode updates |
| `src/ui/mod.rs` | Modify | Add `pub mod welcome_overlay;` |
| `src/app.rs` | Modify | Add ivar, create in `setup_content_views`, add `update_welcome_visibility()`, wire 6 call sites, update `windowDidResize:` |

---

### Task 1: Create the WelcomeOverlay module

**Files:**
- Create: `src/ui/welcome_overlay.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Create `src/ui/welcome_overlay.rs` with the full implementation**

```rust
//! Welcome overlay shown in empty documents.
//!
//! Displays the app name, tagline, keyboard shortcuts, and a
//! mode-dependent hint. Automatically adapts to Light/Dark mode
//! via system label colors.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, MainThreadOnly};
use objc2_app_kit::{NSColor, NSFont, NSTextField, NSView};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use crate::editor::view_mode::ViewMode;

// ---------------------------------------------------------------------------
// PassthroughView — NSView subclass that ignores all mouse events
// ---------------------------------------------------------------------------

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    pub struct PassthroughView;

    unsafe impl NSObjectProtocol for PassthroughView {}

    impl PassthroughView {
        /// Return nil so clicks pass through to the text view underneath.
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut AnyObject {
            std::ptr::null_mut()
        }
    }
);

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const TITLE_H: f64 = 50.0;
const TAGLINE_H: f64 = 20.0;
const GAP_AFTER_TAGLINE: f64 = 24.0;
const SHORTCUT_LINE_H: f64 = 22.0;
const SHORTCUT_COUNT: usize = 4;
const GAP_AFTER_SHORTCUTS: f64 = 20.0;
const HINT_H: f64 = 16.0;

// ---------------------------------------------------------------------------
// WelcomeOverlay
// ---------------------------------------------------------------------------

pub struct WelcomeOverlay {
    container: Retained<NSView>,
    hint_field: Retained<NSTextField>,
}

impl WelcomeOverlay {
    /// Create the overlay with all labels.  Starts **hidden**.
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Self {
        let container: Retained<NSView> = unsafe {
            let obj = PassthroughView::alloc(mtm).set_ivars(());
            msg_send![super(obj), initWithFrame: frame]
        };

        // ── Title: "mdit" ───────────────────────────────────────────────
        let title = Self::make_label(mtm, "mdit", 42.0, true);
        title.setTextColor(Some(&NSColor::secondaryLabelColor()));
        container.addSubview(&title);

        // ── Tagline ─────────────────────────────────────────────────────
        let tagline = Self::make_label(mtm, "A native Markdown editor for macOS", 13.0, false);
        tagline.setTextColor(Some(&NSColor::tertiaryLabelColor()));
        container.addSubview(&tagline);

        // ── Shortcut list ───────────────────────────────────────────────
        let shortcuts = [
            "\u{2318}E    Toggle Editor / Viewer",
            "\u{2318}F    Find & Replace",
            "\u{2318}T    New Tab",
            "\u{2318}+/\u{2212}  Adjust Font Size",
        ];
        for text in &shortcuts {
            let label = Self::make_mono_label(mtm, text, 12.0);
            label.setTextColor(Some(&NSColor::tertiaryLabelColor()));
            container.addSubview(&label);
        }

        // ── Hint line (mode-dependent) ──────────────────────────────────
        let hint_field = Self::make_label(mtm, "", 11.0, false);
        hint_field.setTextColor(Some(&NSColor::quaternaryLabelColor()));
        container.addSubview(&hint_field);

        container.setHidden(true);

        let overlay = Self { container, hint_field };
        overlay.layout_labels(frame);
        overlay.update_mode(ViewMode::Viewer);
        overlay
    }

    /// Show or hide the overlay.
    pub fn set_visible(&self, visible: bool) {
        self.container.setHidden(!visible);
    }

    /// Update the overlay frame (called from `windowDidResize:`).
    pub fn set_frame(&self, frame: NSRect) {
        self.container.setFrame(frame);
        self.layout_labels(frame);
    }

    /// Update the mode-dependent hint text.
    pub fn update_mode(&self, mode: ViewMode) {
        let text = match mode {
            ViewMode::Editor => "Just start typing to begin",
            ViewMode::Viewer => "Press \u{2318}E to start editing, or \u{2318}O to open a file",
        };
        self.hint_field.setStringValue(&NSString::from_str(text));
    }

    /// Access the underlying NSView for adding to the view hierarchy.
    pub fn view(&self) -> &NSView {
        &self.container
    }

    // ── Private helpers ─────────────────────────────────────────────────

    /// Position all child labels centered in the given frame.
    fn layout_labels(&self, frame: NSRect) {
        let w = frame.size.width;
        let h = frame.size.height;
        let label_w = w.min(400.0);
        let x = (w - label_w) / 2.0;

        // Compute total block height to center vertically.
        let shortcuts_h = SHORTCUT_COUNT as f64 * SHORTCUT_LINE_H;
        let total = TITLE_H + TAGLINE_H + GAP_AFTER_TAGLINE
            + shortcuts_h + GAP_AFTER_SHORTCUTS + HINT_H;
        let mut y = (h + total) / 2.0;

        let subviews = unsafe { self.container.subviews() };
        let count = subviews.len();

        // Title (index 0)
        y -= TITLE_H;
        if count > 0 {
            subviews[0].setFrame(NSRect::new(
                NSPoint::new(x, y),
                NSSize::new(label_w, TITLE_H),
            ));
        }

        // Tagline (index 1)
        y -= TAGLINE_H;
        if count > 1 {
            subviews[1].setFrame(NSRect::new(
                NSPoint::new(x, y),
                NSSize::new(label_w, TAGLINE_H),
            ));
        }

        // Gap
        y -= GAP_AFTER_TAGLINE;

        // Shortcuts (indices 2..6)
        for i in 0..SHORTCUT_COUNT {
            y -= SHORTCUT_LINE_H;
            if count > 2 + i {
                subviews[2 + i].setFrame(NSRect::new(
                    NSPoint::new(x, y),
                    NSSize::new(label_w, SHORTCUT_LINE_H),
                ));
            }
        }

        // Gap
        y -= GAP_AFTER_SHORTCUTS;

        // Hint (last subview)
        y -= HINT_H;
        if count > 6 {
            subviews[6].setFrame(NSRect::new(
                NSPoint::new(x, y),
                NSSize::new(label_w, HINT_H),
            ));
        }
    }

    /// Create a non-editable, borderless NSTextField with system font.
    fn make_label(
        mtm: MainThreadMarker,
        text: &str,
        size: f64,
        light_weight: bool,
    ) -> Retained<NSTextField> {
        let field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
        );
        field.setEditable(false);
        field.setSelectable(false);
        field.setBordered(false);
        field.setDrawsBackground(false);
        if light_weight {
            field.setFont(Some(&NSFont::systemFontOfSize_weight(size, -0.4)));
        } else {
            field.setFont(Some(&NSFont::systemFontOfSize(size)));
        }
        field.setStringValue(&NSString::from_str(text));
        unsafe { let _: () = msg_send![&*field, setAlignment: 1_isize]; } // NSTextAlignmentCenter
        field
    }

    /// Create a non-editable, borderless NSTextField with monospace font.
    fn make_mono_label(
        mtm: MainThreadMarker,
        text: &str,
        size: f64,
    ) -> Retained<NSTextField> {
        let field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
        );
        field.setEditable(false);
        field.setSelectable(false);
        field.setBordered(false);
        field.setDrawsBackground(false);
        field.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(size, 0.0)));
        field.setStringValue(&NSString::from_str(text));
        unsafe { let _: () = msg_send![&*field, setAlignment: 1_isize]; } // NSTextAlignmentCenter
        field
    }
}
```

- [ ] **Step 2: Register the module in `src/ui/mod.rs`**

Add after the existing `pub mod find_bar;` line:

```rust
pub mod welcome_overlay;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Build succeeds (module compiles but is not yet used — may get unused warnings, that's fine).

- [ ] **Step 4: Commit**

```bash
git add src/ui/welcome_overlay.rs src/ui/mod.rs
git commit -m "feat: add WelcomeOverlay UI module"
```

---

### Task 2: Add ivar and create overlay in AppDelegate

**Files:**
- Modify: `src/app.rs:1-90` (imports and AppDelegateIvars)
- Modify: `src/app.rs:579-628` (setup_content_views)

- [ ] **Step 1: Add import**

In the `use` block at the top of `src/app.rs`, add alongside the existing UI imports (near line 28):

```rust
use mdit::ui::welcome_overlay::WelcomeOverlay;
```

- [ ] **Step 2: Add ivar to `AppDelegateIvars`**

Add a new field after `body_font_size` (around line 89):

```rust
    welcome_overlay: OnceCell<WelcomeOverlay>,
```

- [ ] **Step 3: Create overlay in `setup_content_views()`**

In `setup_content_views()`, after the find bar block (after line 628, before the `set` calls), add:

```rust
        // Welcome overlay — shown in empty documents, positioned above scroll view.
        let welcome_overlay = WelcomeOverlay::new(mtm, content_target_frame(
            ViewMode::Viewer, 0.0, w, h,
        ));
        content.addSubview(welcome_overlay.view());
```

And in the `set` block at the end of `setup_content_views()`, add:

```rust
        let _ = self.ivars().welcome_overlay.set(welcome_overlay);
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Build succeeds. Overlay is created but always hidden (starts hidden by default).

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: create WelcomeOverlay in AppDelegate setup"
```

---

### Task 3: Add `update_welcome_visibility()` helper

**Files:**
- Modify: `src/app.rs:630-652` (near `content_frame` and `is_editor_mode` helpers)

- [ ] **Step 1: Add the helper method**

Add `update_welcome_visibility` in the `impl AppDelegate` block, after `is_editor_mode()` (around line 652):

```rust
    /// Show the welcome overlay when the active document is empty; hide otherwise.
    /// Also updates the frame (sidebar offset may have changed) and mode-dependent hint text.
    /// When showing, brings the overlay to the front of the z-order so it sits above
    /// the scroll view (which is re-added as a subview on every tab switch).
    fn update_welcome_visibility(&self) {
        let Some(overlay) = self.ivars().welcome_overlay.get() else { return };
        let tm = self.ivars().tab_manager.borrow();
        let Some(tab) = tm.active() else { return };
        let is_empty = unsafe { tab.text_view.textStorage() }
            .is_none_or(|s| s.length() == 0);
        overlay.set_visible(is_empty);
        overlay.update_mode(tab.mode.get());
        overlay.set_frame(self.content_frame());
        // Ensure overlay is above the scroll view in the z-order.
        if is_empty {
            if let Some(win) = self.ivars().window.get() {
                let content = win.contentView().unwrap();
                let view = overlay.view();
                view.removeFromSuperview();
                content.addSubview(view);
            }
        }
    }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Build succeeds (method exists but no callers yet — may warn about dead code).

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: add update_welcome_visibility() helper"
```

---

### Task 4: Wire call sites

**Files:**
- Modify: `src/app.rs` — 6 methods

This task adds `self.update_welcome_visibility()` calls to the 6 locations specified in the design spec. Each insertion is a single line.

- [ ] **Step 1: Wire `text_did_change:` (line ~519)**

At the end of `text_did_change()`, after the path bar word count update block, add:

```rust
            self.update_welcome_visibility();
```

- [ ] **Step 2: Wire `switch_to_tab()` (line ~910)**

At the end of `switch_to_tab()`, after `self.update_text_container_inset();`, add:

```rust
        self.update_welcome_visibility();
```

- [ ] **Step 3: Wire `toggle_mode()` (line ~743)**

At the end of `toggle_mode()`, after the path bar word count block, add:

```rust
        self.update_welcome_visibility();
```

- [ ] **Step 4: Wire `open_file_by_path()` (line ~809)**

At the end of `open_file_by_path()`, after `self.rebuild_tab_bar();`, add:

```rust
        self.update_welcome_visibility();
```

- [ ] **Step 5: Wire `close_tab()` — LastTab branch (line ~1048)**

In the `TabCloseResult::LastTab` branch, after the `pb.update(None);` block (around line 1048), add:

```rust
                self.update_welcome_visibility();
```

Note: The `Removed { new_active }` branch calls `switch_to_tab()` which already has the call, so no duplication needed.

- [ ] **Step 6: Wire `add_empty_tab()` (line ~933)**

At the end of `add_empty_tab()`, after `self.switch_to_tab(new_idx);`, add:

```rust
        self.update_welcome_visibility();
```

Note: `switch_to_tab` already calls it too, but `add_empty_tab` is the authoritative "new empty tab" path — having the explicit call here is harmless (idempotent) and guards against future refactors.

- [ ] **Step 7: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Build succeeds, no warnings about unused `update_welcome_visibility`.

- [ ] **Step 8: Commit**

```bash
git add src/app.rs
git commit -m "feat: wire update_welcome_visibility() into all call sites"
```

---

### Task 5: Update `windowDidResize:`

**Files:**
- Modify: `src/app.rs:156-198` (windowDidResize)

- [ ] **Step 1: Add overlay frame update in `windowDidResize:`**

After the scroll view frame update (around line 195, after `t.scroll_view.setFrame(frame);`), but still inside the method, add:

```rust
            if let Some(overlay) = self.ivars().welcome_overlay.get() {
                overlay.set_frame(frame);
            }
```

This uses the same `frame` from `self.content_frame()` that the scroll view uses, keeping the overlay perfectly aligned.

- [ ] **Step 2: Verify it compiles and run the app**

Run: `cargo build 2>&1 | head -20`
Expected: Build succeeds.

Run: `cargo run`
Expected: App launches. Empty tab shows the centered welcome overlay with "mdit", tagline, shortcuts, and hint text. Typing makes it disappear. Deleting all text brings it back.

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: update welcome overlay frame on window resize"
```

---

### Task 6: Manual verification checklist

Run: `cargo run`

Verify each behavior from the spec:

- [ ] **New empty tab** → overlay visible with "mdit", tagline, 4 shortcuts, viewer hint
- [ ] **Click into text area and type a character** → overlay disappears immediately
- [ ] **Select all (Cmd+A) + Delete** → overlay reappears
- [ ] **Press Cmd+E on empty doc** → overlay stays, hint changes to "Just start typing to begin"
- [ ] **Press Cmd+E again** → hint changes back to viewer text
- [ ] **Open a file (Cmd+O)** → overlay disappears
- [ ] **New tab (Cmd+T), then switch between tabs** → overlay shows/hides based on each tab's content
- [ ] **Close last tab (Cmd+W on only tab)** → content cleared, overlay reappears
- [ ] **Resize window** → overlay stays centered
- [ ] **Toggle Light/Dark mode** → label colors adapt automatically
- [ ] **Click through overlay into text view** → cursor appears, ready to type

- [ ] **Final commit (if any adjustments needed)**

```bash
git add -A
git commit -m "fix: welcome overlay adjustments from manual testing"
```

Only create this commit if changes were actually needed.
