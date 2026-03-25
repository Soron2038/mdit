//! Converts pure-Rust `AttributeRun`s into real AppKit text attributes and
//! applies them to an `NSTextStorage`.
//!
//! This is the bridge between the platform-agnostic renderer and AppKit.
//! `apply_attribute_runs` coordinates four phases: reset, per-run styling,
//! table layout, and syntax highlighting.

use objc2::rc::Retained;
use objc2::msg_send;
use objc2_app_kit::{
    NSBackgroundColorAttributeName, NSColor, NSFont, NSFontAttributeName,
    NSFontDescriptorSymbolicTraits, NSFontWeightBold, NSFontWeightRegular,
    NSForegroundColorAttributeName, NSKernAttributeName, NSLinkAttributeName,
    NSMutableParagraphStyle, NSParagraphStyleAttributeName,
    NSStrikethroughStyleAttributeName, NSSuperscriptAttributeName,
    NSUnderlineStyleAttributeName, NSTextStorage,
};
use objc2_foundation::{NSNumber, NSRange, NSSize, NSString, NSURL};

use crate::editor::renderer::{AttributeRun, TableInfo};
use crate::markdown::attributes::{AttributeSet, TextAttribute};
use crate::markdown::highlighter::highlight;
use crate::markdown::parser::{MarkdownSpan, NodeKind};
use crate::ui::appearance::ColorScheme;

// ---------------------------------------------------------------------------
// Code-block info collection
// ---------------------------------------------------------------------------

/// Metadata about a fenced code block, computed once per edit from the AST.
/// Used by MditTextView to draw the visual box and handle copy-to-clipboard.
#[derive(Debug, Clone)]
pub struct CodeBlockInfo {
    /// UTF-16 code-unit offset of the code block's first character.
    pub start_utf16: usize,
    /// UTF-16 code-unit offset one past the code block's last character.
    pub end_utf16: usize,
    /// UTF-16 code-unit offset of the first code character (after the
    /// opening fence line).  Used to map per-token highlight spans into
    /// document positions.
    pub code_start_utf16: usize,
    /// UTF-16 code-unit offset one past the end of the first code line.
    /// Used to apply paragraph spacing only to that line.
    pub first_code_line_end_utf16: usize,
    /// The raw code content (without fences, trailing newline stripped).
    pub text: String,
    /// The language tag from the opening fence (e.g. "rust"), or empty string.
    pub language: String,
}

/// Walk `spans` to find all `CodeBlock` nodes, convert their byte offsets
/// to UTF-16, and return the list.  Call this after every re-parse.
pub fn collect_code_block_infos(spans: &[MarkdownSpan], text: &str) -> Vec<CodeBlockInfo> {
    let mut result = Vec::new();
    collect_recursive(spans, text, &mut result);
    result
}

fn collect_recursive(spans: &[MarkdownSpan], text: &str, out: &mut Vec<CodeBlockInfo>) {
    for span in spans {
        if let NodeKind::CodeBlock { code, language } = &span.kind {
            // Find where the code content starts (the line after the opening
            // fence), so we can map per-token highlight spans to document
            // UTF-16 positions.
            let block_start = span.source_range.0;
            let block_slice = &text[block_start..span.source_range.1.min(text.len())];
            let code_offset = block_slice.find('\n').map(|p| p + 1).unwrap_or(0);
            let code_start_byte = block_start + code_offset;
            let code_slice = &text[code_start_byte..span.source_range.1.min(text.len())];
            let first_line_len = code_slice.find('\n').map(|p| p + 1).unwrap_or(code_slice.len());
            let code_first_line_end_byte = code_start_byte + first_line_len;

            out.push(CodeBlockInfo {
                start_utf16:               byte_to_utf16(text, block_start),
                end_utf16:                 byte_to_utf16(text, span.source_range.1),
                code_start_utf16:          byte_to_utf16(text, code_start_byte),
                first_code_line_end_utf16: byte_to_utf16(text, code_first_line_end_byte),
                text:                      code.clone(),
                language:                  language.clone(),
            });
        }
        collect_recursive(&span.children, text, out);
    }
}

