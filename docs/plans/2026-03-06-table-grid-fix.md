# Table Grid Fix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace flat position lists with per-table `TableGrid` structs to fix double lines, broken vertical lines, and extra empty column in table rendering.

**Architecture:** Remove `TableSeparatorLine` and `TablePipe` attributes entirely. Compute grid data (`column_seps`, `row_seps`, `bounds`) from `TableInfo` in `apply.rs`. Drawing code draws full-extent grid lines per table instead of per-fragment.

**Tech Stack:** Rust, objc2 (AppKit bindings), NSTextView/NSLayoutManager

---

### Task 1: Remove `TableSeparatorLine` and `TablePipe` from attributes

**Files:**
- Modify: `src/markdown/attributes.rs:23-26`
- Modify: `tests/attributes_tests.rs:77-88`

**Step 1: Update tests — remove attribute-specific tests**

In `tests/attributes_tests.rs`, delete the two tests `table_separator_line_attribute_detectable` and `table_pipe_attribute_detectable` (lines 77-88). These attributes are being removed.

**Step 2: Remove the enum variants**

In `src/markdown/attributes.rs`, delete lines 23-26:
```rust
    /// Marks a table row boundary for horizontal separator line drawing.
    TableSeparatorLine,
    /// Marks a table pipe `|` position for vertical separator line drawing.
    TablePipe,
```

**Step 3: Run `cargo check` to find all compilation errors**

Run: `cargo check 2>&1 | head -40`

This will show all remaining references to the deleted variants. Do NOT fix them yet — they are handled in subsequent tasks.

**Step 4: Commit**

```bash
git add src/markdown/attributes.rs tests/attributes_tests.rs
git commit -m "refactor(attributes): remove TableSeparatorLine and TablePipe variants"
```

---

### Task 2: Simplify `collect_table` in renderer

**Files:**
- Modify: `src/editor/renderer.rs:484-541`
- Modify: `tests/renderer_tests.rs:338-408`

**Step 1: Update renderer tests**

The five affected tests in `tests/renderer_tests.rs` need rewriting. They currently assert on `TablePipe` and `TableSeparatorLine` attributes which no longer exist. Replace them with tests that verify pipes are `Hidden` when cursor is outside, and that `TableInfo` contains correct data.

Replace `table_pipes_hidden_when_cursor_outside` (lines 340-353):
```rust
#[test]
fn table_pipes_hidden_when_cursor_outside() {
    let text = "| A | B |\n|---|---|\n| 1 | 2 |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, Some(999)).runs;
    // Every single-char run at a pipe byte position should be Hidden.
    let pipe_bytes: Vec<usize> = text.match_indices('|').map(|(i, _)| i).collect();
    let hidden_pipes: Vec<_> = runs
        .iter()
        .filter(|r| {
            r.range.1 - r.range.0 == 1
                && pipe_bytes.contains(&r.range.0)
                && r.attrs.contains(&TextAttribute::Hidden)
        })
        .collect();
    assert!(
        !hidden_pipes.is_empty(),
        "expected Hidden runs for pipe characters when cursor is outside"
    );
}
```

Replace `table_pipes_visible_when_cursor_inside` (lines 355-366):
```rust
#[test]
fn table_pipes_visible_when_cursor_inside() {
    let text = "| A | B |\n|---|---|\n| 1 | 2 |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, Some(3)).runs;
    let has_hidden = runs.iter().any(|r| {
        r.attrs.contains(&TextAttribute::Hidden) && r.range.1 - r.range.0 == 1
    });
    assert!(!has_hidden, "pipes should not be hidden when cursor is inside");
}
```

Replace `table_separator_row_hidden_when_cursor_outside` (lines 368-381):
```rust
#[test]
fn table_separator_row_hidden_when_cursor_outside() {
    let text = "| A | B |\n|---|---|\n| 1 | 2 |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, Some(999)).runs;
    // The separator row spans bytes 9..20 (the "---|---|\n" region).
    // It should have a Hidden run covering that range.
    let sep_run = runs.iter().find(|r| r.range.0 == 9 && r.attrs.contains(&TextAttribute::Hidden));
    assert!(sep_run.is_some(), "expected Hidden run for separator row");
}
```

