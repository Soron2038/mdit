# Multi-Tab File I/O — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bestehende .md-Dateien öffnen, bearbeiten und explizit speichern; mehrere Dokumente als Tabs im selben Fenster mit Dirty-Indicator und Path-Bar.

**Architecture:** Custom Tab-Leiste (TabBar) + ein NSScrollView/NSTextView-Paar pro Tab (DocumentState). AppDelegate hält Vec<DocumentState>, kein NSDocumentController. File-I/O über NSOpenPanel/NSSavePanel + std::fs.

**Tech Stack:** Rust, objc2 0.6.3, objc2-appkit 0.3.2, std::fs für Disk-I/O

---

## Task 1: Cargo.toml Features + Pure-Rust Label-Helpers (TDD)

**Files:**
- Modify: `Cargo.toml`
- Create: `tests/tab_label_tests.rs`
- Create: `src/ui/tab_bar.rs` (nur pure-Rust-Teil)
- Modify: `src/ui/mod.rs`

**Step 1: Cargo.toml — neue Features hinzufügen**

```toml
objc2-app-kit = { version = "0.3.2", features = [
    "NSApplication",
    "NSWindow", "NSWindowController",
    "NSGraphics",
    "NSTextView", "NSTextStorage", "NSLayoutManager", "NSTextContainer",
    "NSScrollView",
    "NSDocument", "NSDocumentController",
    "NSColor", "NSFont", "NSFontDescriptor",
    "NSPanel", "NSVisualEffectView",
    "NSButton", "NSButtonCell", "NSControl",
    "NSPrintOperation", "NSPrintInfo",
    "NSMenu", "NSMenuItem", "NSEvent",
    "NSParagraphStyle", "NSText",
    "NSResponder", "NSView",
    "NSTextAttachment",
    "NSAttributedString", "NSAppearance",
    "NSTextField",      // NEU — PathBar
    "NSOpenPanel",      // NEU — File öffnen
    "NSSavePanel",      // NEU — File speichern
    "NSAlert",          // NEU — Dirty-State-Dialog
    "NSBox",            // NEU — Separator zwischen Bereichen
] }
```

**Step 2: Failing tests schreiben**

`tests/tab_label_tests.rs`:
```rust
use mdit::ui::tab_bar::{tab_label, path_label};
use std::path::Path;

#[test]
fn tab_label_untitled_clean() {
    assert_eq!(tab_label(None, false), "Untitled");
}
#[test]
fn tab_label_untitled_dirty() {
    assert_eq!(tab_label(None, true), "• Untitled");
}
#[test]
fn tab_label_named_clean() {
    assert_eq!(tab_label(Some(Path::new("/a/notes.md")), false), "notes.md");
}
#[test]
fn tab_label_named_dirty() {
    assert_eq!(tab_label(Some(Path::new("/a/notes.md")), true), "• notes.md");
}
#[test]
fn path_label_untitled() {
    assert_eq!(path_label(None), "Untitled — not saved");
}
#[test]
fn path_label_with_url() {
    assert_eq!(
        path_label(Some(Path::new("/Users/witt/notes.md"))),
        "/Users/witt/notes.md"
    );
}
```

Run: `cargo test --test tab_label_tests`
Expected: **FAIL** (Modul existiert noch nicht)

**Step 3: `src/ui/tab_bar.rs` anlegen (pure-Rust-Helpers)**

```rust
//! Tab bar for the mdit editor window.
//!
//! Pure-Rust helpers at the top; AppKit view types further below.

use std::path::Path;

// ---------------------------------------------------------------------------
// Pure-Rust helpers (unit-testable, no AppKit)
// ---------------------------------------------------------------------------

/// Label text for a tab button.
/// Prefixes "• " when `is_dirty` is true.
pub fn tab_label(url: Option<&Path>, is_dirty: bool) -> String {
    let name = url
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string());
    if is_dirty { format!("• {}", name) } else { name }
}

/// Full path string for the path bar.
pub fn path_label(url: Option<&Path>) -> String {
    url.map(|p| p.display().to_string())
        .unwrap_or_else(|| "Untitled — not saved".to_string())
}
```

**Step 4: `src/ui/mod.rs` ergänzen**

```rust
pub mod toolbar;
pub mod appearance;
pub mod tab_bar;   // NEU
pub mod path_bar;  // NEU (kommt in Task 3)
```

(path_bar noch auskommentieren bis Task 3)

**Step 5: Tests grün**

Run: `cargo test --test tab_label_tests`
Expected: 6 PASS

