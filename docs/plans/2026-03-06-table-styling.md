# Table Styling Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve table visual appearance with rounded border, background fill, thicker/darker grid lines, and 10px cell padding — matching the code block aesthetic.

**Architecture:** Extend the existing table rendering pipeline (renderer → apply → text_storage → text_view). Add per-table bounding positions to `LayoutPositions` and `TableInfo`. Draw rounded border + fill in the text view drawing methods, clip inner grid lines to the border rect.

**Tech Stack:** Rust, objc2, AppKit (NSBezierPath, NSTextStorage, NSColor, NSParagraphStyle)

---

### Task 1: Add `table_bg` color to `ColorScheme`

**Files:**
- Modify: `src/ui/appearance.rs:3-15` (struct definition)
- Modify: `src/ui/appearance.rs:18-32` (light scheme)
- Modify: `src/ui/appearance.rs:34-48` (dark scheme)
- Modify: `src/ui/appearance.rs:64-71` (resolve_bg)
- Test: `tests/renderer_tests.rs`

**Step 1: Write a failing test**

In `src/ui/appearance.rs`, add to the existing `light_scheme_tokens_resolve` test:

```rust
// Inside the existing tests module, add a new test:
#[test]
fn table_bg_resolves() {
    let light = ColorScheme::light();
    assert!(light.resolve_bg("table_bg").is_some(), "light scheme should resolve table_bg");
    let dark = ColorScheme::dark();
    assert!(dark.resolve_bg("table_bg").is_some(), "dark scheme should resolve table_bg");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test table_bg_resolves -- --nocapture`
Expected: FAIL — `table_bg` field doesn't exist yet.

**Step 3: Add `table_bg` field and values**

In `src/ui/appearance.rs`:

Add field to `ColorScheme` struct (after `code_block_bg`):
```rust
pub table_bg: (f64, f64, f64),
```

Add value in `light()` (after `code_block_bg`):
```rust
table_bg:      (0.93, 0.93, 0.95),
```

Add value in `dark()` (after `code_block_bg`):
```rust
table_bg:      (0.16, 0.16, 0.17),
```

Add match arm in `resolve_bg` (after `code_block_bg`):
```rust
"table_bg"     => Some(self.table_bg),
```

**Step 4: Run test to verify it passes**

Run: `cargo test table_bg_resolves -- --nocapture`
Expected: PASS

**Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests pass (no regressions).

**Step 6: Commit**

```bash
git add src/ui/appearance.rs
git commit -m "feat(appearance): add table_bg color to ColorScheme"
```

---

### Task 2: Add table extent data to `TableInfo` and `LayoutPositions`

**Files:**
- Modify: `src/editor/renderer.rs:16-21` (TableInfo struct)
- Modify: `src/editor/renderer.rs:452-554` (collect_table function)
- Modify: `src/editor/apply.rs:66-75` (LayoutPositions struct)
- Modify: `src/editor/apply.rs:89-195` (apply_attribute_runs function)
- Modify: `src/editor/text_storage.rs` (store + expose table_bounds)
- Test: `tests/renderer_tests.rs`

**Step 1: Write a failing test**

In `tests/renderer_tests.rs`, add:

```rust
#[test]
fn table_info_has_source_range() {
    let text = "| A | B |\n|---|---|\n| 1 | 2 |";
    let spans = parse(text);
    let output = compute_attribute_runs(text, &spans, Some(999));
    assert!(!output.table_infos.is_empty());
    let info = &output.table_infos[0];
    assert_eq!(info.source_range, (0, text.len()), "table source_range should span entire table");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test table_info_has_source_range -- --nocapture`
Expected: FAIL — `source_range` field doesn't exist.

**Step 3: Add `source_range` to `TableInfo`**

In `src/editor/renderer.rs`, modify the `TableInfo` struct:

```rust
pub struct TableInfo {
    /// For each data row (header + body): sorted byte positions of structural pipes.
    pub row_pipes: Vec<Vec<usize>>,
    /// Whether the cursor is currently inside this table.
    pub cursor_inside: bool,
    /// Byte range of the entire table (start, end) from the AST span.
    pub source_range: (usize, usize),
}
```

In `collect_table`, update the `table_infos.push(...)` at the end (line ~550):

```rust
table_infos.push(TableInfo {
    row_pipes: all_row_pipes,
    cursor_inside: cursor_in,
    source_range: (span.source_range.0, span.source_range.1.min(text.len())),
});
```

**Step 4: Run test to verify it passes**

Run: `cargo test table_info_has_source_range -- --nocapture`
Expected: PASS

