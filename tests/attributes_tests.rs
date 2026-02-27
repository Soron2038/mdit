use mdit::markdown::attributes::{AttributeSet, TextAttribute};

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
    let attrs = AttributeSet::for_heading(1);
    assert!(attrs.font_size() > 20.0);
}

#[test]
fn heading3_gets_medium_size() {
    let attrs = AttributeSet::for_heading(3);
    assert!(attrs.font_size() > 16.0);
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
    let attrs = AttributeSet::for_heading(1);
    assert!(attrs.contains(&TextAttribute::HeadingSeparator));
}

#[test]
fn h2_gets_heading_separator() {
    let attrs = AttributeSet::for_heading(2);
    assert!(attrs.contains(&TextAttribute::HeadingSeparator));
}

#[test]
fn h3_does_not_get_heading_separator() {
    let attrs = AttributeSet::for_heading(3);
    assert!(!attrs.contains(&TextAttribute::HeadingSeparator));
}

#[test]
fn code_block_gets_monospace_no_bg_color() {
    let attrs = AttributeSet::for_code_block();
    assert!(attrs.contains(&TextAttribute::Monospace));
    // Background color is now drawn via NSBezierPath overlay, not NSAttributedString.
    assert!(!attrs.contains(&TextAttribute::BackgroundColor("code_block_bg")));
}