**Step 6: Commit**

```bash
git add Cargo.toml tests/tab_label_tests.rs src/ui/tab_bar.rs src/ui/mod.rs
git commit -m "feat(tabs): add tab/path label helpers + new Cargo features"
```

---

## Task 2: DocumentState Struct

**Files:**
- Create: `src/editor/document_state.rs`
- Modify: `src/editor/mod.rs`

**Step 1: Kein Unit-Test möglich (AppKit-Typen)**

**Step 2: `src/editor/document_state.rs` anlegen**

```rust
//! Per-document state: one instance per open tab.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use objc2::rc::Retained;
use objc2_app_kit::{NSScrollView, NSTextView};
use objc2_foundation::MainThreadMarker;

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
    pub fn new_empty(mtm: MainThreadMarker, scheme: ColorScheme, frame: objc2_foundation::NSRect) -> Self {
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
```

**Step 3: `src/editor/mod.rs` ergänzen**

```rust
pub mod text_view;
pub mod text_storage;
pub mod renderer;
pub mod apply;
pub mod cursor_tracker;
pub mod image_handler;
pub mod math_view;
pub mod document_state;   // NEU
```

**Step 4: Build verifizieren**

Run: `cargo build`
Expected: kompiliert ohne Fehler.

**Step 5: Commit**

```bash
git add src/editor/document_state.rs src/editor/mod.rs
git commit -m "feat(tabs): add DocumentState struct (per-tab AppKit state)"
```

---

## Task 3: PathBar View

**Files:**
- Create: `src/ui/path_bar.rs`
- Modify: `src/ui/mod.rs` (Kommentar entfernen)

**Step 1: Kein Unit-Test möglich (AppKit)**

**Step 2: `src/ui/path_bar.rs` anlegen**

```rust
//! Thin status bar at the bottom of the window showing the current file path.

use std::path::Path;

use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSColor, NSFont, NSTextField, NSTextFieldCell, NSView};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

use crate::ui::tab_bar::path_label;

const HEIGHT: f64 = 22.0;

pub struct PathBar {
    field: Retained<NSTextField>,
}

impl PathBar {
    pub fn new(mtm: MainThreadMarker, width: f64) -> Self {
        let frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width, HEIGHT),
        );
        let field = NSTextField::initWithFrame(NSTextField::alloc(mtm), frame);
        field.setEditable(false);
        field.setSelectable(false);
        field.setBordered(false);
        field.setDrawsBackground(false);
        unsafe {
            let font = NSFont::systemFontOfSize_weight(11.0, 0.0); // Regular
            field.setFont(Some(&font));
            field.setTextColor(Some(&NSColor::secondaryLabelColor()));
        }
        field.setStringValue(&NSString::from_str("Untitled — not saved"));
        Self { field }
    }

    /// Update the displayed path.
    pub fn update(&self, url: Option<&Path>) {
        let label = path_label(url);
        self.field.setStringValue(&NSString::from_str(&label));
    }

    pub fn view(&self) -> &NSTextField {
        &self.field
    }

    pub const HEIGHT: f64 = HEIGHT;
}
```

**Step 3: `src/ui/mod.rs`** — Kommentar bei `path_bar` entfernen

**Step 4: Build verifizieren**

Run: `cargo build`
Expected: keine Fehler.

**Step 5: Commit**

```bash
git add src/ui/path_bar.rs src/ui/mod.rs
git commit -m "feat(tabs): add PathBar view (bottom file-path indicator)"
```

---

## Task 4: TabBar AppKit View

**Files:**
- Modify: `src/ui/tab_bar.rs` (AppKit-Teil hinzufügen)

**Step 1: Kein Unit-Test möglich (AppKit)**

**Step 2: AppKit-Teil in `src/ui/tab_bar.rs` anhängen**

