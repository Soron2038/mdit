use mdit::editor::renderer::compute_attribute_runs;
use mdit::markdown::attributes::TextAttribute;
use mdit::markdown::parser::parse;

#[test]
fn bold_span_gets_bold_attribute() {
    let text = "hello **world** end";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    let bold_run = runs.iter().find(|r| r.attrs.contains(&TextAttribute::Bold));
    assert!(bold_run.is_some(), "expected a Bold attribute run");
}

#[test]
fn syntax_markers_hidden_when_cursor_outside() {
    let text = "**bold**";
    let spans = parse(text);
    // Cursor at position 50 → outside the span
    let runs = compute_attribute_runs(text, &spans, Some(50));
    let hidden = runs
        .iter()
        .filter(|r| r.attrs.contains(&TextAttribute::Hidden))
        .count();
    assert!(hidden > 0, "** markers should be hidden when cursor is outside");
}

#[test]
fn syntax_markers_visible_when_cursor_inside() {
    let text = "**bold**";
    let spans = parse(text);
    // Cursor at position 3 → inside **bold**
    let runs = compute_attribute_runs(text, &spans, Some(3));
    let hidden = runs
        .iter()
        .filter(|r| r.attrs.contains(&TextAttribute::Hidden))
        .count();
    assert_eq!(hidden, 0, "** markers should be visible when cursor is inside");
}

#[test]
fn italic_span_gets_italic_attribute() {
    let text = "*italic*";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    assert!(
        runs.iter().any(|r| r.attrs.contains(&TextAttribute::Italic)),
        "expected an Italic attribute run"
    );
}

#[test]
fn inline_code_gets_monospace() {
    let text = "`code`";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    assert!(
        runs.iter()
            .any(|r| r.attrs.contains(&TextAttribute::Monospace)),
        "expected Monospace for inline code"
    );
}

#[test]
fn heading_gets_large_font() {
    let text = "# Title";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    assert!(
        runs.iter().any(|r| r.attrs.font_size() > 20.0),
        "expected large font for H1"
    );
}

#[test]
fn list_item_marker_styled() {
    let text = "- Item one\n- Item two";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    let marker = runs.iter().find(|r| r.attrs.contains(&TextAttribute::ListMarker));
    assert!(marker.is_some(), "expected a ListMarker attribute run for list item");
}

#[test]
fn blockquote_gets_bar_attribute() {
    let text = "> quoted text";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    assert!(
        runs.iter().any(|r| r.attrs.contains(&TextAttribute::BlockquoteBar)),
        "expected BlockquoteBar for blockquote"
    );
}

#[test]
fn table_gets_monospace() {
    let text = "| A | B |\n|---|---|\n| 1 | 2 |";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    assert!(
        runs.iter().any(|r| r.attrs.contains(&TextAttribute::Monospace)),
        "expected Monospace fallback for table"
    );
}

#[test]
fn h1_prefix_hidden_outside_cursor() {
    let text = "# Heading";
    let spans = parse(text);
    // Cursor outside
    let runs = compute_attribute_runs(text, &spans, Some(50));
    let hidden = runs
        .iter()
        .filter(|r| r.attrs.contains(&TextAttribute::Hidden))
        .count();
    assert!(hidden > 0, "# prefix should be hidden when cursor is outside");
    let heading_run = runs.iter().find(|r| r.attrs.font_size() > 20.0);
    assert!(heading_run.is_some(), "heading content should have large font");
}
