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
    LineSpacing(u32), // in tenths of a point (e.g. 96 = 9.6pt)
    /// Marks an H1/H2 heading paragraph: triggers a 1px separator line drawn
    /// above the heading (only when content precedes it in the document).
    HeadingSeparator,
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

    pub fn font_size(&self) -> f64 {
        self.0
            .iter()
            .find_map(|a| {
                if let TextAttribute::FontSize(s) = a {
                    Some(*s as f64)
                } else {
                    None
                }
            })
            .unwrap_or(16.0)
    }

    pub fn attrs(&self) -> &[TextAttribute] {
        &self.0
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

    pub fn for_heading(level: u8) -> Self {
        let size: u8 = match level {
            1 => 32,
            2 => 26,
            3 => 21,
            _ => 16,
        };
        let mut attrs = vec![
            TextAttribute::Bold,
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
            TextAttribute::BackgroundColor("code_block_bg"),
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