use std::cell::{Cell, OnceCell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSBackingStoreType, NSColor, NSTextDelegate, NSTextView, NSTextViewDelegate,
    NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSArray, NSNotification, NSObject, NSObjectProtocol,
    NSPoint, NSRange, NSRect, NSSize, NSString,
};

use mdit::editor::document_state::DocumentState;
use mdit::menu::build_main_menu;
use mdit::ui::appearance::ColorScheme;
use mdit::ui::path_bar::PathBar;
use mdit::ui::tab_bar::{tab_label, TabBar};
use mdit::ui::toolbar::FloatingToolbar;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const TAB_H: f64 = 32.0;
const PATH_H: f64 = 22.0;

// ---------------------------------------------------------------------------
// App Delegate
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AppDelegateIvars {
    window:       OnceCell<Retained<NSWindow>>,
    toolbar:      OnceCell<FloatingToolbar>,
    tab_bar:      OnceCell<TabBar>,
    path_bar:     OnceCell<PathBar>,
    tabs:         RefCell<Vec<DocumentState>>,
    active_index: Cell<usize>,
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

            let initial_scheme = detect_scheme(&app);
            let window = create_window(mtm);

            window.setDelegate(Some(ProtocolObject::from_ref(self)));
            build_main_menu(&app, mtm);
            window.center();
            window.makeKeyAndOrderFront(None);

            let content = unsafe { window.contentView().unwrap() };
            let bounds = content.bounds();
            let w = bounds.size.width;
            let h = bounds.size.height;

            // TabBar at the top
            let tab_bar = TabBar::new(mtm, w);
            tab_bar.view().setFrame(NSRect::new(
                NSPoint::new(0.0, h - TAB_H),
                NSSize::new(w, TAB_H),
            ));
            content.addSubview(tab_bar.view());

            // PathBar at the bottom
            let path_bar = PathBar::new(mtm, w);
            content.addSubview(path_bar.view());

            // Floating toolbar
            let target: &AnyObject = unsafe {
                &*(self as *const AppDelegate as *const AnyObject)
            };
            let toolbar = FloatingToolbar::new(mtm, target);

            self.ivars().window.set(window).unwrap();
            let _ = self.ivars().tab_bar.set(tab_bar);
            let _ = self.ivars().path_bar.set(path_bar);
            let _ = self.ivars().toolbar.set(toolbar);

            app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);

            // First empty tab (sets scheme and inset)
            self.add_empty_tab();
            self.apply_scheme(initial_scheme);
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
            let Some(win) = self.ivars().window.get() else { return };
            let bounds = unsafe { win.contentView().unwrap().bounds() };
            let w = bounds.size.width;
            let h = bounds.size.height;

            if let Some(tb) = self.ivars().tab_bar.get() {
                tb.view().setFrame(NSRect::new(
                    NSPoint::new(0.0, h - TAB_H),
                    NSSize::new(w, TAB_H),
                ));
            }
            if let Some(pb) = self.ivars().path_bar.get() {
                pb.view().setFrame(NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(w, PATH_H),
                ));
            }
            let frame = self.content_frame();
            let idx = self.ivars().active_index.get();
            let tabs = self.ivars().tabs.borrow();
            if let Some(t) = tabs.get(idx) {
                t.scroll_view.setFrame(frame);
            }
            drop(tabs);
            self.update_text_container_inset();
        }
    }

    // ── Action methods ─────────────────────────────────────────────────────
    impl AppDelegate {
        /// File > Export as PDF…  (Cmd+Shift+E)
        #[unsafe(method(exportPDF:))]
        fn export_pdf_action(&self, _sender: &AnyObject) {
            if let Some(tv) = self.active_text_view() {
                mdit::export::pdf::export_pdf(&tv);
            }
        }

        // ── Inline formatting ──────────────────────────────────────────────

        #[unsafe(method(applyBold:))]
        fn apply_bold(&self, _sender: &AnyObject) {
            if let Some(tv) = self.active_text_view() {
                wrap_selection(&tv, "**", "**");
            }
        }

        #[unsafe(method(applyItalic:))]
        fn apply_italic(&self, _sender: &AnyObject) {
            if let Some(tv) = self.active_text_view() {
                wrap_selection(&tv, "_", "_");
            }
        }

        #[unsafe(method(applyInlineCode:))]
        fn apply_inline_code(&self, _sender: &AnyObject) {
            if let Some(tv) = self.active_text_view() {
                wrap_selection(&tv, "`", "`");
            }
        }

        #[unsafe(method(applyLink:))]
        fn apply_link(&self, _sender: &AnyObject) {
            if let Some(tv) = self.active_text_view() {
                wrap_selection(&tv, "[", "]()");
            }
        }

        #[unsafe(method(applyStrikethrough:))]
        fn apply_strikethrough(&self, _sender: &AnyObject) {
            if let Some(tv) = self.active_text_view() {
                wrap_selection(&tv, "~~", "~~");
            }
        }

        // ── Appearance ─────────────────────────────────────────────────────

        #[unsafe(method(applyLightMode:))]
        fn apply_light_mode(&self, _sender: &AnyObject) {
            self.apply_scheme(ColorScheme::light());
        }

        #[unsafe(method(applyDarkMode:))]
        fn apply_dark_mode(&self, _sender: &AnyObject) {
            self.apply_scheme(ColorScheme::dark());
        }

        #[unsafe(method(applySystemMode:))]
        fn apply_system_mode(&self, _sender: &AnyObject) {
            let scheme = detect_scheme(&NSApplication::sharedApplication(self.mtm()));
            self.apply_scheme(scheme);
        }

        // ── Tab management ─────────────────────────────────────────────────

        #[unsafe(method(switchToTab:))]
        fn switch_to_tab_action(&self, sender: &AnyObject) {
            let idx = unsafe { objc2_app_kit::NSControl::tag(
                &*(sender as *const _ as *const objc2_app_kit::NSControl)
            )};
            if idx >= 0 {
                self.switch_to_tab(idx as usize);
            }
        }

        #[unsafe(method(closeTab:))]
        fn close_tab_action(&self, sender: &AnyObject) {
            let idx = unsafe { objc2_app_kit::NSControl::tag(
                &*(sender as *const _ as *const objc2_app_kit::NSControl)
            ) as usize };
            self.close_tab(idx);
        }

        // ── Heading shortcuts ──────────────────────────────────────────────

        #[unsafe(method(applyH1:))]
        fn apply_h1(&self, _sender: &AnyObject) {
            if let Some(tv) = self.active_text_view() {
                prepend_line(&tv, "# ");
            }
        }

        #[unsafe(method(applyH2:))]
        fn apply_h2(&self, _sender: &AnyObject) {
            if let Some(tv) = self.active_text_view() {
                prepend_line(&tv, "## ");
            }
        }

        #[unsafe(method(applyH3:))]
        fn apply_h3(&self, _sender: &AnyObject) {
            if let Some(tv) = self.active_text_view() {
                prepend_line(&tv, "### ");
            }
        }
    }

    // ── NSTextViewDelegate: show/hide toolbar on selection ──────────────────
    unsafe impl NSTextDelegate for AppDelegate {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, _notification: &NSNotification) {
            let idx = self.ivars().active_index.get();
            let already_dirty = {
                let tabs = self.ivars().tabs.borrow();
                tabs.get(idx).map(|t| t.is_dirty.get()).unwrap_or(true)
            };
            if !already_dirty {
                {
                    let tabs = self.ivars().tabs.borrow();
                    if let Some(t) = tabs.get(idx) {
                        t.is_dirty.set(true);
                    }
                }
                self.rebuild_tab_bar();
            }
        }
    }

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

    /// Frame for the active NSScrollView (between TabBar and PathBar).
    fn content_frame(&self) -> NSRect {
        let Some(win) = self.ivars().window.get() else { return NSRect::ZERO };
        let bounds = unsafe { win.contentView().unwrap().bounds() };
        let h = bounds.size.height;
        let w = bounds.size.width;
        NSRect::new(
            NSPoint::new(0.0, PATH_H),
            NSSize::new(w, (h - TAB_H - PATH_H).max(0.0)),
        )
    }

    /// Active text view for formatting actions.
    fn active_text_view(&self) -> Option<Retained<NSTextView>> {
        let idx = self.ivars().active_index.get();
        let tabs = self.ivars().tabs.borrow();
        tabs.get(idx).map(|t| t.text_view.clone())
    }

    /// Rebuild tab bar buttons.
    fn rebuild_tab_bar(&self) {
        let Some(_win) = self.ivars().window.get() else { return };
        let Some(tab_bar) = self.ivars().tab_bar.get() else { return };
        let mtm = self.mtm();
        let active = self.ivars().active_index.get();
        let target: &AnyObject = unsafe {
            &*(self as *const AppDelegate as *const AnyObject)
        };
        let labels: Vec<(String, bool)> = {
            let tabs = self.ivars().tabs.borrow();
            tabs.iter().enumerate().map(|(i, t)| {
                let url = t.url.borrow();
                (tab_label(url.as_deref(), t.is_dirty.get()), i == active)
            }).collect()
        };
        tab_bar.rebuild(mtm, &labels, target);
    }

    /// Switch to tab `index`.
    fn switch_to_tab(&self, index: usize) {
        let Some(win) = self.ivars().window.get() else { return };
        let content = unsafe { win.contentView().unwrap() };

        // Remove old scroll view
        {
            let tabs = self.ivars().tabs.borrow();
            let old = self.ivars().active_index.get();
            if let Some(t) = tabs.get(old) {
                t.scroll_view.removeFromSuperview();
            }
        }

        self.ivars().active_index.set(index);

        // Insert new scroll view
        let frame = self.content_frame();
        {
            let tabs = self.ivars().tabs.borrow();
            if let Some(t) = tabs.get(index) {
                t.scroll_view.setFrame(frame);
                content.addSubview(&t.scroll_view);
            }
        }

        // Update path bar
        if let Some(pb) = self.ivars().path_bar.get() {
            let tabs = self.ivars().tabs.borrow();
            let url = tabs.get(index).and_then(|t| t.url.borrow().clone());
            pb.update(url.as_deref());
        }

        self.rebuild_tab_bar();
        self.update_text_container_inset();
        if let Some(tb) = self.ivars().toolbar.get() {
            tb.hide();
        }
    }

    /// Create a new empty tab and activate it.
    fn add_empty_tab(&self) {
        let mtm = self.mtm();
        let scheme = {
            let tabs = self.ivars().tabs.borrow();
            tabs.first()
                .map(|t| t.editor_delegate.scheme())
                .unwrap_or_else(ColorScheme::light)
        };
        let frame = self.content_frame();
        let tab = DocumentState::new_empty(mtm, scheme, frame);
        // Wire delegate so textViewDidChangeSelection: fires
        tab.text_view.setDelegate(Some(ProtocolObject::from_ref(self)));
        let new_idx = {
            let mut tabs = self.ivars().tabs.borrow_mut();
            tabs.push(tab);
            tabs.len() - 1
        };
        self.switch_to_tab(new_idx);
    }

    /// Switch the color scheme and immediately re-render all documents.
    fn apply_scheme(&self, scheme: ColorScheme) {
        let tabs = self.ivars().tabs.borrow();
        let active = self.ivars().active_index.get();
        for (i, tab) in tabs.iter().enumerate() {
            tab.editor_delegate.set_scheme(scheme);
            let (r, g, b) = scheme.background;
            let bg = NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0);
            tab.scroll_view.setBackgroundColor(&bg);
            tab.text_view.setBackgroundColor(&bg);
            if i == active {
                if let Some(storage) = unsafe { tab.text_view.textStorage() } {
                    tab.editor_delegate.reapply(&storage);
                }
            }
        }
    }

    /// Close tab at `index` — dirty-check, then remove (or clear if last).
    fn close_tab(&self, index: usize) {
        let is_dirty = {
            let tabs = self.ivars().tabs.borrow();
            tabs.get(index).map(|t| t.is_dirty.get()).unwrap_or(false)
        };
        let filename = {
            let tabs = self.ivars().tabs.borrow();
            tabs.get(index)
                .and_then(|t| t.url.borrow().as_deref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "Untitled".to_string())
        };

        if is_dirty {
            match show_save_alert(&filename, self.mtm()) {
                SaveChoice::Save => self.perform_save(Some(index)),
                SaveChoice::DontSave => {}
                SaveChoice::Cancel => return,
            }
        }

        // Last tab → only clear content, don’t remove tab
        if self.ivars().tabs.borrow().len() == 1 {
            let tabs = self.ivars().tabs.borrow();
            if let Some(t) = tabs.first() {
                unsafe {
                    if let Some(storage) = t.text_view.textStorage() {
                        let full = NSRange { location: 0, length: storage.length() };
                        let empty = NSString::from_str("");
                        storage.replaceCharactersInRange_withString(full, &empty);
                    }
                }
                *t.url.borrow_mut() = None;
                t.is_dirty.set(false);
            }
            drop(tabs);
            self.rebuild_tab_bar();
            if let Some(pb) = self.ivars().path_bar.get() {
                pb.update(None);
            }
            return;
        }

        // Remove scroll view from superview
        {
            let tabs = self.ivars().tabs.borrow();
            if let Some(t) = tabs.get(index) {
                t.scroll_view.removeFromSuperview();
            }
        }
        self.ivars().tabs.borrow_mut().remove(index);

        // Correct active index
        let new_idx = {
            let len = self.ivars().tabs.borrow().len();
            let cur = self.ivars().active_index.get();
            if index <= cur && cur > 0 { cur - 1 } else { cur.min(len - 1) }
        };
        self.switch_to_tab(new_idx);
    }

    /// Save tab at `index` (None = active). Full implementation in Task 8.
    fn perform_save(&self, _index: Option<usize>) {
        // TODO: implemented in Task 8
    }

    /// Compute and apply the horizontal text container inset for the active tab.
    fn update_text_container_inset(&self) {
        let Some(win) = self.ivars().window.get() else { return };
        let win_width = win.frame().size.width;
        let max_text_width = 700.0_f64;
        let min_padding = 40.0_f64;
        let h_inset = if win_width > max_text_width + 2.0 * min_padding {
            (win_width - max_text_width) / 2.0
        } else {
            min_padding
        };
        let idx = self.ivars().active_index.get();
        let tabs = self.ivars().tabs.borrow();
        if let Some(t) = tabs.get(idx) {
            t.text_view.setTextContainerInset(NSSize::new(h_inset, 40.0));
        }
    }
}