```rust
// ---------------------------------------------------------------------------
// AppKit view — requires main thread
// ---------------------------------------------------------------------------

use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSColor, NSControl,
    NSFont, NSView,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

pub const HEIGHT: f64 = 32.0;
const BTN_H: f64 = 22.0;
const CLOSE_W: f64 = 18.0;
const TITLE_W: f64 = 100.0;
const TAB_W: f64 = TITLE_W + CLOSE_W;
const PLUS_W: f64 = 28.0;
const PAD: f64 = 4.0;

pub struct TabBar {
    container: Retained<NSView>,
}

impl TabBar {
    pub fn new(mtm: MainThreadMarker, width: f64) -> Self {
        let frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width, HEIGHT),
        );
        let container = NSView::initWithFrame(NSView::alloc(mtm), frame);
        Self { container }
    }

    /// Rebuild all tab buttons.
    ///
    /// `tabs` is a slice of `(label, is_active)` pairs.
    /// Button tags encode the tab index; target receives `switchToTab:` /
    /// `closeTab:` / `newDocument:`.
    pub fn rebuild(
        &self,
        mtm: MainThreadMarker,
        tabs: &[(String, bool)],   // (label, is_active)
        target: &AnyObject,
    ) {
        use objc2_app_kit::NSView as V;
        // Remove old buttons
        for sv in unsafe { self.container.subviews() }.iter() {
            sv.removeFromSuperview();
        }

        let y = (HEIGHT - BTN_H) / 2.0;
        let mut x = PAD;

        for (i, (label, active)) in tabs.iter().enumerate() {
            // Title button
            let title_btn = make_button(
                mtm,
                label,
                NSRect::new(NSPoint::new(x, y), NSSize::new(TITLE_W, BTN_H)),
                unsafe { Sel::register(c"switchToTab:") },
                target,
                i as isize,
            );
            if *active {
                unsafe {
                    title_btn.setWantsLayer(true);
                    if let Some(layer) = title_btn.layer() {
                        layer.setBackgroundColor(
                            NSColor::controlAccentColor().CGColor()
                        );
                    }
                }
            }
            self.container.addSubview(&title_btn);
            x += TITLE_W;

            // Close button (×)
            let close_btn = make_button(
                mtm,
                "×",
                NSRect::new(NSPoint::new(x, y), NSSize::new(CLOSE_W, BTN_H)),
                unsafe { Sel::register(c"closeTab:") },
                target,
                i as isize,
            );
            self.container.addSubview(&close_btn);
            x += CLOSE_W + PAD;
        }

        // + button (always at the end)
        let plus_btn = make_button(
            mtm,
            "+",
            NSRect::new(NSPoint::new(x, y), NSSize::new(PLUS_W, BTN_H)),
            unsafe { Sel::register(c"newDocument:") },
            target,
            -1isize,
        );
        self.container.addSubview(&plus_btn);
    }

    pub fn view(&self) -> &NSView {
        &self.container
    }

    pub fn set_width(&self, width: f64) {
        let mut f = self.container.frame();
        f.size.width = width;
        self.container.setFrame(f);
    }
}

fn make_button(
    mtm: MainThreadMarker,
    title: &str,
    frame: NSRect,
    action: Sel,
    target: &AnyObject,
    tag: isize,
) -> Retained<NSButton> {
    let btn = NSButton::initWithFrame(NSButton::alloc(mtm), frame);
    btn.setTitle(&NSString::from_str(title));
    btn.setButtonType(NSButtonType::MomentaryPushIn);
    btn.setBezelStyle(NSBezelStyle::Inline);
    unsafe {
        NSControl::setTarget(&btn, Some(target));
        NSControl::setAction(&btn, Some(action));
        NSControl::setTag(&btn, tag);
        let font = NSFont::systemFontOfSize_weight(12.0, 0.0);
        btn.setFont(Some(&font));
    }
    btn
}
```

> **Hinweis:** `CGColor()` auf `NSColor` benötigt evtl. explizites `unsafe`-Block und den Import `NSColor::controlAccentColor`. Falls der active-Tab-Highlight Kompilierprobleme macht, zunächst weglassen und als TODO markieren.

**Step 3: Build verifizieren**

Run: `cargo build`
Wenn CGColor-Fehler: Den Active-Highlight-Block auskommentieren und als TODO markieren.

**Step 4: Commit**

```bash
git add src/ui/tab_bar.rs
git commit -m "feat(tabs): add TabBar AppKit view"
```

---

## Task 5: AppDelegate Refactor — Ivars + Layout

**Files:**
- Modify: `src/app.rs` (groß — Ivars, Layout, Helper-Methoden)

**Step 1: Neue Imports in `src/app.rs`**

```rust
use std::path::PathBuf;

use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSBackingStoreType, NSColor, NSTextDelegate, NSTextView, NSTextViewDelegate,
    NSView, NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSArray, NSNotification, NSObject,
    NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize, NSString,
};

use mdit::editor::document_state::DocumentState;
use mdit::editor::text_storage::MditEditorDelegate;
use mdit::menu::build_main_menu;
use mdit::ui::appearance::ColorScheme;
use mdit::ui::path_bar::PathBar;
use mdit::ui::tab_bar::{tab_label, TabBar};
use mdit::ui::toolbar::FloatingToolbar;
```

