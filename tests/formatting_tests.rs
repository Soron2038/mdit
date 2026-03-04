use mdit::editor::formatting::{
    compute_code_block_wrap, compute_inline_toggle, compute_link_wrap, detect_block_prefix,
    find_surrounding_markers, peel_inline_markers, set_block_format, toggle_marker_in_layers,
    wrap_with_layers, InlineToggleResult,
};

// ── detect_block_prefix ──────────────────────────────────────────────────

#[test]
fn detect_no_prefix() {
    assert_eq!(detect_block_prefix("Hello world"), None);
}

#[test]
fn detect_h1() {
    assert_eq!(detect_block_prefix("# Hello"), Some("# "));
}

#[test]
fn detect_h2() {
    assert_eq!(detect_block_prefix("## Hello"), Some("## "));
}

#[test]
fn detect_h3() {
    assert_eq!(detect_block_prefix("### Hello"), Some("### "));
}

#[test]
fn detect_blockquote() {
    assert_eq!(detect_block_prefix("> Hello"), Some("> "));
}

#[test]
fn detect_h1_with_trailing_newline() {
    assert_eq!(detect_block_prefix("# Hello\n"), Some("# "));
}

// ── set_block_format ─────────────────────────────────────────────────────

#[test]
fn plain_to_h1() {
    assert_eq!(set_block_format("Hello", "# "), "# Hello");
}

#[test]
fn h1_to_h1_toggles_off() {
    assert_eq!(set_block_format("# Hello", "# "), "Hello");
}

#[test]
fn h1_to_h2_switches() {
    assert_eq!(set_block_format("# Hello", "## "), "## Hello");
}

#[test]
fn h2_to_h3_switches() {
    assert_eq!(set_block_format("## Hello", "### "), "### Hello");
}

#[test]
fn blockquote_to_h1_switches() {
    assert_eq!(set_block_format("> Hello", "# "), "# Hello");
}

#[test]
fn h1_to_normal() {
    assert_eq!(set_block_format("# Hello", ""), "Hello");
}

#[test]
fn normal_to_normal_noop() {
    assert_eq!(set_block_format("Hello", ""), "Hello");
}

#[test]
fn blockquote_toggles_off() {
    assert_eq!(set_block_format("> Hello", "> "), "Hello");
}

#[test]
fn preserves_trailing_newline() {
    assert_eq!(set_block_format("# Hello\n", "## "), "## Hello\n");
}

// ── find_surrounding_markers ─────────────────────────────────────────────

#[test]
fn no_markers_around() {
    let (layers, pre, post) = find_surrounding_markers("hello ", " world");
    assert!(layers.is_empty());
    assert_eq!(pre, 0);
    assert_eq!(post, 0);
}

#[test]
fn bold_markers_around() {
    let (layers, pre, post) = find_surrounding_markers("**", "**");
    assert_eq!(layers, vec!["**"]);
    assert_eq!(pre, 2);
    assert_eq!(post, 2);
}

#[test]
fn italic_markers_around() {
    let (layers, pre, post) = find_surrounding_markers("_", "_");
    assert_eq!(layers, vec!["_"]);
    assert_eq!(pre, 1);
    assert_eq!(post, 1);
}

#[test]
fn inline_code_markers_around() {
    let (layers, pre, post) = find_surrounding_markers("`", "`");
    assert_eq!(layers, vec!["`"]);
    assert_eq!(pre, 1);
    assert_eq!(post, 1);
}

#[test]
fn strikethrough_markers_around() {
    let (layers, pre, post) = find_surrounding_markers("~~", "~~");
    assert_eq!(layers, vec!["~~"]);
    assert_eq!(pre, 2);
    assert_eq!(post, 2);
}

#[test]
fn markers_with_preceding_text() {
    // "some text **" before selection, "** more" after
    let (layers, pre, post) = find_surrounding_markers("some text **", "** more");
    assert_eq!(layers, vec!["**"]);
    assert_eq!(pre, 2);
    assert_eq!(post, 2);
}

#[test]
fn unmatched_markers_ignored() {
    // ** on left but not on right
    let (layers, _, _) = find_surrounding_markers("**", " end");
    assert!(layers.is_empty());
}

// ── toggle_marker_in_layers ──────────────────────────────────────────────

