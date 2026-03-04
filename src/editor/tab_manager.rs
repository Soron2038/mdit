//! Tab management: encapsulates the list of open documents and the active
//! tab index.  Pure index-correction logic is factored into standalone
//! functions for unit testing without AppKit.

use std::path::Path;

use super::document_state::DocumentState;
use crate::ui::tab_bar::tab_label;

// ---------------------------------------------------------------------------
// Pure helper (testable without AppKit)
// ---------------------------------------------------------------------------

/// Compute the new active index after removing the tab at `closed`.
///
/// Rules:
/// - If `closed` is before or at the active index, shift active left (min 0).
/// - Otherwise, keep active but clamp to `count - 2` (new length after removal).
///
/// Panics if `count == 0` (cannot close from an empty list).
pub fn active_index_after_close(count: usize, active: usize, closed: usize) -> usize {
    assert!(count > 0, "cannot compute close index for empty tab list");
    let new_len = count - 1;
    if new_len == 0 {
        return 0;
    }
    if closed <= active && active > 0 {
        active - 1
    } else {
        active.min(new_len - 1)
    }
}

// ---------------------------------------------------------------------------
// TabManager
// ---------------------------------------------------------------------------

/// Result of removing a tab from the manager.
pub enum TabCloseResult {
    /// The last tab was "closed" — contents should be cleared but the tab
    /// remains (editor always has at least one tab).
    LastTab,
    /// Tab was removed; `new_active` is the index to switch to.
    Removed { new_active: usize },
}

/// Owns the list of open document tabs and the active-tab index.
///
/// Replaces the separate `RefCell<Vec<DocumentState>>` + `Cell<usize>` pair
/// that previously lived in `AppDelegateIvars`, providing a single coherent
/// borrow point and encapsulated index management.
#[derive(Default)]
pub struct TabManager {
    tabs: Vec<DocumentState>,
    active: usize,
}

impl TabManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tab and return its index (always appended at the end).
    pub fn add(&mut self, tab: DocumentState) -> usize {
        self.tabs.push(tab);
        self.tabs.len() - 1
    }

    /// Remove the tab at `index` and return the close result.
    ///
    /// If this is the last tab, returns `LastTab` (caller should clear it
    /// instead of removing).  Otherwise removes the tab and computes the
    /// new active index.
    pub fn remove(&mut self, index: usize) -> TabCloseResult {
        if self.tabs.len() <= 1 {
            return TabCloseResult::LastTab;
        }
        let new_active = active_index_after_close(self.tabs.len(), self.active, index);
        self.tabs.remove(index);
        self.active = new_active;
        TabCloseResult::Removed { new_active }
    }

    /// Set the active tab index.
    pub fn switch_to(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }

    /// The currently active tab (if any).
    pub fn active(&self) -> Option<&DocumentState> {
        self.tabs.get(self.active)
    }

    /// Mutable reference to the active tab.
    pub fn active_mut(&mut self) -> Option<&mut DocumentState> {
        self.tabs.get_mut(self.active)
    }

    /// Current active index.
    pub fn active_index(&self) -> usize {
        self.active
    }

    /// Get a tab by index.
    pub fn get(&self, index: usize) -> Option<&DocumentState> {
        self.tabs.get(index)
    }

    /// Number of open tabs.
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// True when no tabs are open.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Find the index of a tab by its file path.
    pub fn find_by_path(&self, path: &Path) -> Option<usize> {
        self.tabs.iter().position(|t| {
            t.url.borrow().as_deref() == Some(path)
        })
    }

    /// Iterator over all tabs.
    pub fn iter(&self) -> impl Iterator<Item = &DocumentState> {
        self.tabs.iter()
    }

    /// Generate `(label, is_active)` pairs for rebuilding the tab bar.
    pub fn tab_labels(&self) -> Vec<(String, bool)> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let url = t.url.borrow();
                (tab_label(url.as_deref(), t.is_dirty.get()), i == self.active)
            })
            .collect()
    }

    /// The color scheme of the first tab (used as default for new tabs).
    pub fn first_scheme(&self) -> Option<crate::ui::appearance::ColorScheme> {
        self.tabs.first().map(|t| t.editor_delegate.scheme())
    }
}
