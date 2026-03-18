# Code Quality Refactoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate ~475 lines of duplicate code across 5 source files, split oversized functions, and add English doc comments — without changing any behaviour.

**Architecture:** Module-by-module, each file left fully clean before moving on. All 58 existing tests serve as regression guards: run `cargo test` after every commit. No new files created; no public API changes.

**Tech Stack:** Rust, objc2 / AppKit bindings, comrak, syntect. Tests in `tests/` (integration, no AppKit).

---

## File Map

| File | Change type |
|------|-------------|
| `src/editor/renderer.rs` | Extract `collect_symmetric_marker` + `clamp_span_range`, add docs |
| `src/editor/apply.rs` | Extract `mk_utf16_range`, `ParaStyleConfig`/`build_para_style`, split `apply_attribute_runs`, add docs |
| `src/editor/text_view.rs` | Extract `layout_context`, `glyph_for_char`, `frag_rect_for_glyph`, `fill_hline`, `fill_vline`; unify table separator drawing; add docs |
| `src/app.rs` | Extract `dispatch_inline_format`/`dispatch_block_format`, split `did_finish_launching`, add docs |
| `src/ui/sidebar.rs` | Named constants for tracking flags, add docs |
| `src/editor/image_handler.rs` | Replace `#[allow(dead_code)]` with explanatory comment or remove stub |
| `src/editor/math_view.rs` | Replace `#[allow(dead_code)]` with explanatory comment |

---

## Task 1: renderer.rs — baseline

**Files:**
- Modify: `src/editor/renderer.rs`

- [ ] **Step 1: Confirm tests pass before any changes**

```bash
cargo test 2>&1 | tail -5
```
Expected: `test result: ok. 58 passed`

- [ ] **Step 2: No commit needed — Task 1 is a baseline check only**

---

## Task 2: renderer.rs — extract `clamp_span_range`

**Files:**
- Modify: `src/editor/renderer.rs:200-246`

- [ ] **Step 1: Add `clamp_span_range` helper after `syntax_attrs` (line ~76)**

Add this function immediately after `fn syntax_attrs(...)`:

```rust
/// Clamp a span's source range to the actual text length.
/// Avoids repeated `.min(text.len())` calls at each call site.
fn clamp_span_range(span: &MarkdownSpan, text_len: usize) -> (usize, usize) {
    (span.source_range.0, span.source_range.1.min(text_len))
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test
```
Expected: 58 passed (function is not yet used; compiler may warn — OK for now)

---

## Task 3: renderer.rs — extract `collect_symmetric_marker`

**Files:**
- Modify: `src/editor/renderer.rs`

The 6 functions `collect_strong`, `collect_emph`, `collect_strikethrough`, `collect_highlight`, `collect_subscript`, `collect_superscript` all follow the same pattern: open-marker run (hidden), content run with `extra_attrs`, optional children, close-marker run (hidden). Only `marker_size` and `extra_attrs` differ.

- [ ] **Step 1: Add the unified helper after `syntax_attrs`/`clamp_span_range`**

```rust
/// Handle any symmetric inline marker: open-marker | content | close-marker.
///
/// `marker_size` — byte width of the opening and closing markers (1 or 2).
/// `extra_attrs` — `TextAttribute`s added on top of `inherited` for the content run.
fn collect_symmetric_marker(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    inherited: &[TextAttribute],
    syn: &AttributeSet,
    marker_size: usize,
    extra_attrs: &[TextAttribute],
    runs: &mut Vec<AttributeRun>,
    table_infos: &mut Vec<TableInfo>,
) {
    let (start, end) = clamp_span_range(span, text.len());
    let m = marker_size.min(end - start);
    runs.push(AttributeRun { range: (start, start + m), attrs: syn.clone() });
    let mut child_attrs = inherited.to_vec();
    child_attrs.extend_from_slice(extra_attrs);
    if span.children.is_empty() {
        if start + m < end.saturating_sub(m) {
            runs.push(AttributeRun {
                range: (start + m, end - m),
                attrs: AttributeSet::new(child_attrs),
            });
        }
    } else {
        for child in &span.children {
            collect_runs(text, child, cursor_pos, &child_attrs, runs, table_infos);
        }
    }
    runs.push(AttributeRun { range: (end - m, end), attrs: syn.clone() });
}
```

- [ ] **Step 2: Replace the 6 specialized functions with thin calls in `collect_runs`**

In `collect_runs`, replace the match arms for `Strong`, `Emph`, `Strikethrough`, `Highlight`, `Subscript`, `Superscript`:

```rust
NodeKind::Strong => {
    collect_symmetric_marker(text, span, cursor_pos, inherited, &syn, 2,
        &[TextAttribute::Bold], runs, table_infos);
}
NodeKind::Emph => {
    collect_symmetric_marker(text, span, cursor_pos, inherited, &syn, 1,
        &[TextAttribute::Italic], runs, table_infos);
}
NodeKind::Strikethrough => {
    collect_symmetric_marker(text, span, cursor_pos, inherited, &syn, 2,
        &[TextAttribute::Strikethrough, TextAttribute::ForegroundColor("strikethrough")],
        runs, table_infos);
}
NodeKind::Highlight => {
    collect_symmetric_marker(text, span, cursor_pos, inherited, &syn, 2,
        &[TextAttribute::BackgroundColor("highlight_bg")], runs, table_infos);
}
NodeKind::Subscript => {
    collect_symmetric_marker(text, span, cursor_pos, inherited, &syn, 1,
        &[TextAttribute::Subscript, TextAttribute::ForegroundColor("subscript")],
        runs, table_infos);
}
NodeKind::Superscript => {
    collect_symmetric_marker(text, span, cursor_pos, inherited, &syn, 1,
        &[TextAttribute::Superscript, TextAttribute::ForegroundColor("superscript")],
        runs, table_infos);
}
```

- [ ] **Step 3: Delete the 6 now-unused private functions**