#[test]
fn toggle_adds_missing_marker() {
    let result = toggle_marker_in_layers(&[], "**");
    assert_eq!(result, vec!["**"]);
}

#[test]
fn toggle_removes_present_marker() {
    let result = toggle_marker_in_layers(&["**"], "**");
    assert!(result.is_empty());
}

// ── wrap_with_layers ─────────────────────────────────────────────────────

#[test]
fn wrap_no_layers() {
    assert_eq!(wrap_with_layers("hello", &[]), "hello");
}

#[test]
fn wrap_one_layer() {
    assert_eq!(wrap_with_layers("hello", &["**"]), "**hello**");
}

// ── peel_inline_markers ────────────────────────────────────────────────

#[test]
fn peel_no_markers() {
    let (layers, inner) = peel_inline_markers("hello");
    assert!(layers.is_empty());
    assert_eq!(inner, "hello");
}

#[test]
fn peel_bold() {
    let (layers, inner) = peel_inline_markers("**hello**");
    assert_eq!(layers, vec!["**"]);
    assert_eq!(inner, "hello");
}

#[test]
fn peel_italic() {
    let (layers, inner) = peel_inline_markers("_hello_");
    assert_eq!(layers, vec!["_"]);
    assert_eq!(inner, "hello");
}

#[test]
fn peel_nested_bold_italic() {
    let (layers, inner) = peel_inline_markers("**_hello_**");
    assert_eq!(layers, vec!["**", "_"]);
    assert_eq!(inner, "hello");
}

#[test]
fn peel_nested_italic_bold() {
    let (layers, inner) = peel_inline_markers("_**hello**_");
    assert_eq!(layers, vec!["_", "**"]);
    assert_eq!(inner, "hello");
}

#[test]
fn peel_only_opening_no_match() {
    let (layers, inner) = peel_inline_markers("**hello");
    assert!(layers.is_empty());
    assert_eq!(inner, "**hello");
}

// ── nested marker scenarios ──────────────────────────────────────────────

#[test]
fn find_bold_and_italic_nested() {
    // Text is **_hello_** — selection covers "hello"
    // Layers are returned innermost-first: "_" then "**"
    let (layers, pre, post) = find_surrounding_markers("**_", "_**");
    assert_eq!(layers, vec!["_", "**"]);
    assert_eq!(pre, 3);
    assert_eq!(post, 3);
}

#[test]
fn find_italic_and_bold_nested() {
    // Text is _**hello**_ — selection covers "hello"
    // Layers are returned innermost-first: "**" then "_"
    let (layers, pre, post) = find_surrounding_markers("_**", "**_");
    assert_eq!(layers, vec!["**", "_"]);
    assert_eq!(pre, 3);
    assert_eq!(post, 3);
}

#[test]
fn toggle_remove_inner_from_nested() {
    // layers = ["**", "_"], toggle "_" → ["**"]
    let result = toggle_marker_in_layers(&["**", "_"], "_");
    assert_eq!(result, vec!["**"]);
}

#[test]
fn toggle_remove_outer_from_nested() {
    // layers = ["**", "_"], toggle "**" → ["_"]
    let result = toggle_marker_in_layers(&["**", "_"], "**");
    assert_eq!(result, vec!["_"]);
}

#[test]
fn toggle_add_to_existing_layer() {
    // layers = ["**"], toggle "_" → ["**", "_"]
    let result = toggle_marker_in_layers(&["**"], "_");
    assert_eq!(result, vec!["**", "_"]);
}

#[test]
fn wrap_two_layers() {
    assert_eq!(wrap_with_layers("hello", &["**", "_"]), "**_hello_**");
}

#[test]
fn wrap_three_layers() {
    assert_eq!(wrap_with_layers("hello", &["**", "~~", "_"]), "**~~_hello_~~**");
}

// ── full round-trip scenarios ────────────────────────────────────────────

#[test]
fn roundtrip_add_bold_then_italic() {
    // Start: "hello"
    let text = wrap_with_layers("hello", &toggle_marker_in_layers(&[], "**"));
    assert_eq!(text, "**hello**");

    // Add italic
    let (layers, _, _) = find_surrounding_markers("**", "**");
    let new_layers = toggle_marker_in_layers(&layers, "_");
    let text2 = wrap_with_layers("hello", &new_layers);
    assert_eq!(text2, "**_hello_**");
}

