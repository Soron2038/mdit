use objc2_app_kit::{NSPrintInfo, NSPrintOperation, NSTextView};

/// Opens the macOS Print / Export-as-PDF dialog for the given text view.
///
/// This calls `NSPrintOperation::printOperationWithView:printInfo:` and
/// `runOperation` which presents the system print panel where the user can
/// choose "Save as PDF" from the PDF drop-down.
pub fn export_pdf(text_view: &NSTextView) {
    let print_info = NSPrintInfo::sharedPrintInfo();
    // NSTextView → NSText → NSView via Deref chain
    let op = NSPrintOperation::printOperationWithView_printInfo(
        &***text_view,
        &print_info,
    );
    op.runOperation();
}