**Step 5: Add `table_bounds` to `LayoutPositions` and plumb through**

In `src/editor/apply.rs`, add field to `LayoutPositions`:

```rust
pub struct LayoutPositions {
    pub heading_seps: Vec<usize>,
    pub thematic_breaks: Vec<usize>,
    pub table_h_seps: Vec<usize>,
    pub table_pipe_seps: Vec<usize>,
    /// Per-table bounding positions: (start_utf16, end_utf16).
    pub table_bounds: Vec<(usize, usize)>,
}
```

In `apply_attribute_runs`, after the equalize_table_columns loop (~line 187), build the bounds:

```rust
let mut table_bounds: Vec<(usize, usize)> = Vec::new();
for table_info in table_infos {
    if !table_info.cursor_inside {
        equalize_table_columns(storage, text, &table_info.row_pipes);
    }
    let start_u16 = byte_to_utf16(text, table_info.source_range.0);
    let end_u16 = byte_to_utf16(text, table_info.source_range.1);
    table_bounds.push((start_u16, end_u16));
}
```

Update the return value:

```rust
LayoutPositions {
    heading_seps: heading_sep_positions,
    thematic_breaks: thematic_break_positions,
    table_h_seps: table_h_sep_positions,
    table_pipe_seps: table_pipe_sep_positions,
    table_bounds,
}
```

Update the empty early-return at the top of the function to also include `table_bounds: Vec::new()`.

**Step 6: Plumb `table_bounds` through `text_storage.rs`**

In `src/editor/text_storage.rs`:

Add ivar (after `table_pipe_sep_positions`):
```rust
/// Per-table (start_utf16, end_utf16) for drawing rounded borders.
table_bounds: RefCell<Vec<(usize, usize)>>,
```

In `new()`, add initialization:
```rust
table_bounds: RefCell::new(Vec::new()),
```

In `did_process_editing`, after storing `table_pipe_seps`:
```rust
*self.ivars().table_bounds.borrow_mut() = positions.table_bounds;
```

Same in `reapply`.

Add public accessor:
```rust
/// Returns per-table (start_utf16, end_utf16) bounding positions.
pub fn table_bounds(&self) -> Vec<(usize, usize)> {
    self.ivars().table_bounds.borrow().clone()
}
```

**Step 7: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

**Step 8: Commit**

```bash
git add src/editor/renderer.rs src/editor/apply.rs src/editor/text_storage.rs tests/renderer_tests.rs
git commit -m "feat(table): add table extent data to TableInfo and LayoutPositions"
```

---

### Task 3: Draw table background fill and rounded border

**Files:**
- Modify: `src/editor/text_view.rs:52-57` (drawViewBackgroundInRect — add fill)
- Modify: `src/editor/text_view.rs:62-73` (drawRect — add border)
- Add new methods: `table_rects()`, `draw_table_fills()`, `draw_table_borders()`

**Step 1: Add `table_rects()` helper method**

Model this after the existing `code_block_rects()` pattern (line 392-473). Add a new method to the `impl MditTextView` block (after `draw_table_v_separators`):

```rust
/// Compute the bounding rect for each table from its start/end UTF-16 positions.
/// Returns Vec<NSRect> — one rounded-rect bounding box per table.
fn table_rects(&self) -> Vec<NSRect> {
    let delegate_ref = self.ivars().delegate.borrow();
    let delegate = match delegate_ref.as_ref() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let bounds = delegate.table_bounds();
    if bounds.is_empty() {
        return Vec::new();
    }

    let layout_manager = match unsafe { self.layoutManager() } {
        Some(lm) => lm,
        None => return Vec::new(),
    };
    let text_container = match unsafe { self.textContainer() } {
        Some(tc) => tc,
        None => return Vec::new(),
    };

    let tc_origin = self.textContainerOrigin();
    let container_width = text_container.containerSize().width;
    let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();

    let mut result = Vec::new();
    for &(start_u16, end_u16) in &bounds {
        if start_u16 >= end_u16 {
            continue;
        }
        let first_glyph: usize = unsafe {
            msg_send![&*layout_manager, glyphIndexForCharacterAtIndex: start_u16]
        };
        let last_char = end_u16.saturating_sub(1);
        let last_glyph: usize = unsafe {
            msg_send![&*layout_manager, glyphIndexForCharacterAtIndex: last_char]
        };
        if first_glyph >= usize::MAX / 2 || last_glyph >= usize::MAX / 2 {
            continue;
        }

        let top_frag: NSRect = unsafe {
            msg_send![&*layout_manager,
                lineFragmentRectForGlyphAtIndex: first_glyph,
                effectiveRange: null_ptr]
        };
        let bot_frag: NSRect = unsafe {
            msg_send![&*layout_manager,
                lineFragmentRectForGlyphAtIndex: last_glyph,
                effectiveRange: null_ptr]
        };
        if top_frag.size.height == 0.0 || bot_frag.size.height == 0.0 {
            continue;
        }

        let block_y = top_frag.origin.y + tc_origin.y - 8.0;
        let block_bottom = bot_frag.origin.y + bot_frag.size.height + tc_origin.y + 8.0;
        let block_rect = NSRect::new(
            NSPoint::new(tc_origin.x, block_y),
            NSSize::new(container_width, block_bottom - block_y),
        );
        result.push(block_rect);
    }
    result
}
```

