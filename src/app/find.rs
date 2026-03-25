use std::cell::{Cell, RefCell};

use objc2::msg_send;
use objc2_app_kit::{NSBackgroundColorAttributeName, NSColor, NSTextView};
use objc2_foundation::{NSRange, NSString};

use mdit::editor::view_mode::ViewMode;
use mdit::ui::find_bar::{FindBar, FIND_H_COMPACT, FIND_H_EXPANDED};

use super::helpers::find_all_ranges;
use super::PATH_H;

pub(super) enum Direction {
    Next,
    Previous,
}

/// Owns the find/replace state: match list, current index, bar height.
pub(crate) struct FindCoordinator {
    matches: RefCell<Vec<NSRange>>,
    current: Cell<usize>,
    bar_height: Cell<f64>,
}

impl Default for FindCoordinator {
    fn default() -> Self {
        Self {
            matches: RefCell::new(Vec::new()),
            current: Cell::new(0),
            bar_height: Cell::new(0.0),
        }
    }
}

impl FindCoordinator {
    /// Current bar height (0.0 = hidden).
    pub(super) fn bar_height(&self) -> f64 {
        self.bar_height.get()
    }

    /// Whether the find bar is currently open.
    pub(super) fn is_open(&self) -> bool {
        self.bar_height.get() > 0.0
    }

    /// Navigate to the next or previous match.
    pub(super) fn navigate(&self, direction: Direction, fb: &FindBar, tv: &NSTextView) {
        let count = self.matches.borrow().len();
        if count == 0 { return; }
        let current = self.current.get();
        let idx = match direction {
            Direction::Next => (current + 1) % count,
            Direction::Previous => if current == 0 { count - 1 } else { current - 1 },
        };
        self.current.set(idx);

        // Get storage for highlighting
        let storage = unsafe { tv.textStorage() };
        if let Some(ref storage) = storage {
            self.highlight_current_match(storage);
        }
        self.scroll_to_current_match(tv);
        fb.update_count(idx + 1, count);
    }

    /// Run a search against the given text storage and update highlights + count.
    pub(super) fn perform_search(
        &self,
        fb: &FindBar,
        tv: &NSTextView,
        tab_mode: ViewMode,
    ) {
        let storage = unsafe { tv.textStorage() };
        let Some(storage) = storage else { return };

        let query = fb.search_text();

        // Remove previous highlights from old match ranges
        let old_matches: Vec<NSRange> = self.matches.borrow().clone();
        for range in &old_matches {
            unsafe {
                storage.removeAttribute_range(
                    NSBackgroundColorAttributeName,
                    *range,
                );
            }
        }

        if query.is_empty() {
            *self.matches.borrow_mut() = Vec::new();
            self.current.set(0);
            fb.update_count(0, 0);
            fb.set_no_match(false);
            return;
        }

        // Find all matches
        let ns_query = NSString::from_str(&query);
        let case_sensitive = fb.is_case_sensitive();
        let matches = find_all_ranges(&storage.string(), &ns_query, !case_sensitive);
        let count = matches.len();

        // Clamp current index
        let current = self.current.get().min(count.saturating_sub(1));
        self.current.set(current);
        *self.matches.borrow_mut() = matches.clone();

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
            self.scroll_to_current_match(tv);
        } else {
            fb.update_count(0, 0);
            fb.set_no_match(true);
        }

