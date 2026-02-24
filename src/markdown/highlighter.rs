use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
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
// Entry point
// ---------------------------------------------------------------------------

/// Highlight `code` according to `language` and return coloured spans.
///
/// Falls back to a single unstyled span when the language is unknown.
pub fn highlight(code: &str, language: &str) -> HighlightResult {
    if code.is_empty() {
        return HighlightResult { spans: Vec::new() };
    }

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = ss
        .find_syntax_by_token(language)
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let theme = &ts.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, theme);
    let mut spans = Vec::new();
    let mut offset = 0usize;

    // If we resolved to plain-text and the language was explicitly given,
    // treat as unknown → single-span fallback.
    let is_plain = syntax.name == "Plain Text" && !language.is_empty();
    if is_plain {
        return HighlightResult {
            spans: vec![HighlightSpan {
                range: (0, code.len()),
                color: (200, 200, 200),
            }],
        };
    }

    for line in LinesWithEndings::from(code) {
        if let Ok(ranges) = h.highlight_line(line, &ss) {
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