//! Simplified renderer for Editor mode: monospace font with syntax-highlighting colors.
//!
//! Unlike the full `renderer::compute_attribute_runs()` which produces rich visual
//! styling (variable font sizes, hidden markers, custom drawing triggers), this
//! renderer produces flat, uniform-sized output suitable for a raw text editor.

use crate::editor::renderer::AttributeRun;
use crate::markdown::attributes::{AttributeSet, TextAttribute};
use crate::markdown::parser::{MarkdownSpan, NodeKind};

/// Compute a flat list of `AttributeRun`s for editor mode (syntax highlighting only).
///
/// All text uses monospace font at a uniform size. Markdown syntax elements are
/// colored but no structural attributes (HeadingSeparator, ThematicBreak, Hidden)
/// are emitted — the raw markdown is always fully visible.
pub fn compute_editor_runs(text: &str, spans: &[MarkdownSpan]) -> Vec<AttributeRun> {
    let mut runs = Vec::new();
    for span in spans {
        collect_editor_runs(text, span, &mut runs);
    }
    fill_gaps(text.len(), runs)
}

// ---------------------------------------------------------------------------
// Recursive span walker
// ---------------------------------------------------------------------------

fn collect_editor_runs(
    text: &str,
    span: &MarkdownSpan,
    runs: &mut Vec<AttributeRun>,
) {
    let (start, end) = span.source_range;
    if start >= end || start >= text.len() {
        return;
    }
    let end = end.min(text.len());

    match &span.kind {
        NodeKind::Heading { .. } => {
            // Color the entire heading (prefix + text) in heading color.
            runs.push(AttributeRun {
                range: (start, end),
                attrs: editor_heading(),
            });
        }
        NodeKind::Strong => {
            // "**content**" — markers in syntax color, content in bold color.
            let m = 2.min(end - start);
            runs.push(AttributeRun { range: (start, start + m), attrs: editor_syntax() });
            if start + m < end.saturating_sub(m) {
                runs.push(AttributeRun {
                    range: (start + m, end - m),
                    attrs: editor_bold(),
                });
            }
            runs.push(AttributeRun { range: (end - m, end), attrs: editor_syntax() });
        }
        NodeKind::Emph => {
            // "_content_" — markers in syntax color, content in italic style.
            runs.push(AttributeRun { range: (start, start + 1), attrs: editor_syntax() });
            if start + 1 < end.saturating_sub(1) {
                runs.push(AttributeRun {
                    range: (start + 1, end - 1),
                    attrs: editor_italic(),
                });
            }
            runs.push(AttributeRun { range: (end - 1, end), attrs: editor_syntax() });
        }
        NodeKind::Code => {
            // "`content`" — markers in syntax color, content in code color.
            runs.push(AttributeRun { range: (start, start + 1), attrs: editor_syntax() });
            if start + 1 < end.saturating_sub(1) {
                runs.push(AttributeRun {
                    range: (start + 1, end - 1),
                    attrs: editor_code(),
                });
            }
            runs.push(AttributeRun { range: (end - 1, end), attrs: editor_syntax() });
        }
        NodeKind::CodeBlock { .. } => {
            // Entire code block in code color.
            runs.push(AttributeRun {
                range: (start, end),
                attrs: editor_code(),
            });
        }
        NodeKind::Link { .. } => {
            // "[title](url)" — brackets/parens in syntax color, title in link color.
            collect_editor_link(text, span, runs);
        }
        NodeKind::Strikethrough => {
            // "~~content~~" — markers in syntax color, content in strikethrough color.
            let m = 2.min(end - start);
            runs.push(AttributeRun { range: (start, start + m), attrs: editor_syntax() });
            if start + m < end.saturating_sub(m) {
                runs.push(AttributeRun {
                    range: (start + m, end - m),
                    attrs: editor_strikethrough(),
                });
            }
            runs.push(AttributeRun { range: (end - m, end), attrs: editor_syntax() });
        }
        NodeKind::BlockQuote => {
            runs.push(AttributeRun {
                range: (start, end),
                attrs: editor_blockquote(),
            });
        }
        NodeKind::ThematicBreak => {
            runs.push(AttributeRun {
                range: (start, end),
                attrs: editor_syntax(),
            });
        }
        NodeKind::Item => {
            collect_editor_item(text, span, runs);
        }
        NodeKind::List => {
            for child in &span.children {
                collect_editor_runs(text, child, runs);
            }
        }
        NodeKind::Table | NodeKind::TableRow { .. } | NodeKind::TableCell => {
            for child in &span.children {
                collect_editor_runs(text, child, runs);
            }
        }
        NodeKind::Footnote => {
            runs.push(AttributeRun {
                range: (start, end),
                attrs: editor_link(),
            });
        }
        _ => {
            // Recurse into children for any unhandled node.
            for child in &span.children {
                collect_editor_runs(text, child, runs);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Extracted helpers
// ---------------------------------------------------------------------------

fn collect_editor_link(
    text: &str,
    span: &MarkdownSpan,
    runs: &mut Vec<AttributeRun>,
) {
    let (start, end) = (span.source_range.0, span.source_range.1.min(text.len()));

    let (content_start, content_end) = if !span.children.is_empty() {
        (
            span.children.first().unwrap().source_range.0,
            span.children.last().unwrap().source_range.1.min(end),
        )
    } else {
        let bracket = text[start..end].find("](").map(|p| start + p).unwrap_or(end);
        (start + 1, bracket)
    };

    // Opening "[" in syntax color.
    runs.push(AttributeRun { range: (start, content_start), attrs: editor_syntax() });

    // Link title in link color.
    if content_start < content_end {
        runs.push(AttributeRun {
            range: (content_start, content_end),
            attrs: editor_link(),
        });
    }

    // Closing "](url)" in syntax color.
    if content_end < end {
        runs.push(AttributeRun { range: (content_end, end), attrs: editor_syntax() });
    }
}

fn collect_editor_item(
    text: &str,
    span: &MarkdownSpan,
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
            attrs: editor_list_marker(),
        });
    }
    for child in &span.children {
        collect_editor_runs(text, child, runs);
    }
}

// ---------------------------------------------------------------------------
// Attribute constructors for editor mode
// ---------------------------------------------------------------------------

/// All editor attributes use Monospace to signal "use monospace font".
fn editor_heading() -> AttributeSet {
    AttributeSet::new(vec![
        TextAttribute::Monospace,
        TextAttribute::Bold,
        TextAttribute::ForegroundColor("heading"),
    ])
}

fn editor_syntax() -> AttributeSet {
    AttributeSet::new(vec![
        TextAttribute::Monospace,
        TextAttribute::ForegroundColor("syntax"),
    ])
}

fn editor_bold() -> AttributeSet {
    AttributeSet::new(vec![
        TextAttribute::Monospace,
        TextAttribute::Bold,
    ])
}

fn editor_italic() -> AttributeSet {
    AttributeSet::new(vec![
        TextAttribute::Monospace,
        TextAttribute::Italic,
    ])
}

fn editor_code() -> AttributeSet {
    AttributeSet::new(vec![
        TextAttribute::Monospace,
        TextAttribute::ForegroundColor("code_fg"),
    ])
}

fn editor_link() -> AttributeSet {
    AttributeSet::new(vec![
        TextAttribute::Monospace,
        TextAttribute::ForegroundColor("link"),
    ])
}

fn editor_strikethrough() -> AttributeSet {
    AttributeSet::new(vec![
        TextAttribute::Monospace,
        TextAttribute::Strikethrough,
        TextAttribute::ForegroundColor("strikethrough"),
    ])
}

fn editor_blockquote() -> AttributeSet {
    AttributeSet::new(vec![
        TextAttribute::Monospace,
        TextAttribute::ForegroundColor("blockquote"),
    ])
}

fn editor_list_marker() -> AttributeSet {
    AttributeSet::new(vec![
        TextAttribute::Monospace,
        TextAttribute::ForegroundColor("list_marker"),
    ])
}

// ---------------------------------------------------------------------------
// Gap filler (same logic as renderer.rs)
// ---------------------------------------------------------------------------

fn fill_gaps(text_len: usize, mut runs: Vec<AttributeRun>) -> Vec<AttributeRun> {
    // Plain text in editor mode also gets Monospace.
    let plain = AttributeSet::new(vec![TextAttribute::Monospace]);
    runs.sort_by_key(|r| r.range.0);
    let mut result = Vec::new();
    let mut pos = 0usize;
    for run in runs {
        if run.range.0 > pos {
            result.push(AttributeRun {
                range: (pos, run.range.0),
                attrs: plain.clone(),
            });
        }
        pos = run.range.1.max(pos);
        result.push(run);
    }
    if pos < text_len {
        result.push(AttributeRun {
            range: (pos, text_len),
            attrs: plain,
        });
    }
    result
}
