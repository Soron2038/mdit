# Welcome Overlay — Design Spec

**Date:** 2026-03-24
**Status:** Approved
**FINISHING.md item:** Feature-Discoverability: Cmd+E Hinweis (Priority 2)

## Goal

Make the Viewer/Editor toggle (Cmd+E) and other key shortcuts discoverable to new users via a welcome placeholder that appears in empty documents.

## Approach

Overlay NSView — a new `src/ui/welcome_overlay.rs` module following the existing UI component pattern (PathBar, FindBar, FormattingSidebar). The overlay sits above the NSScrollView and shows a centered welcome screen when the document is empty.

## Visual Design

Style: Centered Minimal. All content vertically centered in the content area.

**Content hierarchy (top to bottom):**

1. **App name:** "mdit" — light weight, ~42pt, `secondaryLabelColor`
2. **Tagline:** "A native Markdown editor for macOS" — 13pt, `tertiaryLabelColor`
3. **Shortcut list:** 4 entries, monospaced 12pt:
   - `⌘E` Toggle Editor / Viewer
   - `⌘F` Find & Replace
   - `⌘T` New Tab
   - `⌘+/−` Adjust Font Size
4. **Hint line (mode-dependent):**
   - Editor mode: "Just start typing to begin"
   - Viewer mode: "Press ⌘E to start editing, or ⌘O to open a file"
   - 11pt, `quaternaryLabelColor`

**Language:** English.

**Theming:** System label colors (`secondaryLabelColor`, `tertiaryLabelColor`, `quaternaryLabelColor`) adapt automatically to Light/Dark mode. No custom theming required.

**Layout:** Frame-based centering, consistent with the rest of the codebase (no AutoLayout). Labels have fixed sizes and are manually centered in the container frame.

## Module API

```rust
// src/ui/welcome_overlay.rs

pub struct WelcomeOverlay {
    container: Retained<NSView>,
}

impl WelcomeOverlay {
    /// Create the overlay with all labels, initially hidden.
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Self

    /// Show or hide the overlay.
    pub fn set_visible(&self, visible: bool)

    /// Update the overlay frame (called from windowDidResize:).
    pub fn set_frame(&self, frame: NSRect)

    /// Update the mode-dependent hint text.
    pub fn update_mode(&self, mode: ViewMode)

    /// Access the underlying NSView for adding to the view hierarchy.
    pub fn view(&self) -> &NSView
}
```

## Integration in AppDelegate

**New ivar:** `welcome_overlay: OnceCell<WelcomeOverlay>` in `AppDelegateIvars`.

**Creation:** In `setup_content_views()`, after the other UI components. Added as subview of the window's content view, above the scroll view.

**Helper method:** `update_welcome_visibility(&self)` — checks `textStorage.string().length() == 0` for the active tab and calls `set_visible()` accordingly. Also calls `update_mode()` with the active tab's current mode.

**Call sites for `update_welcome_visibility()`:**

| Location | Reason |
|----------|--------|
| `text_did_change:` | Text added or removed |
| `switch_to_tab()` | New tab may be empty or non-empty |
| `toggle_mode()` | Hint text changes per mode |
| `open_file_by_path()` | Loaded file has content |
| `close_tab()` | Last tab closed → new empty tab created |
| `add_empty_tab()` | New empty tab is always empty |

## Behavior

- **Visibility rule:** `textStorage.string().length() == 0` → visible. Otherwise hidden.
- **Mode switch on empty doc:** Overlay stays visible, hint text updates.
- **Tab switch:** Overlay visibility re-evaluated for the new tab.
- **File open:** Text loaded → `text_did_change:` fires → overlay hides.
- **Select all + delete:** `text_did_change:` fires → `length == 0` → overlay reappears.
- **Mouse events:** The overlay does not accept mouse events (`setAcceptsMouseEvents(false)` or `hitTest:` returning nil). Clicks pass through to the text view underneath so the user can click and start typing immediately.

## Interaction with Other Components

- **Find Bar:** Opens normally even with overlay visible. No conflict — find bar sits below the overlay in the layout stack.
- **Sidebar:** Overlay uses `content_frame()` which already accounts for sidebar width in Editor mode.
- **Word Count:** PathBar shows "0 words, 0 chars" for empty documents. Correct and consistent.
- **Font Size:** Overlay label sizes are fixed (not affected by the user's font size preference). The welcome text is UI chrome, not document content.

## Testing

The visibility logic (`textStorage.length == 0`) is trivial and deterministic. Integration testing via manual verification:
- New empty tab → overlay visible
- Type a character → overlay disappears
- Delete all → overlay reappears
- Switch modes on empty doc → hint text changes
- Open file → overlay disappears
- Switch between empty and non-empty tabs → overlay toggles correctly
