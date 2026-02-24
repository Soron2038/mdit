use std::cell::OnceCell;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSBackingStoreType, NSTextDelegate, NSTextView, NSTextViewDelegate,
    NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSNotification, NSObject, NSObjectProtocol,
    NSPoint, NSRange, NSRect, NSSize,
};

use mdit::editor::text_storage::MditEditorDelegate;
use mdit::ui::toolbar::FloatingToolbar;

// ---------------------------------------------------------------------------
// App Delegate
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AppDelegateIvars {
    window: OnceCell<Retained<NSWindow>>,
    #[allow(dead_code)]
    editor_delegate: OnceCell<Retained<MditEditorDelegate>>,
    toolbar: OnceCell<FloatingToolbar>,
    #[allow(dead_code)]
    text_view: OnceCell<Retained<NSTextView>>,
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

            let (window, text_view, editor_delegate) = create_window(mtm);

            window.setDelegate(Some(ProtocolObject::from_ref(self)));
            window.center();
            window.makeKeyAndOrderFront(None);

            // Wire AppDelegate as text view delegate for selection tracking.
            text_view.setDelegate(Some(ProtocolObject::from_ref(self)));

            // Create floating toolbar (hidden until text is selected).
            let toolbar = FloatingToolbar::new(mtm);

            self.ivars().window.set(window).unwrap();
            let _ = self.ivars().editor_delegate.set(editor_delegate);
            let _ = self.ivars().toolbar.set(toolbar);
            let _ = self.ivars().text_view.set(text_view);

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
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }
    }

    // ── NSTextViewDelegate: show/hide toolbar on selection ────────────────
    unsafe impl NSTextDelegate for AppDelegate {}

    unsafe impl NSTextViewDelegate for AppDelegate {
        #[unsafe(method(textViewDidChangeSelection:))]
        fn text_view_did_change_selection(&self, notification: &NSNotification) {
            let Some(obj) = notification.object() else { return };
            let Ok(tv) = obj.downcast::<NSTextView>() else { return };

            let sel_range: NSRange = unsafe { msg_send![&*tv, selectedRange] };

            let Some(toolbar) = self.ivars().toolbar.get() else { return };

            if sel_range.length > 0 {
                // firstRectForCharacterRange:actualRange: returns screen coords.
                let null_ptr = std::ptr::null_mut::<NSRange>();
                let rect: NSRect = unsafe {
                    msg_send![
                        &*tv,
                        firstRectForCharacterRange: sel_range,
                        actualRange: null_ptr
                    ]
                };
                toolbar.show_near_rect(rect);
            } else {
                toolbar.hide();
            }
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

fn create_window(
    mtm: MainThreadMarker,
) -> (Retained<NSWindow>, Retained<NSTextView>, Retained<MditEditorDelegate>) {
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
    window.setContentMinSize(NSSize::new(500.0, 400.0));

    // Add scroll view + text view (backed by MditTextStorage) as content
    let content = window.contentView().expect("window must have content view");
    let bounds = content.bounds();
    let (scroll_view, text_view, editor_delegate) =
        mdit::editor::text_view::create_editor_view(mtm, bounds);
    content.addSubview(&scroll_view);

    (window, text_view, editor_delegate)
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