Replace `table_separator_row_visible_when_cursor_inside` (lines 383-392):
```rust
#[test]
fn table_separator_row_visible_when_cursor_inside() {
    let text = "| A | B |\n|---|---|\n| 1 | 2 |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, Some(3)).runs;
    // When cursor is inside, separator row gets syntax_visible (ForegroundColor), not Hidden.
    let sep_hidden = runs.iter().any(|r| r.range.0 == 9 && r.attrs.contains(&TextAttribute::Hidden));
    assert!(!sep_hidden, "separator row should not be hidden when cursor is inside");
}
```

Replace `table_multi_body_rows_get_h_separator` (lines 394-408):
```rust
#[test]
fn table_info_row_ranges_includes_all_data_rows() {
    let text = "| A |\n|---|\n| 1 |\n| 2 |";
    let spans = parse(text);
    let output = compute_attribute_runs(text, &spans, Some(999));
    assert!(!output.table_infos.is_empty());
    let info = &output.table_infos[0];
    assert_eq!(
        info.row_ranges.len(),
        3,
        "expected 3 row_ranges (header + 2 body); got {}",
        info.row_ranges.len()
    );
}
```

**Step 2: Simplify `collect_table` in `src/editor/renderer.rs`**

Replace lines 484-541 (from the separator-row marking through the pipe scanning loop) with simplified code. The separator row just gets `syn` (no special attribute). Pipes just get `syn` (no `TablePipe`/`TableSeparatorLine`). Remove `body_row_count`, `needs_h_sep`, `is_first_pipe`.

New separator row handling (replace lines 484-493):
```rust
    // ── Mark separator row (gap between last header and first body row) ──
    if let (Some(sep_start), Some(sep_end)) = (header_end, first_body_start) {
        if sep_start < sep_end {
            runs.push(AttributeRun { range: (sep_start, sep_end), attrs: syn.clone() });
        }
    }
```

New row processing loop (replace lines 495-554):
```rust
    // ── Process each data row ────────────────────────────────────────────
    let mut all_row_pipes: Vec<Vec<usize>> = Vec::new();
    let mut all_row_ranges: Vec<(usize, usize)> = Vec::new();

    for row in &span.children {
        if !matches!(&row.kind, NodeKind::TableRow { .. }) {
            continue;
        }

        // Collect cell byte ranges to distinguish structural pipes from cell content.
        let cell_ranges: Vec<(usize, usize)> = row
            .children
            .iter()
            .filter(|c| matches!(c.kind, NodeKind::TableCell))
            .map(|c| c.source_range)
            .collect();

        // Scan for pipe characters in the row — just mark them Hidden, no special attributes.
        let row_end = row.source_range.1.min(text.len());
        let mut row_pipe_positions: Vec<usize> = Vec::new();
        for pos in row.source_range.0..row_end {
            if text.as_bytes().get(pos) != Some(&b'|') {
                continue;
            }
            let in_cell = cell_ranges.iter().any(|&(cs, ce)| pos >= cs && pos < ce);
            if in_cell {
                continue;
            }
            row_pipe_positions.push(pos);
            runs.push(AttributeRun { range: (pos, pos + 1), attrs: syn.clone() });
        }
        all_row_pipes.push(row_pipe_positions);
        all_row_ranges.push((row.source_range.0, row.source_range.1.min(text.len())));

        // Process cell children for inline formatting.
        for cell in &row.children {
            if !matches!(cell.kind, NodeKind::TableCell) {
                continue;
            }
            for child in &cell.children {
                collect_runs(text, child, cursor_pos, &[], runs, table_infos);
            }
        }
    }
```

**Step 3: Run tests**

Run: `cargo test --tests 2>&1`
Expected: all renderer tests and attribute tests pass. `apply.rs` may still have compile errors (fixed in Task 3).

**Step 4: Commit**

```bash
git add src/editor/renderer.rs tests/renderer_tests.rs
git commit -m "refactor(renderer): simplify collect_table — pipes and separator get only Hidden"
```

---

### Task 3: Add `TableGrid` and rewrite `apply.rs` table handling

