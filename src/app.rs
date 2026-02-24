use std::cell::OnceCell;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSBackingStoreType, NSTextDelegate, NSTextView, NSTextViewDelegate,
    NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSArray, NSNotification, NSObject, NSObjectProtocol,
    NSPoint, NSRange, NSRect, NSSize,
};

use mdit::editor::text_storage::MditEditorDelegate;
use mdit::ui::appearance::ColorScheme;
use mdit::ui::toolbar::FloatingToolbar;

// ---------------------------------------------------------------------------
// App Delegate
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AppDelegateIvars {
    window: OnceCell<Retained<NSWindow>>,
    editor_delegate: OnceCell<Retained<MditEditorDelegate>>,
    toolbar: OnceCell<FloatingToolbar>,
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

            // Detect system appearance before the window appears so the
            // correct color scheme is active from the very first keystroke.
            let initial_scheme = detect_scheme(&app);

            let (window, text_view, editor_delegate) = create_window(mtm);

            // Override the default light scheme if the system is dark.
            editor_delegate.set_scheme(initial_scheme);

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

            // Apply initial text container inset for centred layout.
            self.update_text_container_inset();
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

        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _notification: &NSNotification) {
            self.update_text_container_inset();
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

    /// Compute and apply the horizontal text container inset so the text
    /// area is centred with a maximum width of ~700 pt.
    fn update_text_container_inset(&self) {
        let Some(tv) = self.ivars().text_view.get() else { return };
        let Some(win) = self.ivars().window.get() else { return };
        let win_width = win.frame().size.width;
        let max_text_width = 700.0_f64;
        let min_padding = 40.0_f64;
        let h_inset = if win_width > max_text_width + 2.0 * min_padding {
            (win_width - max_text_width) / 2.0
        } else {
            min_padding
        };
        tv.setTextContainerInset(NSSize::new(h_inset, 40.0));
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
// Appearance detection
// ---------------------------------------------------------------------------

/// Detect the current system appearance and return the matching `ColorScheme`.
fn detect_scheme(app: &NSApplication) -> ColorScheme {
    let appearance = app.effectiveAppearance();
    // NSAppearanceNameAqua / DarkAqua are extern statics (→ unsafe access).
    let is_dark = unsafe {
        let names = NSArray::from_slice(&[
            NSAppearanceNameAqua,
            NSAppearanceNameDarkAqua,
        ]);
        appearance
            .bestMatchFromAppearancesWithNames(&names)
            .map(|name| name.isEqualToString(NSAppearanceNameDarkAqua))
            .unwrap_or(false)
    };
    if is_dark {
        ColorScheme::dark()
    } else {
        ColorScheme::light()
    }
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
