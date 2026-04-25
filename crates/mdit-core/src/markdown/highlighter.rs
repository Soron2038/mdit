use std::sync::OnceLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    /// Byte range within the code string.
    pub range: (usize, usize),
    /// Foreground colour (RGB, 0–255).
    pub color: (u8, u8, u8),
}

pub struct HighlightResult {
    pub spans: Vec<HighlightSpan>,
}

// ---------------------------------------------------------------------------
// Lazy-initialised resources (loaded once, reused on every highlight call)
// ---------------------------------------------------------------------------

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();
static WARM_LIGHT_THEME: OnceLock<Theme> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

fn warm_light_theme() -> &'static Theme {
    WARM_LIGHT_THEME.get_or_init(make_warm_light_theme)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Highlight `code` according to `language` and return coloured spans.
///
/// `is_dark` selects the theme: `true` → ocean.dark (cold but readable on
/// dark backgrounds); `false` → the custom warm light theme that matches
/// the app's beige/amber aesthetic.
///
/// Falls back to a single unstyled span when the language is unknown.
pub fn highlight(code: &str, language: &str, is_dark: bool) -> HighlightResult {
    if code.is_empty() {
        return HighlightResult { spans: Vec::new() };
    }

    let ss = syntax_set();
    let ts = theme_set();

    let syntax = ss
        .find_syntax_by_token(language)
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    // If we resolved to plain-text and the language was explicitly given,
    // treat as unknown → single-span fallback.
    let is_plain = syntax.name == "Plain Text" && !language.is_empty();
    if is_plain {
        return HighlightResult {
            spans: vec![HighlightSpan {
                range: (0, code.len()),
                color: (160, 148, 136), // neutral warm gray fallback
            }],
        };
    }

    let theme: &Theme = if is_dark {
        &ts.themes["base16-ocean.dark"]
    } else {
        warm_light_theme()
    };

    let mut h = HighlightLines::new(syntax, theme);
    let mut spans = Vec::new();
    let mut offset = 0usize;

    for line in LinesWithEndings::from(code) {
        if let Ok(ranges) = h.highlight_line(line, ss) {
            for (style, text) in ranges {
                let c = style.foreground;
                spans.push(HighlightSpan {
                    range: (offset, offset + text.len()),
                    color: (c.r, c.g, c.b),
                });
                offset += text.len();
            }
        } else {
            offset += line.len();
        }
    }

    HighlightResult { spans }
}

// ---------------------------------------------------------------------------
// Warm light theme — colours derived from ColorScheme::light()
// ---------------------------------------------------------------------------

/// Build a custom warm light syntax theme that harmonises with the app's
/// beige/amber aesthetic.
///
/// Colour derivations:
/// - Default text  #2C2826  ← text (0.173, 0.157, 0.149)
/// - Keywords      #C87941  ← bold/accent (0.784, 0.475, 0.255)
/// - Strings       #3D7A52  ← warm green (complements the palette)
/// - Comments      #A6998C  ← syntax_marker (0.65, 0.60, 0.55)
/// - Numbers       #1A66CC  ← link (0.10, 0.40, 0.80)
/// - Types/classes #73408C  ← italic/purple (0.45, 0.25, 0.55)
/// - Functions     #5A4A85  ← muted purple-blue
/// - Operators     #7A6A5E  ← slightly muted text
fn make_warm_light_theme() -> Theme {
    let c = |r: u8, g: u8, b: u8| Color { r, g, b, a: 0xFF };

    let item = |scope: &str, r: u8, g: u8, b: u8| ThemeItem {
        scope: scope.parse::<ScopeSelectors>().expect("valid scope selector"),
        style: StyleModifier {
            foreground: Some(c(r, g, b)),
            background: None,
            font_style: None,
        },
    };

    Theme {
        name: Some("Mdit Warm Light".to_string()),
        author: None,
        settings: ThemeSettings {
            foreground: Some(c(0x2C, 0x28, 0x26)),
            background: Some(c(0xFD, 0xF9, 0xF7)),
            ..Default::default()
        },
        scopes: vec![
            // Comments
            item("comment, comment.line, comment.block", 0xA6, 0x99, 0x8C),
            // Keywords & storage (fn, let, mut, pub, struct, enum, impl, return, …)
            item(
                "keyword, storage.type, storage.modifier, \
                 keyword.control, keyword.other, keyword.declaration",
                0xC8, 0x79, 0x41,
            ),
            // Strings
            item(
                "string, string.quoted, string.unquoted, string.template",
                0x3D, 0x7A, 0x52,
            ),
            // Numeric constants
            item("constant.numeric", 0x1A, 0x66, 0xCC),
            // Language constants (true, false, nil, …)
            item("constant.language", 0xC8, 0x79, 0x41),
            // Escape sequences inside strings
            item("constant.character.escape", 0xC8, 0x79, 0x41),
            // Type names & class names
            item(
                "entity.name.type, entity.name.class, \
                 support.type, support.class, storage.type.numeric",
                0x73, 0x40, 0x8C,
            ),
            // Function / method names
            item(
                "entity.name.function, meta.function-call, support.function",
                0x5A, 0x4A, 0x85,
            ),
            // Operators and common punctuation
            item(
                "keyword.operator, punctuation.separator, \
                 punctuation.terminator, punctuation.section",
                0x7A, 0x6A, 0x5E,
            ),
            // Self / this / language variables
            item("variable.language", 0x8C, 0x5A, 0x3A),
            // Invalid / error
            item("invalid, invalid.illegal", 0xCC, 0x33, 0x33),
        ],
    }
}
