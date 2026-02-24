use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSTextStorage, NSTextStorageDelegate, NSTextStorageEditActions};
use objc2_foundation::{
    MainThreadMarker, NSInteger, NSObject, NSObjectProtocol, NSRange,
};

use crate::markdown::parser::{parse, MarkdownSpan};

// ---------------------------------------------------------------------------
// Ivars
// ---------------------------------------------------------------------------

#[doc(hidden)]
pub struct MditEditorDelegateIvars {
    spans: RefCell<Vec<MarkdownSpan>>,
    cursor_pos: Cell<Option<usize>>,
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
        /// We re-parse the Markdown AST here.
        #[unsafe(method(textStorage:didProcessEditing:range:changeInLength:))]
        fn did_process_editing(
            &self,
            text_storage: &NSTextStorage,
            edited_mask: NSTextStorageEditActions,
            _edited_range: NSRange,
            _delta: NSInteger,
        ) {
            // Only re-parse when characters changed (not just attributes)
            if !edited_mask.contains(NSTextStorageEditActions::EditedCharacters) {
                return;
            }

            let text = text_storage.string().to_string();
            let new_spans = parse(&text);
            *self.ivars().spans.borrow_mut() = new_spans;
        }
    }
);

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

impl MditEditorDelegate {
    /// Create a new delegate.
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MditEditorDelegateIvars {
            spans: RefCell::new(Vec::new()),
            cursor_pos: Cell::new(None),
        });
        unsafe { msg_send![super(this), init] }
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
}
