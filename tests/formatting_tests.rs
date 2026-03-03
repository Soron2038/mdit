use mdit::editor::formatting::{detect_block_prefix, set_block_format};

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
