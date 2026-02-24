use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSColor, NSFont, NSFontWeightRegular, NSScrollView, NSTextView,
};
pub use objc2_app_kit::NSTextView as NSTextViewType;
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};

use super::text_storage::MditEditorDelegate;
use crate::ui::appearance::ColorScheme;

/// Build an NSScrollView containing an NSTextView.
///
/// A `MditEditorDelegate` is wired to the text view's storage so that
/// every character edit triggers a Markdown re-parse.
///
/// Returns `(scroll_view, text_view, delegate)` — the caller must keep the
/// delegate alive for as long as the text view exists.  The `text_view`
/// reference is needed to set an NSTextViewDelegate.
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

    // 2. Standard NSTextView (uses default NSTextStorage internally)
    let text_view = NSTextView::initWithFrame(NSTextView::alloc(mtm), text_rect);

    // Basic appearance — SF Pro body, semantic background color.
    text_view.setRichText(false);
    let body_font = unsafe {
        NSFont::systemFontOfSize_weight(16.0, NSFontWeightRegular)
    };
    text_view.setFont(Some(&body_font));
    text_view.setTextColor(Some(&NSColor::labelColor()));
    text_view.setBackgroundColor(&NSColor::textBackgroundColor());
    text_view.setAutomaticQuoteSubstitutionEnabled(false);
    text_view.setAutomaticDashSubstitutionEnabled(false);
    text_view.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable
            | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    // Initial padding — app.rs will tune this dynamically on window resize.
    text_view.setTextContainerInset(NSSize::new(40.0, 40.0));

    // 3. Wire our delegate to the text view's storage for re-parse on edit.
    //    Default to light scheme; app.rs overrides after appearance detection.
    let delegate = MditEditorDelegate::new(mtm, ColorScheme::light());
    if let Some(storage) = unsafe { text_view.textStorage() } {
        storage.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    }

    scroll.setDocumentView(Some(&*text_view));

    (scroll, text_view, delegate)
}