        // Show/hide replace row based on matches + mode
        self.update_bar_height(fb, count, tab_mode);
    }

    /// Replace the current match with the replacement text.
    pub(super) fn replace_one(
        &self,
        fb: &FindBar,
        tv: &NSTextView,
        tab_mode: ViewMode,
    ) {
        let replace_str = fb.replace_text();
        let range = {
            let matches = self.matches.borrow();
            matches.get(self.current.get()).copied()
        };
        let Some(range) = range else { return };

        let storage = unsafe { tv.textStorage() };
        if let Some(storage) = storage {
            let ns_replace = NSString::from_str(&replace_str);
            storage.replaceCharactersInRange_withString(range, &ns_replace);
        }
        self.perform_search(fb, tv, tab_mode);
    }

    /// Replace all matches with the replacement text.
    pub(super) fn replace_all(
        &self,
        fb: &FindBar,
        tv: &NSTextView,
        tab_mode: ViewMode,
    ) {
        let replace_str = fb.replace_text();
        let ranges: Vec<NSRange> = self.matches.borrow().clone();
        if ranges.is_empty() { return; }

        let storage = unsafe { tv.textStorage() };
        if let Some(storage) = storage {
            let ns_replace = NSString::from_str(&replace_str);
            // Replace in reverse order to preserve offsets
            for range in ranges.iter().rev() {
                storage.replaceCharactersInRange_withString(*range, &ns_replace);
            }
        }
        self.perform_search(fb, tv, tab_mode);
    }

    /// Open the find bar — set height, show, resize scroll view.
    pub(super) fn open_bar(
        &self,
        fb: &FindBar,
        win_width: f64,
    ) {
        let h = FIND_H_COMPACT;
        self.bar_height.set(h);
        fb.view().setFrame(objc2_foundation::NSRect::new(
            objc2_foundation::NSPoint::new(0.0, PATH_H),
            objc2_foundation::NSSize::new(win_width, h),
        ));
        fb.set_height(h);
        fb.show();
        fb.focus_search();
    }

    /// Close the find bar, remove highlights, reset state.
    pub(super) fn close(&self, fb: &FindBar, tv: Option<&NSTextView>, editor_delegate: Option<&mdit::editor::text_storage::MditEditorDelegate>) {
        if !self.is_open() { return; }

        // Remove all highlights
        let matches: Vec<NSRange> = self.matches.borrow().clone();
        if !matches.is_empty() {
            if let Some(tv) = tv {
                if let Some(storage) = unsafe { tv.textStorage() } {
                    for &range in &matches {
                        unsafe {
                            storage.removeAttribute_range(
                                NSBackgroundColorAttributeName,
                                range,
                            );
                        }
                    }
                    // Re-render to restore legitimate highlight colors
                    if let Some(ed) = editor_delegate {
                        ed.reapply(&storage);
                    }
                }
            }
        }

        *self.matches.borrow_mut() = Vec::new();
        self.current.set(0);
        self.bar_height.set(0.0);
        fb.hide();
        fb.show_replace_row(false);
        fb.update_count(0, 0);
        fb.set_no_match(false);
    }

    /// Highlight the current match (update background colors).
    fn highlight_current_match(&self, storage: &objc2_app_kit::NSTextStorage) {
        let matches = self.matches.borrow().clone();
        let current = self.current.get();
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

    /// Scroll the text view so the current match is visible and select it.
    fn scroll_to_current_match(&self, tv: &NSTextView) {
        let matches = self.matches.borrow();
        let current = self.current.get();
        let Some(&range) = matches.get(current) else { return };
        drop(matches);
        unsafe {
            let _: () = msg_send![tv, scrollRangeToVisible: range];
            let _: () = msg_send![tv, setSelectedRange: range];
        }
    }

    /// Show or hide the replace row, updating bar height as needed.
    /// Returns the new bar height so the caller can resize the scroll view.
    pub(super) fn update_bar_height(
        &self,
        fb: &FindBar,
        match_count: usize,
        mode: ViewMode,
    ) {
        let show_replace = match_count > 0 && mode == ViewMode::Editor;
        fb.show_replace_row(show_replace);

        let new_h = if show_replace { FIND_H_EXPANDED } else { FIND_H_COMPACT };
        let old_h = self.bar_height.get();
        if (new_h - old_h).abs() > 0.5 && old_h > 0.0 {
            self.bar_height.set(new_h);
            fb.set_height(new_h);
        }
    }

    /// Get the match count (for use after update_bar_height).
    pub(super) fn match_count(&self) -> usize {
        self.matches.borrow().len()
    }
}