Remove `fn collect_strong`, `fn collect_emph`, `fn collect_strikethrough`, `fn collect_highlight`, `fn collect_subscript`, `fn collect_superscript` (the 6 functions from line ~191 to ~430).

- [ ] **Step 4: Run tests**

```bash
cargo test
```
Expected: 58 passed

---

## Task 4: renderer.rs — documentation

**Files:**
- Modify: `src/editor/renderer.rs`

- [ ] **Step 1: Add module-level doc comment at the top of the file (before `use` statements)**

```rust
//! Converts a parsed Markdown AST into a flat list of [`AttributeRun`]s.
//!
//! # Pipeline
//! 1. [`compute_attribute_runs`] walks the [`MarkdownSpan`] tree produced by the parser.
//! 2. Each node kind dispatches to a specialized helper (`collect_heading`,
//!    `collect_link`, [`collect_symmetric_marker`], …) that appends runs.
//! 3. [`fill_gaps`] fills any byte ranges not covered by a run with plain styling.
//!
//! Runs use UTF-8 byte offsets; [`crate::editor::apply`] converts them to
//! UTF-16 before applying to `NSTextStorage`.
```

- [ ] **Step 2: Add `///` doc comment to `fill_gaps` clarifying why gap-filling is necessary**

```rust
/// Fill every byte range not covered by a run with a plain (unstyled) run.
///
/// The AST only produces runs for syntax-significant spans.  Gaps between
/// spans (ordinary prose) must still receive the default body styling so
/// `NSTextStorage` doesn't retain stale attributes from a previous edit.
fn fill_gaps(text_len: usize, mut runs: Vec<AttributeRun>) -> Vec<AttributeRun> {
```

- [ ] **Step 3: Run tests and commit**

```bash
cargo test
git add src/editor/renderer.rs
git commit -m "refactor(renderer): extract collect_symmetric_marker, add docs

Replaces 6 nearly-identical inline-marker functions with a single
parametric helper. Adds module-level and function-level doc comments.
~100 lines removed.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```
Expected: 58 passed

---

## Task 5: apply.rs — extract `mk_utf16_range`

**Files:**
- Modify: `src/editor/apply.rs`

- [ ] **Step 1: Add `mk_utf16_range` before `byte_to_utf16` (near end of file)**

```rust
/// Convert a UTF-8 byte range to an `NSRange` (UTF-16 code-unit offsets).
///
/// Returns `None` if the range is empty or would exceed `text_len_u16`,
/// allowing call sites to `continue` a loop with a single `let-else`.
fn mk_utf16_range(
    text: &str,
    byte_start: usize,
    byte_end: usize,
    text_len_u16: usize,
) -> Option<NSRange> {
    let start_u16 = byte_to_utf16(text, byte_start);
    let end_u16 = byte_to_utf16(text, byte_end);
    if start_u16 >= end_u16 || end_u16 > text_len_u16 {
        return None;
    }
    Some(NSRange { location: start_u16, length: end_u16 - start_u16 })
}
```

- [ ] **Step 2: Replace the repeated NSRange pattern in the per-run loop (~line 173)**

Find the existing pattern in `apply_attribute_runs`:
```rust
let start_u16 = byte_to_utf16(text, run.range.0);
let end_u16 = byte_to_utf16(text, run.range.1);
if start_u16 >= end_u16 || end_u16 > text_len_u16 {
    continue;
}
let range = NSRange { location: start_u16, length: end_u16 - start_u16 };
```
Replace with:
```rust
let Some(range) = mk_utf16_range(text, run.range.0, run.range.1, text_len_u16) else {
    continue;
};
```

- [ ] **Step 3: Replace the same pattern in the table-processing block (~lines 218–219, 235–240, 255–261, 279–281, 290–293)**

For each occurrence of `byte_to_utf16` + `NSRange { location: ..., length: ... }` in the table processing section, apply the same `mk_utf16_range` replacement. Use `let Some(x) = mk_utf16_range(...) else { continue }` in loops, and `if let Some(x) = mk_utf16_range(...)` for conditional blocks.

- [ ] **Step 4: Run tests**

```bash
cargo test
```
Expected: 58 passed

---

## Task 6: apply.rs — consolidate paragraph style builders

**Files:**
- Modify: `src/editor/apply.rs`

- [ ] **Step 1: Add `ParaStyleConfig` struct and `build_para_style` before the existing para style functions (~line 689)**

```rust
/// Configuration for building an `NSMutableParagraphStyle`.
///
/// All fields default to zero/`None`, which means "use AppKit default".
/// Only set the fields that need non-default values.
#[derive(Default)]
struct ParaStyleConfig {
    /// Additional line spacing below each line (points). Maps to `setLineSpacing`.
    line_spacing: f64,
    /// Extra space above the paragraph (points). Maps to `setParagraphSpacingBefore`.
    spacing_before: f64,
    /// Extra space below the paragraph (points). Maps to `setParagraphSpacing`.
    spacing_after: f64,
    /// Head/tail indent (points). Sets `setHeadIndent`, `setFirstLineHeadIndent`,
    /// and `setTailIndent(-indent)` together.
    indent: f64,
    /// Maximum line height for collapsed rows (e.g. table separator row).
    max_line_height: Option<f64>,
}

/// Build an `NSMutableParagraphStyle` from a [`ParaStyleConfig`].
fn build_para_style(cfg: ParaStyleConfig) -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    style.setLineSpacing(cfg.line_spacing);
    if cfg.spacing_before != 0.0 {
        style.setParagraphSpacingBefore(cfg.spacing_before);
    }
    if cfg.spacing_after != 0.0 {
        style.setParagraphSpacing(cfg.spacing_after);
    }
    if cfg.indent != 0.0 {
        style.setHeadIndent(cfg.indent);
        style.setFirstLineHeadIndent(cfg.indent);
        style.setTailIndent(-cfg.indent);
    }
    if let Some(max_h) = cfg.max_line_height {
        style.setMaximumLineHeight(max_h);
    }
    style
}
```

