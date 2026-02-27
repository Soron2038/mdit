use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBezierPath, NSColor, NSFont, NSFontWeightRegular, NSImage,
    NSPasteboard, NSPasteboardTypeString, NSRectFill, NSScrollView, NSTextView,
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use super::text_storage::MditEditorDelegate;
use crate::ui::appearance::ColorScheme;

// ---------------------------------------------------------------------------
// MditTextView — NSTextView subclass that draws H1/H2 separator lines
// ---------------------------------------------------------------------------

#[doc(hidden)]
pub struct MditTextViewIvars {
    /// Retained reference to the editor delegate so `drawRect:` can read
    /// the current heading separator positions without extra wiring.
    delegate: RefCell<Option<Retained<MditEditorDelegate>>>,
    /// Copy-button rects computed each draw cycle: (icon_rect, code_text).
    /// Populated in draw_code_blocks(), read in mouseDown:.
    copy_button_rects: RefCell<Vec<(NSRect, String)>>,
}

define_class!(
    #[unsafe(super = NSTextView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MditTextViewIvars]
    pub struct MditTextView;

    unsafe impl NSObjectProtocol for MditTextView {}

    impl MditTextView {
        /// After the standard text view draw pass, overlay 1px separator lines
        /// above every H1/H2 heading that has content before it.
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty_rect: NSRect) {
            let _: () = unsafe { msg_send![super(self), drawRect: dirty_rect] };
            self.draw_code_blocks();
            self.draw_heading_separators();
        }
    }
);

impl MditTextView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MditTextViewIvars {
            delegate: RefCell::new(None),
            copy_button_rects: RefCell::new(Vec::new()),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    /// Store a reference to the editor delegate so heading positions are
    /// accessible during `drawRect:`.
    pub fn set_editor_delegate(&self, delegate: Retained<MditEditorDelegate>) {
        *self.ivars().delegate.borrow_mut() = Some(delegate);
    }

    fn draw_heading_separators(&self) {
        let delegate_ref = self.ivars().delegate.borrow();
        let delegate = match delegate_ref.as_ref() {
            Some(d) => d,
            None => return,
        };
        let positions = delegate.heading_sep_positions();
        if positions.is_empty() {
            return;
        }

        let layout_manager = match unsafe { self.layoutManager() } {
            Some(lm) => lm,
            None => return,
        };
        let text_container = match unsafe { self.textContainer() } {
            Some(tc) => tc,
            None => return,
        };

        let tc_origin = self.textContainerOrigin();
        let container_size = text_container.containerSize();
        let x_start = tc_origin.x;
        let x_end = x_start + container_size.width;

        // The content-before check is performed once at attribute-application
        // time (in apply_attribute_runs), so every position in this list needs
        // a separator line — no String allocation required at draw time.
        let sep_color = NSColor::separatorColor();
        sep_color.setFill();

        for &utf16_pos in &positions {
            // Map the heading character index to a glyph index.
            let glyph_idx: usize = unsafe {
                msg_send![&*layout_manager, glyphIndexForCharacterAtIndex: utf16_pos]
            };
            if glyph_idx == usize::MAX {
                continue; // NSNotFound — layout not yet complete
            }

            // Get the bounding rect of the heading's first line fragment.
            let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();
            let frag_rect: NSRect = unsafe {
                msg_send![
                    &*layout_manager,
                    lineFragmentRectForGlyphAtIndex: glyph_idx,
                    effectiveRange: null_ptr
                ]
            };
            if frag_rect.size.height == 0.0 {
                continue; // layout rect not yet available
            }

            // Position the line in the paragraphSpacingBefore space (20pt added
            // in apply.rs), centred at spacing_before / 2 = 10pt above the
            // top of the heading's first line fragment.
            let y = frag_rect.origin.y + tc_origin.y - 10.0;

            // Draw as a filled 0.5pt rect (= 1 physical pixel on Retina).
            let line_rect = NSRect::new(
                NSPoint::new(x_start, y - 0.25),
                NSSize::new(x_end - x_start, 0.5),
            );
            NSRectFill(line_rect);
        }
    }

