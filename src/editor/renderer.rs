//! Converts a parsed Markdown AST into a flat list of [`AttributeRun`]s.
//!
//! # Pipeline
//! 1. [`compute_attribute_runs`] walks the [`MarkdownSpan`] tree produced by the parser.
//! 2. Each node kind dispatches to a specialized helper (`collect_heading`,
//!    `collect_link`, `collect_symmetric_marker`, …) that appends runs.
//! 3. [`fill_gaps`] fills any byte ranges not covered by a run with plain styling.
//!
//! Runs use UTF-8 byte offsets; [`crate::editor::apply`] converts them to
//! UTF-16 before applying to `NSTextStorage`.

use crate::markdown::attributes::{AttributeSet, TextAttribute};
use crate::markdown::parser::{MarkdownSpan, NodeKind};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AttributeRun {
    pub range: (usize, usize),
    pub attrs: AttributeSet,
}

/// Metadata for a single table, used for column width equalization.
#[derive(Debug, Clone)]
pub struct TableInfo {
    /// For each data row (header + body): sorted byte positions of structural pipes.
    pub row_pipes: Vec<Vec<usize>>,
    /// Whether the cursor is currently inside this table.
    pub cursor_inside: bool,
    /// Byte range of the entire table (start, end) from the AST span.
    pub source_range: (usize, usize),
    /// Byte ranges of each data row (header + body rows, excluding separator).
    pub row_ranges: Vec<(usize, usize)>,
}