- [ ] **Step 2: Replace all 6 old para style function calls with `build_para_style`**

| Old call | New call |
|---|---|
| `make_para_style(9.6)` | `build_para_style(ParaStyleConfig { line_spacing: 9.6, ..Default::default() })` |
| `make_para_style_with_spacing_before(9.6, 20.0)` | `build_para_style(ParaStyleConfig { line_spacing: 9.6, spacing_before: 20.0, ..Default::default() })` |
| `make_table_row_para_style(0.0, 10.0, 10.0)` | `build_para_style(ParaStyleConfig { spacing_before: 10.0, spacing_after: 10.0, ..Default::default() })` |
| `make_code_block_para_style(9.6, 10.0)` | `build_para_style(ParaStyleConfig { line_spacing: 9.6, indent: 10.0, ..Default::default() })` |
| `make_code_block_para_style_with_spacing(9.6, 10.0, 4.0)` | `build_para_style(ParaStyleConfig { line_spacing: 9.6, indent: 10.0, spacing_before: 4.0, ..Default::default() })` |
| `make_collapsed_para_style()` | `build_para_style(ParaStyleConfig { max_line_height: Some(0.001), ..Default::default() })` |

- [ ] **Step 3: Delete the 6 old para style functions**

Remove `fn make_para_style`, `fn make_para_style_with_spacing_before`, `fn make_table_row_para_style`, `fn make_code_block_para_style`, `fn make_code_block_para_style_with_spacing`, `fn make_collapsed_para_style`.

- [ ] **Step 4: Run tests**

```bash
cargo test
```
Expected: 58 passed

---

## Task 7: apply.rs — split `apply_attribute_runs` into phases

**Files:**
- Modify: `src/editor/apply.rs`

`apply_attribute_runs` currently handles 4 distinct responsibilities in 269 lines. Extract each into a named private function, making the coordinator easy to scan.

- [ ] **Step 1: Extract `reset_to_body_style`**

Extract lines 147–168 (the reset block inside `apply_attribute_runs`) into:

```rust
/// Reset the entire storage to the default body style.
///
/// Clears span-specific attributes (background, strikethrough, kern,
/// superscript) and applies font, foreground color, and paragraph style
/// uniformly so subsequent per-run overrides start from a clean base.
fn reset_to_body_style(
    storage: &NSTextStorage,
    body_font: &NSFont,
    text_color: &NSColor,
    para_style: &NSMutableParagraphStyle,
    full_range: NSRange,
) {
    unsafe {
        storage.addAttribute_value_range(NSFontAttributeName, body_font, full_range);
        storage.addAttribute_value_range(NSForegroundColorAttributeName, text_color, full_range);
        storage.addAttribute_value_range(NSParagraphStyleAttributeName, para_style, full_range);
        storage.removeAttribute_range(NSBackgroundColorAttributeName, full_range);
        storage.removeAttribute_range(NSStrikethroughStyleAttributeName, full_range);
        storage.removeAttribute_range(NSKernAttributeName, full_range);
        storage.removeAttribute_range(NSSuperscriptAttributeName, full_range);
    }
}
```

- [ ] **Step 2: Extract `apply_runs` (per-run loop)**

Extract the per-run loop (lines 170–205) into:

```rust
/// Apply per-run attribute overrides and collect positions of decorative elements.
///
/// Returns `(heading_sep_positions, thematic_break_positions)` — UTF-16
/// offsets used by `MditTextView` to draw separator lines and horizontal rules.
fn apply_runs(
    storage: &NSTextStorage,
    text: &str,
    runs: &[AttributeRun],
    text_len_u16: usize,
    scheme: &ColorScheme,
) -> (Vec<usize>, Vec<usize>) {
    let mut heading_sep_positions: Vec<usize> = Vec::new();
    let mut thematic_break_positions: Vec<usize> = Vec::new();
    for run in runs {
        let Some(range) = mk_utf16_range(text, run.range.0, run.range.1, text_len_u16) else {
            continue;
        };
        apply_attr_set(storage, range, &run.attrs, scheme);

        if run.attrs.contains(&TextAttribute::HeadingSeparator) {
            let has_content_before = !text[..run.range.0].trim().is_empty();
            if has_content_before {
                let heading_style = build_para_style(ParaStyleConfig {
                    line_spacing: 9.6,
                    spacing_before: 20.0,
                    ..Default::default()
                });
                unsafe {
                    storage.addAttribute_value_range(
                        NSParagraphStyleAttributeName,
                        heading_style.as_ref(),
                        range,
                    );
                }
                heading_sep_positions.push(range.location);
            }
        }
        if run.attrs.contains(&TextAttribute::ThematicBreak) {
            thematic_break_positions.push(range.location);
        }
    }
    (heading_sep_positions, thematic_break_positions)
}
```

- [ ] **Step 3: Extract `process_tables`**

Extract the table block (lines 208–308) from `apply_attribute_runs` verbatim, wrapping it in:

