use mdit_core::render::editor_mode::compute_editor_runs;
use mdit_core::markdown::attributes::TextAttribute;
use mdit_core::markdown::parser::parse;

// ---------------------------------------------------------------------------
// Basic structure
// ---------------------------------------------------------------------------

#[test]
fn all_runs_have_monospace() {
    let text = "# Hello **world** `code`";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    for run in &runs {
        assert!(
            run.attrs.contains(&TextAttribute::Monospace),
            "every editor run should have Monospace, but run {:?} does not",
            run.range,
        );
    }
}

#[test]
fn no_hidden_attributes() {
    let text = "**bold** _italic_ ~~strike~~";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    for run in &runs {
        assert!(
            !run.attrs.contains(&TextAttribute::Hidden),
            "editor mode should never produce Hidden attributes",
        );
    }
}

#[test]
fn no_heading_separator() {
    let text = "# Heading\n\nParagraph";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    for run in &runs {
        assert!(
            !run.attrs.contains(&TextAttribute::HeadingSeparator),
            "editor mode should not produce HeadingSeparator",
        );
    }
}

#[test]
fn no_thematic_break_attribute() {
    let text = "above\n\n---\n\nbelow";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    for run in &runs {
        assert!(
            !run.attrs.contains(&TextAttribute::ThematicBreak),
            "editor mode should not produce ThematicBreak",
        );
    }
}

#[test]
fn no_font_size_variation() {
    let text = "# H1\n## H2\n### H3\nNormal";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    for run in &runs {
        let has_font_size = run.attrs.attrs().iter().any(|a| matches!(a, TextAttribute::FontSize(_)));
        assert!(
            !has_font_size,
            "editor mode should not produce FontSize attributes",
        );
    }
}

// ---------------------------------------------------------------------------
// Heading
// ---------------------------------------------------------------------------

#[test]
fn heading_gets_heading_color() {
    let text = "# Hello";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    let heading_run = runs.iter().find(|r| {
        r.attrs.contains(&TextAttribute::ForegroundColor("heading"))
    });
    assert!(heading_run.is_some(), "heading should get heading color");
}

#[test]
fn heading_gets_bold() {
    let text = "# Hello";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    let bold_run = runs.iter().find(|r| r.attrs.contains(&TextAttribute::Bold));
    assert!(bold_run.is_some(), "heading in editor mode should be bold");
}

// ---------------------------------------------------------------------------
// Inline formatting
// ---------------------------------------------------------------------------

#[test]
fn bold_markers_get_syntax_color() {
    let text = "**bold**";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    // The "**" markers should have syntax color.
    let syntax_runs: Vec<_> = runs.iter().filter(|r| {
        r.attrs.contains(&TextAttribute::ForegroundColor("syntax"))
    }).collect();
    assert!(syntax_runs.len() >= 2, "bold markers should have syntax color");
}

#[test]
fn bold_content_gets_bold() {
    let text = "**bold**";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    let bold_run = runs.iter().find(|r| {
        r.attrs.contains(&TextAttribute::Bold)
            && !r.attrs.contains(&TextAttribute::ForegroundColor("syntax"))
    });
    assert!(bold_run.is_some(), "bold content should have Bold attribute");
}

#[test]
fn italic_markers_get_syntax_color() {
    let text = "_italic_";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    let syntax_runs: Vec<_> = runs.iter().filter(|r| {
        r.attrs.contains(&TextAttribute::ForegroundColor("syntax"))
    }).collect();
    assert!(syntax_runs.len() >= 2, "italic markers should have syntax color");
}

#[test]
fn inline_code_backticks_get_syntax_color() {
    let text = "`code`";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    let syntax_runs: Vec<_> = runs.iter().filter(|r| {
        r.attrs.contains(&TextAttribute::ForegroundColor("syntax"))
    }).collect();
    assert!(syntax_runs.len() >= 2, "backtick markers should have syntax color");
}

#[test]
fn inline_code_content_gets_code_color() {
    let text = "`code`";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    let code_run = runs.iter().find(|r| {
        r.attrs.contains(&TextAttribute::ForegroundColor("code_fg"))
    });
    assert!(code_run.is_some(), "inline code content should have code_fg color");
}

// ---------------------------------------------------------------------------
// Code blocks
// ---------------------------------------------------------------------------

#[test]
fn code_block_gets_code_color() {
    let text = "```rust\nfn main() {}\n```\n";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    let code_run = runs.iter().find(|r| {
        r.attrs.contains(&TextAttribute::ForegroundColor("code_fg"))
    });
    assert!(code_run.is_some(), "code block should have code_fg color");
}

// ---------------------------------------------------------------------------
// Links
// ---------------------------------------------------------------------------

#[test]
fn link_title_gets_link_color() {
    let text = "[title](https://example.com)";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    let link_run = runs.iter().find(|r| {
        r.attrs.contains(&TextAttribute::ForegroundColor("link"))
    });
    assert!(link_run.is_some(), "link title should have link color");
}

#[test]
fn link_brackets_get_syntax_color() {
    let text = "[title](https://example.com)";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    let syntax_runs: Vec<_> = runs.iter().filter(|r| {
        r.attrs.contains(&TextAttribute::ForegroundColor("syntax"))
    }).collect();
    assert!(syntax_runs.len() >= 2, "link brackets should have syntax color");
}

// ---------------------------------------------------------------------------
// Lists and blockquotes
// ---------------------------------------------------------------------------

#[test]
fn list_marker_gets_list_color() {
    let text = "- item one\n- item two";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    let marker_run = runs.iter().find(|r| {
        r.attrs.contains(&TextAttribute::ForegroundColor("list_marker"))
    });
    assert!(marker_run.is_some(), "list marker should have list_marker color");
}

#[test]
fn blockquote_gets_blockquote_color() {
    let text = "> quoted text";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    let quote_run = runs.iter().find(|r| {
        r.attrs.contains(&TextAttribute::ForegroundColor("blockquote"))
    });
    assert!(quote_run.is_some(), "blockquote should have blockquote color");
}

// ---------------------------------------------------------------------------
// Coverage: full text is covered
// ---------------------------------------------------------------------------

#[test]
fn runs_cover_entire_text() {
    let text = "# Hello **world** `code`\n\n---\n\n> quote";
    let spans = parse(text);
    let runs = compute_editor_runs(text, &spans);
    // Check runs are sorted and cover [0, text.len()).
    let mut pos = 0;
    for run in &runs {
        assert!(
            run.range.0 <= pos,
            "gap at byte {} (run starts at {})",
            pos, run.range.0,
        );
        pos = pos.max(run.range.1);
    }
    assert_eq!(pos, text.len(), "runs should cover entire text");
}
