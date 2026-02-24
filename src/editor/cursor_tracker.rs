use crate::markdown::parser::{MarkdownSpan, NodeKind};

/// Return the innermost "interesting" span that contains `pos`.
///
/// Container nodes (`Paragraph`, `List`, `Item`, `Text`, `Other`) are
/// skipped — we only return formatting spans like `Strong`, `Emph`,
/// `Code`, `Heading`, etc.
pub fn find_containing_span(spans: &[MarkdownSpan], pos: usize) -> Option<&MarkdownSpan> {
    for span in spans {
        if pos >= span.source_range.0 && pos <= span.source_range.1 {
            // Check children first (prefer the innermost match)
            if let Some(inner) = find_containing_span(&span.children, pos) {
                return Some(inner);
            }
            if is_interesting(&span.kind) {
                return Some(span);
            }
        }
    }
    None
}

/// Nodes that represent visible formatting (not container / structural).
fn is_interesting(kind: &NodeKind) -> bool {
    !matches!(
        kind,
        NodeKind::Text
            | NodeKind::Other
            | NodeKind::Paragraph
            | NodeKind::List
            | NodeKind::Item
            | NodeKind::HtmlInline
    )
}