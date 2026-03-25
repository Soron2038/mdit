use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication,
    NSBackingStoreType, NSBezelStyle, NSButton, NSControl, NSImage,
    NSTextView, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSArray, NSPoint, NSRange, NSRect, NSSize, NSString,
};

// ---------------------------------------------------------------------------
// Dirty-check dialog
// ---------------------------------------------------------------------------

pub(super) enum SaveChoice {
    Save,
    DontSave,
    Cancel,
}

pub(super) fn show_save_alert(filename: &str, mtm: MainThreadMarker) -> SaveChoice {
    use objc2_app_kit::NSAlert;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(&format!(
        "Do you want to save changes to \"{}\"?",
        filename
    )));
    alert.setInformativeText(&NSString::from_str(
        "Your changes will be lost if you don't save them.",
    ));
    alert.addButtonWithTitle(&NSString::from_str("Save")); // 1000
    alert.addButtonWithTitle(&NSString::from_str("Don't Save")); // 1001
    alert.addButtonWithTitle(&NSString::from_str("Cancel")); // 1002
    let response = alert.runModal();
    match response {
        1000 => SaveChoice::Save,
        1001 => SaveChoice::DontSave,
        _ => SaveChoice::Cancel,
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Toggle an inline marker around the current selection.
pub(super) fn toggle_inline_wrap(tv: &NSTextView, marker: &str) {
    let range: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else {
        return;
    };
    let full_str = storage.string();
    let full_len = full_str.length();

    let selected = full_str.substringWithRange(range).to_string();

    // Grab a few characters on each side for marker detection.
    const MAX_MARKERS: usize = 6;
    let before_start = range.location.saturating_sub(MAX_MARKERS);
    let after_end = (range.location + range.length + MAX_MARKERS).min(full_len);

    let before = full_str
        .substringWithRange(NSRange { location: before_start, length: range.location - before_start })
        .to_string();
    let after = full_str
        .substringWithRange(NSRange {
            location: range.location + range.length,
            length: after_end - (range.location + range.length),
        })
        .to_string();

    let result = mdit::editor::formatting::compute_inline_toggle(&selected, &before, &after, marker);

    let replace_range = NSRange {
        location: range.location - result.consumed_before,
        length: result.consumed_before + range.length + result.consumed_after,
    };
    let ns = NSString::from_str(&result.replacement);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: replace_range] }
}

/// Replace the current NSTextView selection with `prefix + selected + suffix`.
pub(super) fn insert_link_wrap(tv: &NSTextView, prefix: &str, suffix: &str) {
    let range: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else {
        return;
    };
    let selected = storage.string().substringWithRange(range).to_string();
    let text = mdit::editor::formatting::compute_link_wrap(&selected, prefix, suffix);
    let ns = NSString::from_str(&text);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: range] }
}

/// Apply a block-level format to the line containing the caret.
pub(super) fn apply_block_format(tv: &NSTextView, desired_prefix: &str) {
    let caret: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else {
        return;
    };
    let ns_str = storage.string();
    let point = NSRange { location: caret.location, length: 0 };
    let line_range: NSRange = ns_str.lineRangeForRange(point);
    let line_text = ns_str.substringWithRange(line_range).to_string();

    let new_line = mdit::editor::formatting::set_block_format(&line_text, desired_prefix);
    let ns = NSString::from_str(&new_line);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: line_range] }
}

/// Wrap the current selection in a fenced code block.
pub(super) fn insert_code_block(tv: &NSTextView) {
    let range: NSRange = unsafe { msg_send![tv, selectedRange] };
    let Some(storage) = (unsafe { tv.textStorage() }) else {
        return;
    };
    let selected = storage.string().substringWithRange(range).to_string();
    let text = mdit::editor::formatting::compute_code_block_wrap(&selected);
    let ns = NSString::from_str(&text);
    unsafe { msg_send![tv, insertText: &*ns, replacementRange: range] }
}

// ---------------------------------------------------------------------------
// Find‐all helper
// ---------------------------------------------------------------------------

