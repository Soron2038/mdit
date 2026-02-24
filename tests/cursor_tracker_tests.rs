use mdit::editor::cursor_tracker::find_containing_span;
use mdit::markdown::parser::{parse, NodeKind};

#[test]
fn cursor_inside_bold_finds_strong_span() {
    let text = "hello **world** end";
    let spans = parse(text);
    let result = find_containing_span(&spans, 10); // inside "world"
    assert!(result.is_some(), "should find a span at position 10");
    assert_eq!(result.unwrap().kind, NodeKind::Strong);
}

#[test]
fn cursor_outside_all_spans_finds_nothing_interesting() {
    let text = "**bold**";
    let spans = parse(text);
    // Position way past the text
    let result = find_containing_span(&spans, 200);
    assert!(result.is_none(), "should not find a span at position 200");
}

#[test]
fn cursor_in_heading_finds_heading() {
    let text = "# Title";
    let spans = parse(text);
    let result = find_containing_span(&spans, 3);
    assert!(result.is_some());
    assert!(matches!(result.unwrap().kind, NodeKind::Heading { .. }));
}

#[test]
fn cursor_in_italic_finds_emph() {
    let text = "some *italic* text";
    let spans = parse(text);
    let result = find_containing_span(&spans, 7); // inside "italic"
    assert!(result.is_some());
    assert_eq!(result.unwrap().kind, NodeKind::Emph);
}
