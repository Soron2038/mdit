//! Per-document state: one instance per open tab.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use objc2::rc::Retained;
use objc2_app_kit::{NSScrollView, NSTextView};
use objc2_foundation::{MainThreadMarker, NSRect};

use crate::editor::text_storage::MditEditorDelegate;
use crate::editor::text_view::create_editor_view;
use crate::ui::appearance::ColorScheme;

/// All state belonging to one open document (tab).
pub struct DocumentState {
    pub scroll_view: Retained<NSScrollView>,
    pub text_view: Retained<NSTextView>,
    pub editor_delegate: Retained<MditEditorDelegate>,
    /// Disk URL of the document; `None` for new, unsaved documents.
    pub url: RefCell<Option<PathBuf>>,
    /// True when content differs from the last saved version.
    pub is_dirty: Cell<bool>,
}

impl DocumentState {
    /// Create a new, empty document tab using the given colour scheme.
    pub fn new_empty(mtm: MainThreadMarker, scheme: ColorScheme, frame: NSRect) -> Self {
        let (scroll_view, text_view, editor_delegate) =
            create_editor_view(mtm, frame);
        editor_delegate.set_scheme(scheme);
        Self {
            scroll_view,
            text_view,
            editor_delegate,
            url: RefCell::new(None),
            is_dirty: Cell::new(false),
        }
    }
}