/// Combined output of `compute_attribute_runs`.
pub struct RenderOutput {
    pub runs: Vec<AttributeRun>,
    pub table_infos: Vec<TableInfo>,
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Compute a flat list of `AttributeRun`s for the given text + AST.
///
/// `cursor_pos` (byte offset): if `Some`, syntax markers for the span
/// containing the cursor are shown; all others are hidden.
pub fn compute_attribute_runs(
    text: &str,
    spans: &[MarkdownSpan],
    cursor_pos: Option<usize>,
    base_size: f64,
) -> RenderOutput {
    let mut runs = Vec::new();
    let mut table_infos = Vec::new();
    for span in spans {
        collect_runs(text, span, cursor_pos, base_size, &[], &mut runs, &mut table_infos);
    }
    RenderOutput {
        runs: fill_gaps(text.len(), runs),
        table_infos,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn cursor_in_span(pos: Option<usize>, range: (usize, usize)) -> bool {
    match pos {
        None => false,
        Some(p) => p >= range.0 && p <= range.1,
    }
}

/// Pick the right syntax-marker style depending on whether the cursor
/// is inside `span_range`.
fn syntax_attrs(cursor_pos: Option<usize>, span_range: (usize, usize)) -> AttributeSet {
    if cursor_in_span(cursor_pos, span_range) {
        AttributeSet::syntax_visible()
    } else {
        AttributeSet::syntax_hidden()
    }
}

/// Clamp a source range to the actual text length.
/// Avoids repeated `.min(text.len())` calls at each call site.
fn clamp_span_range(start: usize, end: usize, text_len: usize) -> (usize, usize) {
    (start, end.min(text_len))
}

/// Handle any symmetric inline marker: open-marker | content | close-marker.
///
/// `marker_size` — byte width of the opening and closing markers (1 or 2).
/// `extra_attrs` — `TextAttribute`s added on top of `inherited` for the content run.
#[allow(clippy::too_many_arguments)]
fn collect_symmetric_marker(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    base_size: f64,
    inherited: &[TextAttribute],
    syn: &AttributeSet,
    marker_size: usize,
    extra_attrs: &[TextAttribute],
    runs: &mut Vec<AttributeRun>,
    table_infos: &mut Vec<TableInfo>,
) {
    let (start, end) = clamp_span_range(span.source_range.0, span.source_range.1, text.len());
    let m = marker_size.min(end.saturating_sub(start));
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
            collect_runs(text, child, cursor_pos, base_size, &child_attrs, runs, table_infos);
        }
    }
    runs.push(AttributeRun { range: (end - m, end), attrs: syn.clone() });
}

fn collect_runs(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    base_size: f64,
    inherited: &[TextAttribute],
    runs: &mut Vec<AttributeRun>,
    table_infos: &mut Vec<TableInfo>,
) {
    let (start, end) = span.source_range;
    // Guard against degenerate ranges.
    if start >= end || start >= text.len() {
        return;
    }
    let end = end.min(text.len());

    let syn = syntax_attrs(cursor_pos, span.source_range);

    match &span.kind {
        NodeKind::Strong => {
            collect_symmetric_marker(text, span, cursor_pos, base_size, inherited, &syn, 2,
                &[TextAttribute::Bold], runs, table_infos);
        }
        NodeKind::Emph => {
            collect_symmetric_marker(text, span, cursor_pos, base_size, inherited, &syn, 1,
                &[TextAttribute::Italic], runs, table_infos);
        }
        NodeKind::Code => {
            collect_code(start, end, &syn, runs);
        }
        NodeKind::Heading { level } => {
            collect_heading(text, start, end, *level, base_size, &syn, runs);
        }
        NodeKind::Strikethrough => {
            collect_symmetric_marker(text, span, cursor_pos, base_size, inherited, &syn, 2,
                &[TextAttribute::Strikethrough, TextAttribute::ForegroundColor("strikethrough")],
                runs, table_infos);
        }
        NodeKind::Underline => {
            collect_symmetric_marker(text, span, cursor_pos, base_size, inherited, &syn, 2,
                &[TextAttribute::Underline, TextAttribute::ForegroundColor("underline")],
                runs, table_infos);
        }
        NodeKind::Highlight => {
            collect_symmetric_marker(text, span, cursor_pos, base_size, inherited, &syn, 2,
                &[TextAttribute::BackgroundColor("highlight_bg")], runs, table_infos);
        }
        NodeKind::Subscript => {
            collect_symmetric_marker(text, span, cursor_pos, base_size, inherited, &syn, 1,
                &[TextAttribute::Subscript, TextAttribute::ForegroundColor("subscript")],
                runs, table_infos);
        }
        NodeKind::Superscript => {
            collect_symmetric_marker(text, span, cursor_pos, base_size, inherited, &syn, 1,
                &[TextAttribute::Superscript, TextAttribute::ForegroundColor("superscript")],
                runs, table_infos);
        }
        NodeKind::Link { ref url } => {
            collect_link(text, span, cursor_pos, base_size, inherited, &syn, url, runs, table_infos);
        }
        NodeKind::CodeBlock { .. } => {
            collect_code_block(text, span, cursor_pos, runs);
        }
        NodeKind::BlockQuote => {
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_blockquote(),
            });
        }
        NodeKind::ThematicBreak => {
            if cursor_in_span(cursor_pos, span.source_range) {
                // Cursor on HR — show "---" in syntax color.
                runs.push(AttributeRun {
                    range: (start, end),
                    attrs: AttributeSet::syntax_visible(),
                });
            } else {
                // Cursor away — hide text, mark for line drawing.
                runs.push(AttributeRun {
                    range: (start, end),
                    attrs: AttributeSet::syntax_hidden().with(TextAttribute::ThematicBreak),
                });
            }
        }
        NodeKind::List => {
            for child in &span.children {
                collect_runs(text, child, cursor_pos, base_size, inherited, runs, table_infos);
            }
        }
        NodeKind::Item | NodeKind::TaskItem { .. } => {
            collect_item(text, span, cursor_pos, base_size, inherited, runs, table_infos);
        }
        NodeKind::Table => {
            collect_table(text, span, cursor_pos, base_size, runs, table_infos);
        }
        NodeKind::TableRow { .. } | NodeKind::TableCell => {
            for child in &span.children {
                collect_runs(text, child, cursor_pos, base_size, inherited, runs, table_infos);
            }
        }
        NodeKind::Footnote => {
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_link(),
            });
        }
        _ => {
            if span.children.is_empty() {
                if !inherited.is_empty() {
                    runs.push(AttributeRun {
                        range: (start, end),
                        attrs: AttributeSet::new(inherited.to_vec()),
                    });
                }
            } else {
                for child in &span.children {
                    collect_runs(text, child, cursor_pos, base_size, inherited, runs, table_infos);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Extracted match-arm helpers
// ---------------------------------------------------------------------------

/// "`content`" — 1-char backtick markers with inline-code styling.
fn collect_code(
    start: usize,
    end: usize,
    syn: &AttributeSet,
    runs: &mut Vec<AttributeRun>,
) {
    runs.push(AttributeRun { range: (start, start + 1), attrs: syn.clone() });
    if start + 1 < end.saturating_sub(1) {
        runs.push(AttributeRun {
            range: (start + 1, end - 1),
            attrs: AttributeSet::for_inline_code(),
        });
    }
    runs.push(AttributeRun { range: (end - 1, end), attrs: syn.clone() });
}

/// ATX (`# …`) and setext (underline) headings.
fn collect_heading(
    text: &str,
    start: usize,
    end: usize,
    level: u8,
    base_size: f64,
    syn: &AttributeSet,
    runs: &mut Vec<AttributeRun>,
) {
    let is_atx = text.as_bytes().get(start).copied() == Some(b'#');

    if is_atx {
        let prefix_len = (level as usize + 1).min(end - start);
        runs.push(AttributeRun { range: (start, start + prefix_len), attrs: syn.clone() });
        if start + prefix_len < end {
            runs.push(AttributeRun {
                range: (start + prefix_len, end),
                attrs: AttributeSet::for_heading(level, base_size),
            });
        }
    } else {
        let span_slice = &text[start..end];
        if let Some(nl_rel) = span_slice.rfind('\n') {
            let nl_abs = start + nl_rel;
            if start < nl_abs {
                runs.push(AttributeRun {
                    range: (start, nl_abs),
                    attrs: AttributeSet::for_heading(level, base_size),
                });
            }
            if nl_abs < end {
                runs.push(AttributeRun {
                    range: (nl_abs, end),
                    attrs: syn.clone(),
                });
            }
        } else {
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_heading(level, base_size),
            });
        }
    }
}

/// "[title](url)" — asymmetric markers with link styling.
#[allow(clippy::too_many_arguments)]
fn collect_link(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    base_size: f64,
    inherited: &[TextAttribute],
    syn: &AttributeSet,
    url: &str,
    runs: &mut Vec<AttributeRun>,
    table_infos: &mut Vec<TableInfo>,
) {
    let (start, end) = (span.source_range.0, span.source_range.1.min(text.len()));

    // Determine content boundaries from children (safe — comrak parsed it).
    let (content_start, content_end) = if !span.children.is_empty() {
        (
            span.children.first().unwrap().source_range.0,
            span.children.last().unwrap().source_range.1.min(end),
        )
    } else {
        // No children — find "](" as fallback.
        let bracket = text[start..end].find("](").map(|p| start + p).unwrap_or(end);
        (start + 1, bracket)
    };

    // Opening marker: "["
    runs.push(AttributeRun { range: (start, content_start), attrs: syn.clone() });

    // Content: link title with link color + clickable link attribute.
    let mut child_attrs = inherited.to_vec();
    child_attrs.push(TextAttribute::ForegroundColor("link"));
    child_attrs.push(TextAttribute::Link(url.to_string()));
    if span.children.is_empty() {
        if content_start < content_end {
            runs.push(AttributeRun {
                range: (content_start, content_end),
                attrs: AttributeSet::new(child_attrs),
            });
        }
    } else {
        for child in &span.children {
            collect_runs(text, child, cursor_pos, base_size, &child_attrs, runs, table_infos);
        }
    }

    // Closing marker: "](url)"
    if content_end < end {
        runs.push(AttributeRun { range: (content_end, end), attrs: syn.clone() });
    }
}

/// Fenced code block: opening fence, code content, closing fence.
fn collect_code_block(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    runs: &mut Vec<AttributeRun>,
) {
    let (start, end) = (span.source_range.0, span.source_range.1.min(text.len()));
    let slice = &text[start..end];
    if let Some(open_nl) = slice.find('\n') {
        let open_end = start + open_nl + 1;

        let suffix = &text[open_end..end];
        let close_start = open_end + if !suffix.is_empty() {
            suffix[..suffix.len() - 1]
                .rfind('\n')
                .map(|p| p + 1)
                .unwrap_or(0)
        } else {
            0
        };

        runs.push(AttributeRun {
            range: (start, open_end),
            attrs: syntax_attrs(cursor_pos, (start, open_end)),
        });
        if open_end < close_start {
            runs.push(AttributeRun {
                range: (open_end, close_start),
                attrs: AttributeSet::for_code_block(),
            });
        }
        if close_start < end {
            runs.push(AttributeRun {
                range: (close_start, end),
                attrs: syntax_attrs(cursor_pos, (close_start, end)),
            });
        }
    } else {
        runs.push(AttributeRun {
            range: (start, end),
            attrs: AttributeSet::for_code_block(),
        });
    }
}

/// List item: bullet/number marker + child content.
fn collect_item(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    base_size: f64,
    inherited: &[TextAttribute],
    runs: &mut Vec<AttributeRun>,
    table_infos: &mut Vec<TableInfo>,
) {
    let (start, end) = (span.source_range.0, span.source_range.1.min(text.len()));
    let marker_end = span
        .children
        .first()
        .map(|c| c.source_range.0)
        .unwrap_or(start + 2)
        .min(end);
    if start < marker_end {
        runs.push(AttributeRun {
            range: (start, marker_end),
            attrs: AttributeSet::for_list_marker(),
        });
    }
    for child in &span.children {
        collect_runs(text, child, cursor_pos, base_size, inherited, runs, table_infos);
    }
}

/// Table: pipes as syntax markers, separator row hidden, cell content with inline formatting.
fn collect_table(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    base_size: f64,
    runs: &mut Vec<AttributeRun>,
    table_infos: &mut Vec<TableInfo>,
) {
    let cursor_in = cursor_in_span(cursor_pos, span.source_range);
    let syn = syntax_attrs(cursor_pos, span.source_range);

    // Partition children into header and body rows.
    let mut header_end: Option<usize> = None;
    let mut first_body_start: Option<usize> = None;

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
            runs.push(AttributeRun { range: (sep_start, sep_end), attrs: syn.clone() });
        }
    }

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
                collect_runs(text, child, cursor_pos, base_size, &[], runs, table_infos);
            }
        }
    }

    table_infos.push(TableInfo {
        row_pipes: all_row_pipes,
        cursor_inside: cursor_in,
        source_range: (span.source_range.0, span.source_range.1.min(text.len())),
        row_ranges: all_row_ranges,
    });
}

/// Fill every byte range not covered by a run with a plain (unstyled) run.
///
/// The AST only produces runs for syntax-significant spans.  Gaps between
/// spans (ordinary prose) must still receive the default body styling so
/// `NSTextStorage` doesn't retain stale attributes from a previous edit.
fn fill_gaps(text_len: usize, mut runs: Vec<AttributeRun>) -> Vec<AttributeRun> {
    runs.sort_by_key(|r| r.range.0);
    let mut result = Vec::new();
    let mut pos = 0usize;
    for run in runs {
        if run.range.0 > pos {
            result.push(AttributeRun {
                range: (pos, run.range.0),
                attrs: AttributeSet::plain(),
            });
        }
        pos = run.range.1.max(pos);
        result.push(run);
    }
    if pos < text_len {
        result.push(AttributeRun {
            range: (pos, text_len),
            attrs: AttributeSet::plain(),
        });
    }
    result
}