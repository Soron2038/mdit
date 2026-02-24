//! Thin status bar at the bottom of the window showing the current file path.

use std::path::Path;

use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSColor, NSFont, NSTextField, NSView};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

use crate::ui::tab_bar::path_label;

const HEIGHT: f64 = 22.0;

pub struct PathBar {
    field: Retained<NSTextField>,
}

impl PathBar {
    pub fn new(mtm: MainThreadMarker, width: f64) -> Self {
        let frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width, HEIGHT),
        );
        let field = NSTextField::initWithFrame(NSTextField::alloc(mtm), frame);
        field.setEditable(false);
        field.setSelectable(false);
        field.setBordered(false);
        field.setDrawsBackground(false);
        unsafe {
            let font = NSFont::systemFontOfSize_weight(11.0, 0.0); // Regular
            field.setFont(Some(&font));
            field.setTextColor(Some(&NSColor::secondaryLabelColor()));
        }
        field.setStringValue(&NSString::from_str("Untitled — not saved"));
        Self { field }
    }

    /// Update the displayed path.
    pub fn update(&self, url: Option<&Path>) {
        let label = path_label(url);
        self.field.setStringValue(&NSString::from_str(&label));
    }

    pub fn view(&self) -> &NSTextField {
        &self.field
    }

    pub const HEIGHT: f64 = HEIGHT;
}
