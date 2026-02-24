//! Thin status bar at the bottom of the window showing the current file path.

use std::path::Path;

use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSColor, NSFont, NSTextField, NSView};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

use crate::ui::tab_bar::path_label;

pub const HEIGHT: f64 = 22.0;
const LEFT_PAD: f64 = 8.0;
/// Visible text-field height — kept smaller than HEIGHT so the text sits
/// vertically centred within the bar.
const FIELD_H: f64 = 16.0;

pub struct PathBar {
    container: Retained<NSView>,
    field:     Retained<NSTextField>,
}

impl PathBar {
    pub fn new(mtm: MainThreadMarker, width: f64) -> Self {
        // Transparent container that spans the full bottom strip.
        let container = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, HEIGHT)),
        );

        // Text field, inset from the left and centred vertically.
        let v_off = (HEIGHT - FIELD_H) / 2.0;
        let field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(
                NSPoint::new(LEFT_PAD, v_off),
                NSSize::new(width - LEFT_PAD - 4.0, FIELD_H),
            ),
        );
        field.setEditable(false);
        field.setSelectable(false);
        field.setBordered(false);
        field.setDrawsBackground(false);
        unsafe {
            let font = NSFont::systemFontOfSize_weight(11.0, 0.0);
            field.setFont(Some(&font));
            field.setTextColor(Some(&NSColor::secondaryLabelColor()));
        }
        field.setStringValue(&NSString::from_str("Untitled — not saved"));

        container.addSubview(&field);
        Self { container, field }
    }

    /// Update the displayed path.
    pub fn update(&self, url: Option<&Path>) {
        let label = path_label(url);
        self.field.setStringValue(&NSString::from_str(&label));
    }

    /// Returns the container view to be added to the window's content view.
    pub fn view(&self) -> &NSView {
        &self.container
    }

    /// Call from `windowDidResize:` to keep the bar flush-left and full-width.
    pub fn set_width(&self, width: f64) {
        self.container.setFrame(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width, HEIGHT),
        ));
        let v_off = (HEIGHT - FIELD_H) / 2.0;
        self.field.setFrame(NSRect::new(
            NSPoint::new(LEFT_PAD, v_off),
            NSSize::new(width - LEFT_PAD - 4.0, FIELD_H),
        ));
    }
}