**Step 2: Neue `AppDelegateIvars`**

```rust
#[derive(Default)]
struct AppDelegateIvars {
    window:       OnceCell<Retained<NSWindow>>,
    toolbar:      OnceCell<FloatingToolbar>,
    tab_bar:      OnceCell<TabBar>,
    path_bar:     OnceCell<PathBar>,
    tabs:         RefCell<Vec<DocumentState>>,
    active_index: Cell<usize>,
}
```

`use std::cell::{Cell, RefCell};` am Anfang ergänzen.

**Step 3: Hilfsmethoden hinzufügen**

Unterhalb von `impl AppDelegate { ... }` (außerhalb des `define_class!`-Blocks):

```rust
const TAB_H: f64 = 32.0;
const PATH_H: f64 = 22.0;

impl AppDelegate {
    // ...bestehende Methoden bleiben...

    /// Frame für den aktiven NSScrollView (zwischen TabBar und PathBar).
    fn content_frame(&self) -> NSRect {
        let Some(win) = self.ivars().window.get() else { return NSRect::ZERO };
        let bounds = unsafe { win.contentView().unwrap().bounds() };
        let h = bounds.size.height;
        let w = bounds.size.width;
        NSRect::new(
            NSPoint::new(0.0, PATH_H),
            NSSize::new(w, (h - TAB_H - PATH_H).max(0.0)),
        )
    }

    /// Aktiver Tab (borrow).
    fn with_active_tab<F: FnOnce(&DocumentState)>(&self, f: F) {
        let idx = self.ivars().active_index.get();
        let tabs = self.ivars().tabs.borrow();
        if let Some(tab) = tabs.get(idx) {
            f(tab);
        }
    }

    /// Aktive NSTextView für Formatting-Actions.
    fn active_text_view(&self) -> Option<Retained<NSTextView>> {
        let idx = self.ivars().active_index.get();
        let tabs = self.ivars().tabs.borrow();
        tabs.get(idx).map(|t| t.text_view.retain())
    }

    /// Alle Formatting-Actions auf aktive TextView routen.
    fn active_wrap_selection(&self, prefix: &str, suffix: &str) {
        if let Some(tv) = self.active_text_view() {
            wrap_selection(&tv, prefix, suffix);
        }
    }

    /// Tab-Leiste neu aufbauen (nach Add/Remove/Rename/Dirty-Change).
    fn rebuild_tab_bar(&self) {
        let Some(win) = self.ivars().window.get() else { return };
        let Some(tab_bar) = self.ivars().tab_bar.get() else { return };
        let mtm = self.mtm();
        let active = self.ivars().active_index.get();
        let target: &AnyObject = unsafe {
            &*(self as *const AppDelegate as *const AnyObject)
        };
        let labels: Vec<(String, bool)> = {
            let tabs = self.ivars().tabs.borrow();
            tabs.iter().enumerate().map(|(i, t)| {
                let url = t.url.borrow();
                (tab_label(url.as_deref(), t.is_dirty.get()), i == active)
            }).collect()
        };
        tab_bar.rebuild(mtm, &labels, target);
    }

    /// Zu Tab `index` wechseln.
    fn switch_to_tab(&self, index: usize) {
        let Some(win) = self.ivars().window.get() else { return };
        let content = unsafe { win.contentView().unwrap() };

        // Alten Scroll-View entfernen
        {
            let tabs = self.ivars().tabs.borrow();
            let old = self.ivars().active_index.get();
            if let Some(t) = tabs.get(old) {
                t.scroll_view.removeFromSuperview();
            }
        }

        self.ivars().active_index.set(index);

        // Neuen Scroll-View einsetzen
        let frame = self.content_frame();
        {
            let tabs = self.ivars().tabs.borrow();
            if let Some(t) = tabs.get(index) {
                t.scroll_view.setFrame(frame);
                content.addSubview(&t.scroll_view);
            }
        }

        // Path-Bar aktualisieren
        if let Some(pb) = self.ivars().path_bar.get() {
            let tabs = self.ivars().tabs.borrow();
            let url = tabs.get(index).and_then(|t| t.url.borrow().clone());
            pb.update(url.as_deref());
        }

        self.rebuild_tab_bar();
        self.update_text_container_inset();
        self.toolbar.get().map(|tb| tb.hide());
    }

    /// Neuen leeren Tab anlegen und aktivieren.
    fn add_empty_tab(&self) {
        let mtm = self.mtm();
        let scheme = {
            let tabs = self.ivars().tabs.borrow();
            tabs.first()
                .map(|t| t.editor_delegate.scheme())
                .unwrap_or_else(ColorScheme::light)
        };
        let frame = self.content_frame();
        let tab = DocumentState::new_empty(mtm, scheme, frame);
        // Delegate setzen damit textViewDidChangeSelection: feuert
        let target = unsafe {
            ProtocolObject::from_ref(&*(self as *const AppDelegate as *const NSObject))
        };
        tab.text_view.setDelegate(Some(target));
        let new_idx = {
            let mut tabs = self.ivars().tabs.borrow_mut();
            tabs.push(tab);
            tabs.len() - 1
        };
        self.switch_to_tab(new_idx);
    }
}
```

