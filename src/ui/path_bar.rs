//! Thin status bar at the bottom of the window showing the current file path
//! and a Viewer/Editor toggle button.

use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, sel, MainThreadOnly};
use objc2_app_kit::{NSBezelStyle, NSButton, NSColor, NSFont, NSImage, NSTextField, NSView};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

use crate::editor::view_mode::ViewMode;
use crate::ui::tab_bar::path_label;

pub const HEIGHT: f64 = 22.0;
const LEFT_PAD: f64 = 8.0;
/// Visible text-field height — kept smaller than HEIGHT so the text sits
/// vertically centred within the bar.
const FIELD_H: f64 = 16.0;
/// Width reserved for the toggle button on the right.
const BUTTON_W: f64 = 28.0;
const BUTTON_PAD: f64 = 4.0;

pub struct PathBar {
    container: Retained<NSView>,
    field: Retained<NSTextField>,
    toggle_button: Retained<NSButton>,
}

impl PathBar {
    pub fn new(mtm: MainThreadMarker, width: f64, target: &AnyObject) -> Self {
        // Transparent container that spans the full bottom strip.
        let container = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, HEIGHT)),
        );

        let field_w = (width - LEFT_PAD - BUTTON_W - BUTTON_PAD - 4.0).max(0.0);

        // Text field, inset from the left and centred vertically.
        let v_off = (HEIGHT - FIELD_H) / 2.0;
        let field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(
                NSPoint::new(LEFT_PAD, v_off),
                NSSize::new(field_w, FIELD_H),
            ),
        );
        field.setEditable(false);
        field.setSelectable(false);
        field.setBordered(false);
        field.setDrawsBackground(false);
        let font = NSFont::systemFontOfSize_weight(11.0, 0.0);
        field.setFont(Some(&font));
        field.setTextColor(Some(&NSColor::secondaryLabelColor()));
        field.setStringValue(&NSString::from_str("Untitled — not saved"));

        // Toggle button on the right — starts as pencil (Viewer mode → "switch to editor").
        let btn_x = width - BUTTON_W - BUTTON_PAD;
        let btn_y = (HEIGHT - FIELD_H) / 2.0;
        let toggle_button = NSButton::initWithFrame(
            NSButton::alloc(mtm),
            NSRect::new(NSPoint::new(btn_x, btn_y), NSSize::new(BUTTON_W, FIELD_H)),
        );
        toggle_button.setBezelStyle(NSBezelStyle::AccessoryBarAction);
        toggle_button.setBordered(false);
        unsafe {
            toggle_button.setTarget(Some(target));
            toggle_button.setAction(Some(sel!(toggleMode:)));
            let _: () = msg_send![&*toggle_button, setToolTip: &*NSString::from_str("Toggle Editor (⌘E)")];
        }
        Self::apply_icon(&toggle_button, ViewMode::Viewer);

        container.addSubview(&field);
        container.addSubview(&toggle_button);
        Self { container, field, toggle_button }
    }

    /// Update the displayed path.
    pub fn update(&self, url: Option<&Path>) {
        let label = path_label(url);
        self.field.setStringValue(&NSString::from_str(&label));
    }

    /// Update the toggle button icon to reflect the current mode.
    pub fn update_mode_icon(&self, mode: ViewMode) {
        Self::apply_icon(&self.toggle_button, mode);
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
        let field_w = (width - LEFT_PAD - BUTTON_W - BUTTON_PAD - 4.0).max(0.0);
        let v_off = (HEIGHT - FIELD_H) / 2.0;
        self.field.setFrame(NSRect::new(
            NSPoint::new(LEFT_PAD, v_off),
            NSSize::new(field_w, FIELD_H),
        ));
        let btn_x = width - BUTTON_W - BUTTON_PAD;
        let btn_y = (HEIGHT - FIELD_H) / 2.0;
        self.toggle_button.setFrame(NSRect::new(
            NSPoint::new(btn_x, btn_y),
            NSSize::new(BUTTON_W, FIELD_H),
        ));
    }

    /// Set the appropriate SF Symbol icon on the button.
    fn apply_icon(button: &NSButton, mode: ViewMode) {
        // In Viewer mode: show pencil (click to enter editor).
        // In Editor mode: show eye (click to return to viewer).
        let icon_name = match mode {
            ViewMode::Viewer => "square.and.pencil",
            ViewMode::Editor => "eye",
        };
        let name = NSString::from_str(icon_name);
        if let Some(image) =
            NSImage::imageWithSystemSymbolName_accessibilityDescription(&name, None)
        {
            button.setImage(Some(&image));
        }
        button.setTitle(&NSString::from_str(""));
    }
}
