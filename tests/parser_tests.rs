use mdit::markdown::parser::{parse, MarkdownSpan, NodeKind};

/// Flatten the span tree into a flat vec for easier test assertions.
fn flatten(spans: &[MarkdownSpan]) -> Vec<&MarkdownSpan> {
    let mut result = Vec::new();
    for span in spans {
        result.push(span);
        result.extend(flatten(&span.children));
    }
    result
}

#[test]
fn parses_bold() {
    let nodes = parse("**hello**");
    assert!(flatten(&nodes).iter().any(|n| n.kind == NodeKind::Strong), "expected Strong node");
}

#[test]
fn parses_italic() {
    let nodes = parse("*world*");
    assert!(flatten(&nodes).iter().any(|n| n.kind == NodeKind::Emph), "expected Emph node");
}

#[test]
fn parses_heading() {
    let nodes = parse("# Title");
    assert!(
        flatten(&nodes).iter().any(|n| matches!(n.kind, NodeKind::Heading { level: 1 })),
        "expected H1 node"
    );
}

#[test]
fn parses_code_block() {
    let nodes = parse("```rust\nfn main() {}\n```");
    assert!(
        flatten(&nodes).iter().any(|n| matches!(n.kind, NodeKind::CodeBlock { .. })),
        "expected CodeBlock node"
    );
}

#[test]
fn parses_inline_math() {
    let nodes = parse("$x^2$");
    assert!(flatten(&nodes).iter().any(|n| n.kind == NodeKind::Math), "expected Math node");
}

#[test]
fn parses_inline_code() {
    let nodes = parse("`code`");
    assert!(flatten(&nodes).iter().any(|n| n.kind == NodeKind::Code), "expected Code node");
}

#[test]
fn parses_strikethrough() {
    let nodes = parse("~~strike~~");
    assert!(
        flatten(&nodes).iter().any(|n| n.kind == NodeKind::Strikethrough),
        "expected Strikethrough node"
    );
}

#[test]
fn parses_link() {
    let nodes = parse("[label](https://example.com)");
    assert!(
        flatten(&nodes).iter().any(|n| matches!(n.kind, NodeKind::Link { .. })),
        "expected Link node"
    );
}

#[test]
fn bold_source_range_is_correct() {
    let source = "hello **world** end";
    let nodes = parse(source);
    let all = flatten(&nodes);
    let bold = all.iter().find(|n| n.kind == NodeKind::Strong).expect("Strong node");
    let extracted = &source[bold.source_range.0..bold.source_range.1];
    assert!(extracted.contains("world"), "source range should cover bold content, got: {:?}", extracted);
}
