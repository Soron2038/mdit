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
) -> Vec<AttributeRun> {
    let mut runs = Vec::new();
    for span in spans {
        collect_runs(text, span, cursor_pos, &[], &mut runs);
    }
    fill_gaps(text.len(), runs)
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

fn collect_runs(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    inherited: &[TextAttribute],
    runs: &mut Vec<AttributeRun>,
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
            collect_strong(text, span, cursor_pos, inherited, &syn, runs);
        }
        NodeKind::Emph => {
            collect_emph(text, span, cursor_pos, inherited, &syn, runs);
        }
        NodeKind::Code => {
            collect_code(start, end, &syn, runs);
        }
        NodeKind::Heading { level } => {
            collect_heading(text, start, end, *level, &syn, runs);
        }
        NodeKind::Strikethrough => {
            collect_strikethrough(text, span, cursor_pos, inherited, &syn, runs);
        }
        NodeKind::Link { .. } => {
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_link(),
            });
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
        NodeKind::List => {
            for child in &span.children {
                collect_runs(text, child, cursor_pos, inherited, runs);
            }
        }
        NodeKind::Item => {
            collect_item(text, span, cursor_pos, inherited, runs);
        }
        NodeKind::Table => {
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_code_block(),
            });
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
                    collect_runs(text, child, cursor_pos, inherited, runs);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Extracted match-arm helpers
// ---------------------------------------------------------------------------

/// "**content**" — symmetric 2-char markers with Bold attribute.
fn collect_strong(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    inherited: &[TextAttribute],
    syn: &AttributeSet,
    runs: &mut Vec<AttributeRun>,
) {
    let (start, end) = (span.source_range.0, span.source_range.1.min(text.len()));
    let m = 2.min(end - start);
    runs.push(AttributeRun { range: (start, start + m), attrs: syn.clone() });
    let mut child_attrs = inherited.to_vec();
    child_attrs.push(TextAttribute::Bold);
    if span.children.is_empty() {
        if start + m < end.saturating_sub(m) {
            runs.push(AttributeRun {
                range: (start + m, end - m),
                attrs: AttributeSet::new(child_attrs),
            });
        }
    } else {
        for child in &span.children {
            collect_runs(text, child, cursor_pos, &child_attrs, runs);
        }
    }
    runs.push(AttributeRun { range: (end - m, end), attrs: syn.clone() });
}

/// "*content*" — symmetric 1-char markers with Italic attribute.
fn collect_emph(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    inherited: &[TextAttribute],
    syn: &AttributeSet,
    runs: &mut Vec<AttributeRun>,
) {
    let (start, end) = (span.source_range.0, span.source_range.1.min(text.len()));
    runs.push(AttributeRun { range: (start, start + 1), attrs: syn.clone() });
    let mut child_attrs = inherited.to_vec();
    child_attrs.push(TextAttribute::Italic);
    if span.children.is_empty() {
        if start + 1 < end.saturating_sub(1) {
            runs.push(AttributeRun {
                range: (start + 1, end - 1),
                attrs: AttributeSet::new(child_attrs),
            });
        }
    } else {
        for child in &span.children {
            collect_runs(text, child, cursor_pos, &child_attrs, runs);
        }
    }
    runs.push(AttributeRun { range: (end - 1, end), attrs: syn.clone() });
}

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
                attrs: AttributeSet::for_heading(level),
            });
        }
    } else {
        let span_slice = &text[start..end];
        if let Some(nl_rel) = span_slice.rfind('\n') {
            let nl_abs = start + nl_rel;
            if start < nl_abs {
                runs.push(AttributeRun {
                    range: (start, nl_abs),
                    attrs: AttributeSet::for_heading(level),
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
                attrs: AttributeSet::for_heading(level),
            });
        }
    }
}

/// "~~content~~" — symmetric 2-char markers with Strikethrough attribute.
fn collect_strikethrough(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    inherited: &[TextAttribute],
    syn: &AttributeSet,
    runs: &mut Vec<AttributeRun>,
) {
    let (start, end) = (span.source_range.0, span.source_range.1.min(text.len()));
    let m = 2.min(end - start);
    runs.push(AttributeRun { range: (start, start + m), attrs: syn.clone() });
    let mut child_attrs = inherited.to_vec();
    child_attrs.push(TextAttribute::Strikethrough);
    child_attrs.push(TextAttribute::ForegroundColor("strikethrough"));
    if span.children.is_empty() {
        if start + m < end.saturating_sub(m) {
            runs.push(AttributeRun {
                range: (start + m, end - m),
                attrs: AttributeSet::new(child_attrs),
            });
        }
    } else {
        for child in &span.children {
            collect_runs(text, child, cursor_pos, &child_attrs, runs);
        }
    }
    runs.push(AttributeRun { range: (end - m, end), attrs: syn.clone() });
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
    inherited: &[TextAttribute],
    runs: &mut Vec<AttributeRun>,
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
        collect_runs(text, child, cursor_pos, inherited, runs);
    }
}

/// Fill gaps between runs with plain (unstyled) runs.
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