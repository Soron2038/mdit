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
    if is_dirty {
        format!("• {}", name)
    } else {
        name
    }
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
use objc2::{msg_send, ClassType, MainThreadOnly};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSColor, NSControl, NSFont, NSImage, NSView,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

pub const HEIGHT: f64 = 32.0;
const BTN_H: f64 = 22.0;
const CLOSE_W: f64 = 18.0;
const TITLE_W: f64 = 110.0;
const PLUS_W: f64 = 28.0;
const PAD: f64 = 4.0;
/// Width of each icon tool button (Open / Save).
const TOOL_W: f64 = 28.0;
/// Extra gap between tool buttons and the first tab.
const TOOL_SEP: f64 = 6.0;
/// Height of the active-tab underline indicator.
const INDICATOR_H: f64 = 2.0;
/// Height of the bottom separator line.
const SEP_H: f64 = 0.5;

pub struct TabBar {
    container: Retained<NSView>,
    indicator: Retained<NSView>,  // colored underline for active tab
    bottom_sep: Retained<NSView>, // 0.5pt separator at bottom
}

impl TabBar {
    pub fn new(mtm: MainThreadMarker, width: f64) -> Self {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, HEIGHT));
        let container = NSView::initWithFrame(NSView::alloc(mtm), frame);

        // ── Active-tab indicator (2pt colored underline) ──────────────────
        let indicator = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(TITLE_W + CLOSE_W, INDICATOR_H),
            ),
        );
        indicator.setWantsLayer(true);
        // Color is applied in apply_colors()
        container.addSubview(&indicator);

        // ── Bottom separator ──────────────────────────────────────────────
        let bottom_sep = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, SEP_H)),
        );
        bottom_sep.setWantsLayer(true);
        container.addSubview(&bottom_sep);

        let tb = Self {
            container,
            indicator,
            bottom_sep,
        };
        tb.apply_colors();
        tb
    }

    /// Apply dynamic colors to indicator and bottom separator.
    ///
    /// Call after creation and whenever the system appearance changes.
    pub fn apply_colors(&self) {
        // Accent color for active-tab indicator
        if let Some(layer) = self.indicator.layer() {
            let accent = NSColor::controlAccentColor();
            let cg: *mut std::ffi::c_void = unsafe { msg_send![&*accent, CGColor] };
            let raw: *const AnyObject = Retained::as_ptr(&layer).cast();
            let _: () = unsafe { msg_send![raw, setBackgroundColor: cg] };
        }
        // Separator color for bottom line
        if let Some(layer) = self.bottom_sep.layer() {
            let sep = NSColor::separatorColor();
            let cg: *mut std::ffi::c_void = unsafe { msg_send![&*sep, CGColor] };
            let raw: *const AnyObject = Retained::as_ptr(&layer).cast();
            let _: () = unsafe { msg_send![raw, setBackgroundColor: cg] };
        }
    }

    /// Rebuild all tab buttons.
    ///
    /// `tabs` is a slice of `(label, is_active)` pairs.
    /// Button tags encode the tab index; target receives `switchToTab:` /
    /// `closeTab:` / `newDocument:`.
    pub fn rebuild(&self, mtm: MainThreadMarker, tabs: &[(String, bool)], target: &AnyObject) {
        // Remove all subviews, then re-add the permanent chrome views.
        for sv in self.container.subviews().iter() {
            sv.removeFromSuperview();
        }
        self.container.addSubview(&self.indicator);
        self.container.addSubview(&self.bottom_sep);

        let y = (HEIGHT - BTN_H) / 2.0;
        let mut x = PAD;

        // ── Open button (folder icon) ─────────────────────────────────────
        let open_btn = make_icon_button(
            mtm,
            "folder",
            "Open",
            NSRect::new(NSPoint::new(x, y), NSSize::new(TOOL_W, BTN_H)),
            Sel::register(c"openDocument:"),
            target,
            -3,
        );
        self.container.addSubview(&open_btn);
        x += TOOL_W + PAD;

        // ── Save button (download icon) ───────────────────────────────────
        let save_btn = make_icon_button(
            mtm,
            "square.and.arrow.down",
            "Save",
            NSRect::new(NSPoint::new(x, y), NSSize::new(TOOL_W, BTN_H)),
            Sel::register(c"saveDocument:"),
            target,
            -4,
        );
        self.container.addSubview(&save_btn);
        x += TOOL_W + PAD + TOOL_SEP;

        // ── Tab buttons ───────────────────────────────────────────────────
        let mut active_x: Option<f64> = None;
        for (i, (label, is_active)) in tabs.iter().enumerate() {
            if *is_active {
                active_x = Some(x);
            }

            // Title button
            let title_btn = make_button(
                mtm,
                label,
                NSRect::new(NSPoint::new(x, y), NSSize::new(TITLE_W, BTN_H)),
                Sel::register(c"switchToTab:"),
                target,
                i as isize,
            );
            self.container.addSubview(&title_btn);
            x += TITLE_W;

            // Close button (×)
            let close_btn = make_button(
                mtm,
                "\u{00D7}",
                NSRect::new(NSPoint::new(x, y), NSSize::new(CLOSE_W, BTN_H)),
                Sel::register(c"closeTab:"),
                target,
                i as isize,
            );
            self.container.addSubview(&close_btn);
            x += CLOSE_W + PAD;
        }

        // ── + button ─────────────────────────────────────────────────────
        let plus_btn = make_button(
            mtm,
            "+",
            NSRect::new(NSPoint::new(x, y), NSSize::new(PLUS_W, BTN_H)),
            Sel::register(c"newDocument:"),
            target,
            -1isize,
        );
        self.container.addSubview(&plus_btn);

        // ── Position indicator under active tab ───────────────────────────
        let indicator_x = active_x.unwrap_or(0.0);
        self.indicator.setFrame(NSRect::new(
            NSPoint::new(indicator_x, 0.0),
            NSSize::new(TITLE_W + CLOSE_W, INDICATOR_H),
        ));
    }

    pub fn view(&self) -> &NSView {
        &self.container
    }

    pub fn set_width(&self, width: f64) {
        let mut f = self.container.frame();
        f.size.width = width;
        self.container.setFrame(f);
        // Stretch bottom separator
        let mut sf = self.bottom_sep.frame();
        sf.size.width = width;
        self.bottom_sep.setFrame(sf);
    }
}