#[test]
fn roundtrip_remove_italic_from_bold_italic() {
    // "**_hello_**" — remove italic
    let (layers, _, _) = find_surrounding_markers("**_", "_**");
    let new_layers = toggle_marker_in_layers(&layers, "_");
    let text = wrap_with_layers("hello", &new_layers);
    assert_eq!(text, "**hello**");
}

#[test]
fn roundtrip_remove_bold_from_bold_italic() {
    // "**_hello_**" — remove bold
    let (layers, _, _) = find_surrounding_markers("**_", "_**");
    let new_layers = toggle_marker_in_layers(&layers, "**");
    let text = wrap_with_layers("hello", &new_layers);
    assert_eq!(text, "_hello_");
}

// ── compute_inline_toggle ──────────────────────────────────────────────

#[test]
fn inline_toggle_add_bold_to_plain() {
    let r = compute_inline_toggle("hello", "text ", " world", "**");
    assert_eq!(r, InlineToggleResult {
        replacement: "**hello**".into(),
        consumed_before: 0,
        consumed_after: 0,
    });
}

#[test]
fn inline_toggle_remove_bold_surrounding() {
    // "**hello**" with cursor selecting "hello", before="**", after="**"
    let r = compute_inline_toggle("hello", "**", "**", "**");
    assert_eq!(r, InlineToggleResult {
        replacement: "hello".into(),
        consumed_before: 2,
        consumed_after: 2,
    });
}

#[test]
fn inline_toggle_remove_bold_from_nested() {
    // "**_hello_**" — remove bold, keep italic
    let r = compute_inline_toggle("hello", "**_", "_**", "**");
    assert_eq!(r, InlineToggleResult {
        replacement: "_hello_".into(),
        consumed_before: 3,
        consumed_after: 3,
    });
}

#[test]
fn inline_toggle_remove_italic_from_nested() {
    // "**_hello_**" — remove italic, keep bold
    let r = compute_inline_toggle("hello", "**_", "_**", "_");
    assert_eq!(r, InlineToggleResult {
        replacement: "**hello**".into(),
        consumed_before: 3,
        consumed_after: 3,
    });
}

#[test]
fn inline_toggle_add_italic_to_bold() {
    // "**hello**" — add italic inside existing bold.
    // Bold markers are not consumed; italic wraps only the selection.
    // Bridge replaces just the selection → "**_hello_**"
    let r = compute_inline_toggle("hello", "**", "**", "_");
    assert_eq!(r, InlineToggleResult {
        replacement: "_hello_".into(),
        consumed_before: 0,
        consumed_after: 0,
    });
}

#[test]
fn inline_toggle_hidden_marker_case() {
    // Selection includes markers: "**hello**" selected, no surrounding context
    let r = compute_inline_toggle("**hello**", "text ", " world", "**");
    assert_eq!(r, InlineToggleResult {
        replacement: "hello".into(),
        consumed_before: 0,
        consumed_after: 0,
    });
}

#[test]
fn inline_toggle_empty_selection_adds_markers() {
    let r = compute_inline_toggle("", "text ", " world", "**");
    assert_eq!(r, InlineToggleResult {
        replacement: "****".into(),
        consumed_before: 0,
        consumed_after: 0,
    });
}

#[test]
fn inline_toggle_add_code_to_plain() {
    let r = compute_inline_toggle("hello", " ", " ", "`");
    assert_eq!(r, InlineToggleResult {
        replacement: "`hello`".into(),
        consumed_before: 0,
        consumed_after: 0,
    });
}

// ── compute_link_wrap ──────────────────────────────────────────────────

#[test]
fn link_wrap_with_text() {
    assert_eq!(compute_link_wrap("click here", "[", "]()"), "[click here]()");
}

#[test]
fn link_wrap_empty_selection() {
    assert_eq!(compute_link_wrap("", "[", "]()"), "[]()");
}

// ── compute_code_block_wrap ────────────────────────────────────────────

#[test]
fn code_block_wrap_empty() {
    assert_eq!(compute_code_block_wrap(""), "```\n\n```");
}

#[test]
fn code_block_wrap_with_content() {
    assert_eq!(compute_code_block_wrap("let x = 1;"), "```\nlet x = 1;\n```");
}