**Files:**
- Modify: `src/editor/apply.rs:66-77` (LayoutPositions)
- Modify: `src/editor/apply.rs:91-230` (apply_attribute_runs)
- Modify: `src/editor/apply.rs:300-311` (apply_attr_set match)
- Modify: `src/editor/apply.rs:486-515` (add collapsed para style helper)

**Step 1: Add `TableGrid` struct and update `LayoutPositions`**

In `src/editor/apply.rs`, add the `TableGrid` struct before `LayoutPositions` (before line 65):

```rust
/// Per-table grid data for drawing continuous grid lines.
#[derive(Debug, Clone)]
pub struct TableGrid {
    /// UTF-16 positions of inner column pipes (from header row).
    /// Excludes first/last pipe (those are the border).
    pub column_seps: Vec<usize>,
    /// UTF-16 positions of each body row start.
    /// Line at top of each body row = boundary to the row above.
    pub row_seps: Vec<usize>,
    /// Table bounding positions (start_utf16, end_utf16).
    pub bounds: (usize, usize),
}
```

Replace `LayoutPositions` fields (lines 66-77):
```rust
pub struct LayoutPositions {
    /// UTF-16 offsets of H1/H2 heading paragraph starts (separator lines).
    pub heading_seps: Vec<usize>,
    /// UTF-16 offsets of thematic breaks (horizontal rules).
    pub thematic_breaks: Vec<usize>,
    /// Per-table grid data for drawing borders and grid lines.
    pub table_grids: Vec<TableGrid>,
}
```

Update the empty early-return (around line 100-107):
```rust
        return LayoutPositions {
            heading_seps: Vec::new(),
            thematic_breaks: Vec::new(),
            table_grids: Vec::new(),
        };
```

**Step 2: Remove `TableSeparatorLine`/`TablePipe` from run loop and apply_attr_set**

In `apply_attribute_runs`, remove lines 142-143 (`table_h_sep_positions`, `table_pipe_sep_positions` declarations).

Remove lines 177-191 (the `TableSeparatorLine` and `TablePipe` handling inside the run loop).

In `apply_attr_set` (around line 300-311), remove the `TableSeparatorLine` and `TablePipe` arms from the match:
```rust
            // Before:
            TextAttribute::ListMarker
            | TextAttribute::BlockquoteBar
            | TextAttribute::LineSpacing(_)
            | TextAttribute::HeadingSeparator
            | TextAttribute::ThematicBreak
            | TextAttribute::TableSeparatorLine
            | TextAttribute::TablePipe => {}

            // After:
            TextAttribute::ListMarker
            | TextAttribute::BlockquoteBar
            | TextAttribute::LineSpacing(_)
            | TextAttribute::HeadingSeparator
            | TextAttribute::ThematicBreak => {}
```

**Step 3: Rewrite table_infos processing to compute `TableGrid`**

Replace the table_infos loop (around lines 194-221) with:

