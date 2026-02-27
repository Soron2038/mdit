use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBezierPath, NSColor, NSFont, NSFontAttributeName,
    NSFontWeightRegular, NSForegroundColorAttributeName, NSImage, NSPasteboard,
    NSPasteboardTypeString, NSRectFill, NSScrollView, NSTextView,
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
    /// When Some, the copy-feedback state: (block_index, time_of_copy).
    /// Drives the green-checkmark overlay for 1.5s after a copy action.
    copy_feedback: RefCell<Option<(usize, std::time::Instant)>>,
}

define_class!(
    #[unsafe(super = NSTextView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MditTextViewIvars]
    pub struct MditTextView;

    unsafe impl NSObjectProtocol for MditTextView {}

    impl MditTextView {
        /// NSTextView calls this BEFORE the layout manager draws glyphs, making
        /// it the correct place to draw code-block background fills (behind text).
        #[unsafe(method(drawViewBackgroundInRect:))]
        fn draw_view_background_in_rect(&self, rect: NSRect) {
            // Default background fill (editor background color).
            let _: () = unsafe { msg_send![super(self), drawViewBackgroundInRect: rect] };
            // Code-block fills go here — drawn after the background clear but
            // BEFORE NSLayoutManager draws glyphs, so text renders on top.
            self.draw_code_block_fills();
        }

        /// After the standard text view draw pass, overlay borders, copy icons,
        /// and heading separator lines on top of the rendered text.
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty_rect: NSRect) {
            // super.drawRect: calls drawViewBackgroundInRect: (our override above)
            // which draws code-block fills before glyphs are rendered.
            let _: () = unsafe { msg_send![super(self), drawRect: dirty_rect] };
            // Border strokes + copy icons drawn after glyphs (correct for overlays).
            self.draw_code_blocks();
            self.draw_heading_separators();
        }

        /// Timer callback — clears the copy-feedback state and triggers a redraw
        /// to revert the green checkmark back to the normal copy icon.
        #[unsafe(method(clearCopyFeedback))]
        fn clear_copy_feedback(&self) {
            *self.ivars().copy_feedback.borrow_mut() = None;
            let _: () = unsafe { msg_send![self, setNeedsDisplay: true] };
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &objc2_app_kit::NSEvent) {
            // Convert window coords → view coords.
            let window_point = unsafe { event.locationInWindow() };
            let view_point: NSPoint = unsafe {
                self.convertPoint_fromView(window_point, None)
            };

            // Find which copy-button (if any) was clicked.
            let click_result = {
                let rects = self.ivars().copy_button_rects.borrow();
                rects.iter().enumerate().find_map(|(idx, (rect, code_text))| {
                    let in_rect = view_point.x >= rect.origin.x
                        && view_point.x <= rect.origin.x + rect.size.width
                        && view_point.y >= rect.origin.y
                        && view_point.y <= rect.origin.y + rect.size.height;
                    if in_rect { Some((idx, code_text.clone())) } else { None }
                })
            };

            if let Some((block_idx, code_text)) = click_result {
                // Copy content to clipboard.
                unsafe {
                    let pb = NSPasteboard::generalPasteboard();
                    pb.clearContents();
                    let ns_str = NSString::from_str(&code_text);
                    pb.setString_forType(&ns_str, NSPasteboardTypeString);
                }
                // Activate feedback: green checkmark for 1.5s.
                *self.ivars().copy_feedback.borrow_mut() =
                    Some((block_idx, std::time::Instant::now()));
                unsafe {
                    let _: () = msg_send![
                        self,
                        performSelector: objc2::sel!(clearCopyFeedback),
                        withObject: std::ptr::null::<objc2::runtime::AnyObject>(),
                        afterDelay: 1.5f64
                    ];
                    let _: () = msg_send![self, setNeedsDisplay: true];
                }
                return;
            }

            // Not a copy-button click — pass to standard text-view handling.
            let _: () = unsafe { msg_send![super(self), mouseDown: event] };
        }
    }
);

impl MditTextView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MditTextViewIvars {
            delegate:          RefCell::new(None),
            copy_button_rects: RefCell::new(Vec::new()),
            copy_feedback:     RefCell::new(None),
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

    /// Shared geometry: maps code block metadata → (block_rect, icon_rect, code_text, language).
    /// Called by both draw_code_block_fills() and draw_code_blocks() to avoid
    /// duplicating the glyph-index lookup logic.
    fn code_block_rects(&self) -> Vec<(NSRect, NSRect, String, String)> {
        let delegate_ref = self.ivars().delegate.borrow();
        let delegate = match delegate_ref.as_ref() {
            Some(d) => d,
            None => return Vec::new(),
        };
        let infos = delegate.code_block_infos();
        if infos.is_empty() {
            return Vec::new();
        }

        let layout_manager = match unsafe { self.layoutManager() } {
            Some(lm) => lm,
            None => return Vec::new(),
        };
        let text_container = match unsafe { self.textContainer() } {
            Some(tc) => tc,
            None => return Vec::new(),
        };

        let tc_origin = self.textContainerOrigin();
        let container_width = text_container.containerSize().width;
        let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();

        let mut result = Vec::new();
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
            // NSNotFound = NSIntegerMax = 0x7FFFFFFFFFFFFFFF, NOT usize::MAX.
            // Use usize::MAX/2 as sentinel to catch both values safely.
            if first_glyph >= usize::MAX / 2 || last_glyph >= usize::MAX / 2 {
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

            let icon_x = block_rect.origin.x + block_rect.size.width - 20.0;
            let icon_y = block_rect.origin.y + 6.0;
            let icon_rect = NSRect::new(
                NSPoint::new(icon_x, icon_y),
                NSSize::new(14.0, 14.0),
            );

            result.push((block_rect, icon_rect, info.text.clone(), info.language.clone()));
        }
        result
    }