> **Hinweis:** `editor_delegate.scheme()` muss als neue Getter-Methode in `MditEditorDelegate` implementiert werden — gibt `self.ivars().scheme.get()` zurück.

**Step 4: `did_finish_launching` umschreiben**

```rust
fn did_finish_launching(&self, notification: &NSNotification) {
    let mtm = self.mtm();
    let app = notification.object().unwrap()
        .downcast::<NSApplication>().unwrap();

    let initial_scheme = detect_scheme(&app);
    let (window, _) = create_window(mtm);

    window.setDelegate(Some(ProtocolObject::from_ref(self)));
    build_main_menu(&app, mtm);
    window.center();
    window.makeKeyAndOrderFront(None);

    let content = unsafe { window.contentView().unwrap() };
    let bounds = content.bounds();
    let w = bounds.size.width;
    let h = bounds.size.height;

    // TabBar oben
    let tab_bar = TabBar::new(mtm, w);
    tab_bar.view().setFrame(NSRect::new(
        NSPoint::new(0.0, h - TAB_H),
        NSSize::new(w, TAB_H),
    ));
    content.addSubview(tab_bar.view());

    // PathBar unten
    let path_bar = PathBar::new(mtm, w);
    content.addSubview(path_bar.view());

    // Toolbar
    let toolbar = FloatingToolbar::new(
        mtm,
        unsafe { &*(self as *const AppDelegate as *const AnyObject) },
    );

    self.ivars().window.set(window).unwrap();
    let _ = self.ivars().tab_bar.set(tab_bar);
    let _ = self.ivars().path_bar.set(path_bar);
    let _ = self.ivars().toolbar.set(toolbar);

    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    // Ersten leeren Tab anlegen (setzt auch Scheme und Inset)
    self.add_empty_tab();
    self.apply_scheme(initial_scheme);
    self.update_text_container_inset();
}
```

`create_window` gibt jetzt nur noch `(NSWindow, ())` zurück — den Text-View-Teil entfernen da er in `add_empty_tab` steckt.

**Step 5: Alle Formatting-Actions auf `active_text_view()` umstellen**

Alle `if let Some(tv) = self.ivars().text_view.get() { ... }` ersetzen durch:
```rust
if let Some(tv) = self.active_text_view() { ... }
```

**Step 6: Build verifizieren**

Run: `cargo build 2>&1 | head -30`
Compile-Fehler durcharbeiten bis 0 errors.

**Step 7: Alle Tests grün**

Run: `cargo test`
Expected: alle 52 Tests grün.

**Step 8: Commit**

```bash
git add src/app.rs src/editor/text_storage.rs
git commit -m "feat(tabs): refactor AppDelegate to multi-tab architecture"
```

---

## Task 6: switchToTab: und closeTab: Actions

**Files:**
- Modify: `src/app.rs`

**Step 1: Action-Methoden im `define_class!`-Block hinzufügen**

```rust
#[unsafe(method(switchToTab:))]
fn switch_to_tab_action(&self, sender: &AnyObject) {
    let idx = unsafe { NSControl::tag(&*(sender as *const _ as *const NSControl)) };
    if idx >= 0 {
        self.switch_to_tab(idx as usize);
    }
}

#[unsafe(method(closeTab:))]
fn close_tab_action(&self, sender: &AnyObject) {
    let idx = unsafe {
        NSControl::tag(&*(sender as *const _ as *const NSControl)) as usize
    };
    self.close_tab(idx);
}
```

**Step 2: `close_tab` Hilfsmethode**