```rust
    // ── Compute per-table grid data ──────────────────────────────────────
    let mut table_grids: Vec<TableGrid> = Vec::new();
    for table_info in table_infos {
        let start_u16 = byte_to_utf16(text, table_info.source_range.0);
        let end_u16 = byte_to_utf16(text, table_info.source_range.1);
        let bounds = (start_u16, end_u16);

        if !table_info.cursor_inside {
            // Apply kern (10px left padding) to every pipe character.
            for row_pipes in &table_info.row_pipes {
                for &pipe_pos in row_pipes {
                    let u16_pos = byte_to_utf16(text, pipe_pos);
                    let range = NSRange { location: u16_pos, length: 1 };
                    let kern_value = NSNumber::numberWithFloat(10.0);
                    unsafe {
                        storage.addAttribute_value_range(
                            NSKernAttributeName,
                            kern_value.as_ref(),
                            range,
                        );
                    }
                }
            }

            equalize_table_columns(storage, text, &table_info.row_pipes);

            // Apply vertical padding to each data row.
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

            // Collapse the separator row (between header and first body row).
            if table_info.row_ranges.len() >= 2 {
                let sep_start = table_info.row_ranges[0].1;
                let sep_end = table_info.row_ranges[1].0;
                if sep_start < sep_end {
                    let sep_start_u16 = byte_to_utf16(text, sep_start);
                    let sep_end_u16 = byte_to_utf16(text, sep_end);
                    if sep_start_u16 < sep_end_u16 {
                        let sep_range = NSRange {
                            location: sep_start_u16,
                            length: sep_end_u16 - sep_start_u16,
                        };
                        let collapsed = make_collapsed_para_style();
                        unsafe {
                            storage.addAttribute_value_range(
                                NSParagraphStyleAttributeName,
                                collapsed.as_ref(),
                                sep_range,
                            );
                        }
                    }
                }
            }

            // Column separators: inner pipes from header row (skip first/last = border).
            let column_seps = if let Some(first_pipes) = table_info.row_pipes.first() {
                if first_pipes.len() >= 3 {
                    first_pipes[1..first_pipes.len() - 1]
                        .iter()
                        .map(|&pos| byte_to_utf16(text, pos))
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            // Row separators: start of each body row (= all rows after header).
            let row_seps = if table_info.row_ranges.len() >= 2 {
                table_info.row_ranges[1..]
                    .iter()
                    .map(|&(start, _)| byte_to_utf16(text, start))
                    .collect()
            } else {
                Vec::new()
            };

            table_grids.push(TableGrid { column_seps, row_seps, bounds });
        } else {
            // Cursor inside: only bounds for border, no grid lines.
            table_grids.push(TableGrid {
                column_seps: Vec::new(),
                row_seps: Vec::new(),
                bounds,
            });
        }
    }
```

**Step 4: Update return value**

Replace the `LayoutPositions` construction at the end of the function (around lines 223-229):
```rust
    LayoutPositions {
        heading_seps: heading_sep_positions,
        thematic_breaks: thematic_break_positions,
        table_grids,
    }
```

**Step 5: Add `make_collapsed_para_style` helper**

Add after `make_table_row_para_style` (around line 515):
```rust
/// Build a paragraph style that collapses a line to near-zero height.
/// Used for the table separator row (`| --- | --- |`) which must be invisible.
fn make_collapsed_para_style() -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    style.setLineSpacing(0.0);
    style.setParagraphSpacingBefore(0.0);
    style.setParagraphSpacing(0.0);
    style.setMaximumLineHeight(0.001);
    style
}
```

**Step 6: Run `cargo check`**

Run: `cargo check 2>&1 | head -40`
Expected: `apply.rs` and `renderer.rs` compile. Errors only in `text_storage.rs` and `text_view.rs` (fixed in Tasks 4-5).

**Step 7: Commit**

```bash
git add src/editor/apply.rs
git commit -m "refactor(apply): replace flat table position lists with per-table TableGrid"
```

---

### Task 4: Update `text_storage.rs` — plumb `TableGrid`

**Files:**
- Modify: `src/editor/text_storage.rs`

**Step 1: Replace three table ivar fields with one**

In `MditEditorDelegateIvars` (lines 37-42), replace:
```rust
    /// UTF-16 offsets of table row boundaries for horizontal grid lines.
    table_h_sep_positions: RefCell<Vec<usize>>,
    /// UTF-16 offsets of table pipe characters for vertical grid lines.
    table_pipe_sep_positions: RefCell<Vec<usize>>,
    /// Per-table (start_utf16, end_utf16) for drawing rounded borders.
    table_bounds: RefCell<Vec<(usize, usize)>>,
```
with:
```rust
    /// Per-table grid data for drawing borders and grid lines.
    table_grids: RefCell<Vec<TableGrid>>,
```

Add the import at the top (line 10):
```rust
use crate::editor::apply::{apply_attribute_runs, collect_code_block_infos, CodeBlockInfo, TableGrid};
```

**Step 2: Update `new()` constructor** (around line 124-126)

Replace:
```rust
            table_h_sep_positions: RefCell::new(Vec::new()),
            table_pipe_sep_positions: RefCell::new(Vec::new()),
            table_bounds: RefCell::new(Vec::new()),
```
with:
```rust
            table_grids: RefCell::new(Vec::new()),
```

**Step 3: Update `did_process_editing`** (around lines 97-99)