    fn draw_code_blocks(&self) {
        // Clear previous frame's hit rects.
        self.ivars().copy_button_rects.borrow_mut().clear();

        let delegate_ref = self.ivars().delegate.borrow();
        let delegate = match delegate_ref.as_ref() {
            Some(d) => d,
            None => return,
        };
        let infos = delegate.code_block_infos();
        if infos.is_empty() {
            return;
        }

        let layout_manager = match unsafe { self.layoutManager() } {
            Some(lm) => lm,
            None => return,
        };
        let text_container = match unsafe { self.textContainer() } {
            Some(tc) => tc,
            None => return,
        };

        let tc_origin = self.textContainerOrigin();
        let container_width = text_container.containerSize().width;
        let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();

        for info in &infos {
            if info.start_utf16 >= info.end_utf16 {
                continue;
            }

            // ── Map UTF-16 offsets to glyph indices ──────────────────────────
            let first_glyph: usize = unsafe {
                msg_send![&*layout_manager,
                    glyphIndexForCharacterAtIndex: info.start_utf16]
            };
            let last_char = info.end_utf16.saturating_sub(1);
            let last_glyph: usize = unsafe {
                msg_send![&*layout_manager,
                    glyphIndexForCharacterAtIndex: last_char]
            };
            if first_glyph == usize::MAX || last_glyph == usize::MAX {
                continue;
            }

            // ── Get line fragment rects ───────────────────────────────────────
            let top_frag: NSRect = unsafe {
                msg_send![&*layout_manager,
                    lineFragmentRectForGlyphAtIndex: first_glyph,
                    effectiveRange: null_ptr]
            };
            let bot_frag: NSRect = unsafe {
                msg_send![&*layout_manager,
                    lineFragmentRectForGlyphAtIndex: last_glyph,
                    effectiveRange: null_ptr]
            };
            if top_frag.size.height == 0.0 || bot_frag.size.height == 0.0 {
                continue;
            }

            // ── Build full-width block rect (8pt vertical padding) ────────────
            let block_y = top_frag.origin.y + tc_origin.y - 8.0;
            let block_bottom = bot_frag.origin.y + bot_frag.size.height + tc_origin.y + 8.0;
            let block_rect = NSRect::new(
                NSPoint::new(tc_origin.x, block_y),
                NSSize::new(container_width, block_bottom - block_y),
            );

            // ── Fill background ───────────────────────────────────────────────
            let path = unsafe {
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(block_rect, 6.0, 6.0)
            };
            NSColor::controlBackgroundColor().setFill();
            path.fill();

            // ── Draw border ───────────────────────────────────────────────────
            let border_path = unsafe {
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(block_rect, 6.0, 6.0)
            };
            border_path.setLineWidth(0.5);
            NSColor::separatorColor().setStroke();
            border_path.stroke();

            // ── Draw SF Symbol copy icon (14×14pt, 6pt from bottom-right) ────
            let icon_x = block_rect.origin.x + block_rect.size.width - 20.0;
            let icon_y = block_rect.origin.y + 6.0;
            let icon_rect = NSRect::new(
                NSPoint::new(icon_x, icon_y),
                NSSize::new(14.0, 14.0),
            );
            unsafe {
                let name = NSString::from_str("doc.on.doc");
                if let Some(icon) = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                    &name, None,
                ) {
                    NSColor::secondaryLabelColor().set();
                    icon.drawInRect(icon_rect);
                }
            }

            // Store rect for hit-testing in mouseDown:.
            self.ivars().copy_button_rects
                .borrow_mut()
                .push((icon_rect, info.text.clone()));
        }
    }
}

// ---------------------------------------------------------------------------
// Public factory
// ---------------------------------------------------------------------------

/// Build an NSScrollView containing a `MditTextView`.
///
/// A `MditEditorDelegate` is wired to the text view's storage for re-parse on
/// edit, and also stored inside `MditTextView` so `drawRect:` can read heading
/// positions.  The returned `Retained<MditEditorDelegate>` provides a
/// convenient handle for external callers (e.g. to change the colour scheme);
/// the view holds its own strong reference, so the caller's handle is optional.
///
/// The `text_view` reference is needed to set an `NSTextViewDelegate`.
pub fn create_editor_view(
    mtm: MainThreadMarker,
    frame: NSRect,
) -> (Retained<NSScrollView>, Retained<NSTextView>, Retained<MditEditorDelegate>) {
    // 1. Scroll view
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);

    let content_size = scroll.contentSize();
    let text_rect = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(content_size.width, content_size.height),
    );

    // 2. MditTextView (NSTextView subclass with separator-line drawing)
    let mdit_tv = MditTextView::new(mtm, text_rect);

    // Basic appearance — SF Pro body, semantic background color.
    mdit_tv.setRichText(false);
    let body_font = unsafe {
        NSFont::systemFontOfSize_weight(16.0, NSFontWeightRegular)
    };
    mdit_tv.setFont(Some(&body_font));
    mdit_tv.setTextColor(Some(&NSColor::labelColor()));
    // Use an explicit sRGB colour matching ColorScheme::light().background so that
    // apply_scheme() can override it consistently for any scheme, including dark mode.
    let initial_bg = NSColor::colorWithRed_green_blue_alpha(0.98, 0.98, 0.98, 1.0);
    mdit_tv.setBackgroundColor(&initial_bg);
    mdit_tv.setAutomaticQuoteSubstitutionEnabled(false);
    mdit_tv.setAutomaticDashSubstitutionEnabled(false);
    mdit_tv.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable
            | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    // Initial padding — app.rs will tune this dynamically on window resize.
    mdit_tv.setTextContainerInset(NSSize::new(40.0, 40.0));

    // 3. Wire our delegate to the text view's storage for re-parse on edit.
    //    Default to light scheme; app.rs overrides after appearance detection.
    let delegate = MditEditorDelegate::new(mtm, ColorScheme::light());
    if let Some(storage) = unsafe { mdit_tv.textStorage() } {
        storage.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    }

    // 4. Store the delegate reference in MditTextView so drawRect: can read
    //    heading positions without extra indirection.
    mdit_tv.set_editor_delegate(delegate.clone());

    scroll.setDocumentView(Some(&*mdit_tv));

    // Return as NSTextView — the ObjC runtime still dispatches drawRect: to
    // MditTextView's override.  into_super() is the objc2-sanctioned zero-cost
    // upcast from a DefinedClass to its immediate superclass.
    let text_view: Retained<NSTextView> = Retained::into_super(mdit_tv);

    (scroll, text_view, delegate)
}
