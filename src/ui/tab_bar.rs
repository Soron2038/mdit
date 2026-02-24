//! Tab bar for the mdit editor window.
//!
//! Pure-Rust helpers at the top; AppKit view types further below.

use std::path::Path;

// ---------------------------------------------------------------------------
// Pure-Rust helpers (unit-testable, no AppKit)
// ---------------------------------------------------------------------------

/// Label text for a tab button.
/// Prefixes "• " when `is_dirty` is true.
pub fn tab_label(url: Option<&Path>, is_dirty: bool) -> String {
    let name = url
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string());
    if is_dirty { format!("• {}", name) } else { name }
}

/// Full path string for the path bar.
pub fn path_label(url: Option<&Path>) -> String {
    url.map(|p| p.display().to_string())
        .unwrap_or_else(|| "Untitled — not saved".to_string())
}

// ---------------------------------------------------------------------------
// AppKit view — requires main thread
// ---------------------------------------------------------------------------

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSColor, NSControl,
    NSFont, NSView,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

pub const HEIGHT: f64 = 32.0;
const BTN_H:   f64 = 22.0;
const CLOSE_W: f64 = 18.0;
const TITLE_W: f64 = 100.0;
const PLUS_W:  f64 = 28.0;
const PAD:     f64 = 4.0;
/// Width of each left-side tool button (Open / Save).
const TOOL_W:  f64 = 46.0;
/// Extra gap between tool buttons and the first tab.
const TOOL_SEP: f64 = 6.0;

pub struct TabBar {
    container: Retained<NSView>,
}

impl TabBar {
    pub fn new(mtm: MainThreadMarker, width: f64) -> Self {
        let frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width, HEIGHT),
        );
        let container = NSView::initWithFrame(NSView::alloc(mtm), frame);
        Self { container }
    }

    /// Rebuild all tab buttons.
    ///
    /// `tabs` is a slice of `(label, is_active)` pairs.
    /// Button tags encode the tab index; target receives `switchToTab:` /
    /// `closeTab:` / `newDocument:`.
    pub fn rebuild(
        &self,
        mtm: MainThreadMarker,
        tabs: &[(String, bool)],
        target: &AnyObject,
    ) {
        // Remove old buttons
        for sv in unsafe { self.container.subviews() }.iter() {
            sv.removeFromSuperview();
        }

        let y = (HEIGHT - BTN_H) / 2.0;
        let mut x = PAD;

        // ── Tool buttons (Open / Save) ──────────────────────────────────
        let open_btn = make_button(
            mtm,
            "Open",
            NSRect::new(NSPoint::new(x, y), NSSize::new(TOOL_W, BTN_H)),
            unsafe { Sel::register(c"openDocument:") },
            target,
            -3,
        );
        self.container.addSubview(&open_btn);
        x += TOOL_W + PAD;

        let save_btn = make_button(
            mtm,
            "Save",
            NSRect::new(NSPoint::new(x, y), NSSize::new(TOOL_W, BTN_H)),
            unsafe { Sel::register(c"saveDocument:") },
            target,
            -4,
        );
        self.container.addSubview(&save_btn);
        x += TOOL_W + PAD + TOOL_SEP;

        // ── Tab buttons ─────────────────────────────────────────────────
        for (i, (label, _active)) in tabs.iter().enumerate() {
            // Title button
            let title_btn = make_button(
                mtm,
                label,
                NSRect::new(NSPoint::new(x, y), NSSize::new(TITLE_W, BTN_H)),
                unsafe { Sel::register(c"switchToTab:") },
                target,
                i as isize,
            );
            // TODO: active-tab highlight via layer once CGColor binding confirmed
            self.container.addSubview(&title_btn);
            x += TITLE_W;

            // Close button (×)
            let close_btn = make_button(
                mtm,
                "\u{00D7}",
                NSRect::new(NSPoint::new(x, y), NSSize::new(CLOSE_W, BTN_H)),
                unsafe { Sel::register(c"closeTab:") },
                target,
                i as isize,
            );
            self.container.addSubview(&close_btn);
            x += CLOSE_W + PAD;
        }

        // + button (always at the end)
        let plus_btn = make_button(
            mtm,
            "+",
            NSRect::new(NSPoint::new(x, y), NSSize::new(PLUS_W, BTN_H)),
            unsafe { Sel::register(c"newDocument:") },
            target,
            -1isize,
        );
        self.container.addSubview(&plus_btn);
    }

    pub fn view(&self) -> &NSView {
        &self.container
    }

    pub fn set_width(&self, width: f64) {
        let mut f = self.container.frame();
        f.size.width = width;
        self.container.setFrame(f);
    }
}

fn make_button(
    mtm: MainThreadMarker,
    title: &str,
    frame: NSRect,
    action: Sel,
    target: &AnyObject,
    tag: isize,
) -> Retained<NSButton> {
    let btn = NSButton::initWithFrame(NSButton::alloc(mtm), frame);
    btn.setTitle(&NSString::from_str(title));
    btn.setButtonType(NSButtonType::MomentaryPushIn);
    btn.setBezelStyle(NSBezelStyle::Inline);
    unsafe {
        NSControl::setTarget(&btn, Some(target));
        NSControl::setAction(&btn, Some(action));
        NSControl::setTag(&btn, tag);
        let font = NSFont::systemFontOfSize_weight(12.0, 0.0);
        btn.setFont(Some(&font));
    }
    btn
}