```rust
/// Compute per-table grid data and apply table-specific text attributes.
///
/// Returns [`TableGrid`] values for each table — used by `MditTextView`
/// to draw grid lines and borders.
fn process_tables(
    storage: &NSTextStorage,
    text: &str,
    table_infos: &[TableInfo],
    text_len_u16: usize,
) -> Vec<TableGrid> {
    let mut table_grids: Vec<TableGrid> = Vec::new();
    for table_info in table_infos {
        let start_u16 = byte_to_utf16(text, table_info.source_range.0);
        let end_u16   = byte_to_utf16(text, table_info.source_range.1);
        let bounds    = (start_u16, end_u16);

        if !table_info.cursor_inside {
            for row_pipes in &table_info.row_pipes {
                for &pipe_pos in row_pipes {
                    let u16_pos = byte_to_utf16(text, pipe_pos);
                    let range   = NSRange { location: u16_pos, length: 1 };
                    let kern_value = NSNumber::numberWithFloat(10.0);
                    unsafe {
                        storage.addAttribute_value_range(
                            NSKernAttributeName, kern_value.as_ref(), range,
                        );
                    }
                }
            }
            equalize_table_columns(storage, text, &table_info.row_pipes);

            for &(row_start, row_end) in &table_info.row_ranges {
                let Some(row_range) = mk_utf16_range(text, row_start, row_end, text_len_u16)
                    else { continue };
                let style = build_para_style(ParaStyleConfig {
                    spacing_before: 10.0, spacing_after: 10.0, ..Default::default()
                });
                unsafe {
                    storage.addAttribute_value_range(
                        NSParagraphStyleAttributeName, style.as_ref(), row_range,
                    );
                }
            }

            if table_info.row_ranges.len() >= 2 {
                let sep_start = table_info.row_ranges[0].1;
                let sep_end   = table_info.row_ranges[1].0;
                if let Some(sep_range) =
                    mk_utf16_range(text, sep_start, sep_end, text_len_u16)
                {
                    let collapsed = build_para_style(ParaStyleConfig {
                        max_line_height: Some(0.001), ..Default::default()
                    });
                    unsafe {
                        storage.addAttribute_value_range(
                            NSParagraphStyleAttributeName, collapsed.as_ref(), sep_range,
                        );
                    }
                }
            }

            let column_seps = if let Some(first_pipes) = table_info.row_pipes.first() {
                if first_pipes.len() >= 3 {
                    first_pipes[1..first_pipes.len() - 1]
                        .iter()
                        .map(|&pos| byte_to_utf16(text, pos))
                        .collect()
                } else { Vec::new() }
            } else { Vec::new() };

            let row_seps = if table_info.row_ranges.len() >= 2 {
                table_info.row_ranges[1..]
                    .iter()
                    .map(|&(start, _)| byte_to_utf16(text, start))
                    .collect()
            } else { Vec::new() };

            table_grids.push(TableGrid { column_seps, row_seps, bounds });
        } else {
            table_grids.push(TableGrid {
                column_seps: Vec::new(), row_seps: Vec::new(), bounds,
            });
        }
    }
    table_grids
}
```

- [ ] **Step 4: Extract `apply_code_blocks`**

Extract the code block styling + syntax highlighting section (lines 311–383) verbatim:

```rust
/// Apply paragraph styles and per-token syntax highlighting to code blocks.
///
/// Uses pre-computed UTF-16 offsets from `CodeBlockInfo` — no raw `text` parameter needed.
fn apply_code_blocks(
    storage: &NSTextStorage,
    code_block_infos: &[CodeBlockInfo],
    text_len_u16: usize,
    scheme: &ColorScheme,
) {
    for info in code_block_infos {
        if info.start_utf16 >= info.end_utf16 { continue; }
        let range = NSRange {
            location: info.start_utf16,
            length: info.end_utf16 - info.start_utf16,
        };
        let style = build_para_style(ParaStyleConfig {
            line_spacing: 9.6, indent: 10.0, ..Default::default()
        });
        unsafe {
            storage.addAttribute_value_range(NSParagraphStyleAttributeName, style.as_ref(), range);
        }
        if info.code_start_utf16 < info.first_code_line_end_utf16 {
            let first_line_range = NSRange {
                location: info.code_start_utf16,
                length: info.first_code_line_end_utf16 - info.code_start_utf16,
            };
            let spacing_style = build_para_style(ParaStyleConfig {
                line_spacing: 9.6, indent: 10.0, spacing_before: 4.0, ..Default::default()
            });
            unsafe {
                storage.addAttribute_value_range(
                    NSParagraphStyleAttributeName, spacing_style.as_ref(), first_line_range,
                );
            }
        }
    }

    let is_dark = scheme.background.0 < 0.5;
    for info in code_block_infos {
        if info.text.is_empty() { continue; }
        let result = highlight(&info.text, &info.language, is_dark);
        for span in &result.spans {
            let span_start = span.range.0.min(info.text.len());
            let span_end   = span.range.1.min(info.text.len());
            if span_start >= span_end { continue; }
            let s_u16 = info.code_start_utf16
                + info.text[..span_start].encode_utf16().count();
            let e_u16 = info.code_start_utf16
                + info.text[..span_end].encode_utf16().count();
            if s_u16 >= e_u16 || e_u16 > text_len_u16 { continue; }
            let range = NSRange { location: s_u16, length: e_u16 - s_u16 };
            let (r, g, b) = (
                span.color.0 as f64 / 255.0,
                span.color.1 as f64 / 255.0,
                span.color.2 as f64 / 255.0,
            );
            let color = NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0);
            unsafe {
                storage.addAttribute_value_range(
                    NSForegroundColorAttributeName, color.as_ref(), range,
                );
            }
        }
    }
}
```

- [ ] **Step 5: Reduce `apply_attribute_runs` to a coordinator**

```rust
pub fn apply_attribute_runs(
    storage: &NSTextStorage,
    text: &str,
    runs: &[AttributeRun],
    table_infos: &[TableInfo],
    code_block_infos: &[CodeBlockInfo],
    scheme: &ColorScheme,
) -> LayoutPositions {
    let text_len_u16 = text.encode_utf16().count();
    if text_len_u16 == 0 {
        return LayoutPositions {
            heading_seps: Vec::new(),
            thematic_breaks: Vec::new(),
            table_grids: Vec::new(),
        };
    }

    let full_range = NSRange { location: 0, length: text_len_u16 };
    let body_font = serif_font(16.0, false, false);
    let text_color = make_color(scheme.text);
    let para_style = build_para_style(ParaStyleConfig { line_spacing: 9.6, ..Default::default() });

    reset_to_body_style(storage, &body_font, &text_color, &para_style, full_range);
    let (heading_seps, thematic_breaks) = apply_runs(storage, text, runs, text_len_u16, scheme);
    let table_grids = process_tables(storage, text, table_infos, text_len_u16);
    apply_code_blocks(storage, code_block_infos, text_len_u16, scheme);

    LayoutPositions { heading_seps, thematic_breaks, table_grids }
}
```

