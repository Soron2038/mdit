//! Permanent left-margin formatting sidebar with Notion-style icon buttons.
//!
//! Custom `NSView` subclass that draws SF Symbol icons (or styled text for
//! headings) with hover effects: a rounded-rect pill background and accent
//! color tinting.

use std::cell::{Cell, RefCell};
use std::ffi::CStr;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSEvent, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSImage, NSView,
};
use objc2_foundation::{
    MainThreadMarker, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize, NSString,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Width of the sidebar column.
pub const SIDEBAR_W: f64 = 36.0;

const BTN_SIZE: f64 = 28.0;
const BTN_X: f64 = 4.0;
const TOP_PAD: f64 = 8.0;
const GROUP_GAP: f64 = 10.0;
const ICON_SIZE: f64 = 14.0;
const PILL_RADIUS: f64 = 4.0;

// ---------------------------------------------------------------------------
// Button descriptors
// ---------------------------------------------------------------------------

enum ButtonKind {
    SfSymbol(&'static str),
    StyledText {
        text: &'static str,
        font_size: f64,
        /// Raw `CGFloat` font-weight value (0.0 = Regular, 0.23 ≈ Medium, 0.4 ≈ Bold).
        font_weight: f64,
    },
}

struct ButtonDef {
    kind: ButtonKind,
    selector: &'static CStr,
    start_new_group: bool,
    tooltip: &'static str,
    /// Fallback text label if the SF Symbol is not available.
    fallback: &'static str,
}

const BTN_DEFS: &[ButtonDef] = &[
    ButtonDef {
        kind: ButtonKind::StyledText { text: "H1", font_size: 13.0, font_weight: 0.4 },
        selector: c"applyH1:",
        start_new_group: false,
        tooltip: "Heading 1 (\u{2318}1)",
        fallback: "H1",
    },
    ButtonDef {
        kind: ButtonKind::StyledText { text: "H2", font_size: 12.0, font_weight: 0.23 },
        selector: c"applyH2:",
        start_new_group: false,
        tooltip: "Heading 2 (\u{2318}2)",
        fallback: "H2",
    },
    ButtonDef {
        kind: ButtonKind::StyledText { text: "H3", font_size: 11.0, font_weight: 0.0 },
        selector: c"applyH3:",
        start_new_group: false,
        tooltip: "Heading 3 (\u{2318}3)",
        fallback: "H3",
    },
    ButtonDef {
        kind: ButtonKind::SfSymbol("paragraph"),
        selector: c"applyNormal:",
        start_new_group: false,
        tooltip: "Normal text",
        fallback: "\u{00B6}",
    },
    ButtonDef {
        kind: ButtonKind::SfSymbol("text.quote"),
        selector: c"applyBlockquote:",
        start_new_group: false,
        tooltip: "Blockquote",
        fallback: ">",
    },
    ButtonDef {
        kind: ButtonKind::SfSymbol("chevron.left.forwardslash.chevron.right"),
        selector: c"applyCodeBlock:",
        start_new_group: false,
        tooltip: "Code block",
        fallback: "```",
    },
    // ── inline group ─────────────────────────────────────────────────────
    ButtonDef {
        kind: ButtonKind::SfSymbol("bold"),
        selector: c"applyBold:",
        start_new_group: true,
        tooltip: "Bold (\u{2318}B)",
        fallback: "B",
    },
    ButtonDef {
        kind: ButtonKind::SfSymbol("italic"),
        selector: c"applyItalic:",
        start_new_group: false,
        tooltip: "Italic (\u{2318}I)",
        fallback: "I",
    },
    ButtonDef {
        kind: ButtonKind::SfSymbol("curlybraces"),
        selector: c"applyInlineCode:",
        start_new_group: false,
        tooltip: "Inline code (\u{2318}E)",
        fallback: "`",
    },
    ButtonDef {
        kind: ButtonKind::SfSymbol("strikethrough"),
        selector: c"applyStrikethrough:",
        start_new_group: false,
        tooltip: "Strikethrough",
        fallback: "~~",
    },
    // ── highlight / sub / super group ────────────────────────────────────
    ButtonDef {
        kind: ButtonKind::SfSymbol("highlighter"),
        selector: c"applyHighlight:",
        start_new_group: true,
        tooltip: "Highlight (==text==)",
        fallback: "==",
    },
    ButtonDef {
        kind: ButtonKind::SfSymbol("textformat.subscript"),
        selector: c"applySubscript:",
        start_new_group: false,
        tooltip: "Subscript (~text~)",
        fallback: "x\u{2082}",
    },
    ButtonDef {
        kind: ButtonKind::SfSymbol("textformat.superscript"),
        selector: c"applySuperscript:",
        start_new_group: false,
        tooltip: "Superscript (^text^)",
        fallback: "x\u{00B2}",
    },
    // ── insert group ─────────────────────────────────────────────────────
    ButtonDef {
        kind: ButtonKind::SfSymbol("link"),
        selector: c"applyLink:",
        start_new_group: true,
        tooltip: "Insert link (\u{2318}K)",
        fallback: "lnk",
    },
    ButtonDef {
        kind: ButtonKind::SfSymbol("minus"),
        selector: c"applyHRule:",
        start_new_group: false,
        tooltip: "Horizontal rule",
        fallback: "\u{2014}",
    },
];

// ---------------------------------------------------------------------------
// SidebarButtonView — custom NSView subclass
// ---------------------------------------------------------------------------

#[doc(hidden)]
pub struct SidebarButtonViewIvars {
    /// Index of the button currently under the mouse, or `None`.
    hovered_index: Cell<Option<usize>>,
    /// Index of the button currently pressed (mouse-down), or `None`.
    pressed_index: Cell<Option<usize>>,
    /// Precomputed Y origins of each button (bottom-edge, in view coords).
    button_origins: RefCell<Vec<f64>>,
    /// The target object that receives action selectors (the `AppDelegate`).
    /// Raw pointer — the sidebar never outlives the delegate.
    target: Cell<*const AnyObject>,
    /// Cached SF Symbol images, loaded once at init.
    /// Index matches `BTN_DEFS`. `None` for `StyledText` buttons or if the
    /// symbol could not be loaded.
    cached_images: RefCell<Vec<Option<Retained<NSImage>>>>,
    /// The active tracking area, if any.
    tracking_area: RefCell<Option<Retained<AnyObject>>>,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = SidebarButtonViewIvars]
    pub struct SidebarButtonView;

    unsafe impl NSObjectProtocol for SidebarButtonView {}

    impl SidebarButtonView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty_rect: NSRect) {
            let _: () = unsafe { msg_send![super(self), drawRect: dirty_rect] };
            self.draw_buttons();
        }

        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            // Remove the old tracking area if it exists.
            if let Some(old) = self.ivars().tracking_area.borrow().as_ref() {
                let _: () = unsafe { msg_send![self, removeTrackingArea: &**old] };
            }

            // NSTrackingMouseEnteredAndExited | NSTrackingMouseMoved | NSTrackingActiveInActiveApp
            let options: usize = 0x01 | 0x02 | 0x20;
            let bounds = self.bounds();

            let cls = AnyClass::get(c"NSTrackingArea")
                .expect("NSTrackingArea class not found");
            let area: Retained<AnyObject> = unsafe {
                let alloc: *mut AnyObject = msg_send![cls, alloc];
                let obj: *mut AnyObject = msg_send![
                    alloc,
                    initWithRect: bounds,
                    options: options,
                    owner: self,
                    userInfo: std::ptr::null::<AnyObject>()
                ];
                Retained::retain(obj).expect("NSTrackingArea init returned nil")
            };

            let _: () = unsafe { msg_send![self, addTrackingArea: &*area] };
            *self.ivars().tracking_area.borrow_mut() = Some(area);

            let _: () = unsafe { msg_send![super(self), updateTrackingAreas] };
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, event: &NSEvent) {
            self.update_hover(event);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.ivars().hovered_index.set(None);
            let _: () = unsafe { msg_send![self, setToolTip: std::ptr::null::<NSString>()] };
            let _: () = unsafe { msg_send![self, setNeedsDisplay: true] };
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.update_hover(event);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let window_point = event.locationInWindow();
            let view_point: NSPoint = self.convertPoint_fromView(window_point, None);

            if let Some(idx) = self.button_index_at(view_point) {
                // Visual feedback — pressed state.
                self.ivars().pressed_index.set(Some(idx));
                let _: () = unsafe { msg_send![self, setNeedsDisplay: true] };

                // Dispatch the action to the target.
                let target = self.ivars().target.get();
                if !target.is_null() {
                    let sel = Sel::register(BTN_DEFS[idx].selector);
                    let _: *const AnyObject = unsafe {
                        msg_send![target, performSelector: sel, withObject: self]
                    };
                }
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            if self.ivars().pressed_index.get().is_some() {
                self.ivars().pressed_index.set(None);
                let _: () = unsafe { msg_send![self, setNeedsDisplay: true] };
            }
        }
    }
);

