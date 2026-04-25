use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena, Options};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Clone)]
pub enum NodeKind {
    Text,
    Strong,
    Emph,
    Code,
    Math,
    Link { url: String },
    Heading { level: u8 },
    CodeBlock { language: String, code: String },
    Table,
    TableRow { header: bool },
    TableCell,
    Footnote,
    Strikethrough,
    Underline,
    Highlight,
    Subscript,
    Superscript,
    Image { url: String },
    List,
    Item,
    TaskItem { checked: bool },
    BlockQuote,
    ThematicBreak,
    Paragraph,
    HtmlInline,
    Other,
}

#[derive(Debug, Clone)]
pub struct MarkdownSpan {
    pub kind: NodeKind,
    /// Byte-offsets in the original source string [start, end)
    pub source_range: (usize, usize),
    pub children: Vec<MarkdownSpan>,
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

fn make_options() -> Options<'static> {
    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.footnotes = true;
    opts.extension.math_dollars = true;
    opts.extension.highlight = true;
    opts.extension.subscript = true;
    opts.extension.superscript = true;
    opts.extension.underline = true;
    opts.extension.tasklist = true;
    opts
}

// ---------------------------------------------------------------------------
// Byte-offset helpers
// ---------------------------------------------------------------------------

/// Pre-compute byte offset of the start of each line (0-indexed line → byte offset).
fn line_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0usize];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Convert 1-indexed (line, col) from comrak sourcepos to a byte offset.
fn to_offset(offsets: &[usize], line: usize, col: usize) -> usize {
    let line_start = offsets.get(line.saturating_sub(1)).copied().unwrap_or(0);
    line_start + col.saturating_sub(1)
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

pub fn parse(source: &str) -> Vec<MarkdownSpan> {
    let arena = Arena::new();
    let opts = make_options();
    let root = parse_document(&arena, source, &opts);
    let offsets = line_offsets(source);
    collect_spans(root, source, &offsets)
}

fn collect_spans<'a>(
    node: &'a AstNode<'a>,
    source: &str,
    offsets: &[usize],
) -> Vec<MarkdownSpan> {
    let mut spans = Vec::new();
    for child in node.children() {
        if let Some(span) = node_to_span(child, source, offsets) {
            spans.push(span);
        } else {
            // Still recurse for block wrappers we don't directly represent
            spans.extend(collect_spans(child, source, offsets));
        }
    }
    spans
}

fn node_to_span<'a>(
    node: &'a AstNode<'a>,
    source: &str,
    offsets: &[usize],
) -> Option<MarkdownSpan> {
    let data = node.data.borrow();
    let sp = &data.sourcepos;
    let start = to_offset(offsets, sp.start.line, sp.start.column);
    let end = to_offset(offsets, sp.end.line, sp.end.column + 1);
    let source_range = (start.min(source.len()), end.min(source.len()));

    let children = collect_spans(node, source, offsets);

    let kind = match &data.value {
        NodeValue::Strong => NodeKind::Strong,
        NodeValue::Emph => NodeKind::Emph,
        NodeValue::Code(_) => NodeKind::Code,
        NodeValue::Math(_) => NodeKind::Math,
        NodeValue::Link(l) => NodeKind::Link { url: l.url.clone() },
        NodeValue::Image(i) => NodeKind::Image { url: i.url.clone() },
        NodeValue::Heading(h) => NodeKind::Heading { level: h.level },
        NodeValue::CodeBlock(cb) => NodeKind::CodeBlock {
            language: cb.info.trim().to_string(),
            code: cb.literal.trim_end_matches('\n').to_string(),
        },
        NodeValue::Table(_) => NodeKind::Table,
        NodeValue::TableRow(header) => NodeKind::TableRow { header: *header },
        NodeValue::TableCell => NodeKind::TableCell,
        NodeValue::FootnoteDefinition(_) | NodeValue::FootnoteReference(_) => NodeKind::Footnote,
        NodeValue::Strikethrough => NodeKind::Strikethrough,
        NodeValue::Underline => NodeKind::Underline,
        NodeValue::Highlight => NodeKind::Highlight,
        NodeValue::Subscript => NodeKind::Subscript,
        NodeValue::Superscript => NodeKind::Superscript,
        NodeValue::List(_) => NodeKind::List,
        NodeValue::Item(_) => NodeKind::Item,
        NodeValue::TaskItem(ti) => NodeKind::TaskItem { checked: ti.symbol.is_some() },
        NodeValue::BlockQuote => NodeKind::BlockQuote,
        NodeValue::ThematicBreak => NodeKind::ThematicBreak,
        NodeValue::Paragraph => NodeKind::Paragraph,
        NodeValue::Text(_) => NodeKind::Text,
        NodeValue::HtmlInline(_) => NodeKind::HtmlInline,
        _ => NodeKind::Other,
    };

    Some(MarkdownSpan {
        kind,
        source_range,
        children,
    })
}