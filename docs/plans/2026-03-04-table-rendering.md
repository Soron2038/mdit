# Table Rendering Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Render markdown tables with full grid lines (horizontal + vertical), hidden pipes/separators when cursor is outside, and working inline formatting in cells.

**Architecture:** Extend the existing parse → render → apply → draw pipeline. Add `TableRow`/`TableCell` to the parser, a new `collect_table()` to the renderer that marks pipes and separator rows as syntax markers with table-specific attributes, collect those positions in `apply.rs`, and draw grid lines in `text_view.rs`.

**Tech Stack:** Rust, comrak 0.50, objc2 AppKit (NSLayoutManager, NSTextView)

---

### Task 1: Parser — Add TableRow and TableCell NodeKinds

**Files:**
- Modify: `src/markdown/parser.rs:8-29` (NodeKind enum)
- Modify: `src/markdown/parser.rs:115-138` (node_to_span match)
- Test: `tests/parser_tests.rs`

**Step 1: Write the failing tests**

Append to `tests/parser_tests.rs`:

```rust
#[test]
fn parses_table_row() {
    let nodes = parse("| A | B |\n|---|---|\n| 1 | 2 |");
    let all = flatten(&nodes);
    assert!(
        all.iter().any(|n| matches!(n.kind, NodeKind::TableRow { header: true })),
        "expected a header TableRow node"
    );
    assert!(
        all.iter().any(|n| matches!(n.kind, NodeKind::TableRow { header: false })),
        "expected a body TableRow node"
    );
}

#[test]
fn parses_table_cell() {
    let nodes = parse("| A | B |\n|---|---|\n| 1 | 2 |");
    let all = flatten(&nodes);
    assert!(
        all.iter().any(|n| n.kind == NodeKind::TableCell),
        "expected a TableCell node"
    );
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test parser_tests parses_table_row parses_table_cell`
Expected: compile error — `TableRow` and `TableCell` do not exist on `NodeKind`.

**Step 3: Add NodeKind variants and comrak mapping**

In `src/markdown/parser.rs`, add to the `NodeKind` enum (after `Table`):

```rust
TableRow { header: bool },
TableCell,
```

In the `node_to_span` match, add before the `_ => NodeKind::Other` arm:

```rust
NodeValue::TableRow(header) => NodeKind::TableRow { header: *header },
NodeValue::TableCell => NodeKind::TableCell,
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test parser_tests`
Expected: all pass.

**Step 5: Commit**

```bash
git add src/markdown/parser.rs tests/parser_tests.rs
git commit -m "feat(parser): add TableRow and TableCell node kinds"
```

---

### Task 2: Attributes — Add TableSeparatorLine and TablePipe

**Files:**
- Modify: `src/markdown/attributes.rs:1-23` (TextAttribute enum)
- Modify: `src/editor/apply.rs:239-243` (apply_attr_set no-op arm)
- Test: `tests/attributes_tests.rs`

**Step 1: Write the failing test**

Append to `tests/attributes_tests.rs`:

```rust
#[test]
fn table_separator_line_attribute_detectable() {
    let set = AttributeSet::new(vec![TextAttribute::TableSeparatorLine]);
    assert!(set.contains(&TextAttribute::TableSeparatorLine));
    assert!(!set.contains(&TextAttribute::TablePipe));
}

#[test]
fn table_pipe_attribute_detectable() {
    let set = AttributeSet::new(vec![TextAttribute::TablePipe]);
    assert!(set.contains(&TextAttribute::TablePipe));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test attributes_tests table_separator table_pipe`
Expected: compile error — variants do not exist.

**Step 3: Add TextAttribute variants**

In `src/markdown/attributes.rs`, add to `TextAttribute` (after the `ThematicBreak` variant):

```rust
/// Marks a table row boundary for horizontal separator line drawing.
TableSeparatorLine,
/// Marks a table pipe `|` position for vertical separator line drawing.
TablePipe,
```

In `src/editor/apply.rs`, add the new variants to the no-op arm in `apply_attr_set` (line ~243):

```rust
TextAttribute::ListMarker
| TextAttribute::BlockquoteBar
| TextAttribute::LineSpacing(_)
| TextAttribute::HeadingSeparator
| TextAttribute::ThematicBreak
| TextAttribute::TableSeparatorLine
| TextAttribute::TablePipe => {}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test attributes_tests`
Expected: all pass.

