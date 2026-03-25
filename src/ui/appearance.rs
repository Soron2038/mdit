/// Platform-agnostic color scheme.  All color values are sRGB floats in [0, 1].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorScheme {
    pub text: (f64, f64, f64),
    pub background: (f64, f64, f64),
    pub heading: (f64, f64, f64),
    pub bold: (f64, f64, f64),
    pub italic: (f64, f64, f64),
    pub link: (f64, f64, f64),
    pub code_bg: (f64, f64, f64),
    pub code_fg: (f64, f64, f64),
    pub code_block_bg: (f64, f64, f64),
    pub table_bg: (f64, f64, f64),
    pub syntax_marker: (f64, f64, f64),
    pub strikethrough: (f64, f64, f64),
    pub blockquote: (f64, f64, f64),
    pub list_marker: (f64, f64, f64),
    pub highlight_bg: (f64, f64, f64),
    pub subscript: (f64, f64, f64),
    pub superscript: (f64, f64, f64),
    /// UI accent color — used for the tab indicator and sidebar hover state.
    pub accent: (f64, f64, f64),
}

impl ColorScheme {
    pub fn light() -> Self {
        Self {
            text:          (0.173, 0.157, 0.149),
            background:    (0.992, 0.976, 0.969),
            heading:       (0.102, 0.090, 0.078),
            bold:          (0.784, 0.475, 0.255),
            italic:        (0.45,  0.25,  0.55),
            link:          (0.10,  0.40,  0.80),
            code_bg:       (0.953, 0.937, 0.918),
            code_fg:       (0.00,  0.40,  0.40),
            code_block_bg: (0.953, 0.937, 0.918),
            table_bg:      (0.953, 0.937, 0.918),
            syntax_marker: (0.65,  0.60,  0.55),
            strikethrough: (0.55,  0.50,  0.45),
            blockquote:    (0.55,  0.45,  0.35),
            list_marker:   (0.784, 0.475, 0.255),
            highlight_bg:  (1.00,  0.93,  0.70),
            subscript:     (0.55,  0.45,  0.35),
            superscript:   (0.55,  0.45,  0.35),
            accent:        (0.784, 0.475, 0.255),
        }
    }

    pub fn dark() -> Self {
        Self {
            text:          (0.92, 0.92, 0.92),
            background:    (0.11, 0.11, 0.12),
            heading:       (0.55, 0.70, 1.00),
            bold:          (1.00, 0.70, 0.30),
            italic:        (0.80, 0.55, 0.95),
            link:          (0.40, 0.70, 1.00),
            code_bg:       (0.17, 0.17, 0.18),
            code_fg:       (0.40, 0.85, 0.75),
            code_block_bg: (0.16, 0.16, 0.17),
            table_bg:      (0.16, 0.16, 0.17),
            syntax_marker: (0.50, 0.50, 0.55),
            strikethrough: (0.55, 0.55, 0.55),
            blockquote:    (0.50, 0.70, 0.75),
            list_marker:   (0.60, 0.55, 0.80),
            highlight_bg:  (0.55, 0.45, 0.10),
            subscript:     (0.50, 0.70, 0.75),
            superscript:   (0.50, 0.70, 0.75),
            accent:        (1.00, 0.70, 0.30),
        }
    }

    /// Resolve a foreground color token name to an RGB tuple.
    pub fn resolve_fg(&self, token: &str) -> Option<(f64, f64, f64)> {
        match token {
            "heading"      => Some(self.heading),
            "bold"         => Some(self.bold),
            "italic"       => Some(self.italic),
            "link"         => Some(self.link),
            "code_fg"      => Some(self.code_fg),
            "syntax"       => Some(self.syntax_marker),
            "strikethrough"=> Some(self.strikethrough),
            "blockquote"   => Some(self.blockquote),
            "list_marker"  => Some(self.list_marker),
            "subscript"    => Some(self.subscript),
            "superscript"  => Some(self.superscript),
            _ => None,
        }
    }