- [ ] **Step 6: Run tests**

```bash
cargo test
```
Expected: 58 passed

---

## Task 8: apply.rs — documentation + commit

**Files:**
- Modify: `src/editor/apply.rs`

- [ ] **Step 1: Verify the module-level doc comment at the top of the file is still accurate**

The file already has:
```rust
//! Converts pure-Rust `AttributeRun`s into real AppKit text attributes and
//! applies them to an `NSTextStorage`.
//!
//! This is the bridge between the platform-agnostic renderer and AppKit.
```
Update if needed to reflect the new phase structure.

- [ ] **Step 2: Add `///` doc comment to `equalize_table_columns` three-pass algorithm**

Verify the existing doc comment explains Pass 1 (measure), Pass 2 (max), Pass 3 (kern). Add a note that it must be called after fonts are applied (Pass 1 measures rendered widths).

- [ ] **Step 3: Run tests and commit**

```bash
cargo test
git add src/editor/apply.rs
git commit -m "refactor(apply): extract mk_utf16_range, ParaStyleConfig, split apply_attribute_runs

- mk_utf16_range replaces ~20 repeated NSRange construction patterns
- ParaStyleConfig + build_para_style replace 6 overlapping para style functions
- apply_attribute_runs split into 4 named phases (reset, runs, tables, code blocks)

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```
Expected: 58 passed

---

## Task 9: text_view.rs — extract geometry helpers

**Files:**
- Modify: `src/editor/text_view.rs`

- [ ] **Step 1: Add `layout_context` method to `MditTextView` impl block (after `is_viewer_mode`)**

```rust
/// Acquire the layout manager and text container in a single call.
///
/// Both are required by every drawing method. Returns `None` if either
/// is unavailable (layout not yet set up).
fn layout_context(
    &self,
) -> Option<(Retained<NSLayoutManager>, Retained<NSTextContainer>)> {
    let lm = unsafe { self.layoutManager() }?;
    let tc = unsafe { self.textContainer() }?;
    Some((lm, tc))
}
```

- [ ] **Step 2: Add module-level free functions `glyph_for_char` and `frag_rect_for_glyph`**

Add these before `impl MditTextView`:

```rust
/// Look up the glyph index for a UTF-16 character position.
///
/// Returns `None` when the layout manager has not yet laid out that character
/// (NSNotFound ≈ `usize::MAX / 2`).
fn glyph_for_char(lm: &NSLayoutManager, char_idx: usize) -> Option<usize> {
    let idx: usize =
        unsafe { msg_send![lm, glyphIndexForCharacterAtIndex: char_idx] };
    if idx >= usize::MAX / 2 { None } else { Some(idx) }
}

/// Look up the line fragment rect for a glyph index.
///
/// Returns `None` when the layout rect has zero height (layout not yet
/// complete or glyph not visible).
fn frag_rect_for_glyph(lm: &NSLayoutManager, glyph_idx: usize) -> Option<NSRect> {
    let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();
    let rect: NSRect = unsafe {
        msg_send![lm,
            lineFragmentRectForGlyphAtIndex: glyph_idx,
            effectiveRange: null_ptr]
    };
    if rect.size.height == 0.0 { None } else { Some(rect) }
}
```

- [ ] **Step 3: Add `fill_hline` and `fill_vline` free functions**

```rust
/// Fill a 1-point horizontal rule at (x, y) with the given width.
/// The rect is offset by −0.5 on y to centre on the pixel boundary.
fn fill_hline(x: f64, y: f64, width: f64) {
    NSRectFill(NSRect::new(NSPoint::new(x, y - 0.5), NSSize::new(width, 1.0)));
}

/// Fill a 1-point vertical rule at (x, y) with the given height.
/// The rect is offset by −0.5 on x to centre on the pixel boundary.
fn fill_vline(x: f64, y: f64, height: f64) {
    NSRectFill(NSRect::new(NSPoint::new(x - 0.5, y), NSSize::new(1.0, height)));
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test
```
Expected: 58 passed (functions added but not yet used)

---

## Task 10: text_view.rs — update callers to use helpers

**Files:**
- Modify: `src/editor/text_view.rs`

- [ ] **Step 1: Refactor `draw_heading_separators` to use `layout_context`, `glyph_for_char`, `frag_rect_for_glyph`, `fill_hline`**

Replace the repeated boilerplate:
```rust
// OLD — repeated 8-line block
let layout_manager = match unsafe { self.layoutManager() } {
    Some(lm) => lm,
    None => return,
};
let text_container = match unsafe { self.textContainer() } {
    Some(tc) => tc,
    None => return,
};
```
with:
```rust
let (layout_manager, text_container) = match self.layout_context() {
    Some(ctx) => ctx,
    None => return,
};
```

Replace the glyph/frag loop body:
```rust
// OLD
let glyph_idx: usize = unsafe { msg_send![&*layout_manager, glyphIndexForCharacterAtIndex: utf16_pos] };
if glyph_idx == usize::MAX { continue; }
let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();
let frag_rect: NSRect = unsafe { msg_send![...] };
if frag_rect.size.height == 0.0 { continue; }
let y = frag_rect.origin.y + tc_origin.y - 10.0;
let line_rect = NSRect::new(NSPoint::new(x_start, y - 0.5), NSSize::new(x_end - x_start, 1.0));
NSRectFill(line_rect);
```
with:
```rust
let Some(glyph_idx) = glyph_for_char(&layout_manager, utf16_pos) else { continue };
let Some(frag_rect) = frag_rect_for_glyph(&layout_manager, glyph_idx) else { continue };
let y = frag_rect.origin.y + tc_origin.y - 10.0;
fill_hline(x_start, y, x_end - x_start);
```

Apply the same pattern to `draw_thematic_breaks` and `table_rects_from_grids` and `code_block_rects`.

- [ ] **Step 2: Run tests**

```bash
cargo test
```
Expected: 58 passed

---

## Task 11: text_view.rs — unify table separator drawing

