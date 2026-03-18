use std::cell::{Cell, OnceCell, RefCell};
use std::path::PathBuf;
use std::ptr::NonNull;

use block2::StackBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSAnimationContext, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication,
    NSApplicationActivationPolicy, NSApplicationDelegate, NSBackgroundColorAttributeName,
    NSBackingStoreType, NSBezelStyle, NSButton, NSColor, NSControl, NSImage, NSTextDelegate,
    NSTextView, NSTextViewDelegate, NSView, NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSArray, NSNotification, NSObject, NSObjectProtocol, NSPoint,
    NSRange, NSRect, NSSize, NSString,
};
use objc2_quartz_core::{CAMediaTimingFunction, kCAMediaTimingFunctionEaseInEaseOut};

use mdit::editor::document_state::DocumentState;
use mdit::editor::tab_manager::{TabCloseResult, TabManager};
use mdit::editor::view_mode::ViewMode;
use mdit::menu::build_main_menu;
use mdit::ui::appearance::ColorScheme;
use mdit::ui::find_bar::{FindBar, FIND_H_COMPACT, FIND_H_EXPANDED};
use mdit::ui::path_bar::PathBar;
use mdit::ui::sidebar::{FormattingSidebar, SIDEBAR_W};
use mdit::ui::tab_bar::TabBar;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const TAB_H: f64 = 32.0;
const PATH_H: f64 = 22.0;

/// Frame for the sidebar container for a given mode.
///
/// - Viewer → width 0 (hidden)
/// - Editor → width SIDEBAR_W (visible)
fn sidebar_target_frame(mode: ViewMode, content_h: f64) -> NSRect {
    let w = if mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
    NSRect::new(
        NSPoint::new(0.0, PATH_H),
        NSSize::new(w, content_h),
    )
}

/// Frame for the active NSScrollView for a given mode.
///
/// - Viewer → x: 0, full window width
/// - Editor → x: SIDEBAR_W, reduced width
///
/// `find_offset` is the height of the find bar (0.0 when hidden).
fn content_target_frame(mode: ViewMode, find_offset: f64, win_w: f64, win_h: f64) -> NSRect {
    let sidebar_w = if mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
    NSRect::new(
        NSPoint::new(sidebar_w, PATH_H + find_offset),
        NSSize::new(
            (win_w - sidebar_w).max(0.0),
            (win_h - TAB_H - PATH_H - find_offset).max(0.0),
        ),
    )
}