**Step 2: Add `draw_table_fills()` method**

```rust
/// Draw rounded-rect background fills for all tables.
/// Called from drawViewBackgroundInRect: — BEFORE glyphs.
fn draw_table_fills(&self) {
    let rects = self.table_rects();
    if rects.is_empty() {
        return;
    }
    let fill_color = {
        let delegate_ref = self.ivars().delegate.borrow();
        match delegate_ref.as_ref() {
            Some(d) => {
                let (r, g, b) = d.scheme().table_bg;
                NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0)
            }
            None => return,
        }
    };
    for block_rect in &rects {
        let path =
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*block_rect, 6.0, 6.0);
        fill_color.setFill();
        path.fill();
    }
}
```

**Step 3: Add `draw_table_borders()` method**

```rust
/// Draw rounded-rect border strokes for all tables.
/// Called from drawRect: — AFTER glyphs (overlay).
fn draw_table_borders(&self) {
    let rects = self.table_rects();
    for block_rect in &rects {
        let border_path =
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*block_rect, 6.0, 6.0);
        border_path.setLineWidth(1.0);
        NSColor::tertiaryLabelColor().setStroke();
        border_path.stroke();
    }
}
```

**Step 4: Wire into draw methods**

In `draw_view_background_in_rect` (line ~57), add after `self.draw_code_block_fills();`:
```rust
self.draw_table_fills();
```

In `draw_rect` (line ~72), replace the two table separator calls with:
```rust
self.draw_table_borders();
self.draw_table_h_separators();
self.draw_table_v_separators();
```

**Step 5: Build to verify compilation**

Run: `cargo build`
Expected: Compiles without errors.

**Step 6: Commit**

```bash
git add src/editor/text_view.rs
git commit -m "feat(text_view): draw table background fill and rounded border"
```

---

### Task 4: Clip inner grid lines to table border

**Files:**
- Modify: `src/editor/text_view.rs` — `draw_table_h_separators()` and `draw_table_v_separators()`

**Step 1: Refactor grid line drawing to accept clip rects**

Modify `draw_table_h_separators` and `draw_table_v_separators` to:
1. Use `tertiaryLabelColor` instead of `separatorColor`
2. Use 1.0pt line thickness instead of 0.5pt
3. Save/restore graphics state and clip to table rects

Update `draw_table_h_separators`:

```rust
fn draw_table_h_separators(&self) {
    let delegate_ref = self.ivars().delegate.borrow();
    let delegate = match delegate_ref.as_ref() {
        Some(d) => d,
        None => return,
    };
    let positions = delegate.table_h_sep_positions();
    if positions.is_empty() {
        return;
    }

    let layout_manager = match unsafe { self.layoutManager() } {
        Some(lm) => lm,
        None => return,
    };
    let text_container = match unsafe { self.textContainer() } {
        Some(tc) => tc,
        None => return,
    };

    let tc_origin = self.textContainerOrigin();
    let container_size = text_container.containerSize();
    let x_start = tc_origin.x;
    let x_end = x_start + container_size.width;

    // Clip to table border rects so lines don't extend beyond rounded corners.
    let table_rects = self.table_rects();
    if !table_rects.is_empty() {
        unsafe { msg_send![class!(NSGraphicsContext), saveGraphicsState] };
        let clip_path = NSBezierPath::bezierPath();
        for rect in &table_rects {
            let rounded = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*rect, 6.0, 6.0);
            clip_path.appendBezierPath(&rounded);
        }
        clip_path.addClip();
    }

    let sep_color = NSColor::tertiaryLabelColor();
    sep_color.setFill();

    for &utf16_pos in &positions {
        let glyph_idx: usize =
            unsafe { msg_send![&*layout_manager, glyphIndexForCharacterAtIndex: utf16_pos] };
        if glyph_idx == usize::MAX {
            continue;
        }

        let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();
        let frag_rect: NSRect = unsafe {
            msg_send![
                &*layout_manager,
                lineFragmentRectForGlyphAtIndex: glyph_idx,
                effectiveRange: null_ptr
            ]
        };
        if frag_rect.size.height == 0.0 {
            continue;
        }

        let y = frag_rect.origin.y + tc_origin.y;
        let line_rect = NSRect::new(
            NSPoint::new(x_start, y - 0.5),
            NSSize::new(x_end - x_start, 1.0),
        );
        NSRectFill(line_rect);
    }

    if !table_rects.is_empty() {
        unsafe { msg_send![class!(NSGraphicsContext), restoreGraphicsState] };
    }
}
```

