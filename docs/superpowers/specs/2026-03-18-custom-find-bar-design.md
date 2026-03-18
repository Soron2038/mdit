# Custom Find Bar — Design Spec

**Date:** 2026-03-18
**Status:** Approved

---

## Problem

The built-in macOS Find Panel (`usesFindPanel = true`) works well functionally but looks generic — Apple controls its appearance entirely and it cannot be customized. mdit has a distinctive visual identity (cream background, Georgia font, warm orange accent) that a floating system panel doesn't respect.

## Goal

Replace the system find panel with a custom find bar that sits inline at the bottom of the editor window, styled to match mdit's aesthetic.

---

## Behavior

### Three States

**1. Find bar open — matches found — Editor Mode**
- Single find row visible
- Replace row appears automatically below it
- All matches highlighted in light yellow
- Current match highlighted in stronger yellow with orange outline
- Match count shown in orange (e.g. "2 / 2")

**2. Find bar open — no matches**
- Single find row only
- Search input text turns red
- Navigation buttons (◂ ▸) appear disabled
- Replace row does not appear (no matches to replace)

**3. Find bar open — Viewer Mode**
- Single find row only
- Replace row never appears, regardless of match count
- Search and navigation work normally

### Find Bar is Hidden by Default
Cmd+F opens it. Escape closes it and removes all highlights.

### Navigation Wraps

Find Next from the last match jumps to the first. Find Previous from the first match jumps to the last. No end-of-document alert.

---

## Visual Design

### Layout

```
┌────────────────────────────────────────────────────────────────┐  ← 1px border #c8b89a
│  [ search field (Georgia)  ]  [◂]  [▸]  [Aa]  [2 / 2]   [✕]  │  h = 30px
│  [ replace field (Georgia) ]  [Replace]  [All]                 │  h = 26px (auto-shown)
└────────────────────────────────────────────────────────────────┘
```

Position: between the NSScrollView and the PathBar at the bottom of the window.

### Colors

| Element            | Color                         |
|--------------------|-------------------------------|
| Bar background     | `#ece6e1` (same as tab bar)   |
| Top border         | `#c8b89a`                     |
| Input background   | `#fdf9f7` (cream)             |
| Input font         | Georgia                       |
| All match highlight| `rgba(255, 247, 209, 1)` — light yellow |
| Current match      | `rgba(255, 237, 179, 1)` + 1.5px orange outline |
| No-match text      | `rgb(191, 79, 61)` — red      |
| Match count text   | `#c87941` — orange            |
| Aa (active)        | Orange tint + orange border   |

Dark mode: bar background and inputs follow system semantic colors.

### Controls

- **Search field** — Georgia font, cream bg, orange focus ring
- **◂ / ▸** — icon buttons, navigate previous/next match
- **Aa** — toggle for case-sensitive search (inactive = case-insensitive default)
- **match count** — "N / M" or "0 results", orange text
- **✕** — close button
- **Replace field** — same styling as search field, placeholder "Replace with …"
- **Replace** — replace current match
- **All** — replace all matches

---

## Architecture

### Approach

Custom `FindBar` Rust struct wrapping a plain `NSView` container with standard AppKit controls (NSTextField, NSButton). No `define_class!` needed. Search logic lives in `AppDelegate`.

Follows the existing pattern of `PathBar` and `TabBar` in `src/ui/`.

### Window Layout

```
NSWindow.contentView
├── TabBar       (top,  y = win_h − 32,  h = 32)
├── Sidebar      (left, optional, Editor-Mode only)
├── NSScrollView (y = 22 + find_offset, h adjusted)
├── FindBar      (y = 22, h = find_h)    ← NEW
└── PathBar      (y = 0,  h = 22)
```

`find_offset` = 0 when hidden, = current bar height when visible.

### Height Constants

```rust
const FIND_H_COMPACT:  f64 = 30.0;  // find row only
const FIND_H_EXPANDED: f64 = 56.0;  // find + replace rows
```

Height transitions snap (no animation for v1.0).

### Search State (in AppDelegateIvars)

```rust
find_matches:    RefCell<Vec<NSRange>>,
find_current:    Cell<usize>,
find_bar_height: Cell<f64>,  // 0.0 when hidden
```

### Search Algorithm

1. Get full text string from active tab's NSTextView
2. Loop `rangeOfString:options:range:` collecting all match NSRanges
3. Remove highlight attributes from previously matched ranges
4. Apply `NSBackgroundColorAttributeName` to all new match ranges
5. Apply stronger highlight + outline to current match
6. Update count label and no-match styling
7. Show/hide replace row based on `matches.len() > 0 && mode == Editor`
8. Snap bar height and reposition scroll view if height changed

Replace iterates ranges in reverse order to preserve offsets.

---

## Files

| Action | File |
|--------|------|
| **Create** | `src/ui/find_bar.rs` |
| **Modify** | `src/ui/mod.rs` — export FindBar |
| **Modify** | `src/app.rs` — constants, layout, action methods, search logic |
| **Modify** | `src/menu.rs` — update Find submenu selectors |
| **Modify** | `src/editor/text_view.rs` — remove `setUsesFindPanel` |

### New `FindBar` Public API

```rust
pub fn new(mtm, width, target: &AnyObject) -> Self
pub fn view(&self) -> &NSView
pub fn set_width(&self, w: f64)
pub fn set_height(&self, h: f64)
pub fn show(&self)
pub fn hide(&self)
pub fn update_count(&self, current: usize, total: usize)
pub fn set_no_match(&self, no_match: bool)
pub fn show_replace_row(&self, visible: bool)
pub fn search_text(&self) -> String
pub fn replace_text(&self) -> String
pub fn is_case_insensitive(&self) -> bool
pub fn focus_search(&self)
```

### New Action Methods in AppDelegate

```
openFindBar:        show bar, focus search field
closeFindBar:       hide bar, remove highlights, restore scroll view
findNext:           advance current match index, scroll to it
findPrevious:       decrement current match index, scroll to it
replaceOne:         replace current match, re-search
replaceAll:         replace all matches (reverse order), re-search
findBarTextChanged: live search on every keystroke (NSTextFieldDelegate)
findBarToggleAa:    toggle case sensitivity, re-search
```

---

## Keyboard

| Key | Action |
|-----|--------|
| Cmd+F | Open find bar / focus search field |
| Escape | Close find bar |
| Return / Cmd+G | Find next |
| Cmd+Shift+G | Find previous |
| Tab (in search field) | Focus replace field (when visible) |

---

## Verification Checklist

1. `cargo build` succeeds
2. Cmd+F opens bar, search field focused
3. Typing triggers live search, matches highlighted
4. Cmd+G / Cmd+Shift+G navigate matches
5. Aa toggle changes case sensitivity
6. Editor Mode + matches → replace row appears automatically
7. Viewer Mode → replace row never appears
8. Escape closes bar, highlights removed, focus returns to text view
9. Replace / All work correctly
10. No matches → red input, disabled navigation
11. Dark Mode → bar respects color scheme
12. Window resize → bar width updates
13. Tab switch → find state resets