**Files:**
- Modify: `src/editor/text_view.rs`

- [ ] **Step 1: Add `SeparatorAxis` enum before `MditTextView`**

```rust
/// Axis for [`MditTextView::draw_table_separators`].
#[derive(Clone, Copy)]
enum SeparatorAxis {
    Horizontal,
    Vertical,
}
```

- [ ] **Step 2: Replace `draw_table_h_separators` and `draw_table_v_separators` with a single `draw_table_separators`**

The two functions are identical except for which grid field they iterate (`row_seps` vs `column_seps`) and how they compute the line geometry. Unified version:

```rust
/// Draw separator lines for all tables on the given axis.
///
/// `Horizontal` draws row boundaries; `Vertical` draws column boundaries.
/// Both clip to the table's rounded-rect border so lines don't overflow corners.
fn draw_table_separators(&self, axis: SeparatorAxis) {
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

    let (layout_manager, _) = match self.layout_context() {
        Some(ctx) => ctx,
        None => return,
    };
    let tc_origin = self.textContainerOrigin();
    let rects = self.table_rects_from_grids(&grids);

    if !rects.is_empty() {
        let ctx_cls = objc2::runtime::AnyClass::get(c"NSGraphicsContext").unwrap();
        let _: () = unsafe { msg_send![ctx_cls, saveGraphicsState] };
        let clip_path = NSBezierPath::bezierPath();
        for rect in &rects {
            let rounded =
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*rect, 8.0, 8.0);
            clip_path.appendBezierPath(&rounded);
        }
        clip_path.addClip();
    }

    NSColor::tertiaryLabelColor().setFill();

    for (grid, table_rect) in grids.iter().zip(rects.iter()) {
        let positions = match axis {
            SeparatorAxis::Horizontal => &grid.row_seps,
            SeparatorAxis::Vertical => &grid.column_seps,
        };
        for &utf16_pos in positions {
            let Some(glyph_idx) = glyph_for_char(&layout_manager, utf16_pos) else { continue };
            let Some(frag_rect) = frag_rect_for_glyph(&layout_manager, glyph_idx) else { continue };
            match axis {
                SeparatorAxis::Horizontal => {
                    let y = frag_rect.origin.y + tc_origin.y;
                    fill_hline(table_rect.origin.x, y, table_rect.size.width);
                }
                SeparatorAxis::Vertical => {
                    let glyph_loc: NSPoint = unsafe {
                        msg_send![&*layout_manager, locationForGlyphAtIndex: glyph_idx]
                    };
                    let x = frag_rect.origin.x + glyph_loc.x + tc_origin.x;
                    fill_vline(x, table_rect.origin.y, table_rect.size.height);
                }
            }
        }
    }

    if !rects.is_empty() {
        let ctx_cls = objc2::runtime::AnyClass::get(c"NSGraphicsContext").unwrap();
        let _: () = unsafe { msg_send![ctx_cls, restoreGraphicsState] };
    }
}
```

- [ ] **Step 3: Update the two call sites in `draw_rect`**

```rust
// Before:
self.draw_table_h_separators();
self.draw_table_v_separators();

// After:
self.draw_table_separators(SeparatorAxis::Horizontal);
self.draw_table_separators(SeparatorAxis::Vertical);
```

- [ ] **Step 4: Delete `draw_table_h_separators` and `draw_table_v_separators`**

- [ ] **Step 5: Run tests**

```bash
cargo test
```
Expected: 58 passed

---

## Task 12: text_view.rs — documentation + commit

**Files:**
- Modify: `src/editor/text_view.rs`

- [ ] **Step 1: Add `///` doc comments to all public methods and drawing methods**

Key methods needing comments:
- `MditTextView` struct: explain it's an NSTextView subclass with overlay drawing
- `draw_heading_separators`: mention it only runs in Viewer mode and why
- `draw_thematic_breaks`: same note
- `draw_code_block_fills` / `draw_code_blocks`: explain pre- vs post-glyph drawing order
- `table_rects_from_grids`: explain it computes full-width block rects

- [ ] **Step 2: Run tests and commit**

```bash
cargo test
git add src/editor/text_view.rs
git commit -m "refactor(text_view): extract geometry helpers, unify table separator drawing

- layout_context(), glyph_for_char(), frag_rect_for_glyph() replace 6x
  duplicated layout manager acquisition patterns
- fill_hline() / fill_vline() replace repeated NSRectFill idioms
- draw_table_h_separators + draw_table_v_separators merged into
  draw_table_separators(axis: SeparatorAxis)
~175 lines removed.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```
Expected: 58 passed

---

## Task 13: app.rs — extract inline format dispatcher

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Add `dispatch_inline_format` to the `AppDelegate` impl block**

Add as a private helper method (not an ObjC action — no `#[unsafe(method(...))]`):

```rust
/// Forward an inline-format toggle to the active editor text view.
///
/// Switches to Editor mode automatically if currently in Viewer mode,
/// so clicking a sidebar button activates editing.
fn dispatch_inline_format(&self, marker: &'static str) {
    if let Some(tv) = self.editor_text_view() {
        toggle_inline_wrap(&tv, marker);
    }
}
```

- [ ] **Step 2: Reduce the 7 toggle-wrap action methods to one-liners**

```rust
#[unsafe(method(applyBold:))]
fn apply_bold(&self, _sender: &AnyObject) { self.dispatch_inline_format("**"); }

#[unsafe(method(applyItalic:))]
fn apply_italic(&self, _sender: &AnyObject) { self.dispatch_inline_format("_"); }

#[unsafe(method(applyInlineCode:))]
fn apply_inline_code(&self, _sender: &AnyObject) { self.dispatch_inline_format("`"); }

#[unsafe(method(applyStrikethrough:))]
fn apply_strikethrough(&self, _sender: &AnyObject) { self.dispatch_inline_format("~~"); }

#[unsafe(method(applyHighlight:))]
fn apply_highlight(&self, _sender: &AnyObject) { self.dispatch_inline_format("=="); }

