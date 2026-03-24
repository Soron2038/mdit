//! Custom inline find bar for mdit.
//!
//! Sits at the bottom of the editor window. Compact (30px) shows the find
//! row only; expanded (56px) adds a replace row above the find row.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2_core_graphics::CGColor;
use objc2::{msg_send, MainThreadOnly};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSColor, NSControl, NSFont, NSTextField, NSView,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

// ---------------------------------------------------------------------------
// Height constants
// ---------------------------------------------------------------------------

pub const FIND_H_COMPACT: f64 = 30.0;
pub const FIND_H_EXPANDED: f64 = 56.0;

// Find row layout constants
const FIND_ROW_H: f64 = 22.0;  // field height
const BTN_H: f64 = 24.0;        // button height
const REPLACE_ROW_H: f64 = 26.0;

const LEFT_PAD: f64 = 8.0;
const RIGHT_PAD: f64 = 8.0;

// Fixed widths for find-row controls (right of search_field):
const PREV_W: f64 = 26.0;
const NEXT_W: f64 = 26.0;
const AA_W: f64 = 32.0;
const COUNT_W: f64 = 60.0;
const CLOSE_W: f64 = 28.0;
// Gap constants
const GAP_PREV: f64 = 4.0;
const GAP_NEXT: f64 = 2.0;
const GAP_AA: f64 = 4.0;
const GAP_COUNT: f64 = 4.0;
const GAP_CLOSE: f64 = 6.0;

// search_field width = width - LEFT_PAD - PREV_W - GAP_PREV - NEXT_W - GAP_NEXT
//                           - AA_W - GAP_AA - COUNT_W - GAP_COUNT - GAP_CLOSE - CLOSE_W - RIGHT_PAD
// = width - (LEFT_PAD + PREV_W + GAP_PREV + NEXT_W + GAP_NEXT + AA_W + GAP_AA + COUNT_W + GAP_COUNT + GAP_CLOSE + CLOSE_W + RIGHT_PAD)

fn search_field_w(width: f64) -> f64 {
    (width
        - LEFT_PAD
        - GAP_PREV - PREV_W
        - GAP_NEXT - NEXT_W
        - GAP_AA - AA_W
        - GAP_COUNT - COUNT_W
        - GAP_CLOSE - CLOSE_W
        - RIGHT_PAD)
        .max(0.0)
}

// Replace row fixed widths
const REPLACE_BTN_W: f64 = 68.0;
const REPLACE_ALL_W: f64 = 40.0;
const REPLACE_GAP: f64 = 4.0;

fn replace_field_w(width: f64) -> f64 {
    (width - LEFT_PAD - REPLACE_GAP - REPLACE_BTN_W - REPLACE_GAP - REPLACE_ALL_W - RIGHT_PAD)
        .max(0.0)
}

// ---------------------------------------------------------------------------
// FindBar
// ---------------------------------------------------------------------------

pub struct FindBar {
    container:       Retained<NSView>,
    border:          Retained<NSView>,
    search_field:    Retained<NSTextField>,
    prev_btn:        Retained<NSButton>,
    next_btn:        Retained<NSButton>,
    aa_btn:          Retained<NSButton>,
    count_label:     Retained<NSTextField>,
    close_btn:       Retained<NSButton>,
    replace_field:   Retained<NSTextField>,
    replace_btn:     Retained<NSButton>,
    replace_all_btn: Retained<NSButton>,
    case_sensitive:  Cell<bool>,
}