// ---------------------------------------------------------------------------
// Layout positions computed during attribute application
// ---------------------------------------------------------------------------

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

/// Position info for a task-list checkbox, used by MditTextView for drawing and click handling.
#[derive(Debug, Clone)]
pub struct CheckboxInfo {
    /// UTF-16 code-unit offset of the `[` character.
    pub utf16_pos: usize,
    /// Whether the checkbox is checked.
    pub checked: bool,
    /// Byte offset of the `[` in the source text — used by click handler to toggle.
    pub byte_offset: usize,
}

/// Positions of elements that need custom drawing in the text view.
pub struct LayoutPositions {
    /// UTF-16 offsets of H1/H2 heading paragraph starts (separator lines).
    pub heading_seps: Vec<usize>,
    /// UTF-16 offsets of thematic breaks (horizontal rules).
    pub thematic_breaks: Vec<usize>,
    /// Per-table grid data for drawing grid lines and borders.
    pub table_grids: Vec<TableGrid>,
    /// Checkbox positions for task list items.
    pub checkboxes: Vec<CheckboxInfo>,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Apply `runs` to `storage`, replacing all previous text attributes.
///
/// Returns layout positions used by the text view to draw separator lines
/// and horizontal rules.
///
/// Must be called from the main thread (NSTextStorage is not thread-safe).
/// Safe to call from within `textStorage:didProcessEditing:` — the
/// `editing_chars_only` guard in the delegate prevents infinite recursion.
pub fn apply_attribute_runs(
    storage: &NSTextStorage,
    text: &str,
    runs: &[AttributeRun],
    table_infos: &[TableInfo],
    code_block_infos: &[CodeBlockInfo],
    scheme: &ColorScheme,
    base_size: f64,
) -> LayoutPositions {
    let text_len_u16 = text.encode_utf16().count();
    if text_len_u16 == 0 {
        return LayoutPositions {
            heading_seps: Vec::new(),
            thematic_breaks: Vec::new(),
            table_grids: Vec::new(),
            checkboxes: Vec::new(),
        };
    }

    let full_range = NSRange { location: 0, length: text_len_u16 };
    let body_font = serif_font(base_size, false, false);
    let text_color = make_color(scheme.text);
    let para_style = build_para_style(ParaStyleConfig { line_spacing: 9.6, ..Default::default() });

    reset_to_body_style(storage, &body_font, &text_color, &para_style, full_range);
    let (heading_seps, thematic_breaks, checkboxes) = apply_runs(storage, text, runs, text_len_u16, scheme, base_size);
    let table_grids = process_tables(storage, text, table_infos, text_len_u16);
    apply_code_blocks(storage, code_block_infos, text_len_u16, scheme);

    LayoutPositions { heading_seps, thematic_breaks, table_grids, checkboxes }
}

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
        storage.addAttribute_value_range(
            NSFontAttributeName,
            body_font,
            full_range,
        );
        storage.addAttribute_value_range(
            NSForegroundColorAttributeName,
            text_color,
            full_range,
        );
        storage.addAttribute_value_range(
            NSParagraphStyleAttributeName,
            para_style,
            full_range,
        );
        // Clear attributes that only apply to specific spans.
        storage.removeAttribute_range(NSBackgroundColorAttributeName, full_range);
        storage.removeAttribute_range(NSStrikethroughStyleAttributeName, full_range);
        storage.removeAttribute_range(NSKernAttributeName, full_range);
        storage.removeAttribute_range(NSUnderlineStyleAttributeName, full_range);
        storage.removeAttribute_range(NSSuperscriptAttributeName, full_range);
    }
}