#[unsafe(method(applySubscript:))]
fn apply_subscript(&self, _sender: &AnyObject) { self.dispatch_inline_format("~"); }

#[unsafe(method(applySuperscript:))]
fn apply_superscript(&self, _sender: &AnyObject) { self.dispatch_inline_format("^"); }
```

Note: `applyLink:` (method #8) calls `insert_link_wrap` with asymmetric prefix/suffix arguments — leave it as-is.

- [ ] **Step 3: Run tests**

```bash
cargo test
```
Expected: 58 passed

---

## Task 14: app.rs — extract block format dispatcher

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Add `dispatch_block_format` helper**

```rust
/// Apply a block-level prefix to the line containing the caret.
///
/// Delegates to the pure `set_block_format()` in `editor::formatting`.
/// Switches to Editor mode automatically if needed.
fn dispatch_block_format(&self, prefix: &'static str) {
    if let Some(tv) = self.editor_text_view() {
        apply_block_format(&tv, prefix);
    }
}
```

- [ ] **Step 2: Reduce the 5 block-format action methods to one-liners**

```rust
#[unsafe(method(applyH1:))]
fn apply_h1(&self, _sender: &AnyObject) { self.dispatch_block_format("# "); }

#[unsafe(method(applyH2:))]
fn apply_h2(&self, _sender: &AnyObject) { self.dispatch_block_format("## "); }

#[unsafe(method(applyH3:))]
fn apply_h3(&self, _sender: &AnyObject) { self.dispatch_block_format("### "); }

#[unsafe(method(applyNormal:))]
fn apply_normal(&self, _sender: &AnyObject) { self.dispatch_block_format(""); }

