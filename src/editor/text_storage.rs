use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSTextStorage, NSTextStorageDelegate, NSTextStorageEditActions};
use objc2_foundation::{
    MainThreadMarker, NSInteger, NSObject, NSObjectProtocol, NSRange,
};

use crate::editor::apply::{apply_attribute_runs, collect_code_block_infos, CodeBlockInfo};
use crate::editor::renderer::compute_attribute_runs;
use crate::markdown::parser::{parse, MarkdownSpan};
use crate::ui::appearance::ColorScheme;

// ---------------------------------------------------------------------------
// Ivars
// ---------------------------------------------------------------------------

#[doc(hidden)]
pub struct MditEditorDelegateIvars {
    spans: RefCell<Vec<MarkdownSpan>>,
    cursor_pos: Cell<Option<usize>>,
    /// Current color scheme used for attribute rendering.
    scheme: Cell<ColorScheme>,
    /// Set to `true` while `apply_attribute_runs` is active so that the
    /// attribute-only delegate callbacks it triggers are ignored.
    applying: Cell<bool>,
    /// UTF-16 character offsets of H1/H2 heading paragraph starts, updated
    /// after every re-parse. Read by MditTextView to draw separator lines.
    heading_sep_positions: RefCell<Vec<usize>>,
    /// Code block metadata updated after every re-parse.
    /// Read by MditTextView to draw the visual overlay and copy-to-clipboard.
    code_block_infos: RefCell<Vec<CodeBlockInfo>>,
}

// ---------------------------------------------------------------------------
// Class definition — implements NSTextStorageDelegate
// ---------------------------------------------------------------------------

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MditEditorDelegateIvars]
    pub struct MditEditorDelegate;

    unsafe impl NSObjectProtocol for MditEditorDelegate {}

    unsafe impl NSTextStorageDelegate for MditEditorDelegate {
        /// Called after the text storage processes an edit.
        /// Re-parses the Markdown AST and applies visual attributes.
        #[unsafe(method(textStorage:didProcessEditing:range:changeInLength:))]
        fn did_process_editing(
            &self,
            text_storage: &NSTextStorage,
            edited_mask: NSTextStorageEditActions,
            _edited_range: NSRange,
            _delta: NSInteger,
        ) {
            // Ignore attribute-only changes to avoid recursion: when we apply
            // attributes below, NSTextStorage calls this delegate again with
            // only EditedAttributes — we skip those.
            if !edited_mask.contains(NSTextStorageEditActions::EditedCharacters) {
                return;
            }
            // Guard against re-entrancy from within apply_attribute_runs.
            if self.ivars().applying.get() {
                return;
            }

            let text = text_storage.string().to_string();
            let new_spans = parse(&text);
            *self.ivars().spans.borrow_mut() = new_spans;

            // ── Apply visual attributes ───────────────────────────────────
            let cursor_pos = self.ivars().cursor_pos.get();
            let runs = {
                let spans = self.ivars().spans.borrow();
                compute_attribute_runs(&text, &spans, cursor_pos)
            };
            let scheme = self.ivars().scheme.get();
            self.ivars().applying.set(true);
            let positions = apply_attribute_runs(text_storage, &text, &runs, &scheme);
            self.ivars().applying.set(false);
            *self.ivars().heading_sep_positions.borrow_mut() = positions;
            let infos = {
                let spans_ref = self.ivars().spans.borrow();
                collect_code_block_infos(&spans_ref, &text)
            };
            *self.ivars().code_block_infos.borrow_mut() = infos;
        }
    }
);

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

impl MditEditorDelegate {
    /// Create a new delegate with the given initial color scheme.
    pub fn new(mtm: MainThreadMarker, scheme: ColorScheme) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MditEditorDelegateIvars {
            spans: RefCell::new(Vec::new()),
            cursor_pos: Cell::new(None),
            scheme: Cell::new(scheme),
            applying: Cell::new(false),
            heading_sep_positions: RefCell::new(Vec::new()),
            code_block_infos: RefCell::new(Vec::new()),
        });
        unsafe { msg_send![super(this), init] }
    }

    /// Get the current color scheme.
    pub fn scheme(&self) -> ColorScheme {
        self.ivars().scheme.get()
    }

    /// Update the color scheme (e.g. on appearance change).
    pub fn set_scheme(&self, scheme: ColorScheme) {
        self.ivars().scheme.set(scheme);
    }

    /// Get a clone of the current span tree.
    pub fn spans(&self) -> Vec<MarkdownSpan> {
        self.ivars().spans.borrow().clone()
    }

    /// Set the cursor position (byte offset) for syntax-marker visibility.
    pub fn set_cursor_pos(&self, pos: Option<usize>) {
        self.ivars().cursor_pos.set(pos);
    }

    /// Get the current cursor position.
    pub fn cursor_pos(&self) -> Option<usize> {
        self.ivars().cursor_pos.get()
    }

    /// Force re-application of visual attributes using the current scheme.
    ///
    /// Call this after changing the color scheme via `set_scheme` so the
    /// document immediately reflects the new colors without requiring a
    /// keystroke to trigger `didProcessEditing`.
    pub fn reapply(&self, storage: &NSTextStorage) {
        let text = storage.string().to_string();
        if text.is_empty() {
            return;
        }
        let cursor_pos = self.ivars().cursor_pos.get();
        let runs = {
            let spans = self.ivars().spans.borrow();
            compute_attribute_runs(&text, &spans, cursor_pos)
        };
        let scheme = self.ivars().scheme.get();
        self.ivars().applying.set(true);
        let positions = apply_attribute_runs(storage, &text, &runs, &scheme);
        self.ivars().applying.set(false);
        *self.ivars().heading_sep_positions.borrow_mut() = positions;
        let infos = {
            let spans_ref = self.ivars().spans.borrow();
            collect_code_block_infos(&spans_ref, &text)
        };
        *self.ivars().code_block_infos.borrow_mut() = infos;
    }

    /// Returns the UTF-16 character offsets of H1/H2 heading paragraph starts.
    /// Used by `MditTextView` to draw separator lines above headings.
    pub fn heading_sep_positions(&self) -> Vec<usize> {
        self.ivars().heading_sep_positions.borrow().clone()
    }

    /// Returns the code block metadata for all fenced code blocks in the document.
    /// Used by `MditTextView` to draw the visual overlay.
    pub fn code_block_infos(&self) -> Vec<CodeBlockInfo> {
        self.ivars().code_block_infos.borrow().clone()
    }
}