    /// Draw rounded-rect background fills for all code blocks.
    /// Called from drawViewBackgroundInRect: — BEFORE glyphs are drawn,
    /// so the fill is correctly behind the text.
    fn draw_code_block_fills(&self) {
        let rects = self.code_block_rects();
        if rects.is_empty() {
            return;
        }
        // Use the scheme's code_block_bg color (e.g. light lavender-gray in light mode)
        // rather than controlBackgroundColor, which is nearly identical to the editor
        // background and therefore invisible.
        let fill_color = {
            let delegate_ref = self.ivars().delegate.borrow();
            match delegate_ref.as_ref() {
                Some(d) => {
                    let (r, g, b) = d.scheme().code_block_bg;
                    NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0)
                }
                None => return,
            }
        };
        for (block_rect, _, _, _) in rects {
            let path = unsafe {
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(block_rect, 6.0, 6.0)
            };
            fill_color.setFill();
            path.fill();
        }
    }

    /// Draw border strokes and copy icons for all code blocks.
    /// Called from drawRect: AFTER glyphs — correct for overlay rendering.
    /// Also populates copy_button_rects for mouseDown: hit-testing.
    fn draw_code_blocks(&self) {
        // Clear previous frame's hit rects.
        self.ivars().copy_button_rects.borrow_mut().clear();

        let rects = self.code_block_rects();

        for (index, (block_rect, icon_rect, code_text, language)) in rects.into_iter().enumerate() {

            // ── Draw border ───────────────────────────────────────────────────
            let border_path = unsafe {
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(block_rect, 6.0, 6.0)
            };
            border_path.setLineWidth(1.0);
            NSColor::separatorColor().setStroke();
            border_path.stroke();

            // ── Draw language tag in fieldset style (gap in top border) ──────────────
            if !language.is_empty() {
                let ns_lang = NSString::from_str(&language);

                // Build NSMutableAttributedString via msg_send! (avoids NSDictionary complexity).
                let mattr: Retained<objc2::runtime::AnyObject> = unsafe {
                    let cls = objc2::runtime::AnyClass::get(c"NSMutableAttributedString")
                        .expect("NSMutableAttributedString class not found");
                    let obj: *mut objc2::runtime::AnyObject = msg_send![cls, alloc];
                    let obj: *mut objc2::runtime::AnyObject =
                        msg_send![obj, initWithString: &*ns_lang];
                    Retained::retain(obj)
                        .expect("initWithString returned nil")
                };

                let tag_len = language.encode_utf16().count();
                let tag_range = objc2_foundation::NSRange { location: 0, length: tag_len };
                let tag_font = unsafe {
                    NSFont::monospacedSystemFontOfSize_weight(10.0, NSFontWeightRegular)
                };
                let tag_color = NSColor::secondaryLabelColor();
                unsafe {
                    let font_obj: &objc2::runtime::AnyObject = &**tag_font;
                    let color_obj: &objc2::runtime::AnyObject = &**tag_color;
                    let _: () = msg_send![&*mattr,
                        addAttribute: NSFontAttributeName,
                        value: font_obj,
                        range: tag_range];
                    let _: () = msg_send![&*mattr,
                        addAttribute: NSForegroundColorAttributeName,
                        value: color_obj,
                        range: tag_range];
                }

                // Measure the rendered text size.
                let tag_size: NSSize = unsafe { msg_send![&*mattr, size] };

                // Gap: starts 14pt from left edge, 4pt padding each side of text.
                let gap_x = block_rect.origin.x + 14.0;
                let gap_w = tag_size.width + 8.0;
                let gap_y = block_rect.origin.y - tag_size.height / 2.0 - 1.0;
                let gap_h = tag_size.height + 2.0;

                // Erase the border line in the gap with the view's background color.
                let bg = unsafe { self.backgroundColor() };
                bg.setFill();
                NSRectFill(NSRect::new(
                    NSPoint::new(gap_x, gap_y),
                    NSSize::new(gap_w, gap_h),
                ));

                // Draw the attributed string inside the gap.
                let text_rect = NSRect::new(
                    NSPoint::new(gap_x + 4.0, gap_y + 1.0),
                    NSSize::new(tag_size.width, tag_size.height),
                );
                let _: () = unsafe { msg_send![&*mattr, drawInRect: text_rect] };
            }

            // ── Draw SF Symbol copy icon (14×14pt) ────────────────────────────
            // Show green checkmark for 1.5s after a copy, then revert to copy icon.
            let show_checkmark = {
                let fb = self.ivars().copy_feedback.borrow();
                matches!(&*fb, Some((i, t)) if *i == index && t.elapsed().as_secs_f64() < 1.5)
            };
            unsafe {
                let icon_name = if show_checkmark { "checkmark" } else { "doc.on.doc" };
                let name = NSString::from_str(icon_name);
                if let Some(icon) = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                    &name, None,
                ) {
                    if show_checkmark {
                        NSColor::systemGreenColor().set();
                    } else {
                        NSColor::secondaryLabelColor().set();
                    }
                    icon.drawInRect(icon_rect);
                }
            }

            // Store rect for hit-testing in mouseDown:.
            self.ivars().copy_button_rects
                .borrow_mut()
                .push((icon_rect, code_text));
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