Replace:
```rust
            *self.ivars().table_h_sep_positions.borrow_mut() = positions.table_h_seps;
            *self.ivars().table_pipe_sep_positions.borrow_mut() = positions.table_pipe_seps;
            *self.ivars().table_bounds.borrow_mut() = positions.table_bounds;
```
with:
```rust
            *self.ivars().table_grids.borrow_mut() = positions.table_grids;
```

**Step 4: Update `reapply()`** (around lines 179-181)

Same replacement as step 3:
```rust
        *self.ivars().table_grids.borrow_mut() = positions.table_grids;
```

**Step 5: Replace three accessor methods with one** (lines 207-222)

Replace `table_h_sep_positions()`, `table_pipe_sep_positions()`, `table_bounds()` with:
```rust
    /// Returns per-table grid data for drawing borders and grid lines.
    pub fn table_grids(&self) -> Vec<TableGrid> {
        self.ivars().table_grids.borrow().clone()
    }
```

**Step 6: Run `cargo check`**

Run: `cargo check 2>&1 | head -40`
Expected: only `text_view.rs` errors remain (still references old accessor methods).

**Step 7: Commit**

```bash
git add src/editor/text_storage.rs
git commit -m "refactor(text_storage): replace three table fields with single table_grids"
```

---

### Task 5: Rewrite drawing code in `text_view.rs`

**Files:**
- Modify: `src/editor/text_view.rs:275-531`

**Step 1: Add `table_rects_from_grids` helper**

Replace the existing `table_rects` method (lines 430-493) with a method that takes grids as parameter. Keep the same geometry logic but source bounds from `TableGrid`:

```rust
    /// Compute the bounding rect for each table from its `TableGrid` bounds.
    fn table_rects_from_grids(
        &self,
        grids: &[TableGrid],
    ) -> Vec<NSRect> {
        if grids.is_empty() {
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
        for grid in grids {
            let (start_u16, end_u16) = grid.bounds;
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

**Step 2: Update `draw_table_fills`** (lines 497-518)

Replace `self.table_rects()` call with getting grids from delegate and computing rects:

```rust
    fn draw_table_fills(&self) {
        let delegate_ref = self.ivars().delegate.borrow();
        let delegate = match delegate_ref.as_ref() {
            Some(d) => d,
            None => return,
        };
        let grids = delegate.table_grids();
        let rects = self.table_rects_from_grids(&grids);
        if rects.is_empty() {
            return;
        }
        let (r, g, b) = delegate.scheme().table_bg;
        drop(delegate_ref);
        let fill_color = NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0);
        for block_rect in &rects {
            let path =
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*block_rect, 6.0, 6.0);
            fill_color.setFill();
            path.fill();
        }
    }
```

**Step 3: Update `draw_table_borders`** (lines 522-531)

```rust
    fn draw_table_borders(&self) {
        let delegate_ref = self.ivars().delegate.borrow();
        let delegate = match delegate_ref.as_ref() {
            Some(d) => d,
            None => return,
        };
        let grids = delegate.table_grids();
        drop(delegate_ref);
        let rects = self.table_rects_from_grids(&grids);
        for block_rect in &rects {
            let border_path =
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*block_rect, 6.0, 6.0);
            border_path.setLineWidth(1.0);
            NSColor::tertiaryLabelColor().setStroke();
            border_path.stroke();
        }
    }
