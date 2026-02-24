use crate::markdown::attributes::AttributeSet;
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
        collect_runs(text, span, cursor_pos, &mut runs);
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
            // "**content**" — 2-char markers on each side
            let m = 2.min(end - start);
            runs.push(AttributeRun { range: (start, start + m), attrs: syn.clone() });
            if start + m < end.saturating_sub(m) {
                runs.push(AttributeRun {
                    range: (start + m, end - m),
                    attrs: AttributeSet::for_strong(),
                });
            }
            runs.push(AttributeRun { range: (end - m, end), attrs: syn });
        }
        NodeKind::Emph => {
            // "*content*" — 1-char markers
            runs.push(AttributeRun { range: (start, start + 1), attrs: syn.clone() });
            if start + 1 < end.saturating_sub(1) {
                runs.push(AttributeRun {
                    range: (start + 1, end - 1),
                    attrs: AttributeSet::for_emph(),
                });
            }
            runs.push(AttributeRun { range: (end - 1, end), attrs: syn });
        }
        NodeKind::Code => {
            // "`content`" — 1-char markers
            runs.push(AttributeRun { range: (start, start + 1), attrs: syn.clone() });
            if start + 1 < end.saturating_sub(1) {
                runs.push(AttributeRun {
                    range: (start + 1, end - 1),
                    attrs: AttributeSet::for_inline_code(),
                });
            }
            runs.push(AttributeRun { range: (end - 1, end), attrs: syn });
        }
        NodeKind::Heading { level } => {
            // Distinguish ATX headings ("# …") from setext headings ("---" underline style).
            let is_atx = text.as_bytes().get(start).copied() == Some(b'#');

            if is_atx {
                // ATX: the "# " (or "## " etc.) prefix is hidden as a syntax marker.
                let prefix_len = (*level as usize + 1).min(end - start);
                runs.push(AttributeRun { range: (start, start + prefix_len), attrs: syn });
                if start + prefix_len < end {
                    runs.push(AttributeRun {
                        range: (start + prefix_len, end),
                        attrs: AttributeSet::for_heading(*level),
                    });
                }
            } else {
                // Setext: the underline line (--- / ===) is on the last line of the span.
                // Everything before the final newline is heading content; the last line
                // is a syntax marker (shown/hidden based on cursor position).
                let span_slice = &text[start..end];
                if let Some(nl_rel) = span_slice.rfind('\n') {
                    let nl_abs = start + nl_rel;
                    if start < nl_abs {
                        runs.push(AttributeRun {
                            range: (start, nl_abs),
                            attrs: AttributeSet::for_heading(*level),
                        });
                    }
                    if nl_abs < end {
                        runs.push(AttributeRun {
                            range: (nl_abs, end),
                            attrs: syn, // shown/hidden based on cursor position
                        });
                    }
                } else {
                    // No newline in span (degenerate) — treat whole range as content.
                    runs.push(AttributeRun {
                        range: (start, end),
                        attrs: AttributeSet::for_heading(*level),
                    });
                }
            }
        }
        NodeKind::Strikethrough => {
            let m = 2.min(end - start);
            runs.push(AttributeRun { range: (start, start + m), attrs: syn.clone() });
            if start + m < end.saturating_sub(m) {
                runs.push(AttributeRun {
                    range: (start + m, end - m),
                    attrs: AttributeSet::for_strikethrough(),
                });
            }
            runs.push(AttributeRun { range: (end - m, end), attrs: syn });
        }
        NodeKind::Link { .. } => {
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_link(),
            });
        }
        NodeKind::CodeBlock { .. } => {
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_code_block(),
            });
        }
        NodeKind::BlockQuote => {
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_blockquote(),
            });
        }
        NodeKind::List => {
            // Container only — visual structure comes from Item rendering.
            for child in &span.children {
                collect_runs(text, child, cursor_pos, runs);
            }
        }
        NodeKind::Item => {
            // The bullet/number marker (e.g. "- " or "1. ") is implicit in the
            // source but has no child node.  Its range is from item start to
            // the start of the first child (usually a Paragraph).
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
                collect_runs(text, child, cursor_pos, runs);
            }
        }
        NodeKind::Table => {
            // Phase-1 fallback: render the whole table block as monospace.
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_code_block(),
            });
        }
        NodeKind::Footnote => {
            // Footnote definitions/references rendered in muted link color.
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_link(),
            });
        }
        // For remaining container nodes (Paragraph, Text, …) just recurse.
        _ => {
            for child in &span.children {
                collect_runs(text, child, cursor_pos, runs);
            }
        }
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