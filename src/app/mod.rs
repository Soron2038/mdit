mod find;
mod preferences;
mod file_ops;
mod tabs;
mod mode;
pub(crate) mod helpers;

use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSColor, NSTextDelegate, NSTextView, NSTextViewDelegate,
    NSWindowDelegate,
};
use objc2_foundation::{
    MainThreadMarker, NSNotification, NSObject, NSObjectProtocol, NSPoint,
    NSRange, NSRect, NSSize, NSString,
};

use mdit::editor::tab_manager::TabManager;
use mdit::editor::view_mode::ViewMode;
use mdit::menu::build_main_menu;
use mdit::ui::appearance::{ColorScheme, ThemePreference};
use mdit::ui::find_bar::FindBar;
use mdit::ui::path_bar::PathBar;
use mdit::ui::sidebar::{FormattingSidebar, SIDEBAR_W};
use mdit::ui::tab_bar::TabBar;
use mdit::ui::welcome_overlay::WelcomeOverlay;

use find::{FindCoordinator, Direction};
use preferences::Preferences;
use helpers::*;

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
pub(super) struct AppDelegateIvars {
    pub(super) window: OnceCell<Retained<objc2_app_kit::NSWindow>>,
    pub(super) sidebar: OnceCell<FormattingSidebar>,
    pub(super) tab_bar: OnceCell<TabBar>,
    pub(super) path_bar: OnceCell<PathBar>,
    pub(super) tab_manager: RefCell<TabManager>,
    /// File path received via `application:openFile:` before the window exists.
    pub(super) pending_open: RefCell<Option<PathBuf>>,
    // ── Find bar state ───────────────────────────────────────────────────
    pub(super) find_bar: OnceCell<FindBar>,
    pub(super) find: FindCoordinator,
    // ── Preferences ──────────────────────────────────────────────────────
    pub(super) prefs: Preferences,
    pub(super) welcome_overlay: OnceCell<WelcomeOverlay>,
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

            let loaded = Preferences::load();
            self.ivars().prefs.set_theme_no_persist(loaded.theme());
            self.ivars().prefs.set_font_size_no_persist(loaded.font_size());
            let pref = loaded.theme();
            let system_is_dark = detect_is_dark(&app);
            let initial_scheme = pref.resolve(system_is_dark);

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
                let fh = self.ivars().find.bar_height();
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
            if let Some(overlay) = self.ivars().welcome_overlay.get() {
                overlay.set_frame(frame);
            }
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

        #[unsafe(method(applyUnderline:))]
        fn apply_underline(&self, _sender: &AnyObject) { self.dispatch_inline_format("__"); }

        // ── Appearance ─────────────────────────────────────────────────────

        #[unsafe(method(applyLightMode:))]
        fn apply_light_mode(&self, _sender: &AnyObject) {
            self.ivars().prefs.set_theme(ThemePreference::Light);
            self.apply_scheme(ColorScheme::light());
        }

        #[unsafe(method(applyDarkMode:))]
        fn apply_dark_mode(&self, _sender: &AnyObject) {
            self.ivars().prefs.set_theme(ThemePreference::Dark);
            self.apply_scheme(ColorScheme::dark());
        }

        #[unsafe(method(applySystemMode:))]
        fn apply_system_mode(&self, _sender: &AnyObject) {
            self.ivars().prefs.set_theme(ThemePreference::System);
            let app = NSApplication::sharedApplication(self.mtm());
            let scheme = ThemePreference::System.resolve(detect_is_dark(&app));
            self.apply_scheme(scheme);
        }

        // ── Font size ──────────────────────────────────────────────────────────

        #[unsafe(method(increaseFontSize:))]
        fn increase_font_size_action(&self, _sender: &AnyObject) {
            let new_size = (self.ivars().prefs.font_size() + 1.0).min(preferences::MAX_FONT_SIZE);
            self.apply_font_size(new_size);
        }

        #[unsafe(method(decreaseFontSize:))]
        fn decrease_font_size_action(&self, _sender: &AnyObject) {
            let new_size = (self.ivars().prefs.font_size() - 1.0).max(preferences::MIN_FONT_SIZE);
            self.apply_font_size(new_size);
        }

        #[unsafe(method(resetFontSize:))]
        fn reset_font_size_action(&self, _sender: &AnyObject) {
            self.apply_font_size(preferences::DEFAULT_FONT_SIZE);
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

        #[unsafe(method(applyBulletList:))]
        fn apply_bullet_list(&self, _sender: &AnyObject) { self.dispatch_block_format("- "); }

        #[unsafe(method(applyNumberedList:))]
        fn apply_numbered_list(&self, _sender: &AnyObject) { self.dispatch_block_format("1. "); }

        #[unsafe(method(applyTaskList:))]
        fn apply_task_list(&self, _sender: &AnyObject) { self.dispatch_block_format("- [ ] "); }

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
            if !self.ivars().find.is_open() {
                self.open_find_bar();
                return;
            }
            let Some(fb) = self.ivars().find_bar.get() else { return };
            let tm = self.ivars().tab_manager.borrow();
            let Some(tab) = tm.active() else { return };
            self.ivars().find.navigate(Direction::Next, fb, &tab.text_view);
        }

        #[unsafe(method(findPrevious:))]
        fn find_previous_action(&self, _sender: &AnyObject) {
            let Some(fb) = self.ivars().find_bar.get() else { return };
            let tm = self.ivars().tab_manager.borrow();
            let Some(tab) = tm.active() else { return };
            self.ivars().find.navigate(Direction::Previous, fb, &tab.text_view);
        }

        #[unsafe(method(findBarToggleAa:))]
        fn find_bar_toggle_aa(&self, _sender: &AnyObject) {
            if let Some(fb) = self.ivars().find_bar.get() {
                fb.toggle_case_sensitive();
            }
            self.perform_find_search();
        }

        #[unsafe(method(replaceOne:))]
        fn replace_one_action(&self, _sender: &AnyObject) {
            let Some(fb) = self.ivars().find_bar.get() else { return };
            let (tv, tab_mode) = {
                let tm = self.ivars().tab_manager.borrow();
                let Some(tab) = tm.active() else { return };
                (tab.text_view.clone(), tab.mode.get())
            };
            self.ivars().find.replace_one(fb, &tv, tab_mode);
        }

        #[unsafe(method(replaceAll:))]
        fn replace_all_action(&self, _sender: &AnyObject) {
            let Some(fb) = self.ivars().find_bar.get() else { return };
            let (tv, tab_mode) = {
                let tm = self.ivars().tab_manager.borrow();
                let Some(tab) = tm.active() else { return };
                (tab.text_view.clone(), tab.mode.get())
            };
            self.ivars().find.replace_all(fb, &tv, tab_mode);
        }