    /// Resolve a background color token name to an RGB tuple.
    pub fn resolve_bg(&self, token: &str) -> Option<(f64, f64, f64)> {
        match token {
            "code_bg"      => Some(self.code_bg),
            "code_block_bg"=> Some(self.code_block_bg),
            "table_bg"     => Some(self.table_bg),
            "highlight_bg" => Some(self.highlight_bg),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Theme preference — persisted user choice (Light / Dark / System)
// ---------------------------------------------------------------------------

/// The theme the user has explicitly selected.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ThemePreference {
    Light,
    Dark,
    #[default]
    System,
}

impl ThemePreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light  => "light",
            Self::Dark   => "dark",
            Self::System => "system",
        }
    }

    /// Resolve this preference to a concrete `ColorScheme`.
    ///
    /// `system_is_dark` is the result of querying `NSApplication.effectiveAppearance` — pass
    /// `false` when the system is in light mode.
    pub fn resolve(self, system_is_dark: bool) -> ColorScheme {
        match self {
            Self::Light  => ColorScheme::light(),
            Self::Dark   => ColorScheme::dark(),
            Self::System => if system_is_dark { ColorScheme::dark() } else { ColorScheme::light() },
        }
    }
}

impl std::str::FromStr for ThemePreference {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "light" => Self::Light,
            "dark"  => Self::Dark,
            _       => Self::System,
        })
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
        assert!(s.resolve_fg("bold").is_some());
        assert!(s.resolve_fg("italic").is_some());
        assert!(s.resolve_fg("link").is_some());
        assert!(s.resolve_fg("code_fg").is_some());
        assert!(s.resolve_fg("syntax").is_some());
        assert!(s.resolve_fg("strikethrough").is_some());
        assert!(s.resolve_fg("blockquote").is_some());
        assert!(s.resolve_fg("list_marker").is_some());
        assert!(s.resolve_fg("subscript").is_some());
        assert!(s.resolve_fg("superscript").is_some());
        assert!(s.resolve_fg("unknown").is_none());
    }

    #[test]
    fn dark_scheme_tokens_resolve() {
        let s = ColorScheme::dark();
        assert!(s.resolve_bg("code_bg").is_some());
        assert!(s.resolve_bg("code_block_bg").is_some());
        assert!(s.resolve_bg("highlight_bg").is_some());
        assert!(s.resolve_bg("unknown").is_none());
    }

    #[test]
    fn schemes_are_copy() {
        let a = ColorScheme::light();
        let b = a; // copy
        assert_eq!(a.text, b.text);
    }

    #[test]
    fn table_bg_resolves() {
        let light = ColorScheme::light();
        assert!(light.resolve_bg("table_bg").is_some(), "light scheme should resolve table_bg");
        let dark = ColorScheme::dark();
        assert!(dark.resolve_bg("table_bg").is_some(), "dark scheme should resolve table_bg");
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

    // ThemePreference tests
    #[test]
    fn theme_preference_roundtrip() {
        assert_eq!(ThemePreference::Light.as_str().parse::<ThemePreference>().unwrap(), ThemePreference::Light);
        assert_eq!(ThemePreference::Dark.as_str().parse::<ThemePreference>().unwrap(), ThemePreference::Dark);
        assert_eq!(ThemePreference::System.as_str().parse::<ThemePreference>().unwrap(), ThemePreference::System);
    }

    #[test]
    fn theme_preference_unknown_falls_back_to_system() {
        assert_eq!("unknown".parse::<ThemePreference>().unwrap(), ThemePreference::System);
        assert_eq!("".parse::<ThemePreference>().unwrap(), ThemePreference::System);
    }

    #[test]
    fn theme_preference_light_ignores_system_darkness() {
        assert_eq!(ThemePreference::Light.resolve(false).background, ColorScheme::light().background);
        assert_eq!(ThemePreference::Light.resolve(true).background, ColorScheme::light().background);
    }

    #[test]
    fn theme_preference_dark_ignores_system_darkness() {
        assert_eq!(ThemePreference::Dark.resolve(false).background, ColorScheme::dark().background);
        assert_eq!(ThemePreference::Dark.resolve(true).background, ColorScheme::dark().background);
    }

    #[test]
    fn theme_preference_system_follows_os() {
        assert_eq!(ThemePreference::System.resolve(false).background, ColorScheme::light().background);
        assert_eq!(ThemePreference::System.resolve(true).background, ColorScheme::dark().background);
    }
}
