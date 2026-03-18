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
use crate::editor::apply::TableGrid;
use crate::editor::view_mode::ViewMode;
use crate::ui::appearance::ColorScheme;

// ---------------------------------------------------------------------------
// MditTextView — NSTextView subclass that draws H1/H2 separator lines
// ---------------------------------------------------------------------------

/// Groups transient code-block overlay state: hit-test rects for copy buttons
/// and the post-copy feedback (green checkmark) timer info.
struct CodeBlockOverlayState {
    /// Copy-button rects computed each draw cycle: (icon_rect, code_text).
    /// Populated in draw_code_blocks(), read in mouseDown:.
    button_rects: Vec<(NSRect, String)>,
    /// When Some, the copy-feedback state: (block_index, time_of_copy).
    /// Drives the green-checkmark overlay for 1.5s after a copy action.
    feedback: Option<(usize, std::time::Instant)>,
}

#[doc(hidden)]
pub struct MditTextViewIvars {
    /// Retained reference to the editor delegate so `drawRect:` can read
    /// the current heading separator positions without extra wiring.
    delegate: RefCell<Option<Retained<MditEditorDelegate>>>,
    /// Code-block copy-button overlay state (rects + feedback timer).
    overlay: RefCell<CodeBlockOverlayState>,
}

// ---------------------------------------------------------------------------
// Free functions — geometry helpers
// ---------------------------------------------------------------------------

/// Look up the glyph index for a UTF-16 character position.
///
/// Returns `None` when the layout manager has not yet laid out that character
/// (NSNotFound ≈ `usize::MAX / 2`, i.e. NSIntegerMax = 0x7FFFFFFFFFFFFFFF).
fn glyph_for_char(lm: &objc2_app_kit::NSLayoutManager, char_idx: usize) -> Option<usize> {
    let idx: usize =
        unsafe { msg_send![lm, glyphIndexForCharacterAtIndex: char_idx] };
    if idx >= usize::MAX / 2 { None } else { Some(idx) }
}

/// Look up the line fragment rect for a glyph index.
///
/// Returns `None` when the layout rect has zero height (layout not yet
/// complete or glyph not visible).
fn frag_rect_for_glyph(lm: &objc2_app_kit::NSLayoutManager, glyph_idx: usize) -> Option<NSRect> {
    let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();
    let rect: NSRect = unsafe {
        msg_send![lm,
            lineFragmentRectForGlyphAtIndex: glyph_idx,
            effectiveRange: null_ptr]
    };
    if rect.size.height == 0.0 { None } else { Some(rect) }
}

/// Fill a 1-point horizontal rule at (x, y) with the given width.
/// The rect is offset by −0.5 on y to centre on the pixel boundary.
fn fill_hline(x: f64, y: f64, width: f64) {
    NSRectFill(NSRect::new(NSPoint::new(x, y - 0.5), NSSize::new(width, 1.0)));
}

/// Fill a 1-point vertical rule at (x, y) with the given height.
/// The rect is offset by −0.5 on x to centre on the pixel boundary.
fn fill_vline(x: f64, y: f64, height: f64) {
    NSRectFill(NSRect::new(NSPoint::new(x - 0.5, y), NSSize::new(1.0, height)));
}

// ---------------------------------------------------------------------------
// SeparatorAxis — axis selector for draw_table_separators
// ---------------------------------------------------------------------------

/// Axis for [`MditTextView::draw_table_separators`].
#[derive(Clone, Copy)]
enum SeparatorAxis {
    Horizontal,
    Vertical,
}