/// Apply per-run attribute overrides and collect positions of decorative elements.
///
/// Returns `(heading_seps, thematic_breaks, checkboxes)` — UTF-16 offsets used by
/// `MditTextView` to draw separator lines, horizontal rules, and task checkboxes.
fn apply_runs(
    storage: &NSTextStorage,
    text: &str,
    runs: &[AttributeRun],
    text_len_u16: usize,
    scheme: &ColorScheme,
    base_size: f64,
) -> (Vec<usize>, Vec<usize>, Vec<CheckboxInfo>) {
    let mut heading_seps: Vec<usize> = Vec::new();
    let mut thematic_breaks: Vec<usize> = Vec::new();
    let mut checkboxes: Vec<CheckboxInfo> = Vec::new();
    for run in runs {
        let Some(range) = mk_utf16_range(text, run.range.0, run.range.1, text_len_u16) else {
            continue;
        };
        apply_attr_set(storage, range, &run.attrs, scheme, base_size);

        if run.attrs.contains(&TextAttribute::HeadingSeparator) {
            // Only add the spacing / record the position when non-whitespace
            // content precedes this heading.  Checking at attribute-application
            // time (once per edit) avoids allocating a String on every drawRect:.
            let has_content_before = !text[..run.range.0].trim().is_empty();
            if has_content_before {
                // Add extra space above the heading paragraph so the separator
                // line has visual breathing room.
                let heading_style = build_para_style(ParaStyleConfig { line_spacing: 9.6, spacing_before: 20.0, ..Default::default() });
                unsafe {
                    storage.addAttribute_value_range(
                        NSParagraphStyleAttributeName,
                        heading_style.as_ref(),
                        range,
                    );
                }
                heading_seps.push(range.location);
            }
        }

        if run.attrs.contains(&TextAttribute::ThematicBreak) {
            thematic_breaks.push(range.location);
        }

        for attr in run.attrs.attrs() {
            if let TextAttribute::TaskCheckbox { checked, byte_offset } = attr {
                checkboxes.push(CheckboxInfo {
                    utf16_pos: range.location,
                    checked: *checked,
                    byte_offset: *byte_offset,
                });
            }
        }
    }
    (heading_seps, thematic_breaks, checkboxes)
}

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
                let Some(row_range) = mk_utf16_range(text, row_start, row_end, text_len_u16) else {
                    continue;
                };
                let style = build_para_style(ParaStyleConfig { spacing_before: 10.0, spacing_after: 10.0, ..Default::default() });
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
                if let Some(sep_range) = mk_utf16_range(text, sep_start, sep_end, text_len_u16) {
                    let collapsed = build_para_style(ParaStyleConfig { max_line_height: Some(0.001), ..Default::default() });
                    unsafe {
                        storage.addAttribute_value_range(
                            NSParagraphStyleAttributeName,
                            collapsed.as_ref(),
                            sep_range,
                        );
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
    table_grids
}

/// Apply paragraph styles and per-token syntax highlighting to code blocks.
///
/// Uses pre-computed UTF-16 offsets from `CodeBlockInfo` for paragraph
/// styling; maps per-token byte offsets within the stored code text to
/// UTF-16 document positions for syntax highlighting.
fn apply_code_blocks(
    storage: &NSTextStorage,
    code_block_infos: &[CodeBlockInfo],
    text_len_u16: usize,
    scheme: &ColorScheme,
) {
    // ── Apply horizontal padding (indent) to code blocks ───────────────
    for info in code_block_infos {
        if info.start_utf16 >= info.end_utf16 {
            continue;
        }
        let range = NSRange {
            location: info.start_utf16,
            length: info.end_utf16 - info.start_utf16,
        };
        let style = build_para_style(ParaStyleConfig { line_spacing: 9.6, indent: 10.0, ..Default::default() });
        unsafe {
            storage.addAttribute_value_range(
                NSParagraphStyleAttributeName,
                style.as_ref(),
                range,
            );
        }
        // Add spacing before the first code line so it breathes below the separator.
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
                    NSParagraphStyleAttributeName,
                    spacing_style.as_ref(),
                    first_line_range,
                );
            }
        }
    }

    // ── Apply per-token syntax highlighting to code blocks ──────────────
    // The flat `code_fg` baseline was applied earlier via for_code_block();
    // these per-token colors override it, giving warm harmonious highlighting.
    let is_dark = scheme.background.0 < 0.5;
    for info in code_block_infos {
        if info.text.is_empty() {
            continue;
        }
        let result = highlight(&info.text, &info.language, is_dark);
        for span in &result.spans {
            // Map byte offsets within info.text to UTF-16 positions in the
            // full document.
            let span_start = span.range.0.min(info.text.len());
            let span_end   = span.range.1.min(info.text.len());
            if span_start >= span_end {
                continue;
            }
            let s_u16 = info.code_start_utf16
                + info.text[..span_start].encode_utf16().count();
            let e_u16 = info.code_start_utf16
                + info.text[..span_end].encode_utf16().count();
            // Offsets are within info.text (not document bytes), so mk_utf16_range doesn't apply.
            if s_u16 >= e_u16 || e_u16 > text_len_u16 {
                continue;
            }
            let range = NSRange { location: s_u16, length: e_u16 - s_u16 };
            let (r, g, b) = (
                span.color.0 as f64 / 255.0,
                span.color.1 as f64 / 255.0,
                span.color.2 as f64 / 255.0,
            );
            let color = NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0);
            unsafe {
                storage.addAttribute_value_range(
                    NSForegroundColorAttributeName,
                    color.as_ref(),
                    range,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Per-range attribute application
// ---------------------------------------------------------------------------

fn apply_attr_set(
    storage: &NSTextStorage,
    range: NSRange,
    attrs: &AttributeSet,
    scheme: &ColorScheme,
    base_size: f64,
) {
    // Build font from the combination of Bold, Italic, Monospace, FontSize.
    let font = build_font(attrs, base_size);
    unsafe {
        storage.addAttribute_value_range(NSFontAttributeName, font.as_ref(), range);
    }

    // Apply individual non-font attributes.
    for attr in attrs.attrs() {
        match attr {
            TextAttribute::Bold | TextAttribute::Italic | TextAttribute::Monospace
            | TextAttribute::FontSize(_) => {
                // Handled above by build_font.
            }
            TextAttribute::Hidden => {
                let clear = NSColor::clearColor();
                unsafe {
                    storage.addAttribute_value_range(
                        NSForegroundColorAttributeName,
                        clear.as_ref(),
                        range,
                    );
                }
            }
            TextAttribute::ForegroundColor(token) => {
                if let Some(rgb) = scheme.resolve_fg(token) {
                    let color = make_color(rgb);
                    unsafe {
                        storage.addAttribute_value_range(
                            NSForegroundColorAttributeName,
                            color.as_ref(),
                            range,
                        );
                    }
                }
            }
            TextAttribute::BackgroundColor(token) => {
                if let Some(rgb) = scheme.resolve_bg(token) {
                    let color = make_color(rgb);
                    unsafe {
                        storage.addAttribute_value_range(
                            NSBackgroundColorAttributeName,
                            color.as_ref(),
                            range,
                        );
                    }
                }
            }
            TextAttribute::Strikethrough => {
                // NSStrikethroughStyleAttributeName takes an NSNumber (NSUnderlineStyleSingle = 1).
                let num = NSNumber::numberWithInteger(1);
                unsafe {
                    storage.addAttribute_value_range(
                        NSStrikethroughStyleAttributeName,
                        num.as_ref(),
                        range,
                    );
                }
            }
            TextAttribute::Underline => {
                let num = NSNumber::numberWithInteger(1);
                unsafe {
                    storage.addAttribute_value_range(
                        NSUnderlineStyleAttributeName,
                        num.as_ref(),
                        range,
                    );
                }
            }
            TextAttribute::Superscript => {
                let num = NSNumber::numberWithInteger(1);
                unsafe {
                    storage.addAttribute_value_range(
                        NSSuperscriptAttributeName,
                        num.as_ref(),
                        range,
                    );
                }
            }
            TextAttribute::Subscript => {
                let num = NSNumber::numberWithInteger(-1);
                unsafe {
                    storage.addAttribute_value_range(
                        NSSuperscriptAttributeName,
                        num.as_ref(),
                        range,
                    );
                }
            }
            TextAttribute::Link(url) => {
                let ns_str = NSString::from_str(url);
                if let Some(ns_url) = NSURL::URLWithString(&ns_str) {
                    unsafe {
                        storage.addAttribute_value_range(
                            NSLinkAttributeName,
                            &ns_url,
                            range,
                        );
                    }
                }
            }
            // These attributes are conveyed via color tokens above or handled
            // separately in apply_attribute_runs; no direct NSAttributedString
            // key needed here.
            TextAttribute::ListMarker
            | TextAttribute::BlockquoteBar
            | TextAttribute::LineSpacing(_)
            | TextAttribute::HeadingSeparator
            | TextAttribute::ThematicBreak
            | TextAttribute::TaskCheckbox { .. } => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Font helpers
// ---------------------------------------------------------------------------

/// Build the appropriate `NSFont` for an `AttributeSet`.
///
/// Processes Bold + Italic + Monospace + FontSize together so they don't
/// overwrite each other when applied one by one.
fn build_font(attrs: &AttributeSet, base_size: f64) -> Retained<NSFont> {
    // Hidden characters (syntax markers) must not take up layout space.
    // Setting the font to near-zero eliminates the visual indentation caused
    // by invisible '# ' / '*' / '**' characters still occupying their advance width.
    if attrs.contains(&TextAttribute::Hidden) {
        return unsafe { NSFont::systemFontOfSize_weight(0.001, NSFontWeightRegular) };
    }

    let size = attrs.font_size().unwrap_or(base_size);
    let bold = attrs.contains(&TextAttribute::Bold);
    let italic = attrs.contains(&TextAttribute::Italic);
    let mono = attrs.contains(&TextAttribute::Monospace);

    if mono {
        let code_size = size - 2.0;
        let weight = unsafe { if bold { NSFontWeightBold } else { NSFontWeightRegular } };
        let base = NSFont::monospacedSystemFontOfSize_weight(code_size, weight);
        if italic {
            let desc = base.fontDescriptor();
            let mut traits = NSFontDescriptorSymbolicTraits::TraitItalic;
            if bold {
                traits |= NSFontDescriptorSymbolicTraits::TraitBold;
            }
            let italic_desc = desc.fontDescriptorWithSymbolicTraits(traits);
            return NSFont::fontWithDescriptor_size(&italic_desc, code_size)
                .unwrap_or(base);
        }
        return base;
    }

    if bold && italic {
        return serif_font(size, true, true);
    }

    if italic {
        return serif_font(size, false, true);
    }

    if bold {
        return serif_font(size, true, false);
    }

    serif_font(size, false, false)
}

/// Build a Georgia serif font for the given size and style.
/// Falls back to the system font if Georgia is unavailable.
fn serif_font(size: f64, bold: bool, italic: bool) -> Retained<NSFont> {
    let name = match (bold, italic) {
        (true,  true)  => "Georgia-BoldItalic",
        (true,  false) => "Georgia-Bold",
        (false, true)  => "Georgia-Italic",
        (false, false) => "Georgia",
    };
    let ns_name = NSString::from_str(name);
    NSFont::fontWithName_size(&ns_name, size)
        .unwrap_or_else(|| unsafe {
            let weight = if bold { NSFontWeightBold } else { NSFontWeightRegular };
            NSFont::systemFontOfSize_weight(size, weight)
        })
}

// ---------------------------------------------------------------------------
// Table column equalization
// ---------------------------------------------------------------------------

/// Measure cell widths and add kern spacing so that all columns align.
///
/// Uses a three-pass algorithm: (1) measure each cell's rendered width,
/// (2) compute the maximum width per column, (3) kern the last character
/// of shorter cells to pad them to the column maximum.
///
/// Must be called after all fonts have been applied to the storage, because
/// rendered cell widths depend on the font metrics already in place.
fn equalize_table_columns(
    storage: &NSTextStorage,
    text: &str,
    row_pipes: &[Vec<usize>],
) {
    if row_pipes.is_empty() {
        return;
    }

    // Determine the number of columns (= pipes_per_row - 1).
    let num_cols = row_pipes.iter().map(|rp| rp.len().saturating_sub(1)).min().unwrap_or(0);
    if num_cols == 0 {
        return;
    }

    // ── Pass 1: Measure cell widths ──────────────────────────────────────
    // widths[row][col] = rendered width in points
    let mut widths: Vec<Vec<f64>> = Vec::with_capacity(row_pipes.len());
    for rp in row_pipes {
        let mut row_widths = Vec::with_capacity(num_cols);
        for c in 0..num_cols {
            if c + 1 >= rp.len() {
                row_widths.push(0.0);
                continue;
            }
            // Content between pipe[c]+1 and pipe[c+1] (exclusive of the pipes).
            let byte_start = rp[c] + 1;
            let byte_end = rp[c + 1];
            let start_u16 = byte_to_utf16(text, byte_start);
            let end_u16 = byte_to_utf16(text, byte_end);
            if start_u16 >= end_u16 {
                row_widths.push(0.0);
                continue;
            }
            let range = NSRange { location: start_u16, length: end_u16 - start_u16 };
            let width: f64 = unsafe {
                let substr = storage.attributedSubstringFromRange(range);
                let size: NSSize = msg_send![&*substr, size];
                size.width
            };
            row_widths.push(width);
        }
        widths.push(row_widths);
    }

    // ── Pass 2: Compute max width per column ─────────────────────────────
    let mut max_widths = vec![0.0f64; num_cols];
    for row_widths in &widths {
        for (c, &w) in row_widths.iter().enumerate() {
            if w > max_widths[c] {
                max_widths[c] = w;
            }
        }
    }

    // Add 20px per column for cell padding (10px left from pipe kern + 10px right).
    for w in &mut max_widths {
        *w += 20.0;
    }

    // ── Pass 3: Apply kern padding ───────────────────────────────────────
    for (r, rp) in row_pipes.iter().enumerate() {
        for c in 0..num_cols {
            if c + 1 >= rp.len() {
                continue;
            }
            let padding = max_widths[c] - widths[r][c];
            if padding <= 0.5 {
                continue; // Skip negligible differences.
            }
            // Find the last character before the trailing pipe.
            let pipe_byte = rp[c + 1];
            let before_pipe = &text[..pipe_byte];
            if let Some(last_char) = before_pipe.chars().next_back() {
                let kern_byte_start = pipe_byte - last_char.len_utf8();
                let kern_u16_start = byte_to_utf16(text, kern_byte_start);
                let kern_u16_end = byte_to_utf16(text, pipe_byte);
                if kern_u16_start < kern_u16_end {
                    let kern_range = NSRange {
                        location: kern_u16_start,
                        length: kern_u16_end - kern_u16_start,
                    };
                    let kern_value = NSNumber::numberWithFloat(padding as f32);
                    unsafe {
                        storage.addAttribute_value_range(
                            NSKernAttributeName,
                            kern_value.as_ref(),
                            kern_range,
                        );
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

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
    style.setParagraphSpacingBefore(cfg.spacing_before);  // always set, 0.0 is valid
    style.setParagraphSpacing(cfg.spacing_after);         // always set, 0.0 is valid
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

/// Build an `NSColor` from an sRGB float tuple.
fn make_color((r, g, b): (f64, f64, f64)) -> Retained<NSColor> {
    NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0)
}


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

/// Convert a UTF-8 byte offset in `text` to a UTF-16 code-unit offset.
///
/// Necessary because `NSRange` uses UTF-16 indices but comrak returns
/// UTF-8 byte offsets.
fn byte_to_utf16(text: &str, byte_pos: usize) -> usize {
    let clamped = byte_pos.min(text.len());
    text[..clamped].encode_utf16().count()
}
