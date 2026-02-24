/// Platform-agnostic color scheme.  All color values are sRGB floats in [0, 1].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorScheme {
    pub text: (f64, f64, f64),
    pub background: (f64, f64, f64),
    pub heading: (f64, f64, f64),
    pub link: (f64, f64, f64),
    pub code_bg: (f64, f64, f64),
    pub code_fg: (f64, f64, f64),
    pub code_block_bg: (f64, f64, f64),
    pub syntax_marker: (f64, f64, f64),
    pub strikethrough: (f64, f64, f64),
    pub blockquote: (f64, f64, f64),
    pub list_marker: (f64, f64, f64),
}

impl ColorScheme {
    pub fn light() -> Self {
        Self {
            text:          (0.10, 0.10, 0.10),
            background:    (0.98, 0.98, 0.98),
            heading:       (0.10, 0.10, 0.10),
            link:          (0.10, 0.40, 0.80),
            code_bg:       (0.94, 0.94, 0.96),
            code_fg:       (0.20, 0.20, 0.20),
            code_block_bg: (0.93, 0.93, 0.95),
            syntax_marker: (0.70, 0.70, 0.70),
            strikethrough: (0.50, 0.50, 0.50),
            blockquote:    (0.40, 0.40, 0.50),
            list_marker:   (0.30, 0.30, 0.40),
        }
    }

    pub fn dark() -> Self {
        Self {
            text:          (0.92, 0.92, 0.92),
            background:    (0.11, 0.11, 0.12),
            heading:       (0.95, 0.95, 0.95),
            link:          (0.40, 0.70, 1.00),
            code_bg:       (0.17, 0.17, 0.18),
            code_fg:       (0.85, 0.85, 0.85),
            code_block_bg: (0.16, 0.16, 0.17),
            syntax_marker: (0.40, 0.40, 0.40),
            strikethrough: (0.55, 0.55, 0.55),
            blockquote:    (0.50, 0.55, 0.65),
            list_marker:   (0.55, 0.60, 0.70),
        }
    }

    /// Resolve a foreground color token name to an RGB tuple.
    pub fn resolve_fg(&self, token: &str) -> Option<(f64, f64, f64)> {
        match token {
            "heading"      => Some(self.heading),
            "link"         => Some(self.link),
            "code_fg"      => Some(self.code_fg),
            "syntax"       => Some(self.syntax_marker),
            "strikethrough"=> Some(self.strikethrough),
            "blockquote"   => Some(self.blockquote),
            "list_marker"  => Some(self.list_marker),
            _ => None,
        }
    }

    /// Resolve a background color token name to an RGB tuple.
    pub fn resolve_bg(&self, token: &str) -> Option<(f64, f64, f64)> {
        match token {
            "code_bg"      => Some(self.code_bg),
            "code_block_bg"=> Some(self.code_block_bg),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests — pure Rust, no AppKit
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_scheme_tokens_resolve() {
        let s = ColorScheme::light();
        assert!(s.resolve_fg("heading").is_some());
        assert!(s.resolve_fg("link").is_some());
        assert!(s.resolve_fg("code_fg").is_some());
        assert!(s.resolve_fg("syntax").is_some());
        assert!(s.resolve_fg("strikethrough").is_some());
        assert!(s.resolve_fg("blockquote").is_some());
        assert!(s.resolve_fg("list_marker").is_some());
        assert!(s.resolve_fg("unknown").is_none());
    }

    #[test]
    fn dark_scheme_tokens_resolve() {
        let s = ColorScheme::dark();
        assert!(s.resolve_bg("code_bg").is_some());
        assert!(s.resolve_bg("code_block_bg").is_some());
        assert!(s.resolve_bg("unknown").is_none());
    }

    #[test]
    fn schemes_are_copy() {
        let a = ColorScheme::light();
        let b = a; // copy
        assert_eq!(a.text, b.text);
    }

    #[test]
    fn dark_background_differs_from_light() {
        let dark = ColorScheme::dark();
        let light = ColorScheme::light();
        assert_ne!(dark.background, light.background);
        // Dark background must be darker than 0.5 luminance on all channels.
        let (r, g, b) = dark.background;
        assert!(r < 0.5 && g < 0.5 && b < 0.5,
            "dark bg should be dark, got: {:?}", dark.background);
        // Light background must be lighter.
        let (r, g, b) = light.background;
        assert!(r > 0.5 && g > 0.5 && b > 0.5,
            "light bg should be light, got: {:?}", light.background);
    }
}
