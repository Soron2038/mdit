//! macOS menu bar for mdit.
//!
//! Builds the complete 5-menu structure (App / File / Edit / View / Help)
//! and registers it with `NSApplication::setMainMenu`.

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::sel;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::{MainThreadMarker, NSString};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build and install the application main menu.
///
/// Call this once inside `applicationDidFinishLaunching:`, before
/// `makeKeyAndOrderFront`.
pub fn build_main_menu(app: &NSApplication, mtm: MainThreadMarker) {
    let bar = new_menu("MainMenu", mtm);

    bar.addItem(&app_menu(mtm));
    bar.addItem(&file_menu(mtm));
    bar.addItem(&edit_menu(mtm));
    bar.addItem(&view_menu(mtm));
    bar.addItem(&help_menu(mtm));

    app.setMainMenu(Some(&bar));
}

// ---------------------------------------------------------------------------
// Per-menu builders
// ---------------------------------------------------------------------------

fn app_menu(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let menu = new_menu("", mtm);

    menu.addItem(&item("About mdit", None, "", mtm));
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    // Services submenu — AppKit discovers items; we register the placeholder.
    let services_item = item("Services", None, "", mtm);
    let services_menu = new_menu("Services", mtm);
    services_item.setSubmenu(Some(&services_menu));
    menu.addItem(&services_item);
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    menu.addItem(&with_cmd(item("Hide mdit", Some(sel!(hide:)), "h", mtm)));
    menu.addItem(&with_cmd_opt(item(
        "Hide Others",
        Some(sel!(hideOtherApplications:)),
        "h",
        mtm,
    )));
    menu.addItem(&item("Show All", Some(sel!(unhideAllApplications:)), "", mtm));
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    menu.addItem(&with_cmd(item("Quit mdit", Some(sel!(terminate:)), "q", mtm)));

    wrap_in_top_item("", menu, mtm)
}

fn file_menu(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let menu = new_menu("File", mtm);

    menu.addItem(&with_cmd(item("New", Some(sel!(newDocument:)), "n", mtm)));
    menu.addItem(&with_cmd(item("Open…", Some(sel!(openDocument:)), "o", mtm)));
    menu.addItem(&with_cmd(item("Close", Some(sel!(performClose:)), "w", mtm)));
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    menu.addItem(&with_cmd(item("Save", Some(sel!(saveDocument:)), "s", mtm)));
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    // Export as PDF — Cmd+Shift+E → handled by AppDelegate.exportPDF:
    menu.addItem(&with_cmd_shift(item(
        "Export as PDF…",
        Some(sel!(exportPDF:)),
        "e",
        mtm,
    )));

    wrap_in_top_item("File", menu, mtm)
}

fn edit_menu(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let menu = new_menu("Edit", mtm);

    // Undo / Redo — handled by NSTextView via responder chain (target = nil)
    menu.addItem(&with_cmd(item("Undo", Some(sel!(undo:)), "z", mtm)));
    menu.addItem(&with_cmd_shift(item("Redo", Some(sel!(redo:)), "z", mtm)));
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Standard edit actions — NSTextView responds automatically
    menu.addItem(&with_cmd(item("Cut", Some(sel!(cut:)), "x", mtm)));
    menu.addItem(&with_cmd(item("Copy", Some(sel!(copy:)), "c", mtm)));
    menu.addItem(&with_cmd(item("Paste", Some(sel!(paste:)), "v", mtm)));
    menu.addItem(&with_cmd(item("Select All", Some(sel!(selectAll:)), "a", mtm)));
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Markdown formatting — dispatched to AppDelegate action methods
    menu.addItem(&with_cmd(item("Bold", Some(sel!(applyBold:)), "b", mtm)));
    menu.addItem(&with_cmd(item("Italic", Some(sel!(applyItalic:)), "i", mtm)));
    menu.addItem(&with_cmd(item("Inline Code", Some(sel!(applyInlineCode:)), "e", mtm)));
    menu.addItem(&with_cmd(item("Link", Some(sel!(applyLink:)), "k", mtm)));
    menu.addItem(&with_cmd_shift(item(
        "Strikethrough",
        Some(sel!(applyStrikethrough:)),
        "x",
        mtm,
    )));
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Heading shortcuts — Cmd+1/2/3
    menu.addItem(&with_cmd(item("Heading 1", Some(sel!(applyH1:)), "1", mtm)));
    menu.addItem(&with_cmd(item("Heading 2", Some(sel!(applyH2:)), "2", mtm)));
    menu.addItem(&with_cmd(item("Heading 3", Some(sel!(applyH3:)), "3", mtm)));

    wrap_in_top_item("Edit", menu, mtm)
}

fn view_menu(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    // Intentionally minimal for now; appearance switching (Task 19).
    let menu = new_menu("View", mtm);

    wrap_in_top_item("View", menu, mtm)
}

fn help_menu(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let menu = new_menu("Help", mtm);

    wrap_in_top_item("Help", menu, mtm)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a bare `NSMenu` with the given title.
fn new_menu(title: &str, mtm: MainThreadMarker) -> Retained<NSMenu> {
    NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title))
}

/// Create a plain `NSMenuItem` (no modifier mask set).
fn item(title: &str, action: Option<Sel>, key: &str, mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key),
        )
    }
}

/// Attach a submenu to a fresh top-level placeholder item.
fn wrap_in_top_item(title: &str, menu: Retained<NSMenu>, mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let top = item(title, None, "", mtm);
    top.setSubmenu(Some(&menu));
    top
}

/// Set Cmd as the only modifier (leaves existing key equivalent).
fn with_cmd(i: Retained<NSMenuItem>) -> Retained<NSMenuItem> {
    i.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
    i
}

/// Set Cmd+Shift as the modifier.
fn with_cmd_shift(i: Retained<NSMenuItem>) -> Retained<NSMenuItem> {
    i.setKeyEquivalentModifierMask(
        NSEventModifierFlags(NSEventModifierFlags::Command.0 | NSEventModifierFlags::Shift.0),
    );
    i
}

/// Set Cmd+Option as the modifier.
fn with_cmd_opt(i: Retained<NSMenuItem>) -> Retained<NSMenuItem> {
    i.setKeyEquivalentModifierMask(
        NSEventModifierFlags(NSEventModifierFlags::Command.0 | NSEventModifierFlags::Option.0),
    );
    i
}