        // ── Live search delegate ──────────────────────────────────────────

        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, _notification: &NSNotification) {
            self.perform_find_search();
        }

        /// Called when the search field receives a command (e.g. Escape → cancelOperation:).
        /// Returning true means the command was handled; false lets it propagate.
        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn control_text_view_do_command(
            &self,
            _control: &AnyObject,
            _text_view: &AnyObject,
            selector: objc2::runtime::Sel,
        ) -> bool {
            if selector == objc2::sel!(cancelOperation:) {
                self.close_find_bar();
                return true.into();
            }
            false.into()
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
            // Update word/char count in path bar on every text change.
            if let Some(pb) = self.ivars().path_bar.get() {
                let tm = self.ivars().tab_manager.borrow();
                if let Some(t) = tm.active() {
                    if let Some(storage) = unsafe { t.text_view.textStorage() } {
                        pb.update_wordcount(&storage.string().to_string());
                    }
                }
            }
            self.update_welcome_visibility();
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

        // Welcome overlay — shown in empty documents, positioned above scroll view.
        let welcome_overlay = WelcomeOverlay::new(mtm, content_target_frame(
            ViewMode::Viewer, 0.0, w, h,
        ));
        content.addSubview(welcome_overlay.view());

        let _ = self.ivars().tab_bar.set(tab_bar);
        let _ = self.ivars().path_bar.set(path_bar);
        let _ = self.ivars().sidebar.set(sidebar);
        let _ = self.ivars().find_bar.set(find_bar);
        let _ = self.ivars().welcome_overlay.set(welcome_overlay);
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
        let find_offset = self.ivars().find.bar_height();
        content_target_frame(mode, find_offset, bounds.size.width, bounds.size.height)
    }

    /// Returns true if the active tab is in Editor mode.
    fn is_editor_mode(&self) -> bool {
        let tm = self.ivars().tab_manager.borrow();
        tm.active().map(|t| t.mode.get() == ViewMode::Editor).unwrap_or(false)
    }

    /// Show the welcome overlay when the active document is empty; hide otherwise.
    /// Also updates the frame (sidebar offset may have changed) and mode-dependent hint text.
    /// When showing, brings the overlay to the front of the z-order so it sits above
    /// the scroll view (which is re-added as a subview on every tab switch).
    fn update_welcome_visibility(&self) {
        let Some(overlay) = self.ivars().welcome_overlay.get() else { return };
        // Borrow tab_manager, extract what we need, then drop before calling content_frame().
        let (is_empty, mode) = {
            let tm = self.ivars().tab_manager.borrow();
            let Some(tab) = tm.active() else { return };
            let is_empty = unsafe { tab.text_view.textStorage() }
                .is_none_or(|s| s.length() == 0);
            let mode = tab.mode.get();
            (is_empty, mode)
        };
        overlay.set_visible(is_empty);
        overlay.update_mode(mode);
        overlay.set_frame(self.content_frame());
        // Ensure overlay is above the scroll view in the z-order.
        if is_empty {
            if let Some(win) = self.ivars().window.get() {
                let content = win.contentView().unwrap();
                let view = overlay.view();
                view.removeFromSuperview();
                content.addSubview(view);
            }
        }
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

    /// Apply a new base font size to all open tabs and persist it.
    fn apply_font_size(&self, size: f64) {
        self.ivars().prefs.set_font_size(size);

        let tm = self.ivars().tab_manager.borrow();
        for tab in tm.iter() {
            tab.editor_delegate.set_base_size(size);
            if let Some(storage) = unsafe { tab.text_view.textStorage() } {
                tab.editor_delegate.reapply(&storage);
            }
        }
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
        self.ivars().find.open_bar(fb, w);
        // Resize scroll view to make room
        let frame = self.content_frame();
        let tm = self.ivars().tab_manager.borrow();
        if let Some(t) = tm.active() {
            t.scroll_view.setFrame(frame);
        }
        drop(tm);
        self.perform_find_search();
    }

    /// Run a search against the active tab's text and update highlights + count.
    fn perform_find_search(&self) {
        let Some(fb) = self.ivars().find_bar.get() else { return };
        if !self.ivars().find.is_open() { return; }
        let (tv, tab_mode) = {
            let tm = self.ivars().tab_manager.borrow();
            let Some(tab) = tm.active() else { return };
            (tab.text_view.clone(), tab.mode.get())
        };
        self.ivars().find.perform_search(fb, &tv, tab_mode);
        // After search, update bar height frame if needed
        let new_h = self.ivars().find.bar_height();
        if let Some(win) = self.ivars().window.get() {
            let w = win.contentView().unwrap().bounds().size.width;
            fb.view().setFrame(NSRect::new(
                NSPoint::new(0.0, PATH_H),
                NSSize::new(w, new_h),
            ));
            let frame = self.content_frame();
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.active() {
                t.scroll_view.setFrame(frame);
            }
        }
    }

    /// Close the find bar, remove highlights, and restore the scroll view.
    fn close_find_bar(&self) {
        let Some(fb) = self.ivars().find_bar.get() else { return };
        let tm = self.ivars().tab_manager.borrow();
        let (tv, ed) = tm.active()
            .map(|t| (Some(t.text_view.clone()), Some(t.editor_delegate.clone())))
            .unwrap_or((None, None));
        drop(tm);
        self.ivars().find.close(fb, tv.as_deref(), ed.as_deref());
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
    use mdit::ui::find_bar::FIND_H_COMPACT;

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