/// Find all occurrences of `query` in `text`, returning NSRange for each match.
/// Uses NSString's rangeOfString:options:range: for proper Unicode + UTF-16 handling.
pub(crate) fn find_all_ranges(text: &NSString, query: &NSString, case_insensitive: bool) -> Vec<NSRange> {
    let mut ranges = Vec::new();
    let len = text.length();
    if len == 0 || query.length() == 0 { return ranges; }
    let options: usize = if case_insensitive { 1 } else { 0 }; // NSCaseInsensitiveSearch = 1
    let mut search_from = NSRange { location: 0, length: len };
    loop {
        let found: NSRange = unsafe {
            msg_send![text, rangeOfString: query, options: options, range: search_from]
        };
        if found.location >= usize::MAX / 2 { break; } // NSNotFound
        ranges.push(found);
        let next_loc = found.location + found.length.max(1);
        if next_loc >= len { break; }
        search_from = NSRange { location: next_loc, length: len - next_loc };
    }
    ranges
}

// ---------------------------------------------------------------------------
// Window creation
// ---------------------------------------------------------------------------

pub(super) fn create_window(mtm: MainThreadMarker) -> Retained<NSWindow> {
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable;

    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(900.0, 700.0)),
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };

    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(ns_string!("mdit"));
    window.setContentMinSize(NSSize::new(500.0, 400.0));

    window
}

// ---------------------------------------------------------------------------
// Appearance detection
// ---------------------------------------------------------------------------

/// Return `true` when the system is currently in dark mode.
pub(super) fn detect_is_dark(app: &NSApplication) -> bool {
    let appearance = app.effectiveAppearance();
    unsafe {
        let names = NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]);
        appearance
            .bestMatchFromAppearancesWithNames(&names)
            .map(|name| name.isEqualToString(NSAppearanceNameDarkAqua))
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Titlebar accessory (eye toggle + ellipsis)
// ---------------------------------------------------------------------------

/// Add an Eye (toggle mode) button and an ellipsis button to the right side
/// of the macOS title bar using `NSTitlebarAccessoryViewController`.
pub(super) fn add_titlebar_accessory(window: &NSWindow, mtm: MainThreadMarker, target: &AnyObject) {
    let btn_h = 20.0_f64;
    let btn_w = 26.0_f64;
    let acc_h = 28.0_f64;
    let gap = 2.0_f64;
    let total_w = btn_w * 2.0 + gap + 4.0;
    let v_off = (acc_h - btn_h) / 2.0;

    let acc_view = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(total_w, acc_h)),
    );

    // Eye button — toggles Viewer/Editor mode.
    let eye_btn = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(2.0, v_off), NSSize::new(btn_w, btn_h)),
    );
    eye_btn.setBezelStyle(NSBezelStyle::AccessoryBarAction);
    eye_btn.setBordered(false);
    let eye_name = NSString::from_str("eye");
    if let Some(img) = NSImage::imageWithSystemSymbolName_accessibilityDescription(&eye_name, None) {
        eye_btn.setImage(Some(&img));
    }
    eye_btn.setTitle(&NSString::from_str(""));
    unsafe {
        NSControl::setTarget(&eye_btn, Some(target));
        NSControl::setAction(&eye_btn, Some(objc2::sel!(toggleMode:)));
        let _: () = msg_send![&*eye_btn, setToolTip: &*NSString::from_str("Toggle Editor (⌘E)")];
    }
    acc_view.addSubview(&eye_btn);

    // Ellipsis button — placeholder for future options.
    let more_btn = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(2.0 + btn_w + gap, v_off), NSSize::new(btn_w, btn_h)),
    );
    more_btn.setBezelStyle(NSBezelStyle::AccessoryBarAction);
    more_btn.setBordered(false);
    let ellipsis_name = NSString::from_str("ellipsis");
    if let Some(img) = NSImage::imageWithSystemSymbolName_accessibilityDescription(&ellipsis_name, None) {
        more_btn.setImage(Some(&img));
    }
    more_btn.setTitle(&NSString::from_str(""));
    acc_view.addSubview(&more_btn);

    // NSTitlebarAccessoryViewController — set layoutAttribute to .trailing (12).
    let Some(vc_cls) = AnyClass::get(c"NSTitlebarAccessoryViewController") else { return };
    unsafe {
        let alloc: *mut AnyObject = msg_send![vc_cls, alloc];
        let vc: *mut AnyObject = msg_send![alloc, init];
        if vc.is_null() { return; }
        let vc_ret = Retained::retain(vc).expect("NSTitlebarAccessoryViewController");
        let _: () = msg_send![&*vc_ret, setView: &*acc_view];
        let _: () = msg_send![&*vc_ret, setLayoutAttribute: 2isize];  // NSLayoutAttributeRight
        let _: () = msg_send![window, addTitlebarAccessoryViewController: &*vc_ret];
    }
}
