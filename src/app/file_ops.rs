use std::path::PathBuf;

use objc2::{DefinedClass, MainThreadOnly};
use objc2_foundation::{NSRange, NSString};

use super::AppDelegate;

impl AppDelegate {
    /// Open a file by path — used by both the Open dialog and Finder/Dock open events.
    ///
    /// If the file is already open in a tab, that tab is activated instead.
    /// If the active tab is a pristine empty document (no path, no content, not dirty),
    /// the file is loaded into it directly; otherwise a new tab is created first.
    pub(super) fn open_file_by_path(&self, path: PathBuf) {
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
            // Pre-compute word count so it's ready when editor mode is toggled.
            let tm = self.ivars().tab_manager.borrow();
            if let Some(t) = tm.active() {
                if let Some(storage) = unsafe { t.text_view.textStorage() } {
                    pb.update_wordcount(&storage.string().to_string());
                }
            }
        }
        self.rebuild_tab_bar();
        self.update_welcome_visibility();
    }

    /// Save tab at `index`, or the active tab when `index` is `None`.
    ///
    /// If the tab has no associated path, an `NSSavePanel` is presented first.
    pub(super) fn perform_save(&self, index: Option<usize>) {
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

    pub(super) fn run_save_panel(&self) -> Option<std::path::PathBuf> {
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
}
