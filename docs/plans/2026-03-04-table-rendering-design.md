# Table Rendering Design

## Goal

Render markdown tables with full grid lines (horizontal + vertical) and inline formatting support. Tables should appear WYSIWYG when the cursor is outside, showing drawn grid lines instead of pipe characters and separator dashes.

## Behavior

### Cursor outside the table

- Pipe `|` characters hidden (font-size ~0)
- Separator row `|---|---|` hidden
- Horizontal lines drawn between all rows (same style as HeadingSeparator: 0.5pt filled rect)
- Vertical lines drawn at pipe positions
- Inline formatting (bold, italic, code, strikethrough) active in cells

### Cursor inside the table

- Everything visible in syntax color (pipes, separator row, dashes)
- No drawn lines
- Inline formatting still active

## Architecture

### 1. Parser (`parser.rs`)

Add `TableRow` and `TableCell` to `NodeKind` enum. Comrak already provides the tree structure (Table > TableRow > TableCell > inline content). The separator row `|---|---|` is not in the AST — it is identified as the byte-range gap between the header row's end and the first body row's start.

### 2. Renderer (`renderer.rs`)

New `collect_table()` function:
- Identify pipe `|` characters in source text, mark as syntax markers
- Identify separator row by byte-range gap, mark as syntax markers with `TableSeparatorLine` attribute
- Recursively process cell children via existing `collect_runs()` for inline formatting
- Cursor position determines visible/hidden state

### 3. Attributes (`attributes.rs`)

New `TextAttribute` variants:
- `TableSeparatorLine` — marks row boundaries for horizontal line drawing
- `TablePipe` — marks pipe positions for vertical line drawing

### 4. Apply (`apply.rs`) + TextStorage (`text_storage.rs`)

Collect horizontal separator positions and pipe positions as UTF-16 offsets (same pattern as `thematic_break_positions`). Store in `text_storage.rs` via `RefCell<Vec<usize>>`.

### 5. TextView (`text_view.rs`)

- `draw_table_h_separators()` — horizontal lines between rows
- `draw_table_v_separators()` — vertical lines at pipe glyph positions (queried from NSLayoutManager)
- Both use same 0.5pt line style and separator color as HeadingSeparator

## Data Flow

```
Markdown source
    ↓ parse()
Table > TableRow > TableCell > inline spans
    ↓ compute_attribute_runs()
AttributeRuns with syntax markers + TableSeparatorLine/TablePipe
    ↓ apply_attribute_runs()
NSTextStorage + collected positions
    ↓ drawRect()
Drawn grid lines (horizontal + vertical)
```