define_class!(
    #[unsafe(super = NSTextView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MditTextViewIvars]
    /// Custom NSTextView subclass with overlay drawing for Markdown elements.
    pub struct MditTextView;

    unsafe impl NSObjectProtocol for MditTextView {}

    impl MditTextView {
        /// NSTextView calls this BEFORE the layout manager draws glyphs, making
        /// it the correct place to draw code-block background fills (behind text).
        #[unsafe(method(drawViewBackgroundInRect:))]
        fn draw_view_background_in_rect(&self, rect: NSRect) {
            // Default background fill (editor background color).
            let _: () = unsafe { msg_send![super(self), drawViewBackgroundInRect: rect] };
            // Skip custom drawing in Editor mode — only raw text with syntax highlighting.
            if self.is_viewer_mode() {
                self.draw_code_block_fills();
                self.draw_table_fills();
            }
        }

        /// After the standard text view draw pass, overlay borders, copy icons,
        /// and heading separator lines on top of the rendered text.
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty_rect: NSRect) {
            // super.drawRect: calls drawViewBackgroundInRect: (our override above)
            // which draws code-block fills before glyphs are rendered.
            let _: () = unsafe { msg_send![super(self), drawRect: dirty_rect] };
            // Skip custom drawing in Editor mode.
            if self.is_viewer_mode() {
                self.draw_code_blocks();
                self.draw_heading_separators();
                self.draw_thematic_breaks();
                self.draw_table_borders();
                self.draw_table_separators(SeparatorAxis::Horizontal);
                self.draw_table_separators(SeparatorAxis::Vertical);
            }
        }

        /// Timer callback — clears the copy-feedback state and triggers a redraw
        /// to revert the green checkmark back to the normal copy icon.
        #[unsafe(method(clearCopyFeedback))]
        fn clear_copy_feedback(&self) {
            self.ivars().overlay.borrow_mut().feedback = None;
            let _: () = unsafe { msg_send![self, setNeedsDisplay: true] };
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &objc2_app_kit::NSEvent) {
            // Convert window coords → view coords.
            let window_point = event.locationInWindow();
            let view_point: NSPoint = self.convertPoint_fromView(window_point, None);

            // Find which copy-button (if any) was clicked.
            let click_result = {
                let overlay = self.ivars().overlay.borrow();
                overlay.button_rects.iter().enumerate().find_map(|(idx, (rect, code_text))| {
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
                self.ivars().overlay.borrow_mut().feedback =
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
            delegate: RefCell::new(None),
            overlay: RefCell::new(CodeBlockOverlayState {
                button_rects: Vec::new(),
                feedback: None,
            }),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    /// Store a reference to the editor delegate so heading positions are
    /// accessible during `drawRect:`.
    pub fn set_editor_delegate(&self, delegate: Retained<MditEditorDelegate>) {
        *self.ivars().delegate.borrow_mut() = Some(delegate);
    }

    /// Returns `true` if the delegate is in Viewer mode (or if no delegate is set).
    fn is_viewer_mode(&self) -> bool {
        let delegate_ref = self.ivars().delegate.borrow();
        match delegate_ref.as_ref() {
            Some(d) => d.mode() == ViewMode::Viewer,
            None => true,
        }
    }

    /// Acquire the layout manager and text container in a single call.
    ///
    /// Both are required by every drawing method. Returns `None` if either
    /// is unavailable (layout not yet set up).
    fn layout_context(
        &self,
    ) -> Option<(Retained<objc2_app_kit::NSLayoutManager>, Retained<objc2_app_kit::NSTextContainer>)> {
        let lm = unsafe { self.layoutManager() }?;
        let tc = unsafe { self.textContainer() }?;
        Some((lm, tc))
    }

    /// Draw separator lines below H1/H2 headings.
    ///
    /// Only runs in Viewer mode. Draws a 1pt `tertiaryLabelColor` horizontal
    /// rule 10pt above each heading's first line fragment, sitting in the
    /// `paragraphSpacingBefore` gap added by `apply.rs`.
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

        let (layout_manager, text_container) = match self.layout_context() {
            Some(ctx) => ctx,
            None => return,
        };

        let tc_origin = self.textContainerOrigin();
        let container_size = text_container.containerSize();
        let x_start = tc_origin.x;
        let x_end = x_start + container_size.width;

        // The content-before check is performed once at attribute-application
        // time (in apply_attribute_runs), so every position in this list needs
        // a separator line — no String allocation required at draw time.
        let sep_color = NSColor::tertiaryLabelColor();
        sep_color.setFill();

        for &utf16_pos in &positions {
            let Some(glyph_idx) = glyph_for_char(&layout_manager, utf16_pos) else { continue };
            let Some(frag_rect) = frag_rect_for_glyph(&layout_manager, glyph_idx) else { continue };

            // Position the line in the paragraphSpacingBefore space (20pt added
            // in apply.rs), centred at spacing_before / 2 = 10pt above the
            // top of the heading's first line fragment.
            let y = frag_rect.origin.y + tc_origin.y - 10.0;
            fill_hline(x_start, y, x_end - x_start);
        }
    }

    /// Draw full-width horizontal rules for Markdown thematic breaks (`---`).
    ///
    /// Only runs in Viewer mode. Draws a 1pt `tertiaryLabelColor` line
    /// vertically centred within each thematic-break line fragment.
    fn draw_thematic_breaks(&self) {
        let delegate_ref = self.ivars().delegate.borrow();
        let delegate = match delegate_ref.as_ref() {
            Some(d) => d,
            None => return,
        };
        let positions = delegate.thematic_break_positions();
        if positions.is_empty() {
            return;
        }

        let (layout_manager, text_container) = match self.layout_context() {
            Some(ctx) => ctx,
            None => return,
        };

        let tc_origin = self.textContainerOrigin();
        let container_size = text_container.containerSize();
        let x_start = tc_origin.x;
        let x_end = x_start + container_size.width;

        let sep_color = NSColor::tertiaryLabelColor();
        sep_color.setFill();

        for &utf16_pos in &positions {
            let Some(glyph_idx) = glyph_for_char(&layout_manager, utf16_pos) else { continue };
            let Some(frag_rect) = frag_rect_for_glyph(&layout_manager, glyph_idx) else { continue };

            // Centre the line vertically in the line fragment.
            let y = frag_rect.origin.y + tc_origin.y + frag_rect.size.height / 2.0;
            fill_hline(x_start, y, x_end - x_start);
        }
    }

    /// Draw separator lines for all tables on the given axis.
    ///
    /// `Horizontal` draws row boundaries; `Vertical` draws column boundaries.
    /// Both clip to the table's rounded-rect border so lines don't overflow corners.
    fn draw_table_separators(&self, axis: SeparatorAxis) {
        let delegate_ref = self.ivars().delegate.borrow();
        let delegate = match delegate_ref.as_ref() {
            Some(d) => d,
            None => return,
        };
        let grids = delegate.table_grids();
        drop(delegate_ref);
        if grids.is_empty() {
            return;
        }

        let (layout_manager, _text_container) = match self.layout_context() {
            Some(ctx) => ctx,
            None => return,
        };

        let tc_origin = self.textContainerOrigin();
        let rects = self.table_rects_from_grids(&grids);

        // Clip to table border rects so lines don't extend beyond rounded corners.
        if !rects.is_empty() {
            let ctx_cls = objc2::runtime::AnyClass::get(c"NSGraphicsContext").unwrap();
            let _: () = unsafe { msg_send![ctx_cls, saveGraphicsState] };
            let clip_path = NSBezierPath::bezierPath();
            for rect in &rects {
                let rounded = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*rect, 6.0, 6.0);
                clip_path.appendBezierPath(&rounded);
            }
            clip_path.addClip();
        }

        let sep_color = NSColor::tertiaryLabelColor();
        sep_color.setFill();

        for (grid, table_rect) in grids.iter().zip(rects.iter()) {
            let positions = match axis {
                SeparatorAxis::Horizontal => &grid.row_seps,
                SeparatorAxis::Vertical => &grid.column_seps,
            };
            for &utf16_pos in positions {
                let Some(glyph_idx) = glyph_for_char(&layout_manager, utf16_pos) else { continue };
                let Some(frag_rect) = frag_rect_for_glyph(&layout_manager, glyph_idx) else { continue };
                match axis {
                    SeparatorAxis::Horizontal => {
                        let y = frag_rect.origin.y + tc_origin.y;
                        fill_hline(table_rect.origin.x, y, table_rect.size.width);
                    }
                    SeparatorAxis::Vertical => {
                        let glyph_loc: NSPoint = unsafe {
                            msg_send![&*layout_manager, locationForGlyphAtIndex: glyph_idx]
                        };
                        let x = frag_rect.origin.x + glyph_loc.x + tc_origin.x;
                        fill_vline(x, table_rect.origin.y, table_rect.size.height);
                    }
                }
            }
        }

        if !rects.is_empty() {
            let ctx_cls = objc2::runtime::AnyClass::get(c"NSGraphicsContext").unwrap();
            let _: () = unsafe { msg_send![ctx_cls, restoreGraphicsState] };
        }
    }

    /// Compute the full-width bounding rect for each table from its `TableGrid` bounds.
    ///
    /// Each rect spans the full container width and is padded 8pt above the first
    /// line fragment and 8pt below the last. Used for background fills, border strokes,
    /// and clip-path setup in separator drawing.
    fn table_rects_from_grids(
        &self,
        grids: &[TableGrid],
    ) -> Vec<NSRect> {
        if grids.is_empty() {
            return Vec::new();
        }

        let (layout_manager, text_container) = match self.layout_context() {
            Some(ctx) => ctx,
            None => return Vec::new(),
        };

        let tc_origin = self.textContainerOrigin();
        let container_width = text_container.containerSize().width;

        let mut result = Vec::new();
        for grid in grids {
            let (start_u16, end_u16) = grid.bounds;
            if start_u16 >= end_u16 {
                continue;
            }
            let Some(first_glyph) = glyph_for_char(&layout_manager, start_u16) else { continue };
            let last_char = end_u16.saturating_sub(1);
            let Some(last_glyph) = glyph_for_char(&layout_manager, last_char) else { continue };

            let Some(top_frag) = frag_rect_for_glyph(&layout_manager, first_glyph) else { continue };
            let Some(bot_frag) = frag_rect_for_glyph(&layout_manager, last_glyph) else { continue };

            let block_y = top_frag.origin.y + tc_origin.y - 8.0;
            let block_bottom = bot_frag.origin.y + bot_frag.size.height + tc_origin.y + 8.0;
            let block_rect = NSRect::new(
                NSPoint::new(tc_origin.x, block_y),
                NSSize::new(container_width, block_bottom - block_y),
            );
            result.push(block_rect);
        }
        result
    }

    /// Draw rounded-rect background fills for all tables.
    /// Called from drawViewBackgroundInRect: — BEFORE glyphs.
    fn draw_table_fills(&self) {
        let delegate_ref = self.ivars().delegate.borrow();
        let delegate = match delegate_ref.as_ref() {
            Some(d) => d,
            None => return,
        };
        let grids = delegate.table_grids();
        let rects = self.table_rects_from_grids(&grids);
        if rects.is_empty() {
            return;
        }
        let (r, g, b) = delegate.scheme().table_bg;
        drop(delegate_ref);
        let fill_color = NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0);
        for block_rect in &rects {
            let path =
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*block_rect, 6.0, 6.0);
            fill_color.setFill();
            path.fill();
        }
    }

    /// Draw rounded-rect border strokes for all tables.
    /// Called from drawRect: — AFTER glyphs (overlay).
    fn draw_table_borders(&self) {
        let delegate_ref = self.ivars().delegate.borrow();
        let delegate = match delegate_ref.as_ref() {
            Some(d) => d,
            None => return,
        };
        let grids = delegate.table_grids();
        drop(delegate_ref);
        let rects = self.table_rects_from_grids(&grids);
        for block_rect in &rects {
            let border_path =
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(*block_rect, 6.0, 6.0);
            border_path.setLineWidth(1.0);
            NSColor::tertiaryLabelColor().setStroke();
            border_path.stroke();
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

        let (layout_manager, text_container) = match self.layout_context() {
            Some(ctx) => ctx,
            None => return Vec::new(),
        };

        let tc_origin = self.textContainerOrigin();
        let container_width = text_container.containerSize().width;

        let mut result = Vec::new();
        for info in &infos {
            if info.start_utf16 >= info.end_utf16 {
                continue;
            }

            // ── Map UTF-16 offsets to glyph indices ──────────────────────────
            // NSNotFound = NSIntegerMax = 0x7FFFFFFFFFFFFFFF, NOT usize::MAX.
            // Use usize::MAX/2 as sentinel to catch both values safely.
            let Some(first_glyph) = glyph_for_char(&layout_manager, info.start_utf16) else { continue };
            let last_char = info.end_utf16.saturating_sub(1);
            let Some(last_glyph) = glyph_for_char(&layout_manager, last_char) else { continue };

            // ── Get line fragment rects ───────────────────────────────────────
            let Some(top_frag) = frag_rect_for_glyph(&layout_manager, first_glyph) else { continue };
            let Some(bot_frag) = frag_rect_for_glyph(&layout_manager, last_glyph) else { continue };

            // ── Build full-width block rect (7pt vertical padding) ────────────
            let block_y = top_frag.origin.y + tc_origin.y - 7.0;
            let block_bottom = bot_frag.origin.y + bot_frag.size.height + tc_origin.y + 7.0;
            let block_rect = NSRect::new(
                NSPoint::new(tc_origin.x, block_y),
                NSSize::new(container_width, block_bottom - block_y),
            );

            let icon_x = block_rect.origin.x + block_rect.size.width - 20.0;
            let icon_y = block_rect.origin.y + 6.0;
            let icon_rect = NSRect::new(NSPoint::new(icon_x, icon_y), NSSize::new(14.0, 14.0));

            result.push((
                block_rect,
                icon_rect,
                info.text.clone(),
                info.language.clone(),
            ));
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
            let path =
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(block_rect, 6.0, 6.0);
            fill_color.setFill();
            path.fill();
        }
    }

    /// Draw border strokes and copy icons for all code blocks.
    /// Called from drawRect: AFTER glyphs — correct for overlay rendering.
    /// Also populates overlay.button_rects for mouseDown: hit-testing.
    fn draw_code_blocks(&self) {
        self.ivars().overlay.borrow_mut().button_rects.clear();

        let rects = self.code_block_rects();
        for (index, (block_rect, icon_rect, code_text, language)) in rects.into_iter().enumerate() {
            self.draw_code_block_border(block_rect);
            self.draw_code_block_language_tag(block_rect, &language);
            self.draw_code_block_copy_icon(index, icon_rect);
            self.ivars().overlay.borrow_mut().button_rects.push((icon_rect, code_text));
        }
    }

    /// Stroke a rounded-rect border for a single code block.
    fn draw_code_block_border(&self, block_rect: NSRect) {
        let border_path =
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(block_rect, 6.0, 6.0);
        border_path.setLineWidth(1.0);
        NSColor::tertiaryLabelColor().setStroke();
        border_path.stroke();
    }

    /// Draw a fieldset-style language label in the gap of the top border.
    fn draw_code_block_language_tag(&self, block_rect: NSRect, language: &str) {
        if language.is_empty() {
            return;
        }
        let ns_lang = NSString::from_str(language);

        let mattr: Retained<objc2::runtime::AnyObject> = unsafe {
            let cls = objc2::runtime::AnyClass::get(c"NSMutableAttributedString")
                .expect("NSMutableAttributedString class not found");
            let obj: *mut objc2::runtime::AnyObject = msg_send![cls, alloc];
            let obj: *mut objc2::runtime::AnyObject =
                msg_send![obj, initWithString: &*ns_lang];
            Retained::retain(obj).expect("initWithString returned nil")
        };

        let tag_len = language.encode_utf16().count();
        let tag_range = objc2_foundation::NSRange {
            location: 0,
            length: tag_len,
        };
        let tag_font =
            unsafe { NSFont::monospacedSystemFontOfSize_weight(10.0, NSFontWeightRegular) };
        let tag_color = NSColor::secondaryLabelColor();
        unsafe {
            let font_obj: &objc2::runtime::AnyObject = &tag_font;
            let color_obj: &objc2::runtime::AnyObject = &tag_color;
            let _: () = msg_send![&*mattr,
                addAttribute: NSFontAttributeName,
                value: font_obj,
                range: tag_range];
            let _: () = msg_send![&*mattr,
                addAttribute: NSForegroundColorAttributeName,
                value: color_obj,
                range: tag_range];
        }

        let tag_size: NSSize = unsafe { msg_send![&*mattr, size] };

        let gap_x = block_rect.origin.x + 14.0;
        let gap_w = tag_size.width + 8.0;
        let gap_y = block_rect.origin.y - tag_size.height / 2.0 - 1.0;
        let gap_h = tag_size.height + 2.0;

        let bg = self.backgroundColor();
        bg.setFill();
        NSRectFill(NSRect::new(
            NSPoint::new(gap_x, gap_y),
            NSSize::new(gap_w, gap_h),
        ));

        let text_rect = NSRect::new(
            NSPoint::new(gap_x + 4.0, gap_y + 1.0),
            NSSize::new(tag_size.width, tag_size.height),
        );
        let _: () = unsafe { msg_send![&*mattr, drawInRect: text_rect] };
    }

    /// Draw the SF Symbol copy/checkmark icon for a single code block.
    fn draw_code_block_copy_icon(&self, block_index: usize, icon_rect: NSRect) {
        let show_checkmark = {
            let overlay = self.ivars().overlay.borrow();
            matches!(&overlay.feedback, Some((i, t)) if *i == block_index && t.elapsed().as_secs_f64() < 1.5)
        };
        let icon_name = if show_checkmark { "checkmark" } else { "doc.on.doc" };
        let name = NSString::from_str(icon_name);
        if let Some(icon) =
            NSImage::imageWithSystemSymbolName_accessibilityDescription(&name, None)
        {
            if show_checkmark {
                NSColor::systemGreenColor().set();
            } else {
                NSColor::secondaryLabelColor().set();
            }
            icon.drawInRect(icon_rect);
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
) -> (
    Retained<NSScrollView>,
    Retained<NSTextView>,
    Retained<MditEditorDelegate>,
) {
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

    // Basic appearance — Georgia serif body, semantic background color.
    mdit_tv.setRichText(false);
    let body_font = NSFont::fontWithName_size(&NSString::from_str("Georgia"), 16.0)
        .unwrap_or_else(|| unsafe { NSFont::systemFontOfSize_weight(16.0, NSFontWeightRegular) });
    mdit_tv.setFont(Some(&body_font));
    mdit_tv.setTextColor(Some(&NSColor::labelColor()));
    // Use an explicit sRGB colour matching ColorScheme::light().background so that
    // apply_scheme() can override it consistently for any scheme, including dark mode.
    let initial_bg = NSColor::colorWithRed_green_blue_alpha(0.992, 0.976, 0.969, 1.0);
    mdit_tv.setBackgroundColor(&initial_bg);
    mdit_tv.setAutomaticQuoteSubstitutionEnabled(false);
    mdit_tv.setAutomaticDashSubstitutionEnabled(false);
    mdit_tv.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    // Initial padding — app.rs will tune this dynamically on window resize.
    mdit_tv.setTextContainerInset(NSSize::new(40.0, 40.0));

    // Prevent NSTextView from overriding our custom link color with the
    // default blue+underline styling. An empty dictionary means "no special
    // attributes for links" — our ForegroundColor("link") is preserved.
    unsafe {
        let cls = objc2::runtime::AnyClass::get(c"NSDictionary").unwrap();
        let empty: *mut objc2::runtime::AnyObject = msg_send![cls, new];
        let _: () = msg_send![&*mdit_tv, setLinkTextAttributes: empty];
    }

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