// ---------------------------------------------------------------------------
// App Delegate
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AppDelegateIvars {
    window: OnceCell<Retained<NSWindow>>,
    sidebar: OnceCell<FormattingSidebar>,
    tab_bar: OnceCell<TabBar>,
    path_bar: OnceCell<PathBar>,
    tab_manager: RefCell<TabManager>,
    /// File path received via `application:openFile:` before the window exists.
    pending_open: RefCell<Option<PathBuf>>,
    // ── Find bar state ───────────────────────────────────────────────────
    find_bar: OnceCell<FindBar>,
    find_matches: RefCell<Vec<NSRange>>,
    find_current: Cell<usize>,
    /// 0.0 = hidden; FIND_H_COMPACT or FIND_H_EXPANDED when visible.
    find_bar_height: Cell<f64>,
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
            let app = notification.object()
                .unwrap()
                .downcast::<NSApplication>()
                .unwrap();

            let initial_scheme = detect_scheme(&app);

            self.setup_window_and_menu(&app);
            self.setup_content_views();

            app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);

            // Open pending file or start with an empty tab.
            let pending = self.ivars().pending_open.borrow_mut().take();
            self.add_empty_tab();
            self.apply_scheme(initial_scheme);
            if let Some(path) = pending {
                self.open_file_by_path(path);
            }
            self.update_text_container_inset();
        }

        #[unsafe(method(application:openFile:))]
        fn open_file(&self, _sender: &NSApplication, filename: &NSString) -> bool {
            let path = PathBuf::from(filename.to_string());
            if self.ivars().window.get().is_none() {
                // Called before applicationDidFinishLaunching: — stash for later.
                *self.ivars().pending_open.borrow_mut() = Some(path);
            } else {
                self.open_file_by_path(path);
            }
            true
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
            let bounds = win.contentView().unwrap().bounds();
            let w = bounds.size.width;
            let h = bounds.size.height;

            if let Some(tb) = self.ivars().tab_bar.get() {
                tb.view().setFrame(NSRect::new(
                    NSPoint::new(0.0, h - TAB_H),
                    NSSize::new(w, TAB_H),
                ));
            }
            if let Some(pb) = self.ivars().path_bar.get() {
                pb.set_width(w);
            }
            if let Some(fb) = self.ivars().find_bar.get() {
                let fh = self.ivars().find_bar_height.get();
                fb.set_width(w);
                if fh > 0.0 {
                    fb.view().setFrame(NSRect::new(
                        NSPoint::new(0.0, PATH_H),
                        NSSize::new(w, fh),
                    ));
                }
            }
            if let Some(sb) = self.ivars().sidebar.get() {
                let content_h = (h - TAB_H - PATH_H).max(0.0);
                let mode = self.ivars().tab_manager.borrow()
                    .active()
                    .map(|t| t.mode.get())
                    .unwrap_or(ViewMode::Viewer);
                let sidebar_w = if mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
                sb.set_size_direct(sidebar_w, content_h);
            }
            let frame = self.content_frame();
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.active() {
                t.scroll_view.setFrame(frame);
            }
            drop(tm);
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
        fn apply_bold(&self, _sender: &AnyObject) { self.dispatch_inline_format("**"); }

        #[unsafe(method(applyItalic:))]
        fn apply_italic(&self, _sender: &AnyObject) { self.dispatch_inline_format("_"); }

        #[unsafe(method(applyInlineCode:))]
        fn apply_inline_code(&self, _sender: &AnyObject) { self.dispatch_inline_format("`"); }

        #[unsafe(method(applyLink:))]
        fn apply_link(&self, _sender: &AnyObject) {
            if let Some(tv) = self.editor_text_view() {
                insert_link_wrap(&tv, "[", "]()");
            }
        }

        #[unsafe(method(applyStrikethrough:))]
        fn apply_strikethrough(&self, _sender: &AnyObject) { self.dispatch_inline_format("~~"); }

        #[unsafe(method(applyHighlight:))]
        fn apply_highlight(&self, _sender: &AnyObject) { self.dispatch_inline_format("=="); }

        #[unsafe(method(applySubscript:))]
        fn apply_subscript(&self, _sender: &AnyObject) { self.dispatch_inline_format("~"); }

        #[unsafe(method(applySuperscript:))]
        fn apply_superscript(&self, _sender: &AnyObject) { self.dispatch_inline_format("^"); }

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

        // ── View mode toggle ───────────────────────────────────────────

        #[unsafe(method(toggleMode:))]
        fn toggle_mode_action(&self, _sender: &AnyObject) {
            self.toggle_mode();
        }

        // ── File operations ──────────────────────────────────────────────

        #[unsafe(method(newDocument:))]
        fn new_document_action(&self, _sender: &AnyObject) {
            self.add_empty_tab();
        }

        #[unsafe(method(openDocument:))]
        fn open_document_action(&self, _sender: &AnyObject) {
            use objc2_app_kit::NSOpenPanel;
            let panel = NSOpenPanel::openPanel(self.mtm());
            panel.setCanChooseFiles(true);
            panel.setCanChooseDirectories(false);
            panel.setAllowsMultipleSelection(false);
            let response = panel.runModal();
            if response != 1 { return; } // NSModalResponseOK = 1

            let ns_url = panel.URL();
            let Some(ns_url) = ns_url else { return };
            let Some(ns_path) = ns_url.path() else { return };
            let path = std::path::PathBuf::from(ns_path.to_string());
            self.open_file_by_path(path);
        }

        #[unsafe(method(saveDocument:))]
        fn save_document_action(&self, _sender: &AnyObject) {
            self.perform_save(None);
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
        fn apply_h1(&self, _sender: &AnyObject) { self.dispatch_block_format("# "); }

        #[unsafe(method(applyH2:))]
        fn apply_h2(&self, _sender: &AnyObject) { self.dispatch_block_format("## "); }

        #[unsafe(method(applyH3:))]
        fn apply_h3(&self, _sender: &AnyObject) { self.dispatch_block_format("### "); }

        #[unsafe(method(applyNormal:))]
        fn apply_normal(&self, _sender: &AnyObject) { self.dispatch_block_format(""); }

        #[unsafe(method(applyBlockquote:))]
        fn apply_blockquote(&self, _sender: &AnyObject) { self.dispatch_block_format("> "); }

        #[unsafe(method(applyCodeBlock:))]
        fn apply_code_block(&self, _sender: &AnyObject) {
            if let Some(tv) = self.editor_text_view() {
                insert_code_block(&tv);
            }
        }

        #[unsafe(method(applyHRule:))]
        fn apply_h_rule(&self, _sender: &AnyObject) {
            if let Some(tv) = self.editor_text_view() {
                let caret: NSRange = unsafe { msg_send![&*tv, selectedRange] };
                let ns = NSString::from_str("\n---\n");
                unsafe { msg_send![&*tv, insertText: &*ns, replacementRange: caret] }
            }
        }

        // ── Find bar actions ──────────────────────────────────────────────

        #[unsafe(method(openFindBar:))]
        fn open_find_bar_action(&self, _sender: &AnyObject) {
            self.open_find_bar();
        }

        #[unsafe(method(closeFindBar:))]
        fn close_find_bar_action(&self, _sender: &AnyObject) {
            self.close_find_bar();
        }

        #[unsafe(method(findNext:))]
        fn find_next_action(&self, _sender: &AnyObject) {
            // If find bar isn't open, open it instead of cycling
            if self.ivars().find_bar_height.get() == 0.0 {
                self.open_find_bar();
                return;
            }
            let matches = self.ivars().find_matches.borrow();
            let count = matches.len();
            if count == 0 { return; }
            drop(matches);
            let current = self.ivars().find_current.get();
            let next = (current + 1) % count;
            self.ivars().find_current.set(next);
            self.highlight_current_match();
            self.scroll_to_current_match();
            if let Some(fb) = self.ivars().find_bar.get() {
                fb.update_count(next + 1, count);
            }
        }

        #[unsafe(method(findPrevious:))]
        fn find_previous_action(&self, _sender: &AnyObject) {
            let matches = self.ivars().find_matches.borrow();
            let count = matches.len();
            if count == 0 { return; }
            drop(matches);
            let current = self.ivars().find_current.get();
            let prev = if current == 0 { count - 1 } else { current - 1 };
            self.ivars().find_current.set(prev);
            self.highlight_current_match();
            self.scroll_to_current_match();
            if let Some(fb) = self.ivars().find_bar.get() {
                fb.update_count(prev + 1, count);
            }
        }

        #[unsafe(method(findBarToggleAa:))]
        fn find_bar_toggle_aa(&self, _sender: &AnyObject) {
            if let Some(fb) = self.ivars().find_bar.get() {
                fb.toggle_case_sensitive();
            }
            self.perform_search();
        }

        #[unsafe(method(replaceOne:))]
        fn replace_one_action(&self, _sender: &AnyObject) {
            let Some(fb) = self.ivars().find_bar.get() else { return };
            let replace_str = fb.replace_text();
            let current = self.ivars().find_current.get();
            let range = {
                let matches = self.ivars().find_matches.borrow();
                matches.get(current).copied()
            };
            let Some(range) = range else { return };
            let tm = self.ivars().tab_manager.borrow();
            let Some(tab) = tm.active() else { return };
            if let Some(storage) = unsafe { tab.text_view.textStorage() } {
                let ns_replace = NSString::from_str(&replace_str);
                storage.replaceCharactersInRange_withString(range, &ns_replace);
            }
            drop(tm);
            self.perform_search();
        }

        #[unsafe(method(replaceAll:))]
        fn replace_all_action(&self, _sender: &AnyObject) {
            let Some(fb) = self.ivars().find_bar.get() else { return };
            let replace_str = fb.replace_text();
            let ranges: Vec<NSRange> = self.ivars().find_matches.borrow().clone();
            if ranges.is_empty() { return; }
            let tm = self.ivars().tab_manager.borrow();
            let Some(tab) = tm.active() else { return };
            if let Some(storage) = unsafe { tab.text_view.textStorage() } {
                let ns_replace = NSString::from_str(&replace_str);
                // Replace in reverse order to preserve offsets
                for range in ranges.iter().rev() {
                    storage.replaceCharactersInRange_withString(*range, &ns_replace);
                }
            }
            drop(tm);
            self.perform_search();
        }

        // ── Live search delegate ──────────────────────────────────────────

        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, _notification: &NSNotification) {
            self.perform_search();
        }
    }

    // ── NSTextViewDelegate: show/hide toolbar on selection ──────────────────
    unsafe impl NSTextDelegate for AppDelegate {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, _notification: &NSNotification) {
            let already_dirty = {
                let tm = self.ivars().tab_manager.borrow();
                tm.active().map(|t| t.is_dirty.get()).unwrap_or(true)
            };
            if !already_dirty {
                {
                    let tm = self.ivars().tab_manager.borrow();
                    if let Some(t) = tm.active() {
                        t.is_dirty.set(true);
                    }
                }
                self.rebuild_tab_bar();
            }
        }
    }

    unsafe impl NSTextViewDelegate for AppDelegate {
        #[unsafe(method(textViewDidChangeSelection:))]
        fn text_view_did_change_selection(&self, _notification: &NSNotification) {
            // Selection changes are handled by the sidebar (visible in Editor mode only).
            // No floating toolbar to show or hide.
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AppDelegateIvars::default());
        unsafe { msg_send![super(this), init] }
    }

    /// Forward an inline-format toggle to the active editor text view.
    ///
    /// Switches to Editor mode automatically if currently in Viewer mode,
    /// so clicking a sidebar button activates editing.
    fn dispatch_inline_format(&self, marker: &'static str) {
        if let Some(tv) = self.editor_text_view() {
            toggle_inline_wrap(&tv, marker);
        }
    }

    /// Apply a block-level prefix to the line containing the caret.
    ///
    /// Delegates to the pure `set_block_format()` in `editor::formatting`.
    /// Switches to Editor mode automatically if needed.
    fn dispatch_block_format(&self, prefix: &'static str) {
        if let Some(tv) = self.editor_text_view() {
            apply_block_format(&tv, prefix);
        }
    }

    /// Create the main window, build the menu, and present it.
    ///
    /// Stores the window in `self.ivars().window`.
    /// Called once from `applicationDidFinishLaunching:`.
    fn setup_window_and_menu(&self, app: &NSApplication) {
        let mtm = self.mtm();
        let window = create_window(mtm);
        window.setDelegate(Some(ProtocolObject::from_ref(self)));
        build_main_menu(app, mtm);
        window.center();
        window.makeKeyAndOrderFront(None);
        let target: &AnyObject = unsafe {
            &*(self as *const AppDelegate as *const AnyObject)
        };
        add_titlebar_accessory(&window, mtm, target);
        self.ivars().window.set(window).unwrap();
    }

    /// Create and add the content view hierarchy (tab bar, path bar, sidebar).
    ///
    /// Must be called after the window is created and stored in `self.ivars().window`.
    fn setup_content_views(&self) {
        let mtm = self.mtm();
        let window = self.ivars().window.get().expect("window must exist before setup_content_views");
        let content = window.contentView().unwrap();
        let bounds = content.bounds();
        let w = bounds.size.width;
        let h = bounds.size.height;

        let target: &AnyObject = unsafe {
            &*(self as *const AppDelegate as *const AnyObject)
        };

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

        // Sidebar — formatting toolbar
        let content_h = (h - TAB_H - PATH_H).max(0.0);
        // New tabs start in Viewer mode, so the sidebar begins hidden (width = 0).
        let sidebar = FormattingSidebar::new(mtm, content_h, target);
        sidebar.view().setFrame(NSRect::new(
            NSPoint::new(0.0, PATH_H),
            NSSize::new(0.0, content_h),
        ));
        content.addSubview(sidebar.view());

        // FindBar — hidden initially, positioned above PathBar
        let find_bar = FindBar::new(mtm, w, target);
        find_bar.view().setFrame(NSRect::new(
            NSPoint::new(0.0, PATH_H),
            NSSize::new(w, 0.0), // zero height = hidden
        ));
        content.addSubview(find_bar.view());

        // Wire AppDelegate as search field delegate for live search
        let self_obj: &AnyObject = unsafe { &*(self as *const AppDelegate as *const AnyObject) };
        find_bar.set_search_delegate(self_obj);

        let _ = self.ivars().tab_bar.set(tab_bar);
        let _ = self.ivars().path_bar.set(path_bar);
        let _ = self.ivars().sidebar.set(sidebar);
        let _ = self.ivars().find_bar.set(find_bar);
    }

    /// Frame for the active NSScrollView, positioned between the tab bar and path bar.
    ///
    /// Returns `NSRect::ZERO` if the window is not yet initialised.
    /// In Viewer mode the sidebar is hidden, so the frame starts at x:0 and uses full width.
    fn content_frame(&self) -> NSRect {
        let Some(win) = self.ivars().window.get() else {
            return NSRect::ZERO;
        };
        let bounds = win.contentView().unwrap().bounds();
        let mode = self.ivars().tab_manager.borrow()
            .active()
            .map(|t| t.mode.get())
            .unwrap_or(ViewMode::Viewer);
        let find_offset = self.ivars().find_bar_height.get();
        content_target_frame(mode, find_offset, bounds.size.width, bounds.size.height)
    }

    /// Returns true if the active tab is in Editor mode.
    fn is_editor_mode(&self) -> bool {
        let tm = self.ivars().tab_manager.borrow();
        tm.active().map(|t| t.mode.get() == ViewMode::Editor).unwrap_or(false)
    }

    /// Toggle between Viewer and Editor mode for the active tab.
    fn toggle_mode(&self) {
        // ── 1. Collect state ──────────────────────────────────────────────────
        let (new_mode, text_view, editor_delegate, scroll_view) = {
            let tm = self.ivars().tab_manager.borrow();
            let tab = match tm.active() {
                Some(t) => t,
                None => return,
            };
            let new_mode = match tab.mode.get() {
                ViewMode::Viewer => ViewMode::Editor,
                ViewMode::Editor => ViewMode::Viewer,
            };
            tab.mode.set(new_mode);
            (
                new_mode,
                tab.text_view.clone(),
                tab.editor_delegate.clone(),
                tab.scroll_view.clone(),
            )
        };

        // ── 2. Non-visual changes (immediate) ─────────────────────────────────
        editor_delegate.set_mode(new_mode);
        text_view.setEditable(new_mode == ViewMode::Editor);
        if let Some(storage) = unsafe { text_view.textStorage() } {
            editor_delegate.reapply(&storage);
        }
        self.update_text_container_inset();

        // ── 3. Compute target frames ───────────────────────────────────────────
        let Some(win) = self.ivars().window.get() else { return };
        let bounds = win.contentView().unwrap().bounds();
        let (win_w, win_h) = (bounds.size.width, bounds.size.height);
        let content_h = (win_h - TAB_H - PATH_H).max(0.0);

        let target_sb_frame = sidebar_target_frame(new_mode, content_h);
        let find_offset = self.ivars().find_bar_height.get();
        let target_sv_frame = content_target_frame(new_mode, find_offset, win_w, win_h);

        // ── 4. Animated frame changes ──────────────────────────────────────────
        let Some(sb) = self.ivars().sidebar.get() else { return };

        // Safety:
        // sb_ptr — FormattingSidebar lives in a OnceCell for the application
        //   lifetime; the pointer outlives toggle_mode.
        // sv_ptr — scroll_view (Retained<NSScrollView>) is held in this scope
        //   and will not be dropped before runAnimationGroup_completionHandler
        //   returns, keeping sv_ptr valid regardless of when the block executes.
        let sb_ptr: *const NSView = sb.view();
        let sv_ptr: *const objc2_app_kit::NSScrollView = &*scroll_view;

        // Bind to a named variable to avoid temporary lifetime issues.
        let animation_block = StackBlock::new(move |ctx: NonNull<NSAnimationContext>| {
            // Safety: ctx is a valid NSAnimationContext pointer provided by AppKit.
            let ctx = unsafe { ctx.as_ref() };
            ctx.setDuration(0.35);
            let timing = CAMediaTimingFunction::functionWithName(unsafe { kCAMediaTimingFunctionEaseInEaseOut });
            ctx.setTimingFunction(Some(&*timing));

            // Animate sidebar container via raw msg_send on the animator proxy.
            // (The animator proxy is an opaque AnyObject, not a typed NSView.)
            let sb_proxy: *const AnyObject = unsafe { msg_send![sb_ptr, animator] };
            let _: () = unsafe { msg_send![sb_proxy, setFrame: target_sb_frame] };

            // Animate scroll view via raw msg_send on the animator proxy.
            // (The animator proxy is an opaque AnyObject, not a typed NSScrollView.)
            let sv_proxy: *const AnyObject = unsafe { msg_send![sv_ptr, animator] };
            let _: () = unsafe { msg_send![sv_proxy, setFrame: target_sv_frame] };
        });
        NSAnimationContext::runAnimationGroup_completionHandler(
            &animation_block,
            None::<&block2::DynBlock<dyn Fn()>>,
        );

        // Update find bar replace-row visibility based on new mode
        if self.ivars().find_bar_height.get() > 0.0 {
            let count = self.ivars().find_matches.borrow().len();
            self.update_find_bar_height_for_matches(count, new_mode);
        }
    }

    /// Open a file by path — used by both the Open dialog and Finder/Dock open events.
    ///
    /// If the file is already open in a tab, that tab is activated instead.
    /// If the active tab is a pristine empty document (no path, no content, not dirty),
    /// the file is loaded into it directly; otherwise a new tab is created first.
    fn open_file_by_path(&self, path: std::path::PathBuf) {
        // Check if already open → switch to that tab
        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(i) = tm.find_by_path(&path) {
                drop(tm);
                self.switch_to_tab(i);
                return;
            }
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("mdit: cannot read {:?}: {}", path, e);
                return;
            }
        };

        // Reuse the active tab if it's a pristine empty tab, otherwise create a new one.
        let reuse = {
            let tm = self.ivars().tab_manager.borrow();
            tm.active().is_some_and(|t| {
                !t.is_dirty.get()
                    && t.url.borrow().is_none()
                    && unsafe { t.text_view.textStorage() }
                        .is_none_or(|s| s.length() == 0)
            })
        };
        if !reuse {
            self.add_empty_tab();
        }
        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.active() {
                *t.url.borrow_mut() = Some(path.clone());
                t.is_dirty.set(false);
                unsafe {
                    if let Some(storage) = t.text_view.textStorage() {
                        let full = NSRange { location: 0, length: storage.length() };
                        storage.replaceCharactersInRange_withString(
                            full,
                            &NSString::from_str(&content),
                        );
                    }
                }
            }
        }
        if let Some(pb) = self.ivars().path_bar.get() {
            pb.update(Some(path.as_path()));
        }
        self.rebuild_tab_bar();
    }

    /// Active text view for formatting actions.
    fn active_text_view(&self) -> Option<Retained<NSTextView>> {
        let tm = self.ivars().tab_manager.borrow();
        tm.active().map(|t| t.text_view.clone())
    }

    /// Return the active text view, switching to Editor mode first if needed.
    ///
    /// All formatting actions call this instead of `active_text_view`, so that
    /// clicking a sidebar button while in Viewer mode automatically activates
    /// the editor before applying the format.
    fn editor_text_view(&self) -> Option<Retained<NSTextView>> {
        if !self.is_editor_mode() {
            self.toggle_mode();
        }
        self.active_text_view()
    }

    /// Rebuild tab bar buttons.
    fn rebuild_tab_bar(&self) {
        let Some(_win) = self.ivars().window.get() else {
            return;
        };
        let Some(tab_bar) = self.ivars().tab_bar.get() else {
            return;
        };
        let mtm = self.mtm();
        let target: &AnyObject = unsafe { &*(self as *const AppDelegate as *const AnyObject) };
        let labels = self.ivars().tab_manager.borrow().tab_labels();
        tab_bar.rebuild(mtm, &labels, target);
    }

    /// Switch to tab `index`.
    fn switch_to_tab(&self, index: usize) {
        // Reset find bar state on tab switch
        self.close_find_bar();

        let Some(win) = self.ivars().window.get() else {
            return;
        };
        let content = win.contentView().unwrap();

        // Remove old scroll view
        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.active() {
                t.scroll_view.removeFromSuperview();
            }
        }

        self.ivars().tab_manager.borrow_mut().switch_to(index);

        // Insert new scroll view
        let frame = self.content_frame();
        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.get(index) {
                t.scroll_view.setFrame(frame);
                content.addSubview(&t.scroll_view);
            }
        }

        // Snap sidebar to the new tab's mode without animation.
        if let Some(sb) = self.ivars().sidebar.get() {
            let new_tab_mode = self.ivars().tab_manager.borrow()
                .active()
                .map(|t| t.mode.get())
                .unwrap_or(ViewMode::Viewer);
            let bounds = win.contentView().unwrap().bounds();
            let content_h = (bounds.size.height - TAB_H - PATH_H).max(0.0);
            let sidebar_w = if new_tab_mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
            sb.set_size_direct(sidebar_w, content_h);
        }

        // Update path bar
        if let Some(pb) = self.ivars().path_bar.get() {
            let tm = self.ivars().tab_manager.borrow();
            let url = tm.get(index).and_then(|t| t.url.borrow().clone());
            pb.update(url.as_deref());
        }

        self.rebuild_tab_bar();
        self.update_text_container_inset();
    }

    /// Create a new empty tab and activate it.
    /// New tabs start in Viewer mode (non-editable).
    fn add_empty_tab(&self) {
        let mtm = self.mtm();
        let scheme = self.ivars().tab_manager.borrow().first_scheme()
            .unwrap_or_else(ColorScheme::light);
        let frame = self.content_frame();
        let tab = DocumentState::new_empty(mtm, scheme, frame);
        // Default to Viewer mode: non-editable.
        tab.text_view.setEditable(false);
        tab.text_view
            .setDelegate(Some(ProtocolObject::from_ref(self)));
        let new_idx = self.ivars().tab_manager.borrow_mut().add(tab);
        self.switch_to_tab(new_idx);
    }

    /// Switch the color scheme and immediately re-render all documents.
    fn apply_scheme(&self, scheme: ColorScheme) {
        let tm = self.ivars().tab_manager.borrow();
        let active = tm.active_index();
        for (i, tab) in tm.iter().enumerate() {
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
        drop(tm);
        if let Some(sb) = self.ivars().sidebar.get() {
            sb.apply_separator_color();
            let (r, g, b) = scheme.accent;
            sb.set_accent_color(r, g, b);
        }
        if let Some(tb) = self.ivars().tab_bar.get() {
            tb.apply_colors(Some(scheme.accent));
        }
    }

    /// Close tab at `index` — dirty-check, then remove (or clear if last).
    fn close_tab(&self, index: usize) {
        // Close find bar before closing tab
        self.close_find_bar();

        let (is_dirty, filename) = {
            let tm = self.ivars().tab_manager.borrow();
            let tab = match tm.get(index) {
                Some(t) => t,
                None => return,
            };
            let dirty = tab.is_dirty.get();
            let name = tab.url
                .borrow()
                .as_deref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".to_string());
            (dirty, name)
        };

        if is_dirty {
            match show_save_alert(&filename, self.mtm()) {
                SaveChoice::Save => self.perform_save(Some(index)),
                SaveChoice::DontSave => {}
                SaveChoice::Cancel => return,
            }
        }

        // Remove scroll view before mutating the manager.
        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.get(index) {
                t.scroll_view.removeFromSuperview();
            }
        }

        let result = self.ivars().tab_manager.borrow_mut().remove(index);

        match result {
            TabCloseResult::LastTab => {
                // Only tab — clear contents instead of removing.
                let tm = self.ivars().tab_manager.borrow();
                if let Some(t) = tm.active() {
                    unsafe {
                        if let Some(storage) = t.text_view.textStorage() {
                            let full = NSRange { location: 0, length: storage.length() };
                            let empty = NSString::from_str("");
                            storage.replaceCharactersInRange_withString(full, &empty);
                        }
                    }
                    *t.url.borrow_mut() = None;
                    t.is_dirty.set(false);
                    // Re-add the scroll view (we removed it above).
                    let content = self.ivars().window.get().unwrap().contentView().unwrap();
                    t.scroll_view.setFrame(self.content_frame());
                    content.addSubview(&t.scroll_view);
                    // Snap sidebar to the remaining tab's mode.
                    // Read the mode while t/tm is still in scope (no early return here).
                    let remaining_mode = t.mode.get();
                    if let (Some(sb), Some(win)) = (self.ivars().sidebar.get(), self.ivars().window.get()) {
                        let bounds = win.contentView().unwrap().bounds();
                        let content_h = (bounds.size.height - TAB_H - PATH_H).max(0.0);
                        let sidebar_w = if remaining_mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
                        sb.set_size_direct(sidebar_w, content_h);
                    }
                }
                drop(tm);
                self.rebuild_tab_bar();
                if let Some(pb) = self.ivars().path_bar.get() {
                    pb.update(None);
                }
            }
            TabCloseResult::Removed { new_active } => {
                self.switch_to_tab(new_active);
            }
        }
    }

    /// Save tab at `index`, or the active tab when `index` is `None`.
    ///
    /// If the tab has no associated path, an `NSSavePanel` is presented first.
    /// The `None`-index convention lets `saveDocument:` delegate here without
    /// needing to resolve the active index at the call site.
    fn perform_save(&self, index: Option<usize>) {
        let idx = index.unwrap_or_else(|| self.ivars().tab_manager.borrow().active_index());

        // Determine path (or open NSSavePanel)
        let existing_url: Option<std::path::PathBuf> = {
            let tm = self.ivars().tab_manager.borrow();
            match tm.get(idx) {
                None => return,
                Some(t) => t.url.borrow().clone(),
            }
        };
        let path: std::path::PathBuf = match existing_url {
            Some(p) => p,
            None => match self.run_save_panel() {
                Some(p) => p,
                None => return,
            },
        };

        // Read content from TextStorage
        let content = {
            let tm = self.ivars().tab_manager.borrow();
            let tab = match tm.get(idx) {
                Some(t) => t,
                None => return,
            };
            unsafe { tab.text_view.textStorage() }
                .map(|s| s.string().to_string())
                .unwrap_or_default()
        };

        // Write to disk
        if let Err(e) = std::fs::write(&path, content.as_bytes()) {
            eprintln!("mdit: cannot save {:?}: {}", path, e);
            return;
        }

        // Update state
        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.get(idx) {
                *t.url.borrow_mut() = Some(path.clone());
                t.is_dirty.set(false);
            }
        }
        if let Some(pb) = self.ivars().path_bar.get() {
            if idx == self.ivars().tab_manager.borrow().active_index() {
                pb.update(Some(path.as_path()));
            }
        }
        self.rebuild_tab_bar();
    }

    fn run_save_panel(&self) -> Option<std::path::PathBuf> {
        use objc2_app_kit::NSSavePanel;
        let panel = NSSavePanel::savePanel(self.mtm());
        panel.setNameFieldStringValue(&NSString::from_str("Untitled.md"));
        let response = panel.runModal();
        if response != 1 {
            return None;
        } // NSModalResponseOK = 1
        let ns_url = panel.URL()?;
        let ns_path = ns_url.path()?;
        Some(std::path::PathBuf::from(ns_path.to_string()))
    }

    /// Compute and apply the horizontal text container inset for the active tab.
    ///
    /// Centres the text column at up to 700 pt wide with a minimum 40 pt margin
    /// on each side: `inset = max(40, (editor_width − 700) / 2)`.
    fn update_text_container_inset(&self) {
        let Some(win) = self.ivars().window.get() else {
            return;
        };
        let effective_sidebar_w = if self.is_editor_mode() { SIDEBAR_W } else { 0.0 };
        let editor_width = (win.frame().size.width - effective_sidebar_w).max(0.0);
        let max_text_width = 700.0_f64;
        let min_padding = 40.0_f64;
        let h_inset = if editor_width > max_text_width + 2.0 * min_padding {
            (editor_width - max_text_width) / 2.0
        } else {
            min_padding
        };
        let tm = self.ivars().tab_manager.borrow();
        if let Some(t) = tm.active() {
            t.text_view
                .setTextContainerInset(NSSize::new(h_inset, 40.0));
        }
    }

    // ── Find bar private methods ──────────────────────────────────────────

    /// Open the find bar (or re-focus it if already visible).
    fn open_find_bar(&self) {
        let Some(fb) = self.ivars().find_bar.get() else { return };
        let Some(win) = self.ivars().window.get() else { return };
        let w = win.contentView().unwrap().bounds().size.width;
        let h = FIND_H_COMPACT;
        self.ivars().find_bar_height.set(h);
        fb.view().setFrame(NSRect::new(NSPoint::new(0.0, PATH_H), NSSize::new(w, h)));
        fb.set_height(h);
        fb.show();
        // Resize scroll view to make room
        let frame = self.content_frame();
        let tm = self.ivars().tab_manager.borrow();
        if let Some(t) = tm.active() {
            t.scroll_view.setFrame(frame);
        }
        drop(tm);
        // Focus search field and run search if there's already a query
        fb.focus_search();
        self.perform_search();
    }

    /// Run a search against the active tab's text and update highlights + count.
    fn perform_search(&self) {
        let Some(fb) = self.ivars().find_bar.get() else { return };
        // Bar must be visible
        if self.ivars().find_bar_height.get() == 0.0 { return; }

        let query = fb.search_text();

        // Get active tab text
        let (storage, tab_mode) = {
            let tm = self.ivars().tab_manager.borrow();
            let Some(tab) = tm.active() else { return };
            let storage = unsafe { tab.text_view.textStorage() };
            (storage, tab.mode.get())
        };
        let Some(storage) = storage else { return };

        // Remove previous highlights from old match ranges
        let old_matches: Vec<NSRange> = self.ivars().find_matches.borrow().clone();
        for range in &old_matches {
            unsafe {
                storage.removeAttribute_range(
                    NSBackgroundColorAttributeName,
                    *range,
                );
            }
        }

        if query.is_empty() {
            // Clear state
            *self.ivars().find_matches.borrow_mut() = Vec::new();
            self.ivars().find_current.set(0);
            fb.update_count(0, 0);
            fb.set_no_match(false);
            // Update replace row visibility
            self.update_find_bar_height_for_matches(0, tab_mode);
            return;
        }

        // Find all matches
        let ns_query = NSString::from_str(&query);
        let case_sensitive = fb.is_case_sensitive();
        let matches = find_all_ranges(&storage.string(), &ns_query, !case_sensitive);
        let count = matches.len();

        // Clamp current index
        let current = self.ivars().find_current.get().min(count.saturating_sub(1));
        self.ivars().find_current.set(current);
        *self.ivars().find_matches.borrow_mut() = matches.clone();

        // Apply highlight attributes
        let all_match_color = NSColor::colorWithRed_green_blue_alpha(1.0, 0.97, 0.82, 1.0);
        let current_match_color = NSColor::colorWithRed_green_blue_alpha(1.0, 0.93, 0.70, 1.0);
        for (i, &range) in matches.iter().enumerate() {
            let color = if i == current { &current_match_color } else { &all_match_color };
            unsafe {
                storage.addAttribute_value_range(
                    NSBackgroundColorAttributeName,
                    color,
                    range,
                );
            }
        }

        // Update count label and no-match styling
        if count > 0 {
            fb.update_count(current + 1, count);
            fb.set_no_match(false);
            self.scroll_to_current_match();
        } else {
            fb.update_count(0, 0);
            fb.set_no_match(true);
        }

        // Show/hide replace row based on matches + mode
        self.update_find_bar_height_for_matches(count, tab_mode);
    }

    /// Show or hide the replace row, updating bar height and scroll view frame as needed.
    fn update_find_bar_height_for_matches(&self, match_count: usize, mode: ViewMode) {
        let Some(fb) = self.ivars().find_bar.get() else { return };
        let Some(win) = self.ivars().window.get() else { return };
        let w = win.contentView().unwrap().bounds().size.width;

        let show_replace = match_count > 0 && mode == ViewMode::Editor;
        fb.show_replace_row(show_replace);

        let new_h = if show_replace { FIND_H_EXPANDED } else { FIND_H_COMPACT };
        let old_h = self.ivars().find_bar_height.get();
        if (new_h - old_h).abs() > 0.5 && old_h > 0.0 {
            self.ivars().find_bar_height.set(new_h);
            fb.set_height(new_h);
            fb.view().setFrame(NSRect::new(NSPoint::new(0.0, PATH_H), NSSize::new(w, new_h)));
            let frame = self.content_frame();
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.active() {
                t.scroll_view.setFrame(frame);
            }
        }
    }

    /// Scroll the text view so the current match is visible and select it.
    fn scroll_to_current_match(&self) {
        let matches = self.ivars().find_matches.borrow();
        let current = self.ivars().find_current.get();
        let Some(&range) = matches.get(current) else { return };
        drop(matches);
        let tm = self.ivars().tab_manager.borrow();
        let Some(tab) = tm.active() else { return };
        unsafe {
            let _: () = msg_send![&*tab.text_view, scrollRangeToVisible: range];
            let _: () = msg_send![&*tab.text_view, setSelectedRange: range];
        }
    }

    /// Highlight the current match (update background colors).
    fn highlight_current_match(&self) {
        let storage_opt = {
            let tm = self.ivars().tab_manager.borrow();
            tm.active().and_then(|t| unsafe { t.text_view.textStorage() })
        };
        let Some(storage) = storage_opt else { return };

        let matches = self.ivars().find_matches.borrow().clone();
        let current = self.ivars().find_current.get();
        let all_color = NSColor::colorWithRed_green_blue_alpha(1.0, 0.97, 0.82, 1.0);
        let current_color = NSColor::colorWithRed_green_blue_alpha(1.0, 0.93, 0.70, 1.0);
        for (i, &range) in matches.iter().enumerate() {
            let color = if i == current { &current_color } else { &all_color };
            unsafe {
                storage.addAttribute_value_range(
                    NSBackgroundColorAttributeName,
                    color,
                    range,
                );
            }
        }
    }

    /// Close the find bar, remove highlights, and restore the scroll view.
    fn close_find_bar(&self) {
        let Some(fb) = self.ivars().find_bar.get() else { return };
        if self.ivars().find_bar_height.get() == 0.0 { return; }

        // Remove all highlights
        let matches: Vec<NSRange> = self.ivars().find_matches.borrow().clone();
        if !matches.is_empty() {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(tab) = tm.active() {
                if let Some(storage) = unsafe { tab.text_view.textStorage() } {
                    for &range in &matches {
                        unsafe {
                            storage.removeAttribute_range(
                                NSBackgroundColorAttributeName,
                                range,
                            );
                        }
                    }
                    // Re-render to restore legitimate highlight colors
                    tab.editor_delegate.reapply(&storage);
                }
            }
        }

        *self.ivars().find_matches.borrow_mut() = Vec::new();
        self.ivars().find_current.set(0);
        self.ivars().find_bar_height.set(0.0);
        fb.hide();
        fb.show_replace_row(false);
        fb.update_count(0, 0);
        fb.set_no_match(false);

        // Restore scroll view to full height
        let frame = self.content_frame();
        let tm = self.ivars().tab_manager.borrow();
        if let Some(t) = tm.active() {
            t.scroll_view.setFrame(frame);
            // Return focus to text view
            if let Some(win) = self.ivars().window.get() {
                unsafe { let _: () = msg_send![&**win, makeFirstResponder: &*t.text_view]; }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Dirty-check dialog
// ---------------------------------------------------------------------------

enum SaveChoice {
    Save,
    DontSave,
    Cancel,
}

fn show_save_alert(filename: &str, mtm: MainThreadMarker) -> SaveChoice {
    use objc2_app_kit::NSAlert;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(&format!(
        "Do you want to save changes to \"{}\"?",
        filename
    )));
    alert.setInformativeText(&NSString::from_str(
        "Your changes will be lost if you don't save them.",
    ));
    alert.addButtonWithTitle(&NSString::from_str("Save")); // 1000
    alert.addButtonWithTitle(&NSString::from_str("Don't Save")); // 1001
    alert.addButtonWithTitle(&NSString::from_str("Cancel")); // 1002
    let response = alert.runModal();
    match response {
        1000 => SaveChoice::Save,
        1001 => SaveChoice::DontSave,
        _ => SaveChoice::Cancel,
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Toggle an inline marker around the current selection.
///
/// Delegates to the pure `compute_inline_toggle()` for the string logic,
/// then applies the result to the NSTextView.
fn toggle_inline_wrap(tv: &NSTextView, marker: &str) {
    let range: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else {
        return;
    };
    let full_str = storage.string();
    let full_len = full_str.length();

    let selected = full_str.substringWithRange(range).to_string();

    // Grab a few characters on each side for marker detection.
    const MAX_MARKERS: usize = 6;
    let before_start = range.location.saturating_sub(MAX_MARKERS);
    let after_end = (range.location + range.length + MAX_MARKERS).min(full_len);

    let before = full_str
        .substringWithRange(NSRange { location: before_start, length: range.location - before_start })
        .to_string();
    let after = full_str
        .substringWithRange(NSRange {
            location: range.location + range.length,
            length: after_end - (range.location + range.length),
        })
        .to_string();

    let result = mdit::editor::formatting::compute_inline_toggle(&selected, &before, &after, marker);

    let replace_range = NSRange {
        location: range.location - result.consumed_before,
        length: result.consumed_before + range.length + result.consumed_after,
    };
    let ns = NSString::from_str(&result.replacement);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: replace_range] }
}

/// Replace the current NSTextView selection with `prefix + selected + suffix`.
fn insert_link_wrap(tv: &NSTextView, prefix: &str, suffix: &str) {
    let range: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else {
        return;
    };
    let selected = storage.string().substringWithRange(range).to_string();
    let text = mdit::editor::formatting::compute_link_wrap(&selected, prefix, suffix);
    let ns = NSString::from_str(&text);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: range] }
}

