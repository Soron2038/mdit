//! Welcome overlay shown in empty documents.
//!
//! Displays the app name, tagline, keyboard shortcuts, and a
//! mode-dependent hint. Automatically adapts to Light/Dark mode
//! via system label colors.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, MainThreadOnly};
use objc2_app_kit::{NSColor, NSFont, NSTextField, NSView};
use objc2_foundation::{
    MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSUInteger,
};

use crate::editor::view_mode::ViewMode;

// ---------------------------------------------------------------------------
// PassthroughView — NSView subclass that ignores all mouse events
// ---------------------------------------------------------------------------

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    pub struct PassthroughView;

    unsafe impl NSObjectProtocol for PassthroughView {}

    impl PassthroughView {
        /// Return nil so clicks pass through to the text view underneath.
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut AnyObject {
            std::ptr::null_mut()
        }
    }
);

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const TITLE_H: f64 = 50.0;
const TAGLINE_H: f64 = 20.0;
const GAP_AFTER_TAGLINE: f64 = 24.0;
const SHORTCUT_LINE_H: f64 = 22.0;
const SHORTCUT_COUNT: usize = 4;
const GAP_AFTER_SHORTCUTS: f64 = 20.0;
const HINT_H: f64 = 16.0;

// ---------------------------------------------------------------------------
// WelcomeOverlay
// ---------------------------------------------------------------------------

pub struct WelcomeOverlay {
    container: Retained<PassthroughView>,
    hint_field: Retained<NSTextField>,
}

impl WelcomeOverlay {
    /// Create the overlay with all labels.  Starts **hidden**.
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Self {
        let container: Retained<PassthroughView> = unsafe {
            let obj = PassthroughView::alloc(mtm).set_ivars(());
            msg_send![super(obj), initWithFrame: frame]
        };

        // ── Title: "mdit" ───────────────────────────────────────────────
        let title = Self::make_label(mtm, "mdit", 42.0, true);
        title.setTextColor(Some(&NSColor::secondaryLabelColor()));
        container.addSubview(&title);

        // ── Tagline ─────────────────────────────────────────────────────
        let tagline = Self::make_label(mtm, "A native Markdown editor for macOS", 13.0, false);
        tagline.setTextColor(Some(&NSColor::tertiaryLabelColor()));
        container.addSubview(&tagline);

        // ── Shortcut list ───────────────────────────────────────────────
        let shortcuts = [
            "\u{2318}E    Toggle Editor / Viewer",
            "\u{2318}F    Find & Replace",
            "\u{2318}T    New Tab",
            "\u{2318}+/\u{2212}  Adjust Font Size",
        ];
        for text in &shortcuts {
            let label = Self::make_mono_label(mtm, text, 12.0);
            label.setTextColor(Some(&NSColor::tertiaryLabelColor()));
            container.addSubview(&label);
        }

        // ── Hint line (mode-dependent) ──────────────────────────────────
        let hint_field = Self::make_label(mtm, "", 11.0, false);
        hint_field.setTextColor(Some(&NSColor::quaternaryLabelColor()));
        container.addSubview(&hint_field);

        container.setHidden(true);

        let overlay = Self { container, hint_field };
        overlay.layout_labels(frame);
        overlay.update_mode(ViewMode::Viewer);
        overlay
    }

    /// Show or hide the overlay.
    pub fn set_visible(&self, visible: bool) {
        self.container.setHidden(!visible);
    }

    /// Update the overlay frame (called from `windowDidResize:`).
    pub fn set_frame(&self, frame: NSRect) {
        self.container.setFrame(frame);
        self.layout_labels(frame);
    }

    /// Update the mode-dependent hint text.
    pub fn update_mode(&self, mode: ViewMode) {
        let text = match mode {
            ViewMode::Editor => "Just start typing to begin",
            ViewMode::Viewer => "Press \u{2318}E to start editing, or \u{2318}O to open a file",
        };
        self.hint_field.setStringValue(&NSString::from_str(text));
    }

    /// Access the underlying NSView for adding to the view hierarchy.
    pub fn view(&self) -> &NSView {
        &self.container
    }

    // ── Private helpers ─────────────────────────────────────────────────

    /// Position all child labels centered in the given frame.
    fn layout_labels(&self, frame: NSRect) {
        let w = frame.size.width;
        let h = frame.size.height;
        let label_w = w.min(400.0);
        let x = (w - label_w) / 2.0;

        // Compute total block height to center vertically.
        let shortcuts_h = SHORTCUT_COUNT as f64 * SHORTCUT_LINE_H;
        let total = TITLE_H + TAGLINE_H + GAP_AFTER_TAGLINE
            + shortcuts_h + GAP_AFTER_SHORTCUTS + HINT_H;
        let mut y = (h + total) / 2.0;

        let subviews = self.container.subviews();
        let count = subviews.count() as usize;

        // Helper to get a subview by index.
        let get = |i: usize| -> Option<Retained<NSView>> {
            if count > i {
                Some(subviews.objectAtIndex(i as NSUInteger))
            } else {
                None
            }
        };

        // Title (index 0)
        y -= TITLE_H;
        if let Some(v) = get(0) {
            v.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(label_w, TITLE_H)));
        }

        // Tagline (index 1)
        y -= TAGLINE_H;
        if let Some(v) = get(1) {
            v.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(label_w, TAGLINE_H)));
        }

        // Gap
        y -= GAP_AFTER_TAGLINE;

        // Shortcuts (indices 2..6)
        for i in 0..SHORTCUT_COUNT {
            y -= SHORTCUT_LINE_H;
            if let Some(v) = get(2 + i) {
                v.setFrame(NSRect::new(
                    NSPoint::new(x, y),
                    NSSize::new(label_w, SHORTCUT_LINE_H),
                ));
            }
        }

        // Gap
        y -= GAP_AFTER_SHORTCUTS;

        // Hint (last subview, index 6)
        y -= HINT_H;
        if let Some(v) = get(6) {
            v.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(label_w, HINT_H)));
        }
    }

    /// Create a non-editable, borderless NSTextField with system font.
    fn make_label(
        mtm: MainThreadMarker,
        text: &str,
        size: f64,
        light_weight: bool,
    ) -> Retained<NSTextField> {
        let field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
        );
        field.setEditable(false);
        field.setSelectable(false);
        field.setBordered(false);
        field.setDrawsBackground(false);
        if light_weight {
            field.setFont(Some(&NSFont::systemFontOfSize_weight(size, -0.4)));
        } else {
            field.setFont(Some(&NSFont::systemFontOfSize(size)));
        }
        field.setStringValue(&NSString::from_str(text));
        unsafe { let _: () = msg_send![&*field, setAlignment: 1_isize]; } // NSTextAlignmentCenter
        field
    }

    /// Create a non-editable, borderless NSTextField with monospace font.
    fn make_mono_label(
        mtm: MainThreadMarker,
        text: &str,
        size: f64,
    ) -> Retained<NSTextField> {
        let field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
        );
        field.setEditable(false);
        field.setSelectable(false);
        field.setBordered(false);
        field.setDrawsBackground(false);
        field.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(size, 0.0)));
        field.setStringValue(&NSString::from_str(text));
        unsafe { let _: () = msg_send![&*field, setAlignment: 1_isize]; } // NSTextAlignmentCenter
        field
    }
}