// ---------------------------------------------------------------------------
// Button helpers
// ---------------------------------------------------------------------------

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
    btn.setBezelStyle(NSBezelStyle::AccessoryBarAction);
    unsafe {
        NSControl::setTarget(&btn, Some(target));
        NSControl::setAction(&btn, Some(action));
        NSControl::setTag(&btn, tag);
        let font = NSFont::systemFontOfSize_weight(12.0, 0.0);
        btn.setFont(Some(&font));
    }
    btn
}

fn make_icon_button(
    mtm: MainThreadMarker,
    symbol_name: &str,
    fallback_title: &str,
    frame: NSRect,
    action: Sel,
    target: &AnyObject,
    tag: isize,
) -> Retained<NSButton> {
    let btn = NSButton::initWithFrame(NSButton::alloc(mtm), frame);
    btn.setButtonType(NSButtonType::MomentaryPushIn);
    btn.setBezelStyle(NSBezelStyle::AccessoryBarAction);
    unsafe {
        NSControl::setTarget(&btn, Some(target));
        NSControl::setAction(&btn, Some(action));
        NSControl::setTag(&btn, tag);
    }

    // Try SF Symbol image first
    let name_ns = NSString::from_str(symbol_name);
    let img: Option<Retained<NSImage>> = unsafe {
        msg_send![
            NSImage::class(),
            imageWithSystemSymbolName: &*name_ns,
            accessibilityDescription: std::ptr::null::<NSString>()
        ]
    };

    if let Some(img) = img {
        btn.setImage(Some(&img));
        btn.setTitle(&NSString::from_str(""));
    } else {
        // Fallback to text label
        btn.setTitle(&NSString::from_str(fallback_title));
        let font = NSFont::systemFontOfSize_weight(12.0, 0.0);
        btn.setFont(Some(&font));
    }

    btn
}