```

**Step 4: Rewrite `draw_table_h_separators`** (lines 275-349)

Full rewrite — iterate grids, draw full-width horizontal lines at row boundaries:

```rust
    fn draw_table_h_separators(&self) {
        let delegate_ref = self.ivars().delegate.borrow();
        let delegate = match delegate_ref.as_ref() {
            Some(d) => d,
            None => return,
        };
        let grids = delegate.table_grids();
        drop(delegate_ref);
        if grids.is_empty() {
            return;
        }

        let layout_manager = match unsafe { self.layoutManager() } {
            Some(lm) => lm,
            None => return,
        };

        let tc_origin = self.textContainerOrigin();
        let rects = self.table_rects_from_grids(&grids);

        // Clip to table border rects so lines don't extend beyond rounded corners.
        if !rects.is_empty() {
            let ctx_cls = objc2::runtime::AnyClass::get(c"NSGraphicsContext").unwrap();
            let _: () = unsafe { msg_send![ctx_cls, saveGraphicsState] };
            let clip_path = NSBezierPath::bezierPath();
            for rect in &rects {
                let rounded = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*rect, 6.0, 6.0);
                clip_path.appendBezierPath(&rounded);
            }
            clip_path.addClip();
        }

        let sep_color = NSColor::tertiaryLabelColor();
        sep_color.setFill();
        let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();

        for (grid, table_rect) in grids.iter().zip(rects.iter()) {
            for &utf16_pos in &grid.row_seps {
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

                let y = frag_rect.origin.y + tc_origin.y;
                let line_rect = NSRect::new(
                    NSPoint::new(table_rect.origin.x, y - 0.5),
                    NSSize::new(table_rect.size.width, 1.0),
                );
                NSRectFill(line_rect);
            }
        }

        if !rects.is_empty() {
            let ctx_cls = objc2::runtime::AnyClass::get(c"NSGraphicsContext").unwrap();
            let _: () = unsafe { msg_send![ctx_cls, restoreGraphicsState] };
        }
    }
```

**Step 5: Rewrite `draw_table_v_separators`** (lines 352-427)

Full rewrite — iterate grids, draw full-height vertical lines at column boundaries:

```rust
    fn draw_table_v_separators(&self) {
        let delegate_ref = self.ivars().delegate.borrow();
        let delegate = match delegate_ref.as_ref() {
            Some(d) => d,
            None => return,
        };
        let grids = delegate.table_grids();
        drop(delegate_ref);
        if grids.is_empty() {
            return;
        }

        let layout_manager = match unsafe { self.layoutManager() } {
            Some(lm) => lm,
            None => return,
        };

        let tc_origin = self.textContainerOrigin();
        let rects = self.table_rects_from_grids(&grids);

        // Clip to table border rects.
        if !rects.is_empty() {
            let ctx_cls = objc2::runtime::AnyClass::get(c"NSGraphicsContext").unwrap();
            let _: () = unsafe { msg_send![ctx_cls, saveGraphicsState] };
            let clip_path = NSBezierPath::bezierPath();
            for rect in &rects {
                let rounded = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*rect, 6.0, 6.0);
                clip_path.appendBezierPath(&rounded);
            }
            clip_path.addClip();
        }

        let sep_color = NSColor::tertiaryLabelColor();
        sep_color.setFill();
        let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();

        for (grid, table_rect) in grids.iter().zip(rects.iter()) {
            for &utf16_pos in &grid.column_seps {
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
                let line_rect = NSRect::new(
                    NSPoint::new(x - 0.5, table_rect.origin.y),
                    NSSize::new(1.0, table_rect.size.height),
                );
                NSRectFill(line_rect);
            }
        }

        if !rects.is_empty() {
            let ctx_cls = objc2::runtime::AnyClass::get(c"NSGraphicsContext").unwrap();
            let _: () = unsafe { msg_send![ctx_cls, restoreGraphicsState] };
        }
    }
```

**Step 6: Add `TableGrid` import**

At the top of `text_view.rs`, ensure `TableGrid` is imported. Find the existing import from `apply` (if any) or add near the other crate imports:

```rust
use crate::editor::apply::TableGrid;
```

**Step 7: Run `cargo check` then `cargo test --tests`**

Run: `cargo check 2>&1 | head -40`
Run: `cargo test --tests 2>&1`
Expected: all compile, all tests pass.

**Step 8: Commit**

```bash
git add src/editor/text_view.rs
git commit -m "refactor(text_view): grid-based table drawing with full-extent lines"
```

---

### Task 6: Build and visual test

**Step 1: Build the app**

Run: `cargo build 2>&1 | tail -5`
Expected: clean build.

**Step 2: Visual test**

Open the app and load a markdown file with a table. Verify:
- No double horizontal lines between rows
- Vertical lines are continuous (no gaps at row boundaries)
- No extra empty column on the right
- Header/body separator line is at the correct position
- Rounded border and background fill still work
- Grid lines are clipped to rounded corners
- Table looks correct when cursor is inside (raw markdown) and outside (styled)

**Step 3: Commit any fixes if needed**