/// Apply a block-level format to the line containing the caret.
///
/// Uses the pure `set_block_format()` under the hood: same prefix toggles
/// off, different prefix switches, empty prefix strips (Normal).
fn apply_block_format(tv: &NSTextView, desired_prefix: &str) {
    let caret: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else {
        return;
    };
    let ns_str = storage.string();
    let point = NSRange { location: caret.location, length: 0 };
    let line_range: NSRange = ns_str.lineRangeForRange(point);
    let line_text = ns_str.substringWithRange(line_range).to_string();

    let new_line = mdit::editor::formatting::set_block_format(&line_text, desired_prefix);
    let ns = NSString::from_str(&new_line);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: line_range] }
}

/// Wrap the current selection in a fenced code block.
fn insert_code_block(tv: &NSTextView) {
    let range: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else {
        return;
    };
    let selected = storage.string().substringWithRange(range).to_string();
    let text = mdit::editor::formatting::compute_code_block_wrap(&selected);
    let ns = NSString::from_str(&text);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: range] }
}

// ---------------------------------------------------------------------------
// Find‐all helper
// ---------------------------------------------------------------------------

/// Find all occurrences of `query` in `text`, returning NSRange for each match.
/// Uses NSString's rangeOfString:options:range: for proper Unicode + UTF-16 handling.
fn find_all_ranges(text: &NSString, query: &NSString, case_insensitive: bool) -> Vec<NSRange> {
    let mut ranges = Vec::new();
    let len = text.length();
    if len == 0 || query.length() == 0 { return ranges; }
    let options: usize = if case_insensitive { 1 } else { 0 }; // NSCaseInsensitiveSearch = 1
    let mut search_from = NSRange { location: 0, length: len };
    loop {
        let found: NSRange = unsafe {
            msg_send![text, rangeOfString: query, options: options, range: search_from]
        };
        if found.location >= usize::MAX / 2 { break; } // NSNotFound
        ranges.push(found);
        let next_loc = found.location + found.length.max(1);
        if next_loc >= len { break; }
        search_from = NSRange { location: next_loc, length: len - next_loc };
    }
    ranges
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
        let names = NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]);
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
// Titlebar accessory (eye toggle + ellipsis)
// ---------------------------------------------------------------------------

