//! Converts pure-Rust `AttributeRun`s into real AppKit text attributes and
//! applies them to an `NSTextStorage`.
//!
//! This is the bridge between the platform-agnostic renderer and AppKit.

use objc2::rc::Retained;
use objc2::msg_send;
use objc2_app_kit::{
    NSBackgroundColorAttributeName, NSColor, NSFont, NSFontAttributeName,
    NSFontDescriptorSymbolicTraits, NSFontWeightBold, NSFontWeightRegular,
    NSForegroundColorAttributeName, NSKernAttributeName, NSLinkAttributeName,
    NSMutableParagraphStyle, NSParagraphStyleAttributeName,
    NSStrikethroughStyleAttributeName, NSSuperscriptAttributeName, NSTextStorage,
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

/// Positions of elements that need custom drawing in the text view.
pub struct LayoutPositions {
    /// UTF-16 offsets of H1/H2 heading paragraph starts (separator lines).
    pub heading_seps: Vec<usize>,
    /// UTF-16 offsets of thematic breaks (horizontal rules).
    pub thematic_breaks: Vec<usize>,
    /// Per-table grid data for drawing grid lines and borders.
    pub table_grids: Vec<TableGrid>,
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

    // Build reusable base-style objects.
    let body_font = serif_font(16.0, false, false);
    let text_color = make_color(scheme.text);
    let para_style = make_para_style(9.6);

    unsafe {
        // ── Reset entire document to body style ───────────────────────────
        storage.addAttribute_value_range(
            NSFontAttributeName,
            body_font.as_ref(),
            full_range,
        );
        storage.addAttribute_value_range(
            NSForegroundColorAttributeName,
            text_color.as_ref(),
            full_range,
        );
        storage.addAttribute_value_range(
            NSParagraphStyleAttributeName,
            para_style.as_ref(),
            full_range,
        );
        // Clear attributes that only apply to specific spans.
        storage.removeAttribute_range(NSBackgroundColorAttributeName, full_range);
        storage.removeAttribute_range(NSStrikethroughStyleAttributeName, full_range);
        storage.removeAttribute_range(NSKernAttributeName, full_range);
        storage.removeAttribute_range(NSSuperscriptAttributeName, full_range);
    }

    // ── Per-run overrides ─────────────────────────────────────────────────
    let mut heading_sep_positions: Vec<usize> = Vec::new();
    let mut thematic_break_positions: Vec<usize> = Vec::new();
    for run in runs {
        let start_u16 = byte_to_utf16(text, run.range.0);
        let end_u16 = byte_to_utf16(text, run.range.1);
        if start_u16 >= end_u16 || end_u16 > text_len_u16 {
            continue;
        }
        let range = NSRange { location: start_u16, length: end_u16 - start_u16 };
        apply_attr_set(storage, range, &run.attrs, scheme);

        if run.attrs.contains(&TextAttribute::HeadingSeparator) {
            // Only add the spacing / record the position when non-whitespace
            // content precedes this heading.  Checking at attribute-application
            // time (once per edit) avoids allocating a String on every drawRect:.
            let has_content_before = !text[..run.range.0].trim().is_empty();
            if has_content_before {
                // Add extra space above the heading paragraph so the separator
                // line has visual breathing room.
                let heading_style = make_para_style_with_spacing_before(9.6, 20.0);
                unsafe {
                    storage.addAttribute_value_range(
                        NSParagraphStyleAttributeName,
                        heading_style.as_ref(),
                        range,
                    );
                }
                heading_sep_positions.push(start_u16);
            }
        }

        if run.attrs.contains(&TextAttribute::ThematicBreak) {
            thematic_break_positions.push(start_u16);
        }
    }

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
                let style = make_table_row_para_style(0.0, 10.0, 10.0);
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

    // ── Apply horizontal padding (indent) to code blocks ───────────────
    for info in code_block_infos {
        if info.start_utf16 >= info.end_utf16 {
            continue;
        }
        let range = NSRange {
            location: info.start_utf16,
            length: info.end_utf16 - info.start_utf16,
        };
        let style = make_code_block_para_style(9.6, 10.0);
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
            let spacing_style = make_code_block_para_style_with_spacing(9.6, 10.0, 4.0);
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

    LayoutPositions {
        heading_seps: heading_sep_positions,
        thematic_breaks: thematic_break_positions,
        table_grids,
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
) {
    // Build font from the combination of Bold, Italic, Monospace, FontSize.
    let font = build_font(attrs);
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
            | TextAttribute::ThematicBreak => {}
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
fn build_font(attrs: &AttributeSet) -> Retained<NSFont> {
    // Hidden characters (syntax markers) must not take up layout space.
    // Setting the font to near-zero eliminates the visual indentation caused
    // by invisible '# ' / '*' / '**' characters still occupying their advance width.
    if attrs.contains(&TextAttribute::Hidden) {
        return unsafe { NSFont::systemFontOfSize_weight(0.001, NSFontWeightRegular) };
    }

    let size = attrs.font_size(); // returns f64, default 16.0
    let bold = attrs.contains(&TextAttribute::Bold);
    let italic = attrs.contains(&TextAttribute::Italic);
    let mono = attrs.contains(&TextAttribute::Monospace);

    if mono {
        let code_size = if size == 16.0 { 14.0 } else { size };
        let weight = unsafe { if bold { NSFontWeightBold } else { NSFontWeightRegular } };
        let base = NSFont::monospacedSystemFontOfSize_weight(code_size, weight);
        if italic {
            let desc = base.fontDescriptor();
            let mut traits = NSFontDescriptorSymbolicTraits::TraitItalic;
            if bold {
                traits = traits | NSFontDescriptorSymbolicTraits::TraitBold;
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
/// For each pair of adjacent pipes in a row, the content between them forms
/// a column cell.  We measure every cell's rendered width (fonts are already
/// applied), find the maximum per column, and add `NSKernAttributeName` to
/// the last character before each trailing pipe to pad shorter cells.
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

/// Build an `NSColor` from an sRGB float tuple.
fn make_color((r, g, b): (f64, f64, f64)) -> Retained<NSColor> {
    NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0)
}

/// Build an `NSMutableParagraphStyle` with the given `line_spacing` (in points).
fn make_para_style(line_spacing: f64) -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    style.setLineSpacing(line_spacing);
    style
}

/// Build an `NSMutableParagraphStyle` with both line spacing and extra space
/// above the paragraph (used for H1/H2 headings to host the separator line).
fn make_para_style_with_spacing_before(
    line_spacing: f64,
    spacing_before: f64,
) -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    style.setLineSpacing(line_spacing);
    style.setParagraphSpacingBefore(spacing_before);
    style
}

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

/// Build an `NSMutableParagraphStyle` for code block content with horizontal padding.
fn make_code_block_para_style(
    line_spacing: f64,
    indent: f64,
) -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    style.setLineSpacing(line_spacing);
    style.setHeadIndent(indent);
    style.setFirstLineHeadIndent(indent);
    style.setTailIndent(-indent); // negative = inset from trailing margin
    style
}

/// Like `make_code_block_para_style` but with extra space above the paragraph.
/// Applied only to the first code line so it has breathing room below the separator.
fn make_code_block_para_style_with_spacing(
    line_spacing: f64,
    indent: f64,
    spacing_before: f64,
) -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    style.setLineSpacing(line_spacing);
    style.setHeadIndent(indent);
    style.setFirstLineHeadIndent(indent);
    style.setTailIndent(-indent);
    style.setParagraphSpacingBefore(spacing_before);
    style
}

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

/// Convert a UTF-8 byte offset in `text` to a UTF-16 code-unit offset.
///
/// Necessary because `NSRange` uses UTF-16 indices but comrak returns
/// UTF-8 byte offsets.
fn byte_to_utf16(text: &str, byte_pos: usize) -> usize {
    let clamped = byte_pos.min(text.len());
    text[..clamped].encode_utf16().count()
}