// ---------------------------------------------------------------------------
// SidebarButtonView — helper methods
// ---------------------------------------------------------------------------

impl SidebarButtonView {
    fn new(mtm: MainThreadMarker, frame: NSRect, target: &AnyObject) -> Retained<Self> {
        // Pre-load SF Symbol images.
        let mut images: Vec<Option<Retained<NSImage>>> = Vec::with_capacity(BTN_DEFS.len());
        for def in BTN_DEFS {
            match &def.kind {
                ButtonKind::SfSymbol(name) => {
                    let ns_name = NSString::from_str(name);
                    let img = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                        &ns_name, None,
                    );
                    images.push(img);
                }
                ButtonKind::StyledText { .. } => {
                    images.push(None);
                }
            }
        }

        let this = Self::alloc(mtm).set_ivars(SidebarButtonViewIvars {
            hovered_index: Cell::new(None),
            pressed_index: Cell::new(None),
            button_origins: RefCell::new(Vec::new()),
            target: Cell::new(target as *const AnyObject),
            cached_images: RefCell::new(images),
            tracking_area: RefCell::new(None),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.compute_button_origins(frame.size.height);
        view
    }

    /// Recompute button Y origins for the given height.
    fn compute_button_origins(&self, height: f64) {
        let mut origins = Vec::with_capacity(BTN_DEFS.len());
        let mut y = height - TOP_PAD - BTN_SIZE;

        for (i, def) in BTN_DEFS.iter().enumerate() {
            if def.start_new_group && i > 0 {
                y -= GROUP_GAP;
            }
            origins.push(y);
            y -= BTN_SIZE;
        }

        *self.ivars().button_origins.borrow_mut() = origins;
    }

    /// Hit-test: convert a view-local point to a button index.
    fn button_index_at(&self, point: NSPoint) -> Option<usize> {
        let origins = self.ivars().button_origins.borrow();
        for (i, &oy) in origins.iter().enumerate() {
            if point.x >= BTN_X
                && point.x <= BTN_X + BTN_SIZE
                && point.y >= oy
                && point.y <= oy + BTN_SIZE
            {
                return Some(i);
            }
        }
        None
    }

    /// Update hover state from a mouse event.
    fn update_hover(&self, event: &NSEvent) {
        let window_point = event.locationInWindow();
        let view_point: NSPoint = self.convertPoint_fromView(window_point, None);
        let new_idx = self.button_index_at(view_point);
        let old_idx = self.ivars().hovered_index.get();

        if new_idx != old_idx {
            self.ivars().hovered_index.set(new_idx);

            // Update tooltip dynamically.
            match new_idx {
                Some(idx) => {
                    let tip = NSString::from_str(BTN_DEFS[idx].tooltip);
                    let _: () = unsafe { msg_send![self, setToolTip: &*tip] };
                }
                None => {
                    let _: () =
                        unsafe { msg_send![self, setToolTip: std::ptr::null::<NSString>()] };
                }
            }

            let _: () = unsafe { msg_send![self, setNeedsDisplay: true] };
        }
    }

    // ── Drawing ────────────────────────────────────────────────────────────

    fn draw_buttons(&self) {
        let origins = self.ivars().button_origins.borrow();
        let hovered = self.ivars().hovered_index.get();
        let pressed = self.ivars().pressed_index.get();
        let cached = self.ivars().cached_images.borrow();

        for (i, def) in BTN_DEFS.iter().enumerate() {
            let Some(&oy) = origins.get(i) else { continue };

            let btn_rect = NSRect::new(
                NSPoint::new(BTN_X, oy),
                NSSize::new(BTN_SIZE, BTN_SIZE),
            );

            let is_hovered = hovered == Some(i);
            let is_pressed = pressed == Some(i);

            // ── Pill background ────────────────────────────────────────────
            if is_hovered || is_pressed {
                let fill = if is_pressed {
                    NSColor::tertiaryLabelColor()
                } else {
                    NSColor::quaternaryLabelColor()
                };
                fill.setFill();
                let pill = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                    btn_rect, PILL_RADIUS, PILL_RADIUS,
                );
                pill.fill();
            }

            // ── Icon / text color ──────────────────────────────────────────
            let icon_color = if is_hovered || is_pressed {
                NSColor::controlAccentColor()
            } else {
                NSColor::secondaryLabelColor()
            };

            // ── Draw content ───────────────────────────────────────────────
            match &def.kind {
                ButtonKind::SfSymbol(_) => {
                    if let Some(Some(img)) = cached.get(i) {
                        icon_color.set();
                        let ix = btn_rect.origin.x + (BTN_SIZE - ICON_SIZE) / 2.0;
                        let iy = btn_rect.origin.y + (BTN_SIZE - ICON_SIZE) / 2.0;
                        let icon_rect = NSRect::new(
                            NSPoint::new(ix, iy),
                            NSSize::new(ICON_SIZE, ICON_SIZE),
                        );
                        img.drawInRect(icon_rect);
                    } else {
                        // Fallback: draw the text label.
                        self.draw_text_label(def.fallback, 11.0, 0.0, &icon_color, btn_rect);
                    }
                }
                ButtonKind::StyledText { text, font_size, font_weight } => {
                    self.draw_text_label(text, *font_size, *font_weight, &icon_color, btn_rect);
                }
            }
        }
    }

