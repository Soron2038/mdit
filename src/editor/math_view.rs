use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, NSURL};
use objc2_web_kit::{WKWebView, WKWebViewConfiguration};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Creates a `WKWebView` that renders `latex` using KaTeX (loaded from CDN).
///
/// The view starts with a small initial frame; callers should resize it
/// once the page finishes loading (e.g. via `WKNavigationDelegate`).
///
/// # Safety
/// All AppKit / WebKit APIs must be called on the main thread.
///
/// # TODO
/// Actual embedding as `NSTextAttachment` into `MditTextStorage` is the
/// next integration step — see `apply_math_attachments` below.
pub unsafe fn create_math_view(latex: &str, display: bool) -> Retained<WKWebView> {
    let mtm = MainThreadMarker::new().expect("create_math_view must be called on the main thread");
    let html = build_katex_html(latex, display);
    let html_ns = NSString::from_str(&html);

    let config = WKWebViewConfiguration::new(mtm);
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, 40.0));
    let view = WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), frame, &config);

    // Load HTML — baseURL nil is fine for CDN-loaded KaTeX.
    view.loadHTMLString_baseURL(&html_ns, None::<&NSURL>);

    view
}

// ---------------------------------------------------------------------------
// HTML builder
// ---------------------------------------------------------------------------

/// Builds a minimal HTML page that renders `latex` with KaTeX.
///
/// `display = true`  → display mode (centred, large)
/// `display = false` → inline mode
pub fn build_katex_html(latex: &str, display: bool) -> String {
    // Escape for safe embedding in a JS single-quoted string literal.
    let latex_js = latex
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\r', "")
        .replace('\n', " ");

    let display_mode = if display { "true" } else { "false" };

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <link rel="stylesheet"
        href="https://cdn.jsdelivr.net/npm/katex@0.16.9/dist/katex.min.css">
  <script src="https://cdn.jsdelivr.net/npm/katex@0.16.9/dist/katex.min.js">
  </script>
  <style>
    html, body {{ margin: 0; padding: 2px 4px; background: transparent; }}
  </style>
</head>
<body>
  <span id="m"></span>
  <script>
    katex.render('{latex}', document.getElementById('m'), {{
      displayMode: {display},
      throwOnError: false
    }});
  </script>
</body>
</html>"#,
        latex = latex_js,
        display = display_mode,
    )
}

// ---------------------------------------------------------------------------
// Integration hook (called from text_storage::apply_attributes)
// ---------------------------------------------------------------------------

/// Returns `true` if the text in `[start, end)` represents a math span
/// that should be replaced by a `WKWebView` attachment.
///
/// Currently unused — placeholder for the NSTextAttachment integration
/// which will replace each `$$...$$` / `$...$` range with an attachment
/// character backed by a `WKWebView`.
#[allow(dead_code)]
pub fn is_display_math(latex: &str) -> bool {
    latex.starts_with("$$") && latex.ends_with("$$")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_contains_latex() {
        let html = build_katex_html("x^2", false);
        assert!(html.contains("x^2"), "HTML should embed the LaTeX source");
    }

    #[test]
    fn html_contains_katex_cdn() {
        let html = build_katex_html("x^2", false);
        assert!(
            html.contains("katex"),
            "HTML should reference the KaTeX library"
        );
    }

    #[test]
    fn display_mode_true() {
        let html = build_katex_html("E=mc^2", true);
        assert!(html.contains("displayMode: true"));
    }

    #[test]
    fn display_mode_false() {
        let html = build_katex_html("E=mc^2", false);
        assert!(html.contains("displayMode: false"));
    }

    #[test]
    fn backslash_escaped() {
        let html = build_katex_html(r"\frac{1}{2}", false);
        assert!(html.contains("\\\\frac"), "backslash must be JS-escaped");
    }

    #[test]
    fn is_display_math_detection() {
        assert!(is_display_math("$$x^2$$"));
        assert!(!is_display_math("$x^2$"));
    }
}