```rust
fn close_tab(&self, index: usize) {
    // Dirty-Check
    let is_dirty = {
        let tabs = self.ivars().tabs.borrow();
        tabs.get(index).map(|t| t.is_dirty.get()).unwrap_or(false)
    };
    let filename = {
        let tabs = self.ivars().tabs.borrow();
        tabs.get(index)
            .and_then(|t| t.url.borrow().as_deref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "Untitled".to_string())
    };

    if is_dirty {
        let should_save = show_save_alert(&filename, self.mtm());
        match should_save {
            SaveChoice::Save => {
                self.perform_save(Some(index));
            }
            SaveChoice::DontSave => {}
            SaveChoice::Cancel => return,
        }
    }

    // Letzter Tab → nur Inhalt leeren, nicht entfernen
    if self.ivars().tabs.borrow().len() == 1 {
        let tabs = self.ivars().tabs.borrow();
        if let Some(t) = tabs.first() {
            unsafe {
                if let Some(storage) = t.text_view.textStorage() {
                    let full = NSRange { location: 0, length: storage.length() };
                    let empty = NSString::from_str("");
                    storage.replaceCharactersInRange_withString(full, &empty);
                }
            }
            *t.url.borrow_mut() = None;
            t.is_dirty.set(false);
        }
        drop(tabs);
        self.rebuild_tab_bar();
        if let Some(pb) = self.ivars().path_bar.get() {
            pb.update(None);
        }
        return;
    }

    // Tab entfernen
    {
        let tabs = self.ivars().tabs.borrow();
        if let Some(t) = tabs.get(index) {
            t.scroll_view.removeFromSuperview();
        }
    }
    self.ivars().tabs.borrow_mut().remove(index);

    // Active-Index korrigieren
    let new_idx = {
        let len = self.ivars().tabs.borrow().len();
        let cur = self.ivars().active_index.get();
        if index <= cur && cur > 0 { cur - 1 } else { cur.min(len - 1) }
    };
    self.switch_to_tab(new_idx);
}

enum SaveChoice { Save, DontSave, Cancel }

fn show_save_alert(filename: &str, mtm: MainThreadMarker) -> SaveChoice {
    use objc2_app_kit::NSAlert;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(
        &format!("Do you want to save changes to \"{}\"?", filename)
    ));
    alert.setInformativeText(&NSString::from_str(
        "Your changes will be lost if you don't save them."
    ));
    alert.addButtonWithTitle(&NSString::from_str("Save"));        // 1000
    alert.addButtonWithTitle(&NSString::from_str("Don't Save"));  // 1001
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));      // 1002
    let response = unsafe { alert.runModal() };
    match response.0 {
        1000 => SaveChoice::Save,
        1001 => SaveChoice::DontSave,
        _ => SaveChoice::Cancel,
    }
}
```

**Step 3: Build + alle Tests**

Run: `cargo build && cargo test`

**Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat(tabs): add switchToTab: and closeTab: actions"
```

---

## Task 7: Dirty Tracking (textDidChange:)

**Files:**
- Modify: `src/app.rs`

**Step 1: `textDidChange:` in `NSTextDelegate`-Impl ergänzen**

```rust
unsafe impl NSTextDelegate for AppDelegate {
    #[unsafe(method(textDidChange:))]
    fn text_did_change(&self, _notification: &NSNotification) {
        let idx = self.ivars().active_index.get();
        let already_dirty = {
            let tabs = self.ivars().tabs.borrow();
            tabs.get(idx).map(|t| t.is_dirty.get()).unwrap_or(true)
        };
        if !already_dirty {
            {
                let tabs = self.ivars().tabs.borrow();
                if let Some(t) = tabs.get(idx) {
                    t.is_dirty.set(true);
                }
            }
            self.rebuild_tab_bar();
        }
    }
}
```

**Step 2: Build + Tests**

Run: `cargo build && cargo test`

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat(tabs): mark tab dirty on text change (textDidChange:)"
```

---

## Task 8: File Operations (newDocument: + openDocument: + saveDocument:)

**Files:**
- Modify: `src/app.rs`

**Step 1: `newDocument:` (bereits via `add_empty_tab`)**

Prüfen: Die bestehende `new_document`-Action ruft bereits `add_empty_tab` auf? Falls nicht:

```rust
#[unsafe(method(newDocument:))]
fn new_document(&self, _sender: &AnyObject) {
    self.add_empty_tab();
}
```

**Step 2: `openDocument:`**

