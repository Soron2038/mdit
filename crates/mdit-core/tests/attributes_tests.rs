use mdit_core::markdown::attributes::{AttributeSet, TextAttribute};

#[test]
fn bold_gets_bold_font_trait() {
    let attrs = AttributeSet::for_strong();
    assert!(attrs.contains(&TextAttribute::Bold));
}

#[test]
fn italic_gets_italic_font_trait() {
    let attrs = AttributeSet::for_emph();
    assert!(attrs.contains(&TextAttribute::Italic));
}

#[test]
fn heading1_gets_large_size() {
    let attrs = AttributeSet::for_heading(1, 16.0);
    assert!(attrs.font_size().unwrap_or(0.0) > 20.0);
}

#[test]
fn heading3_gets_medium_size() {
    let attrs = AttributeSet::for_heading(3, 16.0);
    // H3 equals body size (16pt at default).
    assert!(attrs.font_size().unwrap_or(0.0) >= 14.0);
}

#[test]
fn code_gets_monospace() {
    let attrs = AttributeSet::for_inline_code();
    assert!(attrs.contains(&TextAttribute::Monospace));
}

#[test]
fn syntax_marker_is_hidden() {
    let attrs = AttributeSet::syntax_hidden();
    assert!(attrs.contains(&TextAttribute::Hidden));
}

#[test]
fn syntax_marker_visible_does_not_hide() {
    let attrs = AttributeSet::syntax_visible();
    assert!(!attrs.contains(&TextAttribute::Hidden));
}

#[test]
fn plain_has_no_attributes() {
    let attrs = AttributeSet::plain();
    assert!(attrs.attrs().is_empty());
}

#[test]
fn h1_gets_heading_separator() {
    let attrs = AttributeSet::for_heading(1, 16.0);
    assert!(attrs.contains(&TextAttribute::HeadingSeparator));
}

#[test]
fn h2_gets_heading_separator() {
    let attrs = AttributeSet::for_heading(2, 16.0);
    assert!(attrs.contains(&TextAttribute::HeadingSeparator));
}

#[test]
fn h3_does_not_get_heading_separator() {
    let attrs = AttributeSet::for_heading(3, 16.0);
    assert!(!attrs.contains(&TextAttribute::HeadingSeparator));
}

#[test]
fn code_block_gets_monospace_no_bg_color() {
    let attrs = AttributeSet::for_code_block();
    assert!(attrs.contains(&TextAttribute::Monospace));
    // Background color is now drawn via NSBezierPath overlay, not NSAttributedString.
    assert!(!attrs.contains(&TextAttribute::BackgroundColor("code_block_bg")));
}

#[test]
fn font_size_returns_none_when_no_size_attribute() {
    let attrs = AttributeSet::for_strong();
    assert_eq!(attrs.font_size(), None);
}

#[test]
fn font_size_returns_some_for_heading() {
    let attrs = AttributeSet::for_heading(1, 16.0);
    assert!(attrs.font_size().is_some());
}

#[test]
fn heading1_scales_proportionally_at_default() {
    let attrs = AttributeSet::for_heading(1, 16.0);
    // 16 * 1.375 = 22.0
    assert_eq!(attrs.font_size(), Some(22.0));
}

#[test]
fn heading1_scales_proportionally_at_20pt() {
    let attrs = AttributeSet::for_heading(1, 20.0);
    // 20 * 1.375 = 27.5 → rounds to 28
    assert_eq!(attrs.font_size(), Some(28.0));
}

#[test]
fn heading2_scales_proportionally_at_default() {
    let attrs = AttributeSet::for_heading(2, 16.0);
    // 16 * 1.125 = 18.0
    assert_eq!(attrs.font_size(), Some(18.0));
}

#[test]
fn heading2_scales_proportionally_at_20pt() {
    let attrs = AttributeSet::for_heading(2, 20.0);
    // 20 * 1.125 = 22.5 → rounds to 23 (round-half-away-from-zero)
    assert_eq!(attrs.font_size(), Some(23.0));
}

#[test]
fn heading3_equals_body_size() {
    let attrs = AttributeSet::for_heading(3, 16.0);
    assert_eq!(attrs.font_size(), Some(16.0));
}

#[test]
fn heading3_equals_body_at_20pt() {
    let attrs = AttributeSet::for_heading(3, 20.0);
    assert_eq!(attrs.font_size(), Some(20.0));
}