/// Add an Eye (toggle mode) button and an ellipsis button to the right side
/// of the macOS title bar using `NSTitlebarAccessoryViewController`.
fn add_titlebar_accessory(window: &NSWindow, mtm: MainThreadMarker, target: &AnyObject) {
    let btn_h = 20.0_f64;
    let btn_w = 26.0_f64;
    let acc_h = 28.0_f64;
    let gap = 2.0_f64;
    let total_w = btn_w * 2.0 + gap + 4.0;
    let v_off = (acc_h - btn_h) / 2.0;

    let acc_view = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(total_w, acc_h)),
    );

    // Eye button — toggles Viewer/Editor mode.
    let eye_btn = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(2.0, v_off), NSSize::new(btn_w, btn_h)),
    );
    eye_btn.setBezelStyle(NSBezelStyle::AccessoryBarAction);
    eye_btn.setBordered(false);
    let eye_name = NSString::from_str("eye");
    if let Some(img) = NSImage::imageWithSystemSymbolName_accessibilityDescription(&eye_name, None) {
        eye_btn.setImage(Some(&img));
    }
    eye_btn.setTitle(&NSString::from_str(""));
    unsafe {
        NSControl::setTarget(&eye_btn, Some(target));
        NSControl::setAction(&eye_btn, Some(sel!(toggleMode:)));
        let _: () = msg_send![&*eye_btn, setToolTip: &*NSString::from_str("Toggle Editor (⌘E)")];
    }
    acc_view.addSubview(&eye_btn);

    // Ellipsis button — placeholder for future options.
    let more_btn = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(2.0 + btn_w + gap, v_off), NSSize::new(btn_w, btn_h)),
    );
    more_btn.setBezelStyle(NSBezelStyle::AccessoryBarAction);
    more_btn.setBordered(false);
    let ellipsis_name = NSString::from_str("ellipsis");
    if let Some(img) = NSImage::imageWithSystemSymbolName_accessibilityDescription(&ellipsis_name, None) {
        more_btn.setImage(Some(&img));
    }
    more_btn.setTitle(&NSString::from_str(""));
    acc_view.addSubview(&more_btn);

    // NSTitlebarAccessoryViewController — set layoutAttribute to .trailing (12).
    let Some(vc_cls) = AnyClass::get(c"NSTitlebarAccessoryViewController") else { return };
    unsafe {
        let alloc: *mut AnyObject = msg_send![vc_cls, alloc];
        let vc: *mut AnyObject = msg_send![alloc, init];
        if vc.is_null() { return; }
        let vc_ret = Retained::retain(vc).expect("NSTitlebarAccessoryViewController");
        let _: () = msg_send![&*vc_ret, setView: &*acc_view];
        let _: () = msg_send![&*vc_ret, setLayoutAttribute: 2isize];  // NSLayoutAttributeRight
        let _: () = msg_send![window, addTitlebarAccessoryViewController: &*vc_ret];
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

#[cfg(test)]
mod tests {
    use super::*;
    use mdit::editor::view_mode::ViewMode;

    #[test]
    fn sidebar_frame_viewer_is_zero_width() {
        let f = sidebar_target_frame(ViewMode::Viewer, 500.0);
        assert_eq!(f.size.width, 0.0);
        assert_eq!(f.size.height, 500.0);
        assert_eq!(f.origin.y, PATH_H);
    }

    #[test]
    fn sidebar_frame_editor_is_sidebar_w() {
        let f = sidebar_target_frame(ViewMode::Editor, 500.0);
        assert_eq!(f.size.width, SIDEBAR_W);
        assert_eq!(f.size.height, 500.0);
    }

    #[test]
    fn content_frame_viewer_starts_at_zero() {
        let f = content_target_frame(ViewMode::Viewer, 0.0, 1000.0, 700.0);
        assert_eq!(f.origin.x, 0.0);
        assert_eq!(f.size.width, 1000.0);
    }

    #[test]
    fn content_frame_editor_offset_by_sidebar() {
        let f = content_target_frame(ViewMode::Editor, 0.0, 1000.0, 700.0);
        assert_eq!(f.origin.x, SIDEBAR_W);
        assert_eq!(f.size.width, 1000.0 - SIDEBAR_W);
    }

    #[test]
    fn content_frame_height_excludes_bars() {
        let f = content_target_frame(ViewMode::Viewer, 0.0, 800.0, 700.0);
        assert_eq!(f.origin.y, PATH_H);
        assert_eq!(f.size.height, 700.0 - TAB_H - PATH_H);
    }

    #[test]
    fn content_frame_with_find_bar_offset() {
        let f = content_target_frame(ViewMode::Viewer, FIND_H_COMPACT, 800.0, 700.0);
        assert_eq!(f.origin.y, PATH_H + FIND_H_COMPACT);
        assert_eq!(f.size.height, 700.0 - TAB_H - PATH_H - FIND_H_COMPACT);
    }
}