Update `draw_table_v_separators` similarly:

```rust
fn draw_table_v_separators(&self) {
    let delegate_ref = self.ivars().delegate.borrow();
    let delegate = match delegate_ref.as_ref() {
        Some(d) => d,
        None => return,
    };
    let positions = delegate.table_pipe_sep_positions();
    if positions.is_empty() {
        return;
    }

    let layout_manager = match unsafe { self.layoutManager() } {
        Some(lm) => lm,
        None => return,
    };

    let tc_origin = self.textContainerOrigin();

    // Clip to table border rects.
    let table_rects = self.table_rects();
    if !table_rects.is_empty() {
        unsafe { msg_send![class!(NSGraphicsContext), saveGraphicsState] };
        let clip_path = NSBezierPath::bezierPath();
        for rect in &table_rects {
            let rounded = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*rect, 6.0, 6.0);
            clip_path.appendBezierPath(&rounded);
        }
        clip_path.addClip();
    }

    let sep_color = NSColor::tertiaryLabelColor();
    sep_color.setFill();

    let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();

    for &utf16_pos in &positions {
        let glyph_idx: usize =
            unsafe { msg_send![&*layout_manager, glyphIndexForCharacterAtIndex: utf16_pos] };
        if glyph_idx >= usize::MAX / 2 {
            continue;
        }

        let frag_rect: NSRect = unsafe {
            msg_send![
                &*layout_manager,
                lineFragmentRectForGlyphAtIndex: glyph_idx,
                effectiveRange: null_ptr
            ]
        };
        if frag_rect.size.height == 0.0 {
            continue;
        }

        let glyph_loc: NSPoint = unsafe {
            msg_send![&*layout_manager, locationForGlyphAtIndex: glyph_idx]
        };

        let x = frag_rect.origin.x + glyph_loc.x + tc_origin.x;
        let y_top = frag_rect.origin.y + tc_origin.y;
        let y_bottom = y_top + frag_rect.size.height;

        let line_rect = NSRect::new(
            NSPoint::new(x - 0.5, y_top),
            NSSize::new(1.0, y_bottom - y_top),
        );
        NSRectFill(line_rect);
    }

    if !table_rects.is_empty() {
        unsafe { msg_send![class!(NSGraphicsContext), restoreGraphicsState] };
    }
}
```

**Step 2: Ensure `NSGraphicsContext` class is available**

Add to the imports in `text_view.rs` if not already present:
```rust
use objc2::class;
```

Check whether `class!` macro is already available — if the crate uses `objc2::runtime::AnyClass::get` instead, use:
```rust
let ctx_cls = objc2::runtime::AnyClass::get(c"NSGraphicsContext").unwrap();
let _: () = unsafe { msg_send![ctx_cls, saveGraphicsState] };
// ... drawing ...
let _: () = unsafe { msg_send![ctx_cls, restoreGraphicsState] };
```

**Step 3: Build to verify compilation**

Run: `cargo build`
Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add src/editor/text_view.rs
git commit -m "feat(text_view): clip grid lines to table border, use tertiaryLabelColor"
```

---

### Task 5: Add horizontal cell padding (kern on pipes + expanded column width)

**Files:**
- Modify: `src/editor/apply.rs:89-195` (apply_attribute_runs — add kern to pipe chars)
- Modify: `src/editor/apply.rs:345-434` (equalize_table_columns — expand max widths)

**Step 1: Add 10px kern to every `TablePipe` character**

In `apply_attribute_runs`, inside the loop over runs where `TablePipe` is handled (~line 177), add kern application:

```rust
if run.attrs.contains(&TextAttribute::TablePipe) {
    table_pipe_sep_positions.push(start_u16);
    // Add 10px left padding: kern on the pipe pushes the next character right.
    let kern_value = NSNumber::numberWithFloat(10.0);
    unsafe {
        storage.addAttribute_value_range(
            NSKernAttributeName,
            kern_value.as_ref(),
            range,
        );
    }
}
```

**Step 2: Expand max column widths by 20px in `equalize_table_columns`**

In `equalize_table_columns`, after computing `max_widths` in Pass 2 (~line 398), add:

```rust
// Add 20px per column for cell padding (10px left from pipe kern + 10px right).
for w in &mut max_widths {
    *w += 20.0;
}
```

This ensures each cell gets 10px extra on the right side (10px on the left is already handled by the pipe kern above).

**Step 3: Build to verify compilation**

Run: `cargo build`
Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add src/editor/apply.rs
git commit -m "feat(apply): add 10px horizontal cell padding via kern"
```

