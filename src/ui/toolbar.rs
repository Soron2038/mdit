//! Floating formatting toolbar that appears above selected text.
//!
//! Appears when the user has an active selection; disappears when no text is
//! selected. Button actions (bold, italic, …) are TODO stubs for now.

use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBackingStoreType, NSBezelStyle, NSButton, NSButtonType, NSPanel,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectView,
    NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

const PANEL_W: f64 = 344.0;
const PANEL_H: f64 = 36.0;
const BTN_W: f64 = 42.0;
const BTN_H: f64 = 26.0;
const BTN_GAP: f64 = 2.0;
const BTN_MARGIN: f64 = 4.0;

/// Labels for the formatting buttons (left-to-right).
const BTN_LABELS: &[&str] = &["B", "I", "Code", "~~", "H1", "H2", "H3"];

// ---------------------------------------------------------------------------
// FloatingToolbar
// ---------------------------------------------------------------------------

/// A small floating NSPanel that appears above selected text and offers
/// one-click formatting actions.
pub struct FloatingToolbar {
    panel: Retained<NSPanel>,
}

impl FloatingToolbar {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(PANEL_W, PANEL_H)),
            NSWindowStyleMask::Borderless | NSWindowStyleMask::HUDWindow,
            NSBackingStoreType::Buffered,
            false,
        );
        panel.setFloatingPanel(true);
        panel.setBecomesKeyOnlyIfNeeded(true);
        unsafe { panel.setReleasedWhenClosed(false) };

        if let Some(content) = panel.contentView() {
            // ── Blur background ──────────────────────────────────────────
            let blur = NSVisualEffectView::initWithFrame(
                NSVisualEffectView::alloc(mtm),
                content.bounds(),
            );
            blur.setBlendingMode(NSVisualEffectBlendingMode::WithinWindow);
            blur.setMaterial(NSVisualEffectMaterial::HUDWindow);
            blur.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            content.addSubview(&blur);

            // ── Format buttons ───────────────────────────────────────────
            let y = (PANEL_H - BTN_H) / 2.0;
            for (i, label) in BTN_LABELS.iter().enumerate() {
                let x = BTN_MARGIN + i as f64 * (BTN_W + BTN_GAP);
                let btn = NSButton::initWithFrame(
                    NSButton::alloc(mtm),
                    NSRect::new(NSPoint::new(x, y), NSSize::new(BTN_W, BTN_H)),
                );
                btn.setTitle(&NSString::from_str(label));
                btn.setButtonType(NSButtonType::MomentaryPushIn);
                btn.setBezelStyle(NSBezelStyle::Toolbar);
                // TODO: setTarget + setAction once text view is accessible
                content.addSubview(&btn);
            }
        }

        Self { panel }
    }

    /// Show the toolbar, centered above `rect` (screen coordinates).
    pub fn show_near_rect(&self, rect: NSRect) {
        let x = (rect.origin.x + rect.size.width / 2.0 - PANEL_W / 2.0).max(0.0);
        let y = rect.origin.y + rect.size.height + 6.0;
        self.panel.setFrameOrigin(NSPoint::new(x, y));
        self.panel.orderFront(None);
    }

    /// Hide the toolbar.
    pub fn hide(&self) {
        self.panel.orderOut(None);
    }
}