```rust
#[unsafe(method(openDocument:))]
fn open_document(&self, _sender: &AnyObject) {
    use objc2_app_kit::NSOpenPanel;
    let panel = NSOpenPanel::openPanel(self.mtm());
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(false);
    // Dateifilter (deprecated aber einfach; auf macOS 13+ durch allowedContentTypes ersetzen)
    unsafe {
        let types = NSArray::from_slice(&[
            &*NSString::from_str("md"),
            &*NSString::from_str("markdown"),
            &*NSString::from_str("txt"),
        ]);
        panel.setAllowedFileTypes(Some(&types));
    }
    let response = unsafe { panel.runModal() };
    if response != objc2_app_kit::NSModalResponse::OK { return; }

    let url = unsafe { panel.URL() };
    let Some(ns_url) = url else { return };
    let Some(ns_path) = (unsafe { ns_url.path() }) else { return };
    let path = std::path::PathBuf::from(ns_path.to_string());

    // Prüfen ob bereits offen
    {
        let tabs = self.ivars().tabs.borrow();
        for (i, t) in tabs.iter().enumerate() {
            if t.url.borrow().as_deref() == Some(path.as_path()) {
                drop(tabs);
                self.switch_to_tab(i);
                return;
            }
        }
    }

    // Datei lesen
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mdit: cannot read {:?}: {}", path, e);
            return;
        }
    };

    // Neuen Tab anlegen
    self.add_empty_tab();
    let idx = self.ivars().active_index.get();
    {
        let tabs = self.ivars().tabs.borrow();
        if let Some(t) = tabs.get(idx) {
            *t.url.borrow_mut() = Some(path.clone());
            t.is_dirty.set(false);
            // Inhalt in TextStorage laden
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
    // Path-Bar + Tab-Label aktualisieren
    if let Some(pb) = self.ivars().path_bar.get() {
        pb.update(Some(path.as_path()));
    }
    self.rebuild_tab_bar();
}
```

**Step 3: `saveDocument:`**

```rust
#[unsafe(method(saveDocument:))]
fn save_document_action(&self, _sender: &AnyObject) {
    self.perform_save(None);
}
```

```rust
/// Speichert Tab `index` (None = aktiver Tab).
fn perform_save(&self, index: Option<usize>) {
    let idx = index.unwrap_or_else(|| self.ivars().active_index.get());

    // URL bestimmen (oder NSSavePanel öffnen)
    let path: PathBuf = {
        let tabs = self.ivars().tabs.borrow();
        let tab = match tabs.get(idx) { Some(t) => t, None => return };
        match tab.url.borrow().clone() {
            Some(p) => p,
            None => {
                drop(tabs);
                match self.run_save_panel(idx) {
                    Some(p) => p,
                    None => return,
                }
            }
        }
    };

    // Inhalt aus TextStorage lesen
    let content = {
        let tabs = self.ivars().tabs.borrow();
        let tab = match tabs.get(idx) { Some(t) => t, None => return };
        unsafe { tab.text_view.textStorage() }
            .map(|s| s.string().to_string())
            .unwrap_or_default()
    };

    // Auf Disk schreiben
    if let Err(e) = std::fs::write(&path, content.as_bytes()) {
        eprintln!("mdit: cannot save {:?}: {}", path, e);
        return;
    }

    // State aktualisieren
    {
        let tabs = self.ivars().tabs.borrow();
        if let Some(t) = tabs.get(idx) {
            *t.url.borrow_mut() = Some(path.clone());
            t.is_dirty.set(false);
        }
    }
    if let Some(pb) = self.ivars().path_bar.get() {
        if idx == self.ivars().active_index.get() {
            pb.update(Some(path.as_path()));
        }
    }
    self.rebuild_tab_bar();
}

fn run_save_panel(&self, index: usize) -> Option<PathBuf> {
    use objc2_app_kit::NSSavePanel;
    let panel = NSSavePanel::savePanel(self.mtm());
    panel.setNameFieldStringValue(&NSString::from_str("Untitled.md"));
    let response = unsafe { panel.runModal() };
    if response != objc2_app_kit::NSModalResponse::OK { return None; }
    let ns_url = unsafe { panel.URL() }?;
    let ns_path = unsafe { ns_url.path() }?;
    Some(PathBuf::from(ns_path.to_string()))
}
```

**Step 4: Build + Tests**

Run: `cargo build && cargo test`

**Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat(tabs): implement openDocument:, saveDocument:, newDocument:"
```

---

## Task 9: apply_scheme + update_text_container_inset für alle Tabs

**Files:**
- Modify: `src/app.rs`

**Step 1: `apply_scheme` auf alle Tabs ausdehnen**

```rust
fn apply_scheme(&self, scheme: ColorScheme) {
    let tabs = self.ivars().tabs.borrow();
    let active = self.ivars().active_index.get();
    for (i, tab) in tabs.iter().enumerate() {
        tab.editor_delegate.set_scheme(scheme);
        let (r, g, b) = scheme.background;
        let bg = NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0);
        tab.scroll_view.setBackgroundColor(&bg);
        tab.text_view.setBackgroundColor(&bg);
        if i == active {
            if let Some(storage) = unsafe { tab.text_view.textStorage() } {
                tab.editor_delegate.reapply(&storage);
            }
        }
    }
}
```

**Step 2: `update_text_container_inset` — aktiver Tab**

```rust
fn update_text_container_inset(&self) {
    let Some(win) = self.ivars().window.get() else { return };
    let win_width = win.frame().size.width;
    let max_text_width = 700.0_f64;
    let min_padding = 40.0_f64;
    let h_inset = if win_width > max_text_width + 2.0 * min_padding {
        (win_width - max_text_width) / 2.0
    } else {
        min_padding
    };
    let idx = self.ivars().active_index.get();
    let tabs = self.ivars().tabs.borrow();
    if let Some(t) = tabs.get(idx) {
        t.text_view.setTextContainerInset(NSSize::new(h_inset, 40.0));
    }
}
```

**Step 3: `windowDidResize:` — auch TabBar und PathBar anpassen**

```rust
fn window_did_resize(&self, _notification: &NSNotification) {
    let Some(win) = self.ivars().window.get() else { return };
    let bounds = unsafe { win.contentView().unwrap().bounds() };
    let w = bounds.size.width;
    let h = bounds.size.height;

    if let Some(tb) = self.ivars().tab_bar.get() {
        tb.view().setFrame(NSRect::new(
            NSPoint::new(0.0, h - TAB_H),
            NSSize::new(w, TAB_H),
        ));
    }
    if let Some(pb) = self.ivars().path_bar.get() {
        pb.view().setFrame(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(w, PATH_H),
        ));
    }
    let frame = self.content_frame();
    let idx = self.ivars().active_index.get();
    let tabs = self.ivars().tabs.borrow();
    if let Some(t) = tabs.get(idx) {
        t.scroll_view.setFrame(frame);
    }
    drop(tabs);
    self.update_text_container_inset();
}
```

**Step 4: Build + alle Tests**

Run: `cargo build && cargo test`
Expected: 52 Tests grün.

**Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat(tabs): fix apply_scheme and resize for multi-tab layout"
```

---

## Task 10: Integration + DMG

**Step 1: Gesamttest**

Run: `cargo test`
Expected: alle 52 Tests grün, 0 Warnings.

**Step 2: Visuell verifizieren**

```bash
cargo run
```

Checkliste:
- Beim Start: ein leerer Tab "Untitled" sichtbar, Path-Bar zeigt "Untitled — not saved"
- Cmd+O: Dateiauswahl öffnet sich, .md-Datei wird in neuem Tab geladen
- Tab-Label zeigt Filename, Path-Bar zeigt vollen Pfad
- Tippen → `•` erscheint im Tab-Label
- Cmd+S ohne URL → NSSavePanel; mit URL → direkt gespeichert, `•` verschwindet
- Cmd+N → neuer leerer Tab
- `×` auf dirty Tab → NSAlert erscheint
- `×` auf letzten Tab → Inhalt wird geleert, kein Absturz
- Appearance-Wechsel → alle Tabs erhalten korrekte Hintergrundfarbe

**Step 3: Release Build + DMG**

```bash
cargo build --release
rm -rf dist/mdit.app
mkdir -p dist/mdit.app/Contents/MacOS dist/mdit.app/Contents/Resources
cp target/release/mdit dist/mdit.app/Contents/MacOS/mdit
cp ressources/Info.plist dist/mdit.app/Contents/Info.plist
cp ressources/mdit-app-icon.icns dist/mdit.app/Contents/Resources/AppIcon.icns
rm -f dist/mdit-0.1.0.dmg
hdiutil create -volname "mdit" -srcfolder dist/mdit.app -ov -format UDZO dist/mdit-0.1.0.dmg
```

**Step 4: STATUS.md aktualisieren**

`docs/STATUS.md` — Multi-Tab File I/O als abgeschlossen markieren.

**Step 5: Final Commit**

```bash
git add docs/STATUS.md
git commit -m "docs: mark multi-tab file I/O as complete"
```
