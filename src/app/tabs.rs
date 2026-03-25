use objc2::{DefinedClass, MainThreadOnly};
use objc2::runtime::AnyObject;
use objc2::runtime::ProtocolObject;

use mdit::editor::document_state::DocumentState;
use mdit::editor::tab_manager::TabCloseResult;
use mdit::editor::view_mode::ViewMode;
use mdit::ui::appearance::ColorScheme;
use mdit::ui::sidebar::SIDEBAR_W;

use super::helpers::{SaveChoice, show_save_alert};
use super::{AppDelegate, TAB_H, PATH_H};

impl AppDelegate {
    /// Rebuild tab bar buttons.
    pub(super) fn rebuild_tab_bar(&self) {
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
    pub(super) fn switch_to_tab(&self, index: usize) {
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
            let (url, wordcount, is_editor) = {
                let tm = self.ivars().tab_manager.borrow();
                let url = tm.get(index).and_then(|t| t.url.borrow().clone());
                let wordcount = tm.get(index).and_then(|t| {
                    unsafe { t.text_view.textStorage() }
                        .map(|s| s.string().to_string())
                });
                let is_editor = tm
                    .get(index)
                    .map(|t| t.mode.get() == ViewMode::Editor)
                    .unwrap_or(false);
                (url, wordcount, is_editor)
            };
            pb.update(url.as_deref());
            if let Some(text) = wordcount {
                pb.update_wordcount(&text);
            }
            let win_w = win.contentView().unwrap().bounds().size.width;
            pb.set_wordcount_visible(is_editor, win_w);
        }

        self.rebuild_tab_bar();
        self.update_text_container_inset();
        self.update_welcome_visibility();
    }

    /// Create a new empty tab and activate it.
    /// New tabs start in Viewer mode (non-editable).
    pub(super) fn add_empty_tab(&self) {
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
        let font_size = self.ivars().prefs.font_size();
        {
            let tm = self.ivars().tab_manager.borrow();
            if let Some(tab) = tm.get(new_idx) {
                tab.editor_delegate.set_base_size(font_size);
            }
        }
        self.switch_to_tab(new_idx);
        self.update_welcome_visibility();
    }

    /// Close tab at `index` — dirty-check, then remove (or clear if last).
    pub(super) fn close_tab(&self, index: usize) {
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
                            let full = objc2_foundation::NSRange { location: 0, length: storage.length() };
                            let empty = objc2_foundation::NSString::from_str("");
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
                self.update_welcome_visibility();
            }
            TabCloseResult::Removed { new_active } => {
                self.switch_to_tab(new_active);
            }
        }
    }
}
