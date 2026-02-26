//! Converts pure-Rust `AttributeRun`s into real AppKit text attributes and
//! applies them to an `NSTextStorage`.
//!
//! This is the bridge between the platform-agnostic renderer and AppKit.

use objc2::rc::Retained;
use objc2_app_kit::{
    NSBackgroundColorAttributeName, NSColor, NSFont, NSFontAttributeName,
    NSFontDescriptorSymbolicTraits, NSFontWeightBold, NSFontWeightRegular,
    NSForegroundColorAttributeName, NSMutableParagraphStyle, NSParagraphStyleAttributeName,
    NSStrikethroughStyleAttributeName, NSTextStorage,
};
use objc2_foundation::{NSNumber, NSRange};

use crate::editor::renderer::AttributeRun;
use crate::markdown::attributes::{AttributeSet, TextAttribute};
use crate::ui::appearance::ColorScheme;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Apply `runs` to `storage`, replacing all previous text attributes.
///
/// Returns the UTF-16 character offsets of all H1/H2 heading paragraph starts,
/// used by the text view to draw 1px separator lines above headings.
///
/// Must be called from the main thread (NSTextStorage is not thread-safe).
/// Safe to call from within `textStorage:didProcessEditing:` — the
/// `editing_chars_only` guard in the delegate prevents infinite recursion.
pub fn apply_attribute_runs(
    storage: &NSTextStorage,
    text: &str,
    runs: &[AttributeRun],
    scheme: &ColorScheme,
) -> Vec<usize> {
    let text_len_u16 = text.encode_utf16().count();
    if text_len_u16 == 0 {
        return Vec::new();
    }

    let full_range = NSRange { location: 0, length: text_len_u16 };

    // Build reusable base-style objects.
    let body_font = unsafe { NSFont::systemFontOfSize_weight(16.0, NSFontWeightRegular) };
    let text_color = make_color(scheme.text);
    let para_style = make_para_style(9.6);

    unsafe {
        // ── Reset entire document to body style ───────────────────────────
        storage.addAttribute_value_range(
            NSFontAttributeName,
            body_font.as_ref(),
            full_range,
        );
        storage.addAttribute_value_range(
            NSForegroundColorAttributeName,
            text_color.as_ref(),
            full_range,
        );
        storage.addAttribute_value_range(
            NSParagraphStyleAttributeName,
            para_style.as_ref(),
            full_range,
        );
        // Clear attributes that only apply to specific spans.
        storage.removeAttribute_range(NSBackgroundColorAttributeName, full_range);
        storage.removeAttribute_range(NSStrikethroughStyleAttributeName, full_range);
    }

    // ── Per-run overrides ─────────────────────────────────────────────────
    let mut heading_sep_positions: Vec<usize> = Vec::new();

    for run in runs {
        let start_u16 = byte_to_utf16(text, run.range.0);
        let end_u16 = byte_to_utf16(text, run.range.1);
        if start_u16 >= end_u16 || end_u16 > text_len_u16 {
            continue;
        }
        let range = NSRange { location: start_u16, length: end_u16 - start_u16 };
        apply_attr_set(storage, range, &run.attrs, scheme);

        if run.attrs.contains(&TextAttribute::HeadingSeparator) {
            // Only add the spacing / record the position when non-whitespace
            // content precedes this heading.  Checking at attribute-application
            // time (once per edit) avoids allocating a String on every drawRect:.
            let has_content_before = !text[..run.range.0].trim().is_empty();
            if has_content_before {
                // Add extra space above the heading paragraph so the separator
                // line has visual breathing room.
                let heading_style = make_para_style_with_spacing_before(9.6, 20.0);
                unsafe {
                    storage.addAttribute_value_range(
                        NSParagraphStyleAttributeName,
                        heading_style.as_ref(),
                        range,
                    );
                }
                heading_sep_positions.push(start_u16);
            }
        }
    }

    heading_sep_positions
}

// ---------------------------------------------------------------------------
// Per-range attribute application
// ---------------------------------------------------------------------------

