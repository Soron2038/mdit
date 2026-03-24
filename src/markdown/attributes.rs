/// Platform-agnostic description of how a text range should be styled.
/// Converted to actual NSAttributedString attributes in the AppKit layer.
#[derive(Debug, Clone, PartialEq)]
pub enum TextAttribute {
    Bold,
    Italic,
    Monospace,
    Hidden,
    FontSize(u8),
    /// Named color token resolved by the active ColorScheme.
    ForegroundColor(&'static str),
    BackgroundColor(&'static str),
    ListMarker,
    BlockquoteBar,
    Strikethrough,
    Superscript,
    Subscript,
    LineSpacing(u32), // in tenths of a point (e.g. 96 = 9.6pt)
    /// Marks an H1/H2 heading paragraph: triggers a 1px separator line drawn
    /// above the heading (only when content precedes it in the document).
    HeadingSeparator,
    /// Marks a thematic break (horizontal rule): triggers a centred
    /// horizontal line drawn across the full width.
    ThematicBreak,
    /// Clickable link — value is the target URL string.
    Link(String),
}

#[derive(Debug, Clone, Default)]
pub struct AttributeSet(Vec<TextAttribute>);

impl AttributeSet {
    pub fn new(attrs: Vec<TextAttribute>) -> Self {
        Self(attrs)
    }

    pub fn contains(&self, attr: &TextAttribute) -> bool {
        self.0.contains(attr)
    }

    pub fn font_size(&self) -> Option<f64> {
        self.0.iter().find_map(|a| {
            if let TextAttribute::FontSize(s) = a {
                Some(*s as f64)
            } else {
                None
            }
        })
    }

    pub fn attrs(&self) -> &[TextAttribute] {
        &self.0
    }

    /// Return a new set with `attr` appended.
    pub fn with(&self, attr: TextAttribute) -> Self {
        let mut v = self.0.clone();
        v.push(attr);
        Self(v)
    }

    // --- Constructors ---

    pub fn for_strong() -> Self {
        Self::new(vec![TextAttribute::Bold])
    }

    pub fn for_emph() -> Self {
        Self::new(vec![TextAttribute::Italic])
    }

    pub fn for_strong_emph() -> Self {
        Self::new(vec![TextAttribute::Bold, TextAttribute::Italic])
    }

    pub fn for_heading(level: u8, base_size: f64) -> Self {
        let size = match level {
            1 => (base_size * 1.375).round() as u8,
            2 => (base_size * 1.125).round() as u8,
            _ => base_size as u8,
        };
        let mut attrs = vec![
            TextAttribute::FontSize(size),
            TextAttribute::ForegroundColor("heading"),
        ];
        if level <= 2 {
            attrs.push(TextAttribute::HeadingSeparator);
        }
        Self::new(attrs)
    }

    pub fn for_inline_code() -> Self {
        Self::new(vec![
            TextAttribute::Monospace,
            TextAttribute::BackgroundColor("code_bg"),
            TextAttribute::ForegroundColor("code_fg"),
        ])
    }

    pub fn for_code_block() -> Self {
        Self::new(vec![
            TextAttribute::Monospace,
            TextAttribute::ForegroundColor("code_fg"),
        ])
    }

    pub fn for_link() -> Self {
        Self::new(vec![TextAttribute::ForegroundColor("link")])
    }

    pub fn for_strikethrough() -> Self {
        Self::new(vec![
            TextAttribute::Strikethrough,
            TextAttribute::ForegroundColor("strikethrough"),
        ])
    }

    pub fn for_highlight() -> Self {
        Self::new(vec![TextAttribute::BackgroundColor("highlight_bg")])
    }

    pub fn for_subscript() -> Self {
        Self::new(vec![
            TextAttribute::Subscript,
            TextAttribute::ForegroundColor("subscript"),
        ])
    }

    pub fn for_superscript() -> Self {
        Self::new(vec![
            TextAttribute::Superscript,
            TextAttribute::ForegroundColor("superscript"),
        ])
    }

    pub fn for_blockquote() -> Self {
        Self::new(vec![
            TextAttribute::BlockquoteBar,
            TextAttribute::ForegroundColor("blockquote"),
        ])
    }

    pub fn for_list_marker() -> Self {
        Self::new(vec![
            TextAttribute::ListMarker,
            TextAttribute::ForegroundColor("list_marker"),
        ])
    }

    pub fn syntax_hidden() -> Self {
        Self::new(vec![TextAttribute::Hidden])
    }

    pub fn syntax_visible() -> Self {
        Self::new(vec![TextAttribute::ForegroundColor("syntax")])
    }

    pub fn plain() -> Self {
        Self::new(vec![])
    }
}