//! Thin status bar at the bottom of the window showing the current file path,
//! live word/character count (Editor mode only), and file metadata.

use std::path::Path;

use objc2::rc::Retained;
use objc2::{msg_send, MainThreadOnly};
use objc2_app_kit::{NSColor, NSFont, NSTextField, NSView};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

use crate::ui::tab_bar::path_label;

pub const HEIGHT: f64 = 22.0;
const LEFT_PAD: f64 = 8.0;
/// Visible text-field height — kept smaller than HEIGHT so the text sits
/// vertically centred within the bar.
const FIELD_H: f64 = 16.0;
/// Width reserved for the file info area on the right.
const INFO_W: f64 = 164.0;
const INFO_PAD: f64 = 8.0;
/// Width of the word/char count field (visible in Editor mode only).
const WORD_W: f64 = 160.0;

pub struct PathBar {
    container: Retained<NSView>,
    field: Retained<NSTextField>,
    word_field: Retained<NSTextField>,
    info_field: Retained<NSTextField>,
}

impl PathBar {
    pub fn new(mtm: MainThreadMarker, width: f64) -> Self {
        // Transparent container that spans the full bottom strip.
        let container = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, HEIGHT)),
        );

        let v_off = (HEIGHT - FIELD_H) / 2.0;
        let font = NSFont::systemFontOfSize_weight(11.0, 0.0);

        // Path text field (left side, flexible width).
        let field_w = (width - LEFT_PAD - INFO_W - INFO_PAD).max(0.0);
        let field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(LEFT_PAD, v_off), NSSize::new(field_w, FIELD_H)),
        );
        field.setEditable(false);
        field.setSelectable(false);
        field.setBordered(false);
        field.setDrawsBackground(false);
        field.setFont(Some(&font));
        field.setTextColor(Some(&NSColor::secondaryLabelColor()));
        field.setStringValue(&NSString::from_str("Untitled — not saved"));

        // Word/char count field (middle, hidden by default, Editor mode only).
        let word_x = (width - WORD_W - INFO_W - INFO_PAD).max(0.0);
        let word_field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(word_x, v_off), NSSize::new(WORD_W, FIELD_H)),
        );
        word_field.setEditable(false);
        word_field.setSelectable(false);
        word_field.setBordered(false);
        word_field.setDrawsBackground(false);
        word_field.setFont(Some(&font));
        word_field.setTextColor(Some(&NSColor::tertiaryLabelColor()));
        word_field.setStringValue(&NSString::from_str(""));
        // Center-align the word count text (NSTextAlignmentCenter = 2).
        unsafe { let _: () = msg_send![&*word_field, setAlignment: 2usize]; }
        unsafe { let _: () = msg_send![&*word_field, setHidden: true]; }

        // File info labels (right side): "UTF-8   LF   Markdown"
        let info_x = (width - INFO_W - INFO_PAD).max(0.0);
        let info_field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(info_x, v_off), NSSize::new(INFO_W, FIELD_H)),
        );
        info_field.setEditable(false);
        info_field.setSelectable(false);
        info_field.setBordered(false);
        info_field.setDrawsBackground(false);
        info_field.setFont(Some(&font));
        info_field.setTextColor(Some(&NSColor::tertiaryLabelColor()));
        info_field.setStringValue(&NSString::from_str(""));
        // Right-align the info text (NSTextAlignmentRight = 1).
        unsafe { let _: () = msg_send![&*info_field, setAlignment: 1usize]; }

        container.addSubview(&field);
        container.addSubview(&word_field);
        container.addSubview(&info_field);

        Self { container, field, word_field, info_field }
    }

    /// Update the displayed path and file info.
    pub fn update(&self, url: Option<&Path>) {
        let label = path_label(url);
        self.field.setStringValue(&NSString::from_str(&label));
        let info = file_info_string(url);
        self.info_field.setStringValue(&NSString::from_str(&info));
    }

    /// Update the word/character count display from raw document text.
    pub fn update_wordcount(&self, text: &str) {
        let label = word_count_label(text);
        self.word_field.setStringValue(&NSString::from_str(&label));
    }

    /// Show or hide the word count field and re-layout.
    pub fn set_wordcount_visible(&self, visible: bool, width: f64) {
        unsafe { let _: () = msg_send![&*self.word_field, setHidden: !visible]; }
        self.set_width(width);
    }

    /// Returns the container view to be added to the window's content view.
    pub fn view(&self) -> &NSView {
        &self.container
    }

    /// Call from `windowDidResize:` to keep the bar full-width.
    pub fn set_width(&self, width: f64) {
        self.container.setFrame(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width, HEIGHT),
        ));
        let v_off = (HEIGHT - FIELD_H) / 2.0;
        let word_visible: bool = unsafe { msg_send![&*self.word_field, isHidden] };
        let word_visible = !word_visible;
        let word_w = if word_visible { WORD_W } else { 0.0 };
        let field_w = (width - LEFT_PAD - word_w - INFO_W - INFO_PAD).max(0.0);
        self.field.setFrame(NSRect::new(
            NSPoint::new(LEFT_PAD, v_off),
            NSSize::new(field_w, FIELD_H),
        ));
        if word_visible {
            let word_x = (width - WORD_W - INFO_W - INFO_PAD).max(0.0);
            self.word_field.setFrame(NSRect::new(
                NSPoint::new(word_x, v_off),
                NSSize::new(WORD_W, FIELD_H),
            ));
        }
        let info_x = (width - INFO_W - INFO_PAD).max(0.0);
        self.info_field.setFrame(NSRect::new(
            NSPoint::new(info_x, v_off),
            NSSize::new(INFO_W, FIELD_H),
        ));
    }
}