**Step 5: Commit**

```bash
git add src/markdown/attributes.rs src/editor/apply.rs tests/attributes_tests.rs
git commit -m "feat(attributes): add TableSeparatorLine and TablePipe"
```

---

### Task 3: Renderer — collect_table() with inline formatting

**Files:**
- Modify: `src/editor/renderer.rs:122-127` (replace Table match arm)
- Modify: `src/editor/renderer.rs` (add `collect_table` function)
- Test: `tests/renderer_tests.rs`

**Step 1: Write the failing test**

Append to `tests/renderer_tests.rs`:

```rust
#[test]
fn table_cell_bold_gets_bold_attribute() {
    let text = "| **bold** | plain |\n|---|---|\n| a | b |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    let bold = runs.iter().find(|r| r.attrs.contains(&TextAttribute::Bold));
    assert!(bold.is_some(), "expected Bold attribute in table cell");
}

#[test]
fn table_cell_italic_gets_italic_attribute() {
    let text = "| *italic* | plain |\n|---|---|\n| a | b |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    assert!(
        runs.iter().any(|r| r.attrs.contains(&TextAttribute::Italic)),
        "expected Italic attribute in table cell"
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test renderer_tests table_cell_bold table_cell_italic`
Expected: FAIL — current Table handler uses `for_code_block()` which doesn't process children.

**Step 3: Implement collect_table()**

In `src/editor/renderer.rs`, replace the Table match arm (lines 122-127):

```rust
NodeKind::Table => {
    collect_table(text, span, cursor_pos, runs);
}
```

Add a new `collect_table` function (after `collect_item`, around line 422):

```rust
/// Table: pipes as syntax markers, separator row hidden, cell content with inline formatting.
fn collect_table(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    runs: &mut Vec<AttributeRun>,
) {
    let cursor_in = cursor_in_span(cursor_pos, span.source_range);
    let syn = syntax_attrs(cursor_pos, span.source_range);

    // Partition children into header and body rows.
    let mut header_end: Option<usize> = None;
    let mut first_body_start: Option<usize> = None;
    let mut body_row_count: usize = 0;

    for row in &span.children {
        match &row.kind {
            NodeKind::TableRow { header: true } => {
                header_end = Some(row.source_range.1);
            }
            NodeKind::TableRow { header: false } => {
                if first_body_start.is_none() {
                    first_body_start = Some(row.source_range.0);
                }
            }
            _ => {}
        }
    }

    // ── Mark separator row (gap between last header and first body row) ──
    if let (Some(sep_start), Some(sep_end)) = (header_end, first_body_start) {
        if sep_start < sep_end {
            let mut attrs = syn.clone();
            if !cursor_in {
                attrs = attrs.with(TextAttribute::TableSeparatorLine);
            }
            runs.push(AttributeRun { range: (sep_start, sep_end), attrs });
        }
    }

    // ── Process each data row ────────────────────────────────────────────
    body_row_count = 0;
    for row in &span.children {
        let is_body = matches!(&row.kind, NodeKind::TableRow { header: false });
        if is_body {
            body_row_count += 1;
        }
        if !matches!(&row.kind, NodeKind::TableRow { .. }) {
            continue;
        }

        let needs_h_sep = is_body && body_row_count > 1;

        // Collect cell byte ranges to distinguish structural pipes from cell content.
        let cell_ranges: Vec<(usize, usize)> = row
            .children
            .iter()
            .filter(|c| matches!(c.kind, NodeKind::TableCell))
            .map(|c| c.source_range)
            .collect();

        // Scan for pipe characters in the row.
        let row_end = row.source_range.1.min(text.len());
        let mut is_first_pipe = true;
        for pos in row.source_range.0..row_end {
            if text.as_bytes().get(pos) != Some(&b'|') {
                continue;
            }
            let in_cell = cell_ranges.iter().any(|&(cs, ce)| pos >= cs && pos < ce);
            if in_cell {
                continue;
            }
            let mut pipe_attrs = syn.clone();
            if !cursor_in {
                pipe_attrs = pipe_attrs.with(TextAttribute::TablePipe);
                if is_first_pipe && needs_h_sep {
                    pipe_attrs = pipe_attrs.with(TextAttribute::TableSeparatorLine);
                }
            }
            runs.push(AttributeRun { range: (pos, pos + 1), attrs: pipe_attrs });
            is_first_pipe = false;
        }

        // Process cell children for inline formatting.
        for cell in &row.children {
            if !matches!(cell.kind, NodeKind::TableCell) {
                continue;
            }
            for child in &cell.children {
                collect_runs(text, child, cursor_pos, &[], runs);
            }
        }
    }
}
```