    /// Draw a centred text label inside `btn_rect`.
    fn draw_text_label(
        &self,
        text: &str,
        font_size: f64,
        font_weight: f64,
        color: &NSColor,
        btn_rect: NSRect,
    ) {
        let ns_text = NSString::from_str(text);

        let mattr: Retained<AnyObject> = unsafe {
            let cls = AnyClass::get(c"NSMutableAttributedString")
                .expect("NSMutableAttributedString class not found");
            let alloc: *mut AnyObject = msg_send![cls, alloc];
            let obj: *mut AnyObject = msg_send![alloc, initWithString: &*ns_text];
            Retained::retain(obj).expect("initWithString returned nil")
        };

        let len = text.encode_utf16().count();
        let range = NSRange { location: 0, length: len };
        let font = NSFont::systemFontOfSize_weight(font_size, font_weight);

        unsafe {
            let font_obj: &AnyObject = &**font;
            let color_obj: &AnyObject = &**color;
            let _: () = msg_send![&*mattr,
                addAttribute: NSFontAttributeName,
                value: font_obj,
                range: range];
            let _: () = msg_send![&*mattr,
                addAttribute: NSForegroundColorAttributeName,
                value: color_obj,
                range: range];
        }

        // Measure and centre.
        let text_size: NSSize = unsafe { msg_send![&*mattr, size] };
        let tx = btn_rect.origin.x + (BTN_SIZE - text_size.width) / 2.0;
        let ty = btn_rect.origin.y + (BTN_SIZE - text_size.height) / 2.0;
        let text_rect = NSRect::new(NSPoint::new(tx, ty), text_size);
        let _: () = unsafe { msg_send![&*mattr, drawInRect: text_rect] };
    }
}

