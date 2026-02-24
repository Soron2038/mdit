use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSColor, NSFont, NSScrollView, NSTextView,
};
pub use objc2_app_kit::NSTextView as NSTextViewType;
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};

use super::text_storage::MditEditorDelegate;

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

    // Basic appearance
    text_view.setRichText(false);
    text_view.setFont(Some(
        &NSFont::userFontOfSize(16.0).unwrap_or_else(|| NSFont::systemFontOfSize(16.0)),
    ));
    text_view.setTextColor(Some(&NSColor::labelColor()));
    text_view.setBackgroundColor(&NSColor::textBackgroundColor());
    text_view.setAutomaticQuoteSubstitutionEnabled(false);
    text_view.setAutomaticDashSubstitutionEnabled(false);
    text_view.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable
            | NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    // 3. Wire our delegate to the text view's storage for re-parse on edit
    let delegate = MditEditorDelegate::new(mtm);
    if let Some(storage) = unsafe { text_view.textStorage() } {
        storage.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    }

    scroll.setDocumentView(Some(&*text_view));

    (scroll, text_view, delegate)
}