Also add defensive match arms for `TableRow`/`TableCell` to the main `collect_runs` match (these shouldn't be reached in normal flow, but prevent fallthrough to `_`):

```rust
NodeKind::TableRow { .. } | NodeKind::TableCell => {
    for child in &span.children {
        collect_runs(text, child, cursor_pos, inherited, runs);
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test renderer_tests`
Expected: all pass, including new table tests and existing tests.

**Step 5: Commit**

```bash
git add src/editor/renderer.rs tests/renderer_tests.rs
git commit -m "feat(renderer): add collect_table with inline formatting support"
```

---

### Task 4: Renderer — Pipe and separator row syntax markers

**Files:**
- Test: `tests/renderer_tests.rs`

**Step 1: Write the failing tests**

Append to `tests/renderer_tests.rs`:

```rust
#[test]
fn table_pipes_hidden_when_cursor_outside() {
    let text = "| A | B |\n|---|---|\n| 1 | 2 |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, Some(999));
    let pipe_runs: Vec<_> = runs
        .iter()
        .filter(|r| r.attrs.contains(&TextAttribute::TablePipe))
        .collect();
    assert!(!pipe_runs.is_empty(), "expected TablePipe runs for pipe characters");
    for pr in &pipe_runs {
        assert!(pr.attrs.contains(&TextAttribute::Hidden), "pipes should be hidden");
    }
}

#[test]
fn table_pipes_visible_when_cursor_inside() {
    let text = "| A | B |\n|---|---|\n| 1 | 2 |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, Some(3));
    let has_table_pipe = runs.iter().any(|r| r.attrs.contains(&TextAttribute::TablePipe));
    assert!(!has_table_pipe, "no TablePipe when cursor is inside table");
    let has_hidden = runs.iter().any(|r| {
        r.attrs.contains(&TextAttribute::Hidden) && r.range.1 - r.range.0 == 1
    });
    assert!(!has_hidden, "pipes should not be hidden when cursor is inside");
}

#[test]
fn table_separator_row_hidden_when_cursor_outside() {
    let text = "| A | B |\n|---|---|\n| 1 | 2 |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, Some(999));
    let sep_run = runs
        .iter()
        .find(|r| r.attrs.contains(&TextAttribute::TableSeparatorLine));
    assert!(sep_run.is_some(), "expected TableSeparatorLine for separator row");
    assert!(
        sep_run.unwrap().attrs.contains(&TextAttribute::Hidden),
        "separator row should be hidden"
    );
}

#[test]
fn table_separator_row_visible_when_cursor_inside() {
    let text = "| A | B |\n|---|---|\n| 1 | 2 |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, Some(3));
    let has_sep = runs
        .iter()
        .any(|r| r.attrs.contains(&TextAttribute::TableSeparatorLine));
    assert!(!has_sep, "no TableSeparatorLine when cursor is inside table");
}
```

**Step 2: Run tests**

Run: `cargo test --test renderer_tests table_pipes table_separator`
Expected: these tests should already PASS from Task 3's implementation (the pipe and separator logic is included in `collect_table`). If any fail, debug and fix.

**Step 3: Write inter-row horizontal separator test**

```rust
#[test]
fn table_multi_body_rows_get_h_separator() {
    let text = "| A |\n|---|\n| 1 |\n| 2 |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, Some(999));
    let sep_runs: Vec<_> = runs
        .iter()
        .filter(|r| r.attrs.contains(&TextAttribute::TableSeparatorLine))
        .collect();
    // One for the separator row, one for the boundary between body rows.
    assert!(
        sep_runs.len() >= 2,
        "expected at least 2 TableSeparatorLine runs (sep row + body boundary); got {}",
        sep_runs.len()
    );
}
```

**Step 4: Run and verify**

Run: `cargo test --test renderer_tests table_multi`
Expected: PASS.

**Step 5: Commit**

```bash
git add tests/renderer_tests.rs
git commit -m "test(renderer): add table pipe and separator marker tests"
```

---

### Task 5: Update existing table test

**Files:**
- Modify: `tests/renderer_tests.rs:95-103`

**Step 1: Update the existing `table_gets_monospace` test**

The old test expects Monospace for tables. Replace it:

```rust
#[test]
fn table_no_longer_monospace() {
    let text = "| A | B |\n|---|---|\n| 1 | 2 |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    // Tables now use inline formatting, not monospace code-block style.
    let has_monospace = runs.iter().any(|r| r.attrs.contains(&TextAttribute::Monospace));
    assert!(!has_monospace, "tables should not use Monospace styling");
}
```

**Step 2: Run all renderer tests**

Run: `cargo test --test renderer_tests`
Expected: all pass.

**Step 3: Commit**

```bash
git add tests/renderer_tests.rs
git commit -m "test(renderer): update table test — no longer monospace"
```

---

### Task 6: Apply + TextStorage — Collect and store table positions

**Files:**
- Modify: `src/editor/apply.rs:64-70` (LayoutPositions)
- Modify: `src/editor/apply.rs:84-166` (apply_attribute_runs)
- Modify: `src/editor/text_storage.rs:20-37` (ivars)
- Modify: `src/editor/text_storage.rs:55-94` (did_process_editing)
- Modify: `src/editor/text_storage.rs:147-168` (reapply)
- Modify: `src/editor/text_storage.rs:170-186` (accessor methods)

**Step 1: Extend LayoutPositions**

In `src/editor/apply.rs`, add fields to `LayoutPositions`:

```rust
pub struct LayoutPositions {
    pub heading_seps: Vec<usize>,
    pub thematic_breaks: Vec<usize>,
    /// UTF-16 offsets of table row boundaries (horizontal separator lines).
    pub table_h_seps: Vec<usize>,
    /// UTF-16 offsets of table pipe characters (vertical separator lines).
    pub table_pipe_seps: Vec<usize>,
}
```

Update the early return in `apply_attribute_runs` (line ~92):

```rust
return LayoutPositions {
    heading_seps: Vec::new(),
    thematic_breaks: Vec::new(),
    table_h_seps: Vec::new(),
    table_pipe_seps: Vec::new(),
};
```

**Step 2: Add collection logic**

In `apply_attribute_runs`, add two new position vectors (after `thematic_break_positions`):

```rust
let mut table_h_sep_positions: Vec<usize> = Vec::new();
let mut table_pipe_sep_positions: Vec<usize> = Vec::new();
```

After the ThematicBreak check (around line 159), add:

```rust
if run.attrs.contains(&TextAttribute::TableSeparatorLine) {
    table_h_sep_positions.push(start_u16);
}
if run.attrs.contains(&TextAttribute::TablePipe) {
    table_pipe_sep_positions.push(start_u16);
}
```

Update the return value:

```rust
LayoutPositions {
    heading_seps: heading_sep_positions,
    thematic_breaks: thematic_break_positions,
    table_h_seps: table_h_sep_positions,
    table_pipe_seps: table_pipe_sep_positions,
}
```

**Step 3: Add ivars and accessors in text_storage**

In `MditEditorDelegateIvars`, add:

```rust
/// UTF-16 offsets of table row boundaries for horizontal grid lines.
table_h_sep_positions: RefCell<Vec<usize>>,
/// UTF-16 offsets of table pipe characters for vertical grid lines.
table_pipe_sep_positions: RefCell<Vec<usize>>,
```

In `MditEditorDelegate::new`, add to the ivars initialization:

```rust
table_h_sep_positions: RefCell::new(Vec::new()),
table_pipe_sep_positions: RefCell::new(Vec::new()),
```

In `did_process_editing`, after the thematic_break_positions update (line ~88), add:

```rust
*self.ivars().table_h_sep_positions.borrow_mut() = positions.table_h_seps;
*self.ivars().table_pipe_sep_positions.borrow_mut() = positions.table_pipe_seps;
```

In `reapply`, add the same two lines after the thematic_break_positions update (line ~162).

Add accessor methods:

```rust
pub fn table_h_sep_positions(&self) -> Vec<usize> {
    self.ivars().table_h_sep_positions.borrow().clone()
}

pub fn table_pipe_sep_positions(&self) -> Vec<usize> {
    self.ivars().table_pipe_sep_positions.borrow().clone()
}
```

**Step 4: Run full test suite**

Run: `cargo test`
Expected: all pass — apply/text_storage changes are additive.

**Step 5: Commit**

```bash
git add src/editor/apply.rs src/editor/text_storage.rs
git commit -m "feat(apply): collect table separator and pipe positions"
```

---

### Task 7: TextView — Draw table horizontal separator lines

**Files:**
- Modify: `src/editor/text_view.rs:63-71` (draw_rect)
- Modify: `src/editor/text_view.rs` (add draw function)

**Step 1: Add `draw_table_h_separators()`**

Add to `impl MditTextView` (after `draw_thematic_breaks`):

```rust
/// Draw horizontal separator lines between table rows.
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

    let sep_color = NSColor::separatorColor();
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

        // Draw at the top of the line fragment (= boundary between rows).
        let y = frag_rect.origin.y + tc_origin.y;
        let line_rect = NSRect::new(
            NSPoint::new(x_start, y - 0.25),
            NSSize::new(x_end - x_start, 0.5),
        );
        NSRectFill(line_rect);
    }
}
```

**Step 2: Integrate in draw_rect**

In the `draw_rect` method (line ~63-71), add the call:

```rust
fn draw_rect(&self, dirty_rect: NSRect) {
    let _: () = unsafe { msg_send![super(self), drawRect: dirty_rect] };
    self.draw_code_blocks();
    self.draw_heading_separators();
    self.draw_thematic_breaks();
    self.draw_table_h_separators();
}
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: compiles. Manual test: open a markdown file with a table, verify horizontal lines appear between rows.

**Step 4: Commit**

```bash
git add src/editor/text_view.rs
git commit -m "feat(text_view): draw horizontal table separator lines"
```

---

### Task 8: TextView — Draw table vertical separator lines

**Files:**
- Modify: `src/editor/text_view.rs` (add draw function, integrate)

**Step 1: Add `draw_table_v_separators()`**

Add to `impl MditTextView`:

```rust
/// Draw vertical separator lines at table pipe positions.
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

    let sep_color = NSColor::separatorColor();
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

        // Glyph location within the line fragment.
        let glyph_loc: NSPoint = unsafe {
            msg_send![&*layout_manager, locationForGlyphAtIndex: glyph_idx]
        };

        let x = frag_rect.origin.x + glyph_loc.x + tc_origin.x;
        let y_top = frag_rect.origin.y + tc_origin.y;
        let y_bottom = y_top + frag_rect.size.height;

        let line_rect = NSRect::new(
            NSPoint::new(x - 0.25, y_top),
            NSSize::new(0.5, y_bottom - y_top),
        );
        NSRectFill(line_rect);
    }
}
```

**Step 2: Integrate in draw_rect**

```rust
fn draw_rect(&self, dirty_rect: NSRect) {
    let _: () = unsafe { msg_send![super(self), drawRect: dirty_rect] };
    self.draw_code_blocks();
    self.draw_heading_separators();
    self.draw_thematic_breaks();
    self.draw_table_h_separators();
    self.draw_table_v_separators();
}
```

**Step 3: Build and manually test**

Run: `cargo build && cargo run`
Test with a markdown file containing:
```markdown
Some text before.

| Header 1 | **Bold Header** |
|-----------|-----------------|
| *italic*  | `code`          |
| plain     | ~~strike~~      |

Some text after.
```

Verify:
- [ ] Cursor outside: pipes and separator row hidden, grid lines visible
- [ ] Cursor inside: everything visible in syntax color, no drawn lines
- [ ] Bold, italic, code, strikethrough work in cells
- [ ] Horizontal lines between all rows
- [ ] Vertical lines at pipe positions

**Step 4: Commit**

```bash
git add src/editor/text_view.rs
git commit -m "feat(text_view): draw vertical table separator lines"
```

---

### Task 9: Run full test suite and verify

**Step 1: Run all tests**

Run: `cargo test`
Expected: all pass.

**Step 2: Run clippy**

Run: `cargo clippy`
Expected: no warnings in changed code.

**Step 3: Final commit if any fixes**

```bash
git add -A
git commit -m "chore: address clippy warnings from table rendering"
```
