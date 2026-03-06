# Table Grid Fix Design

## Problem

The current table rendering has three visual bugs:

1. **Double horizontal lines** — The separator row (`| --- | --- |`) is hidden
   (font 0.001) but still occupies layout space due to default `lineSpacing: 9.6`.
   Its position is recorded for h-sep drawing, creating a spurious line near the
   actual row boundary line.
2. **Broken vertical lines** — Vertical lines are drawn per-row spanning only
   that row's line fragment height. Gaps between fragments (paragraph spacing,
   separator row) cause visible breaks.
3. **Extra empty column** — The last pipe in each row is treated as a column
   separator, drawing a vertical line that creates a phantom column on the right.

## Root Cause

The data flow uses flat lists of UTF-16 positions (`table_h_seps`,
`table_pipe_seps`) collected from per-character attributes (`TableSeparatorLine`,
`TablePipe`). The drawing code maps each position to a single line fragment and
draws within that fragment. This per-fragment approach cannot produce continuous
grid lines across rows.

## Solution: Per-Table Grid Data

Replace flat position lists with a per-table `TableGrid` struct that gives the
drawing code everything it needs to draw a complete, continuous grid.

### New Data Structure

```rust
pub struct TableGrid {
    /// UTF-16 positions of inner column pipes (from header row).
    /// Excludes first/last pipe (border). Used for vertical line x-coords.
    pub column_seps: Vec<usize>,

    /// UTF-16 positions of each body row start.
    /// Used for horizontal line y-coords. Line at top of each body row
    /// = boundary to the row above.
    pub row_seps: Vec<usize>,

    /// Table bounding positions (start_utf16, end_utf16).
    pub bounds: (usize, usize),
}
```

### Attribute Simplification

Remove `TableSeparatorLine` and `TablePipe` from `TextAttribute`. They existed
only to shuttle positions to the drawing code — `TableGrid` replaces this.

Pipe characters and the separator row still get `Hidden` via `syntax_attrs()`.
Kern on pipes and grid positions are computed directly from `TableInfo.row_pipes`
in `apply.rs`.

### Separator Row Collapse

The separator row range is derived as the gap between `row_ranges[0].1` (header
end) and `row_ranges[1].0` (first body row start). A collapsed paragraph style
is applied: `lineSpacing: 0`, `paragraphSpacingBefore: 0`, `paragraphSpacing: 0`,
`maximumLineHeight: 0.001`.

### Drawing: Full-Extent Grid Lines

**Vertical lines**: For each `column_sep`, compute x from glyph location, then
draw from `table_rect.top` to `table_rect.bottom` (full height, not per-fragment).

**Horizontal lines**: For each `row_sep`, compute y from line fragment top, then
draw from `table_rect.left` to `table_rect.right` (full width).

Both clipped to the 6pt rounded border rect.

### Helper Refactor

`table_rects()` becomes `table_rects_from_grids(&[TableGrid]) -> Vec<NSRect>` to
avoid double delegate borrowing and share between fills, borders, and separators.

## Changes Per File

| File | Change |
|---|---|
| `attributes.rs` | Remove `TableSeparatorLine`, `TablePipe` variants |
| `renderer.rs` | Simplify `collect_table`: pipes/separator get only `Hidden`; remove `needs_h_sep` |
| `apply.rs` | Add `TableGrid` struct; compute grid from `TableInfo`; apply kern directly; collapse separator row; replace flat lists in `LayoutPositions` |
| `text_storage.rs` | Replace three table fields with `table_grids: Vec<TableGrid>` + accessor |
| `text_view.rs` | Rewrite `draw_table_v/h_separators` for full-extent grid; add `table_rects_from_grids` helper |
| `formatting_tests.rs` | Update attribute expectations (no more `TablePipe`/`TableSeparatorLine`) |