/// Format a word/char count as `"243 words · 1,204 chars"`.
pub fn word_count_label(text: &str) -> String {
    let words = text.split_whitespace().count();
    let chars = text.chars().count();
    let w_label = if words == 1 { "word" } else { "words" };
    format!("{} {} · {} chars", fmt_thousands(words), w_label, fmt_thousands(chars))
}

/// Insert thousands separators into a number (e.g. 1204 → "1,204").
fn fmt_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Build the file info string from a path (e.g. "UTF-8   LF   Markdown").
fn file_info_string(url: Option<&Path>) -> String {
    let Some(path) = url else { return String::new() };
    let file_type = match path.extension().and_then(|e| e.to_str()) {
        Some("md") | Some("markdown") => "Markdown",
        Some("txt") => "Plain Text",
        Some("rs") => "Rust",
        Some("toml") => "TOML",
        Some("json") => "JSON",
        Some("yaml") | Some("yml") => "YAML",
        Some(ext) => ext,
        None => "Plain Text",
    };
    format!("UTF-8   LF   {}", file_type)
}

#[cfg(test)]
mod tests {
    use super::{fmt_thousands, word_count_label};

    #[test]
    fn test_fmt_thousands() {
        assert_eq!(fmt_thousands(0), "0");
        assert_eq!(fmt_thousands(999), "999");
        assert_eq!(fmt_thousands(1000), "1,000");
        assert_eq!(fmt_thousands(1204), "1,204");
        assert_eq!(fmt_thousands(1_000_000), "1,000,000");
    }

    #[test]
    fn test_word_count_label_empty() {
        assert_eq!(word_count_label(""), "0 words · 0 chars");
    }

    #[test]
    fn test_word_count_label_singular() {
        assert_eq!(word_count_label("hello"), "1 word · 5 chars");
    }

    #[test]
    fn test_word_count_label_plural() {
        assert_eq!(word_count_label("hello world"), "2 words · 11 chars");
    }

    #[test]
    fn test_word_count_label_thousands() {
        let text = "word ".repeat(1500);
        let label = word_count_label(&text);
        assert!(label.starts_with("1,500 words"));
    }

    #[test]
    fn test_word_count_counts_raw_markdown() {
        // Markdown syntax chars are included in the count
        assert_eq!(word_count_label("**bold**"), "1 word · 8 chars");
    }
}