impl FindBar {
    /// Create the find bar. Initially hidden. Width = window width.
    pub fn new(mtm: MainThreadMarker, width: f64, target: &AnyObject) -> Self {
        // ── Container ────────────────────────────────────────────────────────
        let container = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, 0.0)),
        );
        container.setWantsLayer(true);
        // Set bar background color #ece6e1 via CALayer
        set_layer_bg(&container, (0.925, 0.902, 0.882));

        // ── Top border (1px, color #c8b89a) ──────────────────────────────────
        let border = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, -1.0), NSSize::new(width, 1.0)),
        );
        border.setWantsLayer(true);
        set_layer_bg(&border, (0.784, 0.722, 0.604));
        container.addSubview(&border);

        // ── Fonts ─────────────────────────────────────────────────────────────
        let georgia_13 =
            NSFont::fontWithName_size(&NSString::from_str("Georgia"), 13.0)
                .unwrap_or_else(|| NSFont::systemFontOfSize_weight(13.0, 0.0));
        let sys_12 = NSFont::systemFontOfSize_weight(12.0, 0.0);

        // ── Search field ──────────────────────────────────────────────────────
        let sf_w = search_field_w(width);
        let search_field = make_input_field(
            mtm,
            NSRect::new(
                NSPoint::new(LEFT_PAD, 0.0), // y set in set_height
                NSSize::new(sf_w, FIND_ROW_H),
            ),
            "Find\u{2026}",
            &georgia_13,
        );
        // Wire action: Return triggers findNext:
        set_target_action(&search_field, target, objc2::sel!(findNext:));
        container.addSubview(&search_field);

        // ── prev_btn ◂ ────────────────────────────────────────────────────────
        let prev_x = LEFT_PAD + sf_w + GAP_PREV;
        let prev_btn = make_text_btn(
            mtm,
            "\u{25C2}",
            NSRect::new(NSPoint::new(prev_x, 0.0), NSSize::new(PREV_W, BTN_H)),
            objc2::sel!(findPrevious:),
            target,
            &sys_12,
        );
        container.addSubview(&prev_btn);

        // ── next_btn ▸ ────────────────────────────────────────────────────────
        let next_x = prev_x + PREV_W + GAP_NEXT;
        let next_btn = make_text_btn(
            mtm,
            "\u{25B8}",
            NSRect::new(NSPoint::new(next_x, 0.0), NSSize::new(NEXT_W, BTN_H)),
            objc2::sel!(findNext:),
            target,
            &sys_12,
        );
        container.addSubview(&next_btn);

        // ── aa_btn (Aa / case-sensitive toggle) ───────────────────────────────
        let aa_x = next_x + NEXT_W + GAP_AA;
        let aa_btn = make_text_btn(
            mtm,
            "Aa",
            NSRect::new(NSPoint::new(aa_x, 0.0), NSSize::new(AA_W, BTN_H)),
            objc2::sel!(findBarToggleAa:),
            target,
            &sys_12,
        );
        container.addSubview(&aa_btn);

        // ── count_label ───────────────────────────────────────────────────────
        let count_x = aa_x + AA_W + GAP_COUNT;
        let count_label = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(count_x, 0.0), NSSize::new(COUNT_W, FIND_ROW_H)),
        );
        count_label.setEditable(false);
        count_label.setSelectable(false);
        count_label.setBordered(false);
        count_label.setDrawsBackground(false);
        count_label.setFont(Some(&sys_12));
        // Orange text color
        let orange = NSColor::colorWithRed_green_blue_alpha(0.784, 0.475, 0.255, 1.0);
        count_label.setTextColor(Some(&orange));
        // Right-align: NSTextAlignmentRight = 1
        unsafe { let _: () = msg_send![&*count_label, setAlignment: 1usize]; }
        count_label.setStringValue(&NSString::from_str(""));
        container.addSubview(&count_label);

        // ── close_btn ✕ ───────────────────────────────────────────────────────
        let close_x = width - CLOSE_W - GAP_CLOSE;
        let close_btn = make_text_btn(
            mtm,
            "\u{00D7}",
            NSRect::new(NSPoint::new(close_x, 0.0), NSSize::new(CLOSE_W, BTN_H)),
            objc2::sel!(closeFindBar:),
            target,
            &sys_12,
        );
        container.addSubview(&close_btn);

        // ── Replace field ─────────────────────────────────────────────────────
        let rf_w = replace_field_w(width);
        let replace_field = make_input_field(
            mtm,
            NSRect::new(
                NSPoint::new(LEFT_PAD, 0.0), // y set in set_height / show_replace_row
                NSSize::new(rf_w, FIND_ROW_H),
            ),
            "Replace\u{2026}",
            &georgia_13,
        );
        replace_field.setHidden(true);
        container.addSubview(&replace_field);

        // ── replace_btn ───────────────────────────────────────────────────────
        let rb_x = LEFT_PAD + rf_w + REPLACE_GAP;
        let replace_btn = make_text_btn(
            mtm,
            "Replace",
            NSRect::new(NSPoint::new(rb_x, 0.0), NSSize::new(REPLACE_BTN_W, BTN_H)),
            objc2::sel!(replaceOne:),
            target,
            &sys_12,
        );
        replace_btn.setHidden(true);
        container.addSubview(&replace_btn);

        // ── replace_all_btn ───────────────────────────────────────────────────
        let ra_x = rb_x + REPLACE_BTN_W + REPLACE_GAP;
        let replace_all_btn = make_text_btn(
            mtm,
            "All",
            NSRect::new(NSPoint::new(ra_x, 0.0), NSSize::new(REPLACE_ALL_W, BTN_H)),
            objc2::sel!(replaceAll:),
            target,
            &sys_12,
        );
        replace_all_btn.setHidden(true);
        container.addSubview(&replace_all_btn);

        // Start hidden
        container.setHidden(true);

        let fb = Self {
            container,
            border,
            search_field,
            prev_btn,
            next_btn,
            aa_btn,
            count_label,
            close_btn,
            replace_field,
            replace_btn,
            replace_all_btn,
            case_sensitive: Cell::new(false),
        };

        // Position controls for initial compact height
        fb.set_height(FIND_H_COMPACT);
        fb
    }

    /// Returns the container view to add to the window's content view.
    pub fn view(&self) -> &NSView {
        &self.container
    }

    /// Update width (called from windowDidResize).
    pub fn set_width(&self, w: f64) {
        let mut f = self.container.frame();
        f.size.width = w;
        self.container.setFrame(f);

        // Resize border
        let mut bf = self.border.frame();
        bf.size.width = w;
        self.border.setFrame(bf);

        // Resize search_field
        let sf_w = search_field_w(w);
        let mut sff = self.search_field.frame();
        sff.size.width = sf_w;
        self.search_field.setFrame(sff);

        // Reposition prev_btn
        let prev_x = LEFT_PAD + sf_w + GAP_PREV;
        let mut pf = self.prev_btn.frame();
        pf.origin.x = prev_x;
        self.prev_btn.setFrame(pf);

        // Reposition next_btn
        let next_x = prev_x + PREV_W + GAP_NEXT;
        let mut nf = self.next_btn.frame();
        nf.origin.x = next_x;
        self.next_btn.setFrame(nf);

        // Reposition aa_btn
        let aa_x = next_x + NEXT_W + GAP_AA;
        let mut af = self.aa_btn.frame();
        af.origin.x = aa_x;
        self.aa_btn.setFrame(af);

        // Reposition count_label
        let count_x = aa_x + AA_W + GAP_COUNT;
        let mut cf = self.count_label.frame();
        cf.origin.x = count_x;
        self.count_label.setFrame(cf);

        // Reposition close_btn flush-right
        let close_x = w - CLOSE_W - GAP_CLOSE;
        let mut clf = self.close_btn.frame();
        clf.origin.x = close_x;
        self.close_btn.setFrame(clf);

        // Resize replace_field
        let rf_w = replace_field_w(w);
        let mut rff = self.replace_field.frame();
        rff.size.width = rf_w;
        self.replace_field.setFrame(rff);

        // Reposition replace_btn
        let rb_x = LEFT_PAD + rf_w + REPLACE_GAP;
        let mut rbf = self.replace_btn.frame();
        rbf.origin.x = rb_x;
        self.replace_btn.setFrame(rbf);

        // Reposition replace_all_btn
        let ra_x = rb_x + REPLACE_BTN_W + REPLACE_GAP;
        let mut raf = self.replace_all_btn.frame();
        raf.origin.x = ra_x;
        self.replace_all_btn.setFrame(raf);
    }

    /// Set height and reposition controls (snap, no animation).
    /// Find row controls are pinned to the TOP of the bar.
    /// Replace row controls are at the BOTTOM of the bar (y = 2..28).
    pub fn set_height(&self, h: f64) {
        // Resize container
        let mut f = self.container.frame();
        f.size.height = h;
        self.container.setFrame(f);

        // Border sits at top of container: y = h - 1
        let border_y = h - 1.0;
        self.border.setFrame(NSRect::new(
            NSPoint::new(0.0, border_y),
            NSSize::new(self.border.frame().size.width, 1.0),
        ));

        // Find row controls: vertically centered in the top 30px of the bar
        // Top of find row is at y = h - FIND_H_COMPACT
        let find_row_bottom = h - FIND_H_COMPACT;
        let find_field_y = find_row_bottom + (FIND_H_COMPACT - FIND_ROW_H) / 2.0;
        let find_btn_y   = find_row_bottom + (FIND_H_COMPACT - BTN_H) / 2.0;

        unsafe {
            reposition_y(&*self.search_field as *const _ as *const AnyObject, find_field_y);
            reposition_y(&*self.prev_btn     as *const _ as *const AnyObject, find_btn_y);
            reposition_y(&*self.next_btn     as *const _ as *const AnyObject, find_btn_y);
            reposition_y(&*self.aa_btn       as *const _ as *const AnyObject, find_btn_y);
            reposition_y(&*self.count_label  as *const _ as *const AnyObject, find_field_y);
            reposition_y(&*self.close_btn    as *const _ as *const AnyObject, find_btn_y);
        }

        // Replace row: vertically centered in the bottom 26px (REPLACE_ROW_H)
        let replace_field_y = (REPLACE_ROW_H - FIND_ROW_H) / 2.0;
        let replace_btn_y   = (REPLACE_ROW_H - BTN_H) / 2.0;

        unsafe {
            reposition_y(&*self.replace_field   as *const _ as *const AnyObject, replace_field_y);
            reposition_y(&*self.replace_btn     as *const _ as *const AnyObject, replace_btn_y);
            reposition_y(&*self.replace_all_btn as *const _ as *const AnyObject, replace_btn_y);
        }
    }

    /// Show the find bar.
    pub fn show(&self) {
        self.container.setHidden(false);
    }

    /// Hide the find bar.
    pub fn hide(&self) {
        self.container.setHidden(true);
    }

    /// Update the match count label. `current` is 1-based.
    /// Shows "N / M" when total > 0, "0 results" when total == 0.
    pub fn update_count(&self, current: usize, total: usize) {
        let text = if total == 0 {
            "0 results".to_string()
        } else {
            format!("{} / {}", current, total)
        };
        self.count_label.setStringValue(&NSString::from_str(&text));
    }

    /// When true: set search field text color to red. When false: restore to labelColor.
    pub fn set_no_match(&self, no_match: bool) {
        if no_match {
            let red = NSColor::colorWithRed_green_blue_alpha(0.75, 0.31, 0.24, 1.0);
            self.search_field.setTextColor(Some(&red));
        } else {
            self.search_field.setTextColor(Some(&NSColor::labelColor()));
        }
    }

    /// Show or hide the replace row (replace_field, replace_btn, replace_all_btn).
    pub fn show_replace_row(&self, visible: bool) {
        self.replace_field.setHidden(!visible);
        self.replace_btn.setHidden(!visible);
        self.replace_all_btn.setHidden(!visible);
    }

    /// Return the current text in the search field.
    pub fn search_text(&self) -> String {
        self.search_field.stringValue().to_string()
    }

    /// Return the current text in the replace field.
    pub fn replace_text(&self) -> String {
        self.replace_field.stringValue().to_string()
    }

    /// Whether the Aa button is in "case-sensitive" state (true = case-sensitive).
    pub fn is_case_sensitive(&self) -> bool {
        self.case_sensitive.get()
    }

    /// Toggle case-sensitive state and update Aa button visual.
    pub fn toggle_case_sensitive(&self) {
        let new_val = !self.case_sensitive.get();
        self.case_sensitive.set(new_val);
        self.update_aa_visual();
    }

    /// Focus the search field and select all text.
    pub fn focus_search(&self) {
        unsafe {
            if let Some(window) = self.search_field.window() {
                let _: () = msg_send![&*window, makeFirstResponder: &*self.search_field];
            }
        }
        unsafe { let _: () = msg_send![&*self.search_field, selectAll: std::ptr::null::<AnyObject>()]; }
    }

    /// Apply colors from the given color scheme.
    /// `bg_dark` is the dark-mode override for the bar background; ignored for now
    /// (the bar uses a fixed warm color regardless of mode, but callers may pass it).
    pub fn apply_colors(&self, _bg_dark: (f64, f64, f64)) {
        // Bar background stays warm regardless of system appearance.
        set_layer_bg(&self.container, (0.925, 0.902, 0.882));
        set_layer_bg(&self.border, (0.784, 0.722, 0.604));
    }

    /// Set the AppDelegate as the search field's delegate (for live search).
    pub fn set_search_delegate(&self, delegate: &AnyObject) {
        unsafe { let _: () = msg_send![&*self.search_field, setDelegate: delegate]; }
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn update_aa_visual(&self) {
        if self.case_sensitive.get() {
            // Active: orange text
            let orange = NSColor::colorWithRed_green_blue_alpha(0.784, 0.475, 0.255, 1.0);
            unsafe { let _: () = msg_send![&*self.aa_btn, setContentTintColor: &*orange]; }
        } else {
            // Inactive: secondary label color
            unsafe {
                let _: () = msg_send![
                    &*self.aa_btn,
                    setContentTintColor: &*NSColor::secondaryLabelColor()
                ];
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Set the CALayer backgroundColor of an NSView.
fn set_layer_bg(view: &NSView, (r, g, b): (f64, f64, f64)) {
    if let Some(layer) = view.layer() {
        let color = NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0);
        let cg: *const CGColor = unsafe { msg_send![&*color, CGColor] };
        let raw: *const AnyObject = Retained::as_ptr(&layer).cast();
        let _: () = unsafe { msg_send![raw, setBackgroundColor: cg] };
    }
}

/// Create an editable text field styled as an input box (input background, Georgia font).
fn make_input_field(
    mtm: MainThreadMarker,
    frame: NSRect,
    placeholder: &str,
    font: &NSFont,
) -> Retained<NSTextField> {
    let field = NSTextField::initWithFrame(NSTextField::alloc(mtm), frame);
    field.setEditable(true);
    field.setSelectable(true);
    field.setDrawsBackground(true);
    // Input background: #fdf9f7
    let bg = NSColor::colorWithRed_green_blue_alpha(0.992, 0.976, 0.969, 1.0);
    field.setBackgroundColor(Some(&bg));
    field.setBordered(true);
    field.setFont(Some(font));
    unsafe {
        let _: () = msg_send![&*field, setPlaceholderString: &*NSString::from_str(placeholder)];
    }
    field
}

/// Create a borderless text button.
fn make_text_btn(
    mtm: MainThreadMarker,
    title: &str,
    frame: NSRect,
    action: Sel,
    target: &AnyObject,
    font: &NSFont,
) -> Retained<NSButton> {
    let btn = NSButton::initWithFrame(NSButton::alloc(mtm), frame);
    btn.setTitle(&NSString::from_str(title));
    btn.setButtonType(NSButtonType::MomentaryPushIn);
    btn.setBezelStyle(NSBezelStyle::AccessoryBarAction);
    btn.setBordered(false);
    btn.setFont(Some(font));
    unsafe {
        NSControl::setTarget(&btn, Some(target));
        NSControl::setAction(&btn, Some(action));
    }
    btn
}

/// Set target and action on an NSTextField (fires on Return).
fn set_target_action(field: &NSTextField, target: &AnyObject, action: Sel) {
    unsafe {
        NSControl::setTarget(field, Some(target));
        NSControl::setAction(field, Some(action));
    }
}

/// Reposition a view's y origin via msg_send!, keeping x, width, and height unchanged.
///
/// `view` must be a reference to any NSView-derived object.
unsafe fn reposition_y(view: *const AnyObject, y: f64) {
    let f: NSRect = msg_send![view, frame];
    let new_f = NSRect::new(NSPoint::new(f.origin.x, y), f.size);
    let _: () = msg_send![view, setFrame: new_f];
}