---

### Task 6: Add vertical cell padding (paragraph spacing on table rows)

**Files:**
- Modify: `src/editor/apply.rs` (apply paragraph spacing to table rows)
- Modify: `src/editor/renderer.rs` (expose table row byte ranges)

**Step 1: Add row ranges to `TableInfo`**

In `src/editor/renderer.rs`, add a field to `TableInfo`:

```rust
pub struct TableInfo {
    pub row_pipes: Vec<Vec<usize>>,
    pub cursor_inside: bool,
    pub source_range: (usize, usize),
    /// Byte ranges of each data row (header + body rows, excluding separator).
    pub row_ranges: Vec<(usize, usize)>,
}
```

In `collect_table`, collect row ranges:

Add `let mut all_row_ranges: Vec<(usize, usize)> = Vec::new();` alongside `all_row_pipes`.

Inside the `for row in &span.children` loop, after the pipe scanning block, for rows that matched `NodeKind::TableRow { .. }`:
```rust
all_row_ranges.push((row.source_range.0, row.source_range.1.min(text.len())));
```

Update the `table_infos.push`:
```rust
table_infos.push(TableInfo {
    row_pipes: all_row_pipes,
    cursor_inside: cursor_in,
    source_range: (span.source_range.0, span.source_range.1.min(text.len())),
    row_ranges: all_row_ranges,
});
```

**Step 2: Apply paragraph spacing in `apply_attribute_runs`**

In `apply_attribute_runs`, inside the `for table_info in table_infos` loop, add paragraph spacing to each row:

```rust
for table_info in table_infos {
    if !table_info.cursor_inside {
        equalize_table_columns(storage, text, &table_info.row_pipes);

        // Apply vertical padding to each table row.
        for &(row_start, row_end) in &table_info.row_ranges {
            let row_start_u16 = byte_to_utf16(text, row_start);
            let row_end_u16 = byte_to_utf16(text, row_end);
            if row_start_u16 >= row_end_u16 {
                continue;
            }
            let row_range = NSRange { location: row_start_u16, length: row_end_u16 - row_start_u16 };
            let style = make_table_row_para_style(9.6, 10.0, 10.0);
            unsafe {
                storage.addAttribute_value_range(
                    NSParagraphStyleAttributeName,
                    style.as_ref(),
                    row_range,
                );
            }
        }
    }
    // ... table_bounds push ...
}
```

**Step 3: Add the `make_table_row_para_style` helper**

In `src/editor/apply.rs`, after the existing `make_para_style_with_spacing_before` function:

```rust
/// Build an `NSMutableParagraphStyle` for table rows with vertical cell padding.
fn make_table_row_para_style(
    line_spacing: f64,
    spacing_before: f64,
    spacing_after: f64,
) -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    style.setLineSpacing(line_spacing);
    style.setParagraphSpacingBefore(spacing_before);
    style.setParagraphSpacing(spacing_after);
    style
}
```

**Step 4: Build to verify compilation**

Run: `cargo build`
Expected: Compiles without errors.

**Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add src/editor/renderer.rs src/editor/apply.rs
git commit -m "feat(apply): add 10px vertical cell padding via paragraph spacing"
```

---

### Task 7: Final build + visual smoke test

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

**Step 2: Build release**

Run: `cargo build --release`
Expected: Compiles without errors.

**Step 3: Manual visual verification**

Launch the app and open a file with a table like:
```markdown
| Name | Age | City |
|------|-----|------|
| Alice | 30 | Berlin |
| Bob | 25 | Munich |
```

Verify:
- Rounded border around the entire table (6pt radius, same as code blocks)
- Subtle background fill inside the border
- Grid lines are 1pt thick, darker than before (tertiaryLabelColor)
- Grid lines don't extend beyond the rounded border corners
- 10px padding on all sides of each cell
- Cursor-in behavior still works (pipes become visible, formatting reverts)

**Step 4: Commit any fixups if needed, then final commit**

```bash
git add -A
git commit -m "feat(table): styled tables with rounded border, background, and cell padding"
```