#[unsafe(method(applyBlockquote:))]
fn apply_blockquote(&self, _sender: &AnyObject) { self.dispatch_block_format("> "); }
```

Note: `applyCodeBlock:` calls `insert_code_block` and `applyHRule:` does an inline text insertion — both stay as-is.

- [ ] **Step 3: Run tests**

```bash
cargo test
```
Expected: 58 passed

---

## Task 15: app.rs — split `did_finish_launching`

**Files:**
- Modify: `src/app.rs`

The 66-line `did_finish_launching` method does 3 distinct things: (a) create + show the window, (b) build the content view hierarchy (tab bar, path bar, sidebar), (c) activate the app and open the initial content.

- [ ] **Step 1: Extract `setup_window_and_menu`**

Move lines 60–78 into a new private method:

```rust
/// Create the main window, build the menu, and present it.
///
/// Stores the window in `self.ivars().window`.
/// Called once from `applicationDidFinishLaunching:`.
fn setup_window_and_menu(&self, app: &NSApplication) {
    let mtm = self.mtm();
    let window = create_window(mtm);
    window.setDelegate(Some(ProtocolObject::from_ref(self)));
    build_main_menu(app, mtm);
    window.center();
    window.makeKeyAndOrderFront(None);
    let target: &AnyObject = unsafe {
        &*(self as *const AppDelegate as *const AnyObject)
    };
    add_titlebar_accessory(&window, mtm, target);
    self.ivars().window.set(window).unwrap();
}
```

> **Implementation note:** `detect_scheme` is called in the coordinator (`did_finish_launching`) **before** `setup_window_and_menu`, so `initial_scheme` is available as a local variable in the coordinator. Do **not** call `detect_scheme` inside `setup_window_and_menu` — the code listing above already reflects this (no `detect_scheme` call inside the helper). The coordinator snippet in Step 3 is the authoritative version.

- [ ] **Step 2: Extract `setup_content_views`**

Move lines 80–110 (TabBar, PathBar, Sidebar creation and storage) into:

```rust
/// Create and add the content view hierarchy (tab bar, path bar, sidebar).
///
/// Must be called after the window is created and stored in `self.ivars().window`.
fn setup_content_views(&self) {
    let mtm = self.mtm();
    let Some(window) = self.ivars().window.get() else { return };
    let content = window.contentView().unwrap();
    let bounds = content.bounds();
    let w = bounds.size.width;
    let h = bounds.size.height;

    let tab_bar = TabBar::new(mtm, w);
    tab_bar.view().setFrame(NSRect::new(
        NSPoint::new(0.0, h - TAB_H),
        NSSize::new(w, TAB_H),
    ));
    content.addSubview(tab_bar.view());

    let path_bar = PathBar::new(mtm, w);
    content.addSubview(path_bar.view());

    let content_h = (h - TAB_H - PATH_H).max(0.0);
    let target: &AnyObject = unsafe {
        &*(self as *const AppDelegate as *const AnyObject)
    };
    let sidebar = FormattingSidebar::new(mtm, content_h, target);
    sidebar.view().setFrame(NSRect::new(
        NSPoint::new(0.0, PATH_H),
        NSSize::new(SIDEBAR_W, content_h),
    ));
    content.addSubview(sidebar.view());

    let _ = self.ivars().tab_bar.set(tab_bar);
    let _ = self.ivars().path_bar.set(path_bar);
    let _ = self.ivars().sidebar.set(sidebar);
}
```

- [ ] **Step 3: Update `did_finish_launching` to be a slim coordinator**

```rust
#[unsafe(method(applicationDidFinishLaunching:))]
fn did_finish_launching(&self, notification: &NSNotification) {
    let app = notification
        .object().unwrap()
        .downcast::<NSApplication>().unwrap();
    let initial_scheme = detect_scheme(&app);

    self.setup_window_and_menu(&app);
    self.setup_content_views();

    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    let pending = self.ivars().pending_open.borrow_mut().take();
    self.add_empty_tab();
    self.apply_scheme(initial_scheme);
    if let Some(path) = pending {
        self.open_file_by_path(path);
    }
    self.update_text_container_inset();
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test
```
Expected: 58 passed

---

## Task 16: app.rs — documentation + commit

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Add `///` doc comments to the key `AppDelegate` methods**

Methods that benefit most from comments:
- `content_frame` — explain the sidebar offset
- `editor_text_view` — explain why it auto-toggles to Editor mode
- `open_file_by_path` — explain the pristine-tab-reuse logic
- `perform_save` — explain the None-index convention
- `update_text_container_inset` — explain the max-width centring formula
- `dispatch_inline_format` / `dispatch_block_format` — brief note already added above

- [ ] **Step 2: Run tests and commit**

```bash
cargo test
git add src/app.rs
git commit -m "refactor(app): extract format dispatchers, split did_finish_launching, add docs

- dispatch_inline_format / dispatch_block_format replace 12 near-identical action stubs
- did_finish_launching split into setup_window_and_menu + setup_content_views
- Doc comments on key AppDelegate methods
~75 lines removed.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```
Expected: 58 passed

---

## Task 17: sidebar.rs — tracking flags + documentation

**Files:**
- Modify: `src/ui/sidebar.rs`

- [ ] **Step 1: Replace raw NSTrackingOptions bit flags with named constants**

Find in `update_tracking_areas`:
```rust
let options: usize = 0x01 | 0x02 | 0x20;
```

Replace with named constants defined at the top of the file (after the other constants):
```rust
// NSTrackingArea option flags (not exposed as typed constants in objc2 0.6).
// Values from NSTrackingAreaOptions in AppKit headers.
const NS_TRACKING_MOUSE_ENTERED_AND_EXITED: usize = 0x01;
const NS_TRACKING_MOUSE_MOVED: usize = 0x02;
const NS_TRACKING_ACTIVE_IN_ACTIVE_APP: usize = 0x20;
```

And in `update_tracking_areas`:
```rust
let options: usize = NS_TRACKING_MOUSE_ENTERED_AND_EXITED
    | NS_TRACKING_MOUSE_MOVED
    | NS_TRACKING_ACTIVE_IN_ACTIVE_APP;
```

- [ ] **Step 2: Verify/update the module-level `//!` doc comment**

The file already starts with:
```rust
//! Permanent left-margin formatting sidebar with Notion-style icon buttons.
//!
//! Custom `NSView` subclass that draws SF Symbol icons (or styled text for
//! headings) with hover effects: a rounded-rect pill background and accent
//! color tinting.
```
Read it and confirm it still accurately describes the file (it should — no structural changes were made). No edit needed if accurate.

- [ ] **Step 3: Add `///` doc comments to `SidebarButtonView`, `ButtonKind`, and public methods**

- `SidebarButtonView`: "Custom NSView that draws formatting buttons with hover/press feedback."
- `ButtonKind`: Explain why `StyledText` needs font size/weight (H1/H2/H3 hierarchy without SF Symbols).
- `FormattingSidebar::new`: Already has a good doc comment — verify it's still accurate.
- `FormattingSidebar::set_height`, `set_accent_color`, `apply_separator_color`: add brief doc comments if missing.

- [ ] **Step 4: Run tests and commit**

```bash
cargo test
git add src/ui/sidebar.rs
git commit -m "refactor(sidebar): named constants for tracking flags, add doc comments

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```
Expected: 58 passed

---

## Task 18: dead code cleanup

**Files:**
- Modify: `src/editor/image_handler.rs`
- Modify: `src/editor/math_view.rs`

- [ ] **Step 1: Resolve `#[allow(dead_code)]` in `image_handler.rs`**

`save_image_from_clipboard` is a `todo!()` stub. The function is part of a planned "paste image from clipboard" feature. Replace the suppressor with a clear comment:

```rust
// Intentionally kept: will be wired up when image-paste support is added.
// Requires NSPasteboard integration on the main thread — see image_handler.rs TODO.
#[allow(dead_code)]
pub fn save_image_from_clipboard(doc_path: &Path) -> Option<String> {
```

No behavioural change; the comment replaces the implicit "suppress silently" intent.

- [ ] **Step 2: Resolve `#[allow(dead_code)]` in `math_view.rs`**

`is_display_math` is a placeholder for KaTeX NSTextAttachment integration. It already has tests (6 passing). Replace the suppressor with:

```rust
// Intentionally kept: part of the future KaTeX math rendering integration.
// Will be used when $$...$$ spans are replaced by WKWebView attachments.
#[allow(dead_code)]
pub fn is_display_math(latex: &str) -> bool {
```

- [ ] **Step 3: Run tests and commit**

```bash
cargo test
git add src/editor/image_handler.rs src/editor/math_view.rs
git commit -m "chore: document intentional dead_code suppressors

Replaces silent #[allow(dead_code)] annotations with explanatory comments
describing the future features they are placeholders for.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```
Expected: 58 passed

---

## Task 19: final verification

- [ ] **Step 1: Full test run**

```bash
cargo test 2>&1 | tail -10
```
Expected: `test result: ok. 58 passed; 0 failed`

- [ ] **Step 2: Clippy pass**

```bash
cargo clippy -- -D warnings 2>&1 | head -40
```
Expected: no new warnings introduced by the refactoring. If clippy flags something, fix it.

- [ ] **Step 3: Release build check**

```bash
cargo build --release 2>&1 | tail -5
```
Expected: clean build, no new warnings.

- [ ] **Step 4: Manual smoke test**

Launch the app (`cargo run` or open `dist/` build):
- Open a `.md` file with headings, tables, and code blocks — Viewer mode renders correctly
- Switch to Editor mode (⌘E) — sidebar buttons work (bold, italic, H1, etc.)
- Toggle back to Viewer mode — rendering unchanged
- Open a second tab, close one tab — tab management works
- Save a file — save dialog / path bar update work

---

## Summary

| Task | File | ~Lines saved |
|------|------|-------------|
| 2–4 | renderer.rs | −100 |
| 5–8 | apply.rs | −110 |
| 9–12 | text_view.rs | −175 |
| 13–16 | app.rs | −75 |
| 17 | sidebar.rs | −10 |
| 18 | image_handler.rs, math_view.rs | 0 (doc only) |
| **Total** | | **~−470 lines** |