fn apply_attr_set(
    storage: &NSTextStorage,
    range: NSRange,
    attrs: &AttributeSet,
    scheme: &ColorScheme,
) {
    // Build font from the combination of Bold, Italic, Monospace, FontSize.
    let font = build_font(attrs);
    unsafe {
        storage.addAttribute_value_range(NSFontAttributeName, font.as_ref(), range);
    }

    // Apply individual non-font attributes.
    for attr in attrs.attrs() {
        match attr {
            TextAttribute::Bold | TextAttribute::Italic | TextAttribute::Monospace
            | TextAttribute::FontSize(_) => {
                // Handled above by build_font.
            }
            TextAttribute::Hidden => {
                let clear = NSColor::clearColor();
                unsafe {
                    storage.addAttribute_value_range(
                        NSForegroundColorAttributeName,
                        clear.as_ref(),
                        range,
                    );
                }
            }
            TextAttribute::ForegroundColor(token) => {
                if let Some(rgb) = scheme.resolve_fg(token) {
                    let color = make_color(rgb);
                    unsafe {
                        storage.addAttribute_value_range(
                            NSForegroundColorAttributeName,
                            color.as_ref(),
                            range,
                        );
                    }
                }
            }
            TextAttribute::BackgroundColor(token) => {
                if let Some(rgb) = scheme.resolve_bg(token) {
                    let color = make_color(rgb);
                    unsafe {
                        storage.addAttribute_value_range(
                            NSBackgroundColorAttributeName,
                            color.as_ref(),
                            range,
                        );
                    }
                }
            }
            TextAttribute::Strikethrough => {
                // NSStrikethroughStyleAttributeName takes an NSNumber (NSUnderlineStyleSingle = 1).
                let num = NSNumber::numberWithInteger(1);
                unsafe {
                    storage.addAttribute_value_range(
                        NSStrikethroughStyleAttributeName,
                        num.as_ref(),
                        range,
                    );
                }
            }
            // These attributes are conveyed via color tokens above or handled
            // separately in apply_attribute_runs; no direct NSAttributedString
            // key needed here.
            TextAttribute::ListMarker
            | TextAttribute::BlockquoteBar
            | TextAttribute::LineSpacing(_)
            | TextAttribute::HeadingSeparator => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Font helpers
// ---------------------------------------------------------------------------

/// Build the appropriate `NSFont` for an `AttributeSet`.
///
/// Processes Bold + Italic + Monospace + FontSize together so they don't
/// overwrite each other when applied one by one.
fn build_font(attrs: &AttributeSet) -> Retained<NSFont> {
    // Hidden characters (syntax markers) must not take up layout space.
    // Setting the font to near-zero eliminates the visual indentation caused
    // by invisible '# ' / '*' / '**' characters still occupying their advance width.
    if attrs.contains(&TextAttribute::Hidden) {
        return unsafe { NSFont::systemFontOfSize_weight(0.001, NSFontWeightRegular) };
    }

    let size = attrs.font_size(); // returns f64, default 16.0
    let bold = attrs.contains(&TextAttribute::Bold);
    let italic = attrs.contains(&TextAttribute::Italic);
    let mono = attrs.contains(&TextAttribute::Monospace);

    if mono {
        // Code font is always regular-weight monospaced.
        let code_size = if size == 16.0 { 14.0 } else { size };
        return unsafe {
            NSFont::monospacedSystemFontOfSize_weight(code_size, NSFontWeightRegular)
        };
    }

    if bold && italic {
        // Combine bold weight + italic trait via NSFontDescriptor.
        let base = unsafe { NSFont::systemFontOfSize_weight(size, NSFontWeightBold) };
        let desc = base.fontDescriptor();
        let traits = NSFontDescriptorSymbolicTraits::TraitBold
            | NSFontDescriptorSymbolicTraits::TraitItalic;
        let italic_desc = desc.fontDescriptorWithSymbolicTraits(traits);
        return NSFont::fontWithDescriptor_size(&italic_desc, size)
            .unwrap_or(base);
    }

    if italic {
        let base = unsafe { NSFont::systemFontOfSize_weight(size, NSFontWeightRegular) };
        let desc = base.fontDescriptor();
        let italic_desc =
            desc.fontDescriptorWithSymbolicTraits(NSFontDescriptorSymbolicTraits::TraitItalic);
        return NSFont::fontWithDescriptor_size(&italic_desc, size)
            .unwrap_or(base);
    }

    if bold {
        return unsafe { NSFont::systemFontOfSize_weight(size, NSFontWeightBold) };
    }

    unsafe { NSFont::systemFontOfSize_weight(size, NSFontWeightRegular) }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Build an `NSColor` from an sRGB float tuple.
fn make_color((r, g, b): (f64, f64, f64)) -> Retained<NSColor> {
    NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0)
}

/// Build an `NSMutableParagraphStyle` with the given `line_spacing` (in points).
fn make_para_style(line_spacing: f64) -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    style.setLineSpacing(line_spacing);
    style
}

/// Build an `NSMutableParagraphStyle` with both line spacing and extra space
/// above the paragraph (used for H1/H2 headings to host the separator line).
fn make_para_style_with_spacing_before(
    line_spacing: f64,
    spacing_before: f64,
) -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    style.setLineSpacing(line_spacing);
    style.setParagraphSpacingBefore(spacing_before);
    style
}

/// Convert a UTF-8 byte offset in `text` to a UTF-16 code-unit offset.
///
/// Necessary because `NSRange` uses UTF-16 indices but comrak returns
/// UTF-8 byte offsets.
fn byte_to_utf16(text: &str, byte_pos: usize) -> usize {
    let clamped = byte_pos.min(text.len());
    text[..clamped].encode_utf16().count()
}
