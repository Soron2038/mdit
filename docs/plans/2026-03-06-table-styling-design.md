# Table Styling Design

## Goal

Improve table visual appearance to match the polished look of code blocks:
rounded border, subtle background, thicker/darker grid lines, and cell padding.

## Changes

### 1. Rounded Border

- `NSBezierPath` rounded rect with 6.0pt corner radius (matches code blocks)
- 1.0pt line width, `tertiaryLabelColor`
- Bounding box spans first to last table row, full text container width
- New data: per-table start/end UTF-16 positions stored in `LayoutPositions`

### 2. Background Fill

- New `table_bg` color in `ColorScheme`, similar subtlety to `code_block_bg`
- Drawn in `drawViewBackgroundInRect:` (before glyphs, same layer as code block fills)

### 3. Inner Grid Lines

- Thickness: 0.5pt -> 1.0pt
- Color: `separatorColor` -> `tertiaryLabelColor`
- Clipped to the rounded rect so lines don't extend beyond the border

### 4. Cell Padding (10px all sides)

- **Horizontal**: Kern attribute on pipe characters (left padding) + expanded
  column max-width by +20px in `equalize_table_columns` (right padding)
- **Vertical**: `NSParagraphStyle` with `paragraphSpacingBefore: 10.0` and
  `paragraphSpacing: 10.0` on each table row

## Affected Files

- `src/editor/text_view.rs` — draw border, background, clipped grid lines
- `src/editor/apply.rs` — kern padding on pipes, paragraph spacing, collect table bounds
- `src/editor/text_storage.rs` — store per-table bounding positions
- `src/editor/renderer.rs` — expose table extent info
- `src/editor/formatting.rs` — add `table_bg` to `ColorScheme`
