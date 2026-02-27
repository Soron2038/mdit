//! Permanent left-margin formatting sidebar.
//!
//! Replaces the old floating `FloatingToolbar`. Always visible at the left edge
//! of the content area, providing one-click access to all formatting actions.

use std::ffi::CStr;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{msg_send, MainThreadOnly};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSColor, NSControl, NSFont, NSView,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

/// Width of the sidebar column.
pub const SIDEBAR_W: f64 = 36.0;

const BTN_H: f64 = 26.0;
const BTN_W: f64 = 32.0;
const BTN_X: f64 = 2.0;   // left margin within sidebar
const TOP_PAD: f64 = 8.0;
const GROUP_GAP: f64 = 8.0;

/// (label, ObjC selector, start_new_group)
///
/// `start_new_group = true` inserts GROUP_GAP vertical space before this button.
const BTN_DEFS: &[(&str, &CStr, bool)] = &[
    ("H1",  c"applyH1:",            false),
    ("H2",  c"applyH2:",            false),
    ("H3",  c"applyH3:",            false),
    ("\u{00B6}", c"applyNormal:",   false),  // ¶ pilcrow = normal/paragraph
    (">",   c"applyBlockquote:",    false),
    ("```", c"applyCodeBlock:",     false),
    // ── inline group ─────────────────────────────────────────────
    ("B",   c"applyBold:",          true),
    ("I",   c"applyItalic:",        false),
    ("`",   c"applyInlineCode:",    false),
    ("~~",  c"applyStrikethrough:", false),
    // ── insert group ─────────────────────────────────────────────
    ("lnk", c"applyLink:",          true),
    ("\u{2014}", c"applyHRule:",    false),  // — em-dash as HR symbol
];

pub struct FormattingSidebar {
    container: Retained<NSView>,
    buttons:   Vec<Retained<NSButton>>,
    border:    Retained<NSView>,
}

impl FormattingSidebar {
    /// Create the sidebar.
    ///
    /// `height` = content area height (between tab bar and path bar).
    /// `target` must implement all formatting selectors listed in `BTN_DEFS`.
    pub fn new(mtm: MainThreadMarker, height: f64, target: &AnyObject) -> Self {
        let container = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(SIDEBAR_W, height)),
        );

        // ── Buttons ──────────────────────────────────────────────────────────
        let mut buttons = Vec::with_capacity(BTN_DEFS.len());
        for (label, sel_name, _) in BTN_DEFS {
            let btn = NSButton::initWithFrame(
                NSButton::alloc(mtm),
                // y=0 placeholder; actual positions set by position_buttons()
                NSRect::new(NSPoint::new(BTN_X, 0.0), NSSize::new(BTN_W, BTN_H)),
            );
            btn.setTitle(&NSString::from_str(label));
            btn.setButtonType(NSButtonType::MomentaryPushIn);
            btn.setBezelStyle(NSBezelStyle::Inline);
            unsafe {
                NSControl::setTarget(&btn, Some(target));
                NSControl::setAction(&btn, Some(Sel::register(sel_name)));
                let font = NSFont::systemFontOfSize_weight(11.0, 0.0);
                btn.setFont(Some(&font));
            }
            container.addSubview(&btn);
            buttons.push(btn);
        }

        // ── 1 pt right border ────────────────────────────────────────────────
        let border = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(
                NSPoint::new(SIDEBAR_W - 1.0, 0.0),
                NSSize::new(1.0, height),
            ),
        );
        border.setWantsLayer(true);
        container.addSubview(&border);

        let s = Self { container, buttons, border };
        s.position_buttons(height);
        s.apply_separator_color();
        s
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Reposition all buttons from the top of `height`.
    fn position_buttons(&self, height: f64) {
        let mut y = height - TOP_PAD - BTN_H;
        for (i, btn) in self.buttons.iter().enumerate() {
            if BTN_DEFS[i].2 {
                y -= GROUP_GAP;
            }
            let mut f = btn.frame();
            f.origin.y = y;
            btn.setFrame(f);
            y -= BTN_H;
        }
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /// Refresh the right-border color from the current system separatorColor.
    ///
    /// Call this once during setup and again whenever the system appearance
    /// changes.
    pub fn apply_separator_color(&self) {
        if let Some(layer) = unsafe { self.border.layer() } {
            let color = unsafe { NSColor::separatorColor() };
            let cg: *mut std::ffi::c_void =
                unsafe { msg_send![&*color, CGColor] };
            let _: () = unsafe { msg_send![&*layer, setBackgroundColor: cg] };
        }
    }

    /// Update the sidebar height on window resize.
    pub fn set_height(&self, height: f64) {
        // Resize container
        let mut f = self.container.frame();
        f.size.height = height;
        self.container.setFrame(f);
        // Resize border
        let mut bf = self.border.frame();
        bf.size.height = height;
        self.border.setFrame(bf);
        // Reposition buttons
        self.position_buttons(height);
    }

    pub fn view(&self) -> &NSView {
        &self.container
    }
}