// ---------------------------------------------------------------------------
// Dirty-check dialog
// ---------------------------------------------------------------------------

enum SaveChoice { Save, DontSave, Cancel }

fn show_save_alert(filename: &str, mtm: MainThreadMarker) -> SaveChoice {
    use objc2_app_kit::NSAlert;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(
        &format!("Do you want to save changes to \"{}\"?", filename)
    ));
    alert.setInformativeText(&NSString::from_str(
        "Your changes will be lost if you don't save them."
    ));
    alert.addButtonWithTitle(&NSString::from_str("Save"));        // 1000
    alert.addButtonWithTitle(&NSString::from_str("Don't Save"));  // 1001
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));      // 1002
    let response = unsafe { alert.runModal() };
    match response {
        1000 => SaveChoice::Save,
        1001 => SaveChoice::DontSave,
        _ => SaveChoice::Cancel,
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Replace the current NSTextView selection with `prefix + selected + suffix`.
///
/// Uses `insertText:replacementRange:` so the edit is registered with undo.
fn wrap_selection(tv: &NSTextView, prefix: &str, suffix: &str) {
    let range: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else { return };
    let selected: Retained<NSString> = storage.string().substringWithRange(range);
    let combined = format!("{}{}{}", prefix, selected, suffix);
    let ns = NSString::from_str(&combined);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: range] }
}

/// Insert `prefix` at the beginning of the line that contains the caret.
///
/// Works on the NSString level so it correctly handles multi-byte content.
fn prepend_line(tv: &NSTextView, prefix: &str) {
    let caret: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else { return };
    let ns_str = storage.string();
    // NSString.lineRangeForRange: gives us the UTF-16 range of the whole line.
    let point = NSRange { location: caret.location, length: 0 };
    let line_range: NSRange = ns_str.lineRangeForRange(point);
    let insert_at = NSRange { location: line_range.location, length: 0 };
    let ns = NSString::from_str(prefix);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: insert_at] }
}

// ---------------------------------------------------------------------------
// Window creation
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
    window.setContentMinSize(NSSize::new(500.0, 400.0));

    window
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
