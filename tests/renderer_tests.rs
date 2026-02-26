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

// ── Setext heading tests ──────────────────────────────────────────────────────

#[test]
fn setext_h2_does_not_hide_content_prefix() {
    // "kursiv\n-\n" is a setext H2. Content bytes 0..6 ("kursiv") must have
    // heading font size and must NOT be hidden — there is no '#' prefix to hide.
    let text = "kursiv\n-\n";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);

    let content_run = runs.iter().find(|r| r.range == (0, 6));
    assert!(content_run.is_some(),
        "expected a run for 'kursiv' at (0, 6); runs: {:?}", runs.iter().map(|r| r.range).collect::<Vec<_>>());
    let content_run = content_run.unwrap();
    assert!(content_run.attrs.font_size() > 20.0,
        "setext H2 content must have heading font size, got {}", content_run.attrs.font_size());
    assert!(!content_run.attrs.contains(&TextAttribute::Hidden),
        "setext H2 content must not be hidden");
}

#[test]
fn setext_h2_underline_is_syntax_marker() {
    // The underline region ("\n-", starting at byte 6) must be a syntax marker.
    // With cursor=None, syntax markers are hidden.
    let text = "kursiv\n-\n";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);

    let underline_run = runs.iter()
        .find(|r| r.range.0 == 6)
        .expect("expected a run starting at byte 6 (underline '\\n-')");
    assert!(underline_run.attrs.contains(&TextAttribute::Hidden),
        "setext underline must be hidden when cursor is outside");
}

#[test]
fn atx_heading_prefix_still_hidden() {
    // Regression: ATX headings must still hide the '## ' prefix (3 bytes for H2).
    let text = "## Hello\n";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);

    let prefix_run = runs.iter().find(|r| r.range == (0, 3));
    assert!(prefix_run.is_some(),
        "expected ATX prefix run at (0, 3); runs: {:?}", runs.iter().map(|r| r.range).collect::<Vec<_>>());
    assert!(prefix_run.unwrap().attrs.contains(&TextAttribute::Hidden),
        "ATX prefix '## ' must be hidden");
}

#[test]
fn atx_h1_content_gets_heading_separator() {
    // HeadingSeparator must be present on the content run of an ATX H1.
    // (No content precedes it, but the attribute is unconditionally emitted
    // by the renderer; the content-before filter lives in apply_attribute_runs.)
    let text = "# Title\n";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    let sep_run = runs.iter().find(|r| r.attrs.contains(&TextAttribute::HeadingSeparator));
    assert!(sep_run.is_some(), "expected HeadingSeparator on ATX H1 content run");
}

#[test]
fn setext_h1_content_gets_heading_separator() {
    // HeadingSeparator must be present on the content run of a setext H1.
    let text = "Title\n=====\n";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    let sep_run = runs.iter().find(|r| r.attrs.contains(&TextAttribute::HeadingSeparator));
    assert!(sep_run.is_some(), "expected HeadingSeparator on setext H1 content run");
}

#[test]
fn h3_content_does_not_get_heading_separator() {
    // HeadingSeparator must NOT appear for H3 or below.
    let text = "### Section\n";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    let sep_run = runs.iter().find(|r| r.attrs.contains(&TextAttribute::HeadingSeparator));
    assert!(sep_run.is_none(), "HeadingSeparator must not appear on H3");
}
