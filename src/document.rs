//! NSDocument subclass — handles open/save for Markdown (.md) files.
//!
//! Full Cmd+O integration requires Info.plist `CFBundleDocumentTypes`
//! registration; the class itself is available for programmatic use today.

use objc2::rc::Retained;
use objc2::{define_class, msg_send, MainThreadOnly};
use objc2_app_kit::NSDocument;
use objc2_foundation::{
    MainThreadMarker, NSData, NSError, NSObjectProtocol, NSString,
};

// ---------------------------------------------------------------------------
// MditDocument
// ---------------------------------------------------------------------------

define_class!(
    #[unsafe(super = NSDocument)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    pub struct MditDocument;

    unsafe impl NSObjectProtocol for MditDocument {}

    impl MditDocument {
        /// Load document from raw bytes (UTF-8 Markdown text).
        ///
        /// Called by NSDocumentController when opening a file.
        /// TODO: decode UTF-8 and push content into the shared text storage.
        #[unsafe(method(readFromData:ofType:error:))]
        fn read_from_data(
            &self,
            _data: &NSData,
            _type_name: &NSString,
            _error: *mut *mut NSError,
        ) -> bool {
            true
        }

        /// Serialize document content to raw bytes for saving.
        ///
        /// TODO: read from shared text storage and encode as UTF-8 NSData.
        #[unsafe(method_id(dataOfType:error:))]
        fn data_of_type(
            &self,
            _type_name: &NSString,
            _error: *mut *mut NSError,
        ) -> Option<Retained<NSData>> {
            None
        }
    }
);

impl MditDocument {
    /// Create a new, empty document.
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}
