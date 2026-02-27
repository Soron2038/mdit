# UI Redesign: Sidebar Toolbar + Tab Bar

**Date:** 2026-02-27
**Status:** Approved

---

## Context

The floating formatting toolbar that appeared on text selection was functional but visually disruptive. The goal is to make formatting actions permanently accessible as a clean left-margin sidebar, while also professionalizing the tab bar with SF Symbol icons and an active-tab underline indicator.

---

## Feature 1: Left Sidebar Formatting Toolbar

### Layout

A new fixed-width column (`SIDEBAR_W = 36pt`) sits to the left of the editor scroll view, spanning the full content height (between tab bar and path bar). A 1pt `separatorColor` line forms its right edge.

### New File

`src/ui/sidebar.rs` — `FormattingSidebar` struct

### Buttons (12 total, 3 groups)

**Group 1 — Block types** (6 buttons, 26pt each):

| Label | Selector | Status |
|-------|----------|--------|
| H1    | `applyH1:` | exists |
| H2    | `applyH2:` | exists |
| H3    | `applyH3:` | exists |
| ¶     | `applyNormal:` | **new** — strips `# ` prefix from line |
| >     | `applyBlockquote:` | **new** — prepends `> ` to current line |
| \`\`\` | `applyCodeBlock:` | **new** — wraps selection in fenced code block |

**Group 2 — Inline formatting** (4 buttons):

| Label | Selector | Status |
|-------|----------|--------|
| B     | `applyBold:` | exists |
| I     | `applyItalic:` | exists |
| \`    | `applyInlineCode:` | exists |
| ~~    | `applyStrikethrough:` | exists |

**Group 3 — Insert** (2 buttons):

| Label | Selector | Status |
|-------|----------|--------|
| lnk   | `applyLink:` | exists |
| —     | `applyHRule:` | **new** — inserts `\n---\n` |

### Styling

- Button frame: 32×26pt (2pt horizontal margin)
- Background: transparent (shows window background)
- Bezel: none (`NSBezelStyle::Inline`)
- Font: System 11pt, `secondaryLabelColor` default, `labelColor` on hover
- Group separator: 8pt vertical gap between groups
- Right border: 1pt vertical line at x=35.5pt using `NSColor::separatorColor`

### New Action Methods (in app.rs)

1. `applyNormal:` — uses `replace_line_prefix()` helper to strip `#+ ` from active line
2. `applyBlockquote:` — prepends `> ` to current line (same pattern as applyH1/H2/H3)
3. `applyCodeBlock:` — wraps selection in ` ```\n{selection}\n``` ` or inserts and positions cursor inside
4. `applyHRule:` — inserts `\n---\n` at cursor

### Layout Changes (app.rs)

- Replace `toolbar: OnceCell<FloatingToolbar>` with `sidebar: OnceCell<FormattingSidebar>`
- `content_frame()` helper: `x = SIDEBAR_W`, `width = total_width - SIDEBAR_W`
- `windowDidResize`: update sidebar height alongside scroll view
- `textViewDidChangeSelection`: remove `toolbar.show_near_rect()` / `toolbar.hide()` calls; simplify
- Delete `src/ui/toolbar.rs` (floating toolbar no longer needed)

---

## Feature 2: Tab Bar Redesign

### File

`src/ui/tab_bar.rs` — refactor existing code

### Open/Save Buttons

- Replace text `"Open"` / `"Save"` with SF Symbol images:
  - Open: `NSImage(systemName: "folder")`
  - Save: `NSImage(systemName: "square.and.arrow.down")`
- Width: 28pt each (down from 46pt)
- Style: borderless, hover-only highlight

### Tab Buttons

- Width: 110pt (up from 100pt for better readability)
- Inactive: System 12pt, `secondaryLabelColor`, transparent background
- Active: `labelColor`, + 2pt accent-color indicator line at bottom

### Active-Tab Indicator

A dedicated `Retained<NSView>` stored as `indicator` in `TabBar`. On `rebuild()`, it is positioned under the active tab button: `frame = (tab_x, 0, tab_width, 2)`. Color: `NSColor::controlAccentColor`.

### Bottom Separator

A 0.5pt `separatorColor` line spanning the full tab bar width at y=0, drawn as a thin `NSView` added once in `TabBar::new()`.

### Plus Button

- Keeps `"+"` text label
- `NSBezelStyle::Inline`, 28pt wide

### New Fields in TabBar

```rust
pub struct TabBar {
    container: Retained<NSView>,
    indicator: OnceCell<Retained<NSView>>,   // active tab underline
    separator: OnceCell<Retained<NSView>>,   // bottom border line
}
```

---

## Files to Modify

| File | Change |
|------|--------|
| `src/ui/sidebar.rs` | **new** — FormattingSidebar struct |
| `src/ui/toolbar.rs` | **delete** — replaced by sidebar |
| `src/ui/tab_bar.rs` | refactor — icons, indicator, separator |
| `src/app.rs` | update layout, add new actions, swap toolbar→sidebar |
| `src/lib.rs` / `src/ui/mod.rs` | add `pub mod sidebar;`, remove `toolbar` |

---

## Verification

1. Build: `cargo build` — no warnings, no errors
2. Run: `cargo run` — confirm sidebar visible with all 12 buttons
3. Click each sidebar button — confirm correct markdown is applied
4. Resize window — confirm sidebar height and scroll view position stay correct
5. Tab bar: confirm SF Symbol icons render, underline moves to active tab
6. Light/dark mode switch — confirm `separatorColor` and `secondaryLabelColor` adapt
7. `cargo test` — all 64 existing tests still pass
