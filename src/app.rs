use std::cell::OnceCell;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSAutoresizingMaskOptions, NSBackingStoreType, NSColor, NSFont,
    NSScrollView, NSTextView, NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSNotification, NSObject, NSObjectProtocol,
    NSPoint, NSRect, NSSize,
};

// ---------------------------------------------------------------------------
// App Delegate
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct AppDelegateIvars {
    window: OnceCell<Retained<NSWindow>>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppDelegateIvars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, notification: &NSNotification) {
            let mtm = self.mtm();

            let app = notification.object()
                .unwrap()
                .downcast::<NSApplication>()
                .unwrap();

            let window = create_window(mtm);

            window.setDelegate(Some(ProtocolObject::from_ref(self)));
            window.center();
            window.makeKeyAndOrderFront(None);

            self.ivars().window.set(window).unwrap();

            app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
        }

        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn should_terminate_after_last_window_closed(&self, _sender: &NSApplication) -> bool {
            true
        }
    }

    unsafe impl NSWindowDelegate for AppDelegate {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            unsafe { NSApplication::sharedApplication(self.mtm()).terminate(None) };
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AppDelegateIvars::default());
        unsafe { msg_send![super(this), init] }
    }
}

// ---------------------------------------------------------------------------
// Window + Text View
// ---------------------------------------------------------------------------

fn create_window(mtm: MainThreadMarker) -> Retained<NSWindow> {
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable;

    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(900.0, 700.0)),
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };

    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(ns_string!("mdit"));
    unsafe { window.setContentMinSize(NSSize::new(500.0, 400.0)) };

    // Add scroll view + text view as content
    let content = window.contentView().expect("window must have content view");
    let bounds = content.bounds();
    let scroll_view = create_scroll_text_view(mtm, bounds);
    unsafe { content.addSubview(&scroll_view) };

    window
}

pub fn create_scroll_text_view(
    mtm: MainThreadMarker,
    frame: NSRect,
) -> Retained<NSScrollView> {
    let scroll = unsafe { NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame) };
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);

    let content_size = scroll.contentSize();
    let text_rect = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(content_size.width, content_size.height.max(content_size.height)),
    );

    let text_view =
        unsafe { NSTextView::initWithFrame(NSTextView::alloc(mtm), text_rect) };

    // Basic appearance
    unsafe {
        text_view.setRichText(false);
        text_view.setFont(Some(&NSFont::userFontOfSize(16.0).unwrap_or_else(|| {
            NSFont::systemFontOfSize(16.0)
        })));
        text_view.setTextColor(Some(&NSColor::labelColor()));
        text_view.setBackgroundColor(&NSColor::textBackgroundColor());
        text_view.setAutomaticQuoteSubstitutionEnabled(false);
        text_view.setAutomaticDashSubstitutionEnabled(false);
        text_view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
    }

    scroll.setDocumentView(Some(&*text_view));
    scroll
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() {
    let mtm = MainThreadMarker::new().expect("must run on main thread");
    let app = NSApplication::sharedApplication(mtm);
    let delegate = AppDelegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();
}
