//! Thin status bar at the bottom of the window showing the current file path
//! and file metadata (encoding, line endings, type).

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

pub struct PathBar {
    container: Retained<NSView>,
    field: Retained<NSTextField>,
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
        container.addSubview(&info_field);

        Self { container, field, info_field }
    }

    /// Update the displayed path and file info.
    pub fn update(&self, url: Option<&Path>) {
        let label = path_label(url);
        self.field.setStringValue(&NSString::from_str(&label));
        let info = file_info_string(url);
        self.info_field.setStringValue(&NSString::from_str(&info));
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
        let field_w = (width - LEFT_PAD - INFO_W - INFO_PAD).max(0.0);
        self.field.setFrame(NSRect::new(
            NSPoint::new(LEFT_PAD, v_off),
            NSSize::new(field_w, FIELD_H),
        ));
        let info_x = (width - INFO_W - INFO_PAD).max(0.0);
        self.info_field.setFrame(NSRect::new(
            NSPoint::new(info_x, v_off),
            NSSize::new(INFO_W, FIELD_H),
        ));
    }
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