// ---------------------------------------------------------------------------
// FormattingSidebar — public wrapper (same API as before)
// ---------------------------------------------------------------------------

pub struct FormattingSidebar {
    container: Retained<NSView>,
    sidebar_view: Retained<SidebarButtonView>,
    border: Retained<NSView>,
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

        // Custom sidebar button view fills the container.
        let sidebar_view = SidebarButtonView::new(
            mtm,
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(SIDEBAR_W, height)),
            target,
        );
        container.addSubview(&sidebar_view);

        // ── 1 pt right border ────────────────────────────────────────────
        let border = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(SIDEBAR_W - 1.0, 0.0), NSSize::new(1.0, height)),
        );
        border.setWantsLayer(true);
        container.addSubview(&border);

        let s = Self { container, sidebar_view, border };
        s.apply_separator_color();
        s
    }

    // ── Public API ─────────────────────────────────────────────────────────

    /// Refresh the right-border color from the current system separatorColor.
    ///
    /// Call this once during setup and again whenever the system appearance
    /// changes.
    pub fn apply_separator_color(&self) {
        if let Some(layer) = self.border.layer() {
            let color = NSColor::separatorColor();
            let cg: *mut std::ffi::c_void = unsafe { msg_send![&*color, CGColor] };
            let raw: *const AnyObject = Retained::as_ptr(&layer).cast();
            let _: () = unsafe { msg_send![raw, setBackgroundColor: cg] };
        }
    }

    /// Update the sidebar height on window resize.
    pub fn set_height(&self, height: f64) {
        // Resize container.
        let mut f = self.container.frame();
        f.size.height = height;
        self.container.setFrame(f);

        // Resize sidebar view.
        let mut sf = self.sidebar_view.frame();
        sf.size.height = height;
        self.sidebar_view.setFrame(sf);
        self.sidebar_view.compute_button_origins(height);

        // Resize border.
        let mut bf = self.border.frame();
        bf.size.height = height;
        self.border.setFrame(bf);

        let _: () = unsafe { msg_send![&*self.sidebar_view, setNeedsDisplay: true] };
    }

    pub fn view(&self) -> &NSView {
        &self.container
    }
}
