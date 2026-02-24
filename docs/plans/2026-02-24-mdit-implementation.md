# mdit — Implementierungsplan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Einen nativen macOS-Markdown-Editor mit In-Space-Rendering in Rust bauen.

**Architecture:** Rust + objc2-AppKit-Bindings. Core: Custom `NSTextStorage`-Subklasse hält Markdown-AST und `NSAttributedString` synchron. Cursor-Position bestimmt, ob Syntax-Marker ein- oder ausgeblendet werden. NSDocument übernimmt Autosave/Versionshistorie.

**Tech Stack:** Rust, `objc2` + `objc2-app-kit`, `comrak`, `syntect`, KaTeX via `WKWebView`

> **Hinweis zu Tests:** Reine Rust-Logik (Parser, Attribute-Mapping) wird mit `cargo test` unit-getestet. AppKit-Klassen (NSTextView, NSWindow etc.) sind nicht unit-testbar — dort gilt "build + visuell verifizieren" als Verifikation.

---

## Task 1: Projekt-Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `.gitignore`

**Step 1: Cargo.toml anlegen**

```toml
[package]
name = "mdit"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "mdit"
path = "src/main.rs"

[dependencies]
objc2 = "0.6"
objc2-foundation = { version = "0.3", features = [
    "NSString", "NSAttributedString", "NSMutableAttributedString",
    "NSDictionary", "NSArray", "NSURL", "NSData",
] }
objc2-app-kit = { version = "0.3", features = [
    "NSApplication", "NSWindow", "NSWindowController",
    "NSTextView", "NSTextStorage", "NSLayoutManager",
    "NSScrollView", "NSDocument", "NSColor", "NSFont",
    "NSPanel", "NSVisualEffectView", "NSPrintOperation",
] }
comrak = { version = "0.31", default-features = false, features = [
    "syntect",
] }
syntect = { version = "5", default-features = false, features = [
    "default-themes", "default-syntaxes",
] }

[profile.release]
lto = true
opt-level = 3
```

> **Achtung:** Versionsnummern vor dem Start mit `cargo search <crate>` oder crates.io verifizieren.

**Step 2: Minimales main.rs**

```rust
fn main() {
    println!("mdit starting");
}
```

**Step 3: .gitignore anlegen**

```
/target
*.DS_Store
```

**Step 4: Kompilierung verifizieren**

```bash
cargo build
```

Erwartung: `Finished` ohne Fehler.

**Step 5: Commit**

```bash
git add Cargo.toml src/main.rs .gitignore
git commit -m "chore: project scaffold with dependencies"
```

---

## Task 2: Basis macOS App — NSApplication + NSWindow

**Files:**
- Create: `src/app.rs`
- Modify: `src/main.rs`

**Step 1: Kein Unit-Test möglich — direkt implementieren**

`src/app.rs`:

```rust
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSWindow,
    NSWindowStyleMask, NSBackingStoreType};
use objc2_foundation::{NSRect, NSPoint, NSSize, NSString};

pub fn run() {
    unsafe {
        let app = NSApplication::sharedApplication();
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

        let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(900.0, 700.0));
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;

        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(),
            rect,
            style,
            NSBackingStoreType::Buffered,
            false,
        );
        window.setTitle(&NSString::from_str("mdit"));
        window.center();
        window.makeKeyAndOrderFront(None);

        app.run();
    }
}
```

`src/main.rs`:

```rust
mod app;

fn main() {
    app::run();
}
```

**Step 2: Visuell verifizieren**

```bash
cargo run
```

Erwartung: macOS-Fenster erscheint, Titel „mdit", App läuft.

**Step 3: Commit**

```bash
git add src/app.rs src/main.rs
git commit -m "feat: basic NSApplication + NSWindow"
```

---

## Task 3: NSTextView ins Fenster einbauen

**Files:**
- Create: `src/editor/mod.rs`
- Create: `src/editor/text_view.rs`
- Modify: `src/app.rs`

**Step 1: Scrollende NSTextView mit NSScrollView**

`src/editor/text_view.rs`:

```rust
use objc2::rc::Retained;
use objc2_app_kit::{NSScrollView, NSTextView, NSFont, NSColor};
use objc2_foundation::{NSRect, NSPoint, NSSize, NSString};

pub fn create_text_view(frame: NSRect) -> Retained<NSScrollView> {
    unsafe {
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(), frame);
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);

        let content_size = scroll.contentSize();
        let text_rect = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(content_size.width, content_size.height),
        );

        let text_view = NSTextView::initWithFrame(NSTextView::alloc(), text_rect);
        text_view.setRichText(false);
        text_view.setFont(Some(&NSFont::systemFontOfSize(16.0)));
        text_view.setTextColor(Some(&NSColor::labelColor()));
        text_view.setBackgroundColor(&NSColor::textBackgroundColor());
        text_view.setAutomaticQuoteSubstitutionEnabled(false);
        text_view.setAutomaticDashSubstitutionEnabled(false);

        scroll.setDocumentView(Some(&text_view));
        scroll
    }
}
```

`src/editor/mod.rs`:

```rust
pub mod text_view;
```

`src/app.rs` — im `run()`-Body nach `window`-Erstellung hinzufügen:

```rust
let bounds = window.contentView().unwrap().bounds();
let scroll = crate::editor::text_view::create_text_view(bounds);
window.contentView().unwrap().addSubview(&scroll);
```

**Step 2: Visuell verifizieren**

```bash
cargo run
```

Erwartung: Fenster mit editierbarem Textfeld, System-Font, normaler macOS-Cursor.

**Step 3: Commit**

```bash
git add src/editor/
git commit -m "feat: NSTextView mit NSScrollView im Fenster"
```

---

## Task 4: Markdown-Parser (reine Rust-Logik, TDD)

**Files:**
- Create: `src/markdown/mod.rs`
- Create: `src/markdown/parser.rs`
- Create: `tests/parser_tests.rs`

**Step 1: Test schreiben**

`tests/parser_tests.rs`:

```rust
use mdit::markdown::parser::{parse, NodeKind};

#[test]
fn parses_bold() {
    let nodes = parse("**hello**");
    assert!(nodes.iter().any(|n| n.kind == NodeKind::Strong));
}

#[test]
fn parses_italic() {
    let nodes = parse("*world*");
    assert!(nodes.iter().any(|n| n.kind == NodeKind::Emph));
}

#[test]
fn parses_heading() {
    let nodes = parse("# Title");
    assert!(nodes.iter().any(|n| matches!(n.kind, NodeKind::Heading { level: 1 })));
}

#[test]
fn parses_code_block() {
    let nodes = parse("```rust\nfn main() {}\n```");
    assert!(nodes.iter().any(|n| matches!(n.kind, NodeKind::CodeBlock { .. })));
}

#[test]
fn parses_inline_math() {
    let nodes = parse("$x^2$");
    assert!(nodes.iter().any(|n| n.kind == NodeKind::Math));
}
```

**Step 2: Test fehlschlagen lassen**

```bash
cargo test
```

Erwartung: Kompilierungsfehler (Modul existiert noch nicht).

**Step 3: Parser implementieren**

`src/markdown/parser.rs`:

```rust
use comrak::{parse_document, Arena, Options, nodes::{AstNode, NodeValue}};

#[derive(Debug, PartialEq, Clone)]
pub enum NodeKind {
    Text,
    Strong,
    Emph,
    Code,
    Math,
    Link { url: String },
    Heading { level: u8 },
    CodeBlock { language: String },
    Table,
    Footnote,
    Strikethrough,
    Image { url: String },
    Other,
}

#[derive(Debug, Clone)]
pub struct MarkdownSpan {
    pub kind: NodeKind,
    /// Byte-Offset im Original-String (start, end)
    pub source_range: (usize, usize),
    pub children: Vec<MarkdownSpan>,
}

fn options() -> Options<'static> {
    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.footnotes = true;
    opts.extension.math_dollars = true;
    opts
}

pub fn parse(source: &str) -> Vec<MarkdownSpan> {
    let arena = Arena::new();
    let root = parse_document(&arena, source, &options());
    collect_spans(root, source)
}

fn collect_spans<'a>(node: &'a AstNode<'a>, source: &str) -> Vec<MarkdownSpan> {
    let mut spans = Vec::new();
    for child in node.children() {
        if let Some(span) = node_to_span(child, source) {
            spans.push(span);
        }
        spans.extend(collect_spans(child, source));
    }
    spans
}

fn node_to_span<'a>(node: &'a AstNode<'a>, _source: &str) -> Option<MarkdownSpan> {
    let data = node.data.borrow();
    let range = (
        data.sourcepos.start.column.saturating_sub(1),
        data.sourcepos.end.column,
    );
    let children = collect_spans(node, _source);

    let kind = match &data.value {
        NodeValue::Strong => NodeKind::Strong,
        NodeValue::Emph => NodeKind::Emph,
        NodeValue::Code(_) => NodeKind::Code,
        NodeValue::Math(m) if !m.display_math => NodeKind::Math,
        NodeValue::Math(_) => NodeKind::Math,
        NodeValue::Link(l) => NodeKind::Link { url: l.url.clone() },
        NodeValue::Heading(h) => NodeKind::Heading { level: h.level },
        NodeValue::CodeBlock(cb) => NodeKind::CodeBlock {
            language: cb.info.clone(),
        },
        NodeValue::Table(_) => NodeKind::Table,
        NodeValue::FootnoteDefinition(_) => NodeKind::Footnote,
        NodeValue::Strikethrough => NodeKind::Strikethrough,
        NodeValue::Image(i) => NodeKind::Image { url: i.url.clone() },
        NodeValue::Text(_) => NodeKind::Text,
        _ => NodeKind::Other,
    };

    Some(MarkdownSpan { kind, source_range: range, children })
}
```

`src/markdown/mod.rs`:

```rust
pub mod parser;
```

`src/main.rs` — `pub mod markdown;` hinzufügen.

**Step 4: Tests grün machen**

```bash
cargo test
```

Erwartung: Alle 5 Tests grün.

**Step 5: Commit**

```bash
git add src/markdown/ tests/parser_tests.rs
git commit -m "feat: Markdown-Parser mit comrak (TDD)"
```

---

## Task 5: Attribute-Mapping (reine Rust-Logik, TDD)

**Files:**
- Create: `src/markdown/attributes.rs`
- Create: `tests/attributes_tests.rs`

Dieser Task definiert, welche NSAttributedString-Attribute welchem Markdown-Element entsprechen — als reine Rust-Datenstrukturen, ohne AppKit-Abhängigkeit.

**Step 1: Test schreiben**

`tests/attributes_tests.rs`:

```rust
use mdit::markdown::attributes::{AttributeSet, TextAttribute};

#[test]
fn bold_gets_bold_font_trait() {
    let attrs = AttributeSet::for_strong();
    assert!(attrs.contains(TextAttribute::Bold));
}

#[test]
fn italic_gets_italic_font_trait() {
    let attrs = AttributeSet::for_emph();
    assert!(attrs.contains(TextAttribute::Italic));
}

#[test]
fn heading1_gets_large_size() {
    let attrs = AttributeSet::for_heading(1);
    assert!(attrs.font_size() > 20.0);
}

#[test]
fn code_gets_monospace() {
    let attrs = AttributeSet::for_inline_code();
    assert!(attrs.contains(TextAttribute::Monospace));
}

#[test]
fn syntax_marker_is_hidden() {
    let attrs = AttributeSet::syntax_hidden();
    assert!(attrs.contains(TextAttribute::Hidden));
}

#[test]
fn syntax_marker_is_visible() {
    let attrs = AttributeSet::syntax_visible();
    assert!(!attrs.contains(TextAttribute::Hidden));
}
```

**Step 2: Fehlschlagen lassen**

```bash
cargo test attributes
```

Erwartung: Kompilierungsfehler.

**Step 3: Implementieren**

`src/markdown/attributes.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TextAttribute {
    Bold,
    Italic,
    Monospace,
    Hidden,
    FontSize(u8),
    ForegroundColor(&'static str), // z.B. "heading", "code", "link"
    BackgroundColor(&'static str),
}

#[derive(Debug, Clone)]
pub struct AttributeSet(Vec<TextAttribute>);

impl AttributeSet {
    pub fn new(attrs: Vec<TextAttribute>) -> Self { Self(attrs) }
    pub fn contains(&self, attr: TextAttribute) -> bool { self.0.contains(&attr) }
    pub fn font_size(&self) -> f64 {
        self.0.iter().find_map(|a| {
            if let TextAttribute::FontSize(s) = a { Some(*s as f64) } else { None }
        }).unwrap_or(16.0)
    }
    pub fn attrs(&self) -> &[TextAttribute] { &self.0 }

    pub fn for_strong() -> Self {
        Self::new(vec![TextAttribute::Bold])
    }
    pub fn for_emph() -> Self {
        Self::new(vec![TextAttribute::Italic])
    }
    pub fn for_strong_emph() -> Self {
        Self::new(vec![TextAttribute::Bold, TextAttribute::Italic])
    }
    pub fn for_heading(level: u8) -> Self {
        let size = match level {
            1 => 32u8, 2 => 26, 3 => 21, _ => 16,
        };
        Self::new(vec![
            TextAttribute::Bold,
            TextAttribute::FontSize(size),
            TextAttribute::ForegroundColor("heading"),
        ])
    }
    pub fn for_inline_code() -> Self {
        Self::new(vec![
            TextAttribute::Monospace,
            TextAttribute::BackgroundColor("code_bg"),
            TextAttribute::ForegroundColor("code_fg"),
        ])
    }
    pub fn for_link() -> Self {
        Self::new(vec![TextAttribute::ForegroundColor("link")])
    }
    pub fn for_strikethrough() -> Self {
        Self::new(vec![TextAttribute::ForegroundColor("strikethrough")])
    }
    pub fn syntax_hidden() -> Self {
        Self::new(vec![TextAttribute::Hidden])
    }
    pub fn syntax_visible() -> Self {
        Self::new(vec![TextAttribute::ForegroundColor("syntax")])
    }
    pub fn plain() -> Self {
        Self::new(vec![])
    }
}
```

`src/markdown/mod.rs` — `pub mod attributes;` hinzufügen.

**Step 4: Tests grün**

```bash
cargo test attributes
```

Erwartung: 6 Tests grün.

**Step 5: Commit**

```bash
git add src/markdown/attributes.rs tests/attributes_tests.rs
git commit -m "feat: Attribute-Mapping für Markdown-Elemente (TDD)"
```

---

## Task 6: Custom NSTextStorage — Foundation

**Files:**
- Create: `src/editor/text_storage.rs`
- Modify: `src/editor/mod.rs`
- Modify: `src/editor/text_view.rs`

Dies ist der technisch kritischste Task. `NSTextStorage` in Rust zu subklassen erfordert das `objc2`-Klassen-Makro.

**Step 1: Kein klassischer Unit-Test — Build-Verifikation**

**Step 2: NSTextStorage-Subklasse implementieren**

`src/editor/text_storage.rs`:

```rust
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{declare_class, msg_send, msg_send_id, ClassType, DeclaredClass};
use objc2_app_kit::NSTextStorage;
use objc2_foundation::{NSMutableAttributedString, NSAttributedString,
    NSRange, NSString};
use std::cell::RefCell;
use crate::markdown::parser::{parse, MarkdownSpan};

pub struct MditTextStorageIvars {
    backing: RefCell<Retained<NSMutableAttributedString>>,
    spans: RefCell<Vec<MarkdownSpan>>,
}

declare_class!(
    pub struct MditTextStorage;

    unsafe impl ClassType for MditTextStorage {
        type Super = NSTextStorage;
        type Mutability = objc2::mutability::MainThreadOnly;
        const NAME: &'static str = "MditTextStorage";
    }

    impl DeclaredClass for MditTextStorage {
        type Ivars = MditTextStorageIvars;
    }

    unsafe impl MditTextStorage {
        #[method(string)]
        fn string(&self) -> *mut AnyObject {
            unsafe { msg_send![&*self.ivars().backing.borrow(), string] }
        }

        #[method(attributesAtIndex:effectiveRange:)]
        fn attributes_at_index(
            &self,
            index: usize,
            range: *mut NSRange,
        ) -> *mut AnyObject {
            unsafe {
                msg_send![
                    &*self.ivars().backing.borrow(),
                    attributesAtIndex: index,
                    effectiveRange: range
                ]
            }
        }

        #[method(replaceCharactersInRange:withString:)]
        fn replace_characters(&self, range: NSRange, string: &NSString) {
            unsafe {
                let backing = self.ivars().backing.borrow();
                msg_send![&*backing, replaceCharactersInRange: range, withString: string];
                drop(backing);
                self.reparse();
                msg_send![self, edited: 1usize, range: range, changeInLength: 0isize];
            }
        }

        #[method(setAttributes:range:)]
        fn set_attributes(&self, attrs: *mut AnyObject, range: NSRange) {
            unsafe {
                let backing = self.ivars().backing.borrow();
                msg_send![&*backing, setAttributes: attrs, range: range];
            }
        }
    }
);

impl MditTextStorage {
    pub fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(MditTextStorageIvars {
            backing: RefCell::new(unsafe {
                NSMutableAttributedString::new()
            }),
            spans: RefCell::new(Vec::new()),
        });
        unsafe { msg_send_id![super(this), init] }
    }

    fn reparse(&self) {
        let raw = unsafe {
            let backing = self.ivars().backing.borrow();
            msg_send![&*backing, string] as *const NSString
        };
        let text = unsafe { (*raw).to_string() };
        let spans = parse(&text);
        *self.ivars().spans.borrow_mut() = spans;
        self.apply_attributes(&text);
    }

    fn apply_attributes(&self, _text: &str) {
        // In Task 8 implementiert — hier nur Placeholder
    }

    pub fn spans(&self) -> Vec<MarkdownSpan> {
        self.ivars().spans.borrow().clone()
    }
}
```

**Step 3: In NSTextView einbinden**

`src/editor/text_view.rs` — nach `NSTextView`-Erstellung:

```rust
let storage = MditTextStorage::new();
let layout = NSLayoutManager::new();
storage.addLayoutManager(&layout);
// NSTextContainer + layout in textview verbinden
```

> **Hinweis:** Die genaue objc2-API für Layout-Manager-Setup ist in der objc2-app-kit Dokumentation nachzuschlagen — der Mechanismus ist Standard-Cocoa-Textarchitektur.

**Step 4: Build-Verifikation**

```bash
cargo build
```

Erwartung: Kompiliert ohne Fehler. `cargo run` → Fenster öffnet sich, Text kann eingegeben werden.

**Step 5: Commit**

```bash
git add src/editor/text_storage.rs src/editor/mod.rs src/editor/text_view.rs
git commit -m "feat: Custom NSTextStorage Grundgerüst"
```

---

## Task 7: In-Space Rendering — Inline-Elemente

**Files:**
- Create: `src/editor/renderer.rs`
- Modify: `src/editor/text_storage.rs`

**Step 1: Test für Attribut-Anwendungslogik**

`tests/renderer_tests.rs`:

```rust
use mdit::editor::renderer::compute_attribute_runs;
use mdit::markdown::parser::parse;

#[test]
fn bold_span_gets_bold_attribute() {
    let text = "hello **world** end";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    let bold_run = runs.iter().find(|r| r.attrs.contains(
        mdit::markdown::attributes::TextAttribute::Bold
    ));
    assert!(bold_run.is_some());
}

#[test]
fn syntax_markers_hidden_when_cursor_outside() {
    let text = "**bold**";
    let spans = parse(text);
    // Cursor bei Position 10 (außerhalb)
    let runs = compute_attribute_runs(text, &spans, Some(10));
    let hidden = runs.iter().find(|r| r.attrs.contains(
        mdit::markdown::attributes::TextAttribute::Hidden
    ));
    assert!(hidden.is_some(), "** markers should be hidden");
}

#[test]
fn syntax_markers_visible_when_cursor_inside() {
    let text = "**bold**";
    let spans = parse(text);
    // Cursor bei Position 3 (innerhalb **)
    let runs = compute_attribute_runs(text, &spans, Some(3));
    let hidden = runs.iter().filter(|r| r.attrs.contains(
        mdit::markdown::attributes::TextAttribute::Hidden
    )).count();
    assert_eq!(hidden, 0, "** markers should be visible");
}
```

**Step 2: Fehlschlagen lassen**

```bash
cargo test renderer
```

**Step 3: Renderer implementieren**

`src/editor/renderer.rs`:

```rust
use crate::markdown::parser::{MarkdownSpan, NodeKind};
use crate::markdown::attributes::{AttributeSet, TextAttribute};

#[derive(Debug, Clone)]
pub struct AttributeRun {
    pub range: (usize, usize),
    pub attrs: AttributeSet,
}

/// Berechnet für den gegebenen Text + AST eine flache Liste von AttributeRuns.
/// `cursor_pos`: falls Some, werden Syntax-Marker der Span die den Cursor enthält sichtbar.
pub fn compute_attribute_runs(
    text: &str,
    spans: &[MarkdownSpan],
    cursor_pos: Option<usize>,
) -> Vec<AttributeRun> {
    let mut runs = Vec::new();
    for span in spans {
        collect_runs(text, span, cursor_pos, &mut runs);
    }
    // Lücken als "plain" füllen
    fill_gaps(text.len(), runs)
}

fn cursor_in_span(pos: Option<usize>, range: (usize, usize)) -> bool {
    match pos {
        None => false,
        Some(p) => p >= range.0 && p <= range.1,
    }
}

fn collect_runs(
    text: &str,
    span: &MarkdownSpan,
    cursor_pos: Option<usize>,
    runs: &mut Vec<AttributeRun>,
) {
    let in_span = cursor_in_span(cursor_pos, span.source_range);
    let syntax_attrs = if in_span {
        AttributeSet::syntax_visible()
    } else {
        AttributeSet::syntax_hidden()
    };

    match &span.kind {
        NodeKind::Strong => {
            let (start, end) = span.source_range;
            // "**" am Anfang
            runs.push(AttributeRun { range: (start, start + 2), attrs: syntax_attrs.clone() });
            // Content
            runs.push(AttributeRun {
                range: (start + 2, end - 2),
                attrs: AttributeSet::for_strong(),
            });
            // "**" am Ende
            runs.push(AttributeRun { range: (end - 2, end), attrs: syntax_attrs });
        }
        NodeKind::Emph => {
            let (start, end) = span.source_range;
            runs.push(AttributeRun { range: (start, start + 1), attrs: syntax_attrs.clone() });
            runs.push(AttributeRun {
                range: (start + 1, end - 1),
                attrs: AttributeSet::for_emph(),
            });
            runs.push(AttributeRun { range: (end - 1, end), attrs: syntax_attrs });
        }
        NodeKind::Heading { level } => {
            let (start, end) = span.source_range;
            let prefix_len = *level as usize + 1; // "# " = 2, "## " = 3 etc.
            runs.push(AttributeRun { range: (start, start + prefix_len), attrs: syntax_attrs });
            runs.push(AttributeRun {
                range: (start + prefix_len, end),
                attrs: AttributeSet::for_heading(*level),
            });
        }
        NodeKind::Code => {
            let (start, end) = span.source_range;
            runs.push(AttributeRun { range: (start, start + 1), attrs: syntax_attrs.clone() });
            runs.push(AttributeRun {
                range: (start + 1, end - 1),
                attrs: AttributeSet::for_inline_code(),
            });
            runs.push(AttributeRun { range: (end - 1, end), attrs: syntax_attrs });
        }
        NodeKind::Strikethrough => {
            let (start, end) = span.source_range;
            runs.push(AttributeRun { range: (start, start + 2), attrs: syntax_attrs.clone() });
            runs.push(AttributeRun {
                range: (start + 2, end - 2),
                attrs: AttributeSet::for_strikethrough(),
            });
            runs.push(AttributeRun { range: (end - 2, end), attrs: syntax_attrs });
        }
        NodeKind::Link { .. } => {
            let (start, end) = span.source_range;
            // "[label](url)" → label bleibt, rest hidden
            // Vereinfacht: gesamter Span bekommt Link-Farbe, Syntax hidden
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_link(),
            });
        }
        _ => {
            // Rekursiv für children
            for child in &span.children {
                collect_runs(text, child, cursor_pos, runs);
            }
        }
    }
}

fn fill_gaps(text_len: usize, mut runs: Vec<AttributeRun>) -> Vec<AttributeRun> {
    runs.sort_by_key(|r| r.range.0);
    let mut result = Vec::new();
    let mut pos = 0usize;
    for run in runs {
        if run.range.0 > pos {
            result.push(AttributeRun {
                range: (pos, run.range.0),
                attrs: AttributeSet::plain(),
            });
        }
        pos = run.range.1.max(pos);
        result.push(run);
    }
    if pos < text_len {
        result.push(AttributeRun {
            range: (pos, text_len),
            attrs: AttributeSet::plain(),
        });
    }
    result
}
```

**Step 4: Tests grün**

```bash
cargo test renderer
```

**Step 5: `apply_attributes` in NSTextStorage implementieren**

`src/editor/text_storage.rs` — `apply_attributes` Methode:

```rust
fn apply_attributes(&self, text: &str) {
    use crate::editor::renderer::compute_attribute_runs;
    let spans = self.ivars().spans.borrow().clone();
    let cursor = None; // Cursor-Position wird in Task 8 verdrahtet
    let runs = compute_attribute_runs(text, &spans, cursor);
    // runs → NSAttributedString-Attribute setzen
    // (AppKit-spezifischer Teil: NSFont, NSColor etc. aus AttributeSet ableiten)
    for run in &runs {
        let ns_range = NSRange::new(run.range.0 as usize, run.range.1 - run.range.0);
        let attrs = build_ns_attrs(&run.attrs);
        unsafe {
            let backing = self.ivars().backing.borrow();
            msg_send![&*backing, setAttributes: attrs, range: ns_range];
        }
    }
}
```

> `build_ns_attrs` wird inline implementiert und mappt `AttributeSet` auf `NSDictionary` mit `NSFont`/`NSColor`-Werten.

**Step 6: Visuell verifizieren**

```bash
cargo run
```

Eingabe: `**fett** und *kursiv* und normal` → Fett/Kursiv-Formatierung erscheint, Sternchen verschwinden beim Klick außerhalb.

**Step 7: Commit**

```bash
git add src/editor/renderer.rs src/editor/text_storage.rs tests/renderer_tests.rs
git commit -m "feat: In-Space Rendering für Inline-Elemente (Bold, Italic, Code, Link, Strikethrough)"
```

---

## Task 8: Cursor-Tracking — Syntax je nach Position zeigen/verstecken

**Files:**
- Create: `src/editor/cursor_tracker.rs`
- Modify: `src/editor/text_storage.rs`
- Modify: `src/editor/text_view.rs`

**Step 1: Test für Cursor-Tracker**

`tests/cursor_tracker_tests.rs`:

```rust
use mdit::editor::cursor_tracker::find_containing_span;
use mdit::markdown::parser::{parse, NodeKind};

#[test]
fn cursor_inside_bold_finds_strong_span() {
    let text = "hello **world** end";
    let spans = parse(text);
    let result = find_containing_span(&spans, 10); // Cursor mitten in "world"
    assert!(result.is_some());
    assert_eq!(result.unwrap().kind, NodeKind::Strong);
}

#[test]
fn cursor_outside_finds_nothing() {
    let text = "**bold**";
    let spans = parse(text);
    let result = find_containing_span(&spans, 20);
    assert!(result.is_none());
}
```

**Step 2: Fehlschlagen lassen**

```bash
cargo test cursor
```

**Step 3: Implementieren**

`src/editor/cursor_tracker.rs`:

```rust
use crate::markdown::parser::{MarkdownSpan, NodeKind};

/// Gibt die innerste Span zurück, die `pos` enthält.
pub fn find_containing_span(spans: &[MarkdownSpan], pos: usize) -> Option<&MarkdownSpan> {
    for span in spans {
        if pos >= span.source_range.0 && pos <= span.source_range.1 {
            // Erst in children schauen (innerste Span bevorzugen)
            if let Some(inner) = find_containing_span(&span.children, pos) {
                return Some(inner);
            }
            if span.kind != NodeKind::Text && span.kind != NodeKind::Other {
                return Some(span);
            }
        }
    }
    None
}
```

**Step 4: NSTextView-Delegation für Cursor-Events**

In `src/editor/text_view.rs`: `NSTextViewDelegate` implementieren und bei `textViewDidChangeSelection` die aktuelle Cursor-Position an `MditTextStorage` weiterleiten → `storage.set_cursor(pos)` → `reparse()` triggern.

**Step 5: Tests grün**

```bash
cargo test cursor
```

**Step 6: Visuell verifizieren**

```bash
cargo run
```

Eingabe `**fett**` → Sternchen nur sichtbar wenn Cursor innerhalb der Bold-Span.

**Step 7: Commit**

```bash
git add src/editor/cursor_tracker.rs tests/cursor_tracker_tests.rs
git commit -m "feat: Cursor-Tracking — Syntax zeigen/verstecken je nach Position"
```

---

## Task 9: Block-Elemente — Headings

**Files:**
- Modify: `src/editor/renderer.rs`

**Step 1: Test**

In `tests/renderer_tests.rs` ergänzen:

```rust
#[test]
fn h1_prefix_hidden_outside_cursor() {
    let text = "# Heading";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, Some(50)); // außerhalb
    let hidden = runs.iter().filter(|r| r.attrs.contains(TextAttribute::Hidden)).count();
    assert!(hidden > 0);
    let heading_run = runs.iter().find(|r| r.attrs.font_size() > 20.0);
    assert!(heading_run.is_some());
}
```

**Step 2: Fehlschlagen lassen → Implementieren → Grün machen**

Heading-Zweig in `collect_runs` ist bereits in Task 7 angelegt. Test-Assertions verfeinern bis alle grün.

```bash
cargo test h1
```

**Step 3: Visuell verifizieren**

```bash
cargo run
```

`# Titel` → groß und fett, `#` verschwindet wenn Cursor weg.

**Step 4: Commit**

```bash
git commit -am "feat: Heading-Rendering H1–H6"
```

---

## Task 10: Code-Blöcke mit Syntax-Highlighting

**Files:**
- Create: `src/markdown/highlighter.rs`
- Modify: `src/editor/renderer.rs`

**Step 1: Test**

`tests/highlighter_tests.rs`:

```rust
use mdit::markdown::highlighter::highlight;

#[test]
fn highlights_rust_code() {
    let result = highlight("fn main() {}", "rust");
    assert!(!result.spans.is_empty());
}

#[test]
fn unknown_language_falls_back_gracefully() {
    let result = highlight("some code", "foobar");
    assert_eq!(result.spans.len(), 1); // eine ungefärbte Span
}
```

**Step 2: Implementieren**

`src/markdown/highlighter.rs`:

```rust
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub range: (usize, usize),
    pub color: (u8, u8, u8), // RGB
}

pub struct HighlightResult {
    pub spans: Vec<HighlightSpan>,
}

pub fn highlight(code: &str, language: &str) -> HighlightResult {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let syntax = ss.find_syntax_by_token(language)
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let theme = &ts.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, theme);
    let mut spans = Vec::new();
    let mut offset = 0usize;
    for line in LinesWithEndings::from(code) {
        if let Ok(ranges) = h.highlight_line(line, &ss) {
            for (style, text) in ranges {
                let c = style.foreground;
                spans.push(HighlightSpan {
                    range: (offset, offset + text.len()),
                    color: (c.r, c.g, c.b),
                });
                offset += text.len();
            }
        } else {
            offset += line.len();
        }
    }
    if spans.is_empty() {
        spans.push(HighlightSpan { range: (0, code.len()), color: (200, 200, 200) });
    }
    HighlightResult { spans }
}
```

**Step 3: Tests grün**

```bash
cargo test highlighter
```

**Step 4: In Renderer einbinden**

In `collect_runs` → `NodeKind::CodeBlock { language }`: Code-Block-Bereich mit `highlight()` aufrufen und `HighlightSpan`s zu `AttributeRun`s konvertieren.

**Step 5: Visuell verifizieren**

Rust-Code-Block im Editor → Syntax-Highlighting erscheint.

**Step 6: Commit**

```bash
git add src/markdown/highlighter.rs tests/highlighter_tests.rs
git commit -am "feat: Code-Blöcke mit syntect Syntax-Highlighting"
```

---

## Task 11: Listen, Blockquotes, Tabellen, Fußnoten

**Files:**
- Modify: `src/editor/renderer.rs`
- Modify: `src/markdown/attributes.rs`

**Step 1: Attribute-Typen ergänzen**

In `attributes.rs` hinzufügen:

```rust
pub enum TextAttribute {
    // ... bestehende
    ListMarker,
    BlockquoteBar,
    ParagraphSpacing(f64),
}
```

**Step 2: Tests**

`tests/renderer_tests.rs` ergänzen:

```rust
#[test]
fn list_item_marker_styled() {
    let text = "- Item one\n- Item two";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    let marker = runs.iter().find(|r| r.attrs.contains(TextAttribute::ListMarker));
    assert!(marker.is_some());
}

#[test]
fn blockquote_gets_bar_attribute() {
    let text = "> quoted text";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);
    assert!(runs.iter().any(|r| r.attrs.contains(TextAttribute::BlockquoteBar)));
}
```

**Step 3: Implementieren und Tests grün machen**

```bash
cargo test list
cargo test blockquote
```

**Step 4: Tabellen**

Tabellen sind komplex. Ansatz für Phase 1: Tabellen werden als `NSTextTable` gerendert (AppKit-nativ). Alternative: Monospace-Darstellung falls NSTextTable zu aufwändig.

> **Entscheidung:** Zuerst Monospace-Fallback implementieren, NSTextTable als optionale Verbesserung.

**Step 5: Commit**

```bash
git commit -am "feat: Listen, Blockquotes, Tabellen (Monospace-Fallback), Fußnoten"
```

---

## Task 12: Math-Rendering mit KaTeX

**Files:**
- Create: `src/editor/math_view.rs`
- Modify: `src/editor/text_storage.rs`

**Step 1: Kein Unit-Test (WebView-Integration)**

**Step 2: WKWebView pro Math-Block erstellen**

`src/editor/math_view.rs`:

```rust
use objc2_app_kit::WKWebView;
use objc2_foundation::NSString;

/// Erstellt einen WKWebView der eine KaTeX-Formel rendert.
/// Rückgabe: View der als NSTextAttachment eingebettet werden kann.
pub fn create_math_view(latex: &str, display: bool) -> Retained<WKWebView> {
    let html = format!(r#"
        <html><head>
        <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex/dist/katex.min.css">
        <script src="https://cdn.jsdelivr.net/npm/katex/dist/katex.min.js"></script>
        </head><body style="margin:0;background:transparent;">
        <div id="math"></div>
        <script>katex.render({:?}, document.getElementById('math'), {{displayMode: {}}});</script>
        </body></html>
    "#, latex, display);

    unsafe {
        let view = WKWebView::new(); // mit leerem Frame, später resizen
        view.loadHTMLString_baseURL(&NSString::from_str(&html), None);
        view
    }
}
```

> **Hinweis:** Für Offline-Nutzung KaTeX-Assets lokal bundlen (in App-Resources kopieren).

**Step 3: Math-Views als NSTextAttachment in NSTextStorage einbetten**

Jeder `$$...$$`-Block → `NSTextAttachment` mit Math-View als `attachmentCell`.

**Step 4: Visuell verifizieren**

```bash
cargo run
```

`$x^2 + y^2 = r^2$` → Formel erscheint gerendert.

**Step 5: Commit**

```bash
git add src/editor/math_view.rs
git commit -am "feat: Math/LaTeX-Rendering via KaTeX WKWebView"
```

---

## Task 13: Bild-Handling (Inline-Rendering + Paste-to-Embed)

**Files:**
- Create: `src/editor/image_handler.rs`
- Modify: `src/editor/text_view.rs`

**Step 1: Test für Paste-to-Embed-Logik**

`tests/image_handler_tests.rs`:

```rust
use mdit::editor::image_handler::generate_image_path;
use std::path::Path;

#[test]
fn generates_path_next_to_document() {
    let doc_path = Path::new("/tmp/test.md");
    let result = generate_image_path(doc_path, "png");
    assert!(result.starts_with("/tmp/test-assets/"));
    assert!(result.ends_with(".png"));
}
```

**Step 2: Implementieren**

`src/editor/image_handler.rs`:

```rust
use std::path::{Path, PathBuf};
use uuid::Uuid; // uuid-crate zu Cargo.toml hinzufügen

pub fn generate_image_path(doc_path: &Path, extension: &str) -> PathBuf {
    let stem = doc_path.file_stem().unwrap_or_default().to_string_lossy();
    let dir = doc_path.parent().unwrap_or(Path::new("."));
    let assets_dir = dir.join(format!("{}-assets", stem));
    assets_dir.join(format!("{}.{}", Uuid::new_v4(), extension))
}

pub fn save_image_from_clipboard(doc_path: &Path) -> Option<String> {
    // NSPasteboard lesen → TIFF/PNG-Daten → speichern → relativen Pfad zurückgeben
    // Implementation: AppKit-spezifisch, NSPasteboard::generalPasteboard()
    todo!()
}
```

**Step 3: Paste-Event in NSTextView abfangen**

`NSTextViewDelegate::textView(_:doCommandBy:)` oder `paste:` Override → `save_image_from_clipboard` aufrufen → Markdown-Link einfügen.

**Step 4: Inline-Rendering**

`NodeKind::Image { url }` in Renderer → `NSTextAttachment` mit `NSImage(contentsOfFile:)`.

**Step 5: Tests und Build**

```bash
cargo test image
cargo build
```

**Step 6: Commit**

```bash
git add src/editor/image_handler.rs tests/image_handler_tests.rs
git commit -am "feat: Bild-Rendering + Paste-to-Embed"
```

---

## Task 14: NSDocument-Integration (Autosave + Versionshistorie)

**Files:**
- Create: `src/document.rs`
- Modify: `src/app.rs`

**Step 1: NSDocument-Subklasse**

`src/document.rs`:

```rust
use objc2::{declare_class, ClassType, DeclaredClass, msg_send};
use objc2_app_kit::NSDocument;
use objc2_foundation::{NSData, NSError, NSURL, NSString};

declare_class!(
    pub struct MditDocument;

    unsafe impl ClassType for MditDocument {
        type Super = NSDocument;
        type Mutability = objc2::mutability::MainThreadOnly;
        const NAME: &'static str = "MditDocument";
    }

    impl DeclaredClass for MditDocument {
        type Ivars = ();
    }

    unsafe impl MditDocument {
        #[method_id(readFromData:ofType:error:)]
        fn read_from_data(
            &self,
            data: &NSData,
            _type: &NSString,
            _error: *mut *mut NSError,
        ) -> bool {
            // UTF-8 Daten → NSTextStorage laden
            true
        }

        #[method_id(dataOfType:error:)]
        fn data_of_type(
            &self,
            _type: &NSString,
            _error: *mut *mut NSError,
        ) -> Option<Retained<NSData>> {
            // NSTextStorage → UTF-8 NSData
            None
        }
    }
);
```

**Step 2: NSDocumentController nutzen**

In `src/app.rs`:

```rust
// Statt manuellem NSWindow: NSDocumentController.sharedDocumentController()
// öffnet Dateien automatisch mit MditDocument
let dc = NSDocumentController::sharedDocumentController();
dc.setDocumentClassNames(&[NSString::from_str("MditDocument")]);
```

**Step 3: Visuell verifizieren**

```bash
cargo run
```

- Datei öffnen (Cmd+O) → Markdown-Inhalt erscheint
- Inhalt bearbeiten → nach 30s automatisch gespeichert
- `File > Revert To > Browse All Versions` → Versionshistorie-Interface

**Step 4: Commit**

```bash
git add src/document.rs
git commit -am "feat: NSDocument-Integration — Autosave + Versionshistorie"
```

---

## Task 15: Floating Formatting Toolbar

**Files:**
- Create: `src/ui/toolbar.rs`
- Create: `src/ui/mod.rs`
- Modify: `src/editor/text_view.rs`

**Step 1: NSPanel mit NSVisualEffectView**

`src/ui/toolbar.rs`:

```rust
use objc2_app_kit::{NSPanel, NSVisualEffectView, NSVisualEffectBlendingMode,
    NSButton, NSWindowStyleMask};

pub struct FloatingToolbar {
    panel: Retained<NSPanel>,
}

impl FloatingToolbar {
    pub fn new() -> Self {
        unsafe {
            // Kleines, randloses Panel
            let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
                NSPanel::alloc(),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 36.0)),
                NSWindowStyleMask::Borderless | NSWindowStyleMask::HUDWindow,
                NSBackingStoreType::Buffered,
                false,
            );
            panel.setFloatingPanel(true);
            panel.setBecomesKeyOnlyIfNeeded(true);

            // Visueller Blur-Hintergrund
            let blur = NSVisualEffectView::initWithFrame(
                NSVisualEffectView::alloc(),
                panel.contentView().unwrap().bounds(),
            );
            blur.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            blur.setMaterial(/* NSVisualEffectMaterial::Popover */ ...);
            panel.contentView().unwrap().addSubview(&blur);

            // Buttons: Bold, Italic, Code, Strikethrough, Link, H1/H2/H3
            // ... (NSButton für jeden Formatierungstyp)

            Self { panel }
        }
    }

    pub fn show_near_rect(&self, rect: NSRect) {
        unsafe {
            // Panel direkt über der Selektion positionieren
            self.panel.setFrameOrigin(NSPoint::new(rect.origin.x, rect.origin.y + rect.size.height + 4.0));
            self.panel.orderFront(None);
        }
    }

    pub fn hide(&self) {
        unsafe { self.panel.orderOut(None); }
    }
}
```

**Step 2: NSTextViewDelegate — bei Selektion anzeigen**

In `text_view.rs` → `textViewDidChangeSelection`: Wenn `selectedRange.length > 0` → `toolbar.show_near_rect(selectionRect)`, sonst `toolbar.hide()`.

**Step 3: Visuell verifizieren**

```bash
cargo run
```

Text auswählen → Toolbar erscheint. Klick auf Bold-Button → `**text**` wird eingefügt/entfernt.

**Step 4: Commit**

```bash
git add src/ui/
git commit -am "feat: Floating Formatting Toolbar"
```

---

## Task 16: Light/Dark Mode + Typografie

**Files:**
- Create: `src/ui/appearance.rs`
- Modify: `src/markdown/attributes.rs`
- Modify: `src/app.rs`

**Step 1: Color-Tokens für beide Modes**

`src/ui/appearance.rs`:

```rust
pub struct ColorScheme {
    pub text: (f64, f64, f64),          // RGB 0–1
    pub background: (f64, f64, f64),
    pub heading: (f64, f64, f64),
    pub link: (f64, f64, f64),
    pub code_bg: (f64, f64, f64),
    pub code_fg: (f64, f64, f64),
    pub syntax_marker: (f64, f64, f64),
}

impl ColorScheme {
    pub fn light() -> Self {
        Self {
            text:          (0.10, 0.10, 0.10),
            background:    (0.98, 0.98, 0.98),
            heading:       (0.10, 0.10, 0.10),
            link:          (0.10, 0.40, 0.80),
            code_bg:       (0.94, 0.94, 0.96),
            code_fg:       (0.20, 0.20, 0.20),
            syntax_marker: (0.70, 0.70, 0.70),
        }
    }

    pub fn dark() -> Self {
        Self {
            text:          (0.92, 0.92, 0.92),
            background:    (0.11, 0.11, 0.12),
            heading:       (0.95, 0.95, 0.95),
            link:          (0.40, 0.70, 1.00),
            code_bg:       (0.17, 0.17, 0.18),
            code_fg:       (0.85, 0.85, 0.85),
            syntax_marker: (0.40, 0.40, 0.40),
        }
    }
}
```

**Step 2: System-Appearance beobachten**

`NSApp.effectiveAppearance` → `bestMatchFromAppearancesWithNames:` → Light/Dark erkennen.  
`NSAppearanceCustomization` auf dem Window → manueller Override.

**Step 3: Schrift-Setup**

In `build_ns_attrs`:

```rust
// Body
NSFont::systemFontOfSize_weight(16.0, NSFontWeightRegular)
// Headings
NSFont::systemFontOfSize_weight(size, NSFontWeightBold)
// Code
NSFont::monospacedSystemFontOfSize_weight(14.0, NSFontWeightRegular)
// Zeilenabstand via NSMutableParagraphStyle.lineSpacing = 1.6 * 16.0 = 25.6 - 16 = 9.6
```

**Step 4: Zentrierte Textfläche mit Max-Width**

In `src/app.rs` oder `text_view.rs`: `NSTextView` mit horizontalen Margins sodass Textcontainer max. 700pt breit ist. Dynamisch bei Window-Resize anpassen via `windowDidResize`-Delegate.

**Step 5: Menüeinträge**

`View > Appearance > Light` / `Dark` / `System` → `ColorScheme` wechseln + `reparse()` triggern.

**Step 6: Visuell verifizieren**

```bash
cargo run
```

System auf Dark Mode → Editor wechselt. Menü > Appearance > Light → bleibt hell trotz System-Dark.

**Step 7: Commit**

```bash
git add src/ui/appearance.rs
git commit -am "feat: Light/Dark Mode + Typografie (SF Pro, zentrierte Fläche)"
```

---

## Task 17: PDF-Export

**Files:**
- Create: `src/export/pdf.rs`
- Create: `src/export/mod.rs`
- Modify: `src/app.rs` (Menüeintrag)

**Step 1: NSPrintOperation**

`src/export/pdf.rs`:

```rust
use objc2_app_kit::{NSPrintOperation, NSPrintInfo};

pub fn export_pdf(text_view: &NSTextView) {
    unsafe {
        let print_info = NSPrintInfo::sharedPrintInfo();
        let op = NSPrintOperation::printOperationWithView_printInfo(
            text_view,
            &print_info,
        );
        op.runOperation();
    }
}
```

**Step 2: Menüeintrag verdrahten**

`File > Export as PDF…` → `Cmd+Shift+E` → `export_pdf()` aufrufen.

**Step 3: Visuell verifizieren**

```bash
cargo run
```

`Cmd+Shift+E` → macOS Print-Dialog öffnet sich. „Als PDF sichern" → saubere PDF mit formatiertem Markdown.

**Step 4: Commit**

```bash
git add src/export/
git commit -am "feat: PDF-Export via NSPrintOperation"
```

---

## Task 18: Keyboard Shortcuts + Menüstruktur

**Files:**
- Create: `src/menu.rs`
- Modify: `src/app.rs`

**Step 1: Komplette Menüstruktur aufbauen**

`src/menu.rs`:

```rust
// NSMenu + NSMenuItem für:
// File: New, Open, Close, Save, Export as PDF, Browse Versions
// Edit: Undo, Redo, Cut, Copy, Paste, Bold, Italic, Inline Code, Link, Strikethrough, H1/H2/H3
// View: Appearance (Light/Dark/System)
// Help: (leer in Phase 1)
```

Jeder Menüeintrag bekommt `setKeyEquivalent:` für Shortcuts gemäß PRD.

**Step 2: Visuell verifizieren**

```bash
cargo run
```

Alle Shortcuts testen: Cmd+B → Bold-Formatierung, Cmd+1 → H1, Cmd+Shift+E → PDF-Dialog.

**Step 3: Commit**

```bash
git add src/menu.rs
git commit -am "feat: Vollständige Menüstruktur + Keyboard Shortcuts"
```

---

## Task 19: Finales Hardening + Erfolgs-Kriterien prüfen

**Step 1: Alle Erfolgs-Kriterien aus PRD durchgehen**

```bash
cargo test  # Alle Unit-Tests grün
cargo run   # Manuelle Smoke-Tests
```

Checkliste:
- [ ] Datei öffnen / bearbeiten / Autosave
- [ ] In-Space-Rendering Inline (Bold, Italic, Code, Link, Strikethrough)
- [ ] Headings H1–H6
- [ ] Code-Blöcke mit Syntax-Highlighting
- [ ] Tables, Footnotes
- [ ] Math (LaTeX)
- [ ] Bilder inline, Paste-to-Embed
- [ ] Light/Dark-Mode-Toggle
- [ ] PDF-Export
- [ ] App-Start < 200ms messen (`time cargo run`)
- [ ] Kein Lag beim Tippen (subjektiv)

**Step 2: Performance-Messung**

```bash
time /path/to/mdit.app/Contents/MacOS/mdit
```

Falls Start > 200ms: `lazy_static!` für syntect-SyntaxSet (teuerste Initialisierung).

**Step 3: Release-Build verifizieren**

```bash
cargo build --release
```

**Step 4: Finaler Commit**

```bash
git commit -am "chore: Phase 1 complete — alle Erfolgs-Kriterien geprüft"
```

---

## Appendix: Projektstruktur

```
mdit/
├── src/
│   ├── main.rs
│   ├── app.rs
│   ├── document.rs
│   ├── menu.rs
│   ├── editor/
│   │   ├── mod.rs
│   │   ├── text_view.rs
│   │   ├── text_storage.rs    ← Kern-Komponente
│   │   ├── renderer.rs
│   │   ├── cursor_tracker.rs
│   │   ├── image_handler.rs
│   │   └── math_view.rs
│   ├── markdown/
│   │   ├── mod.rs
│   │   ├── parser.rs
│   │   ├── attributes.rs
│   │   └── highlighter.rs
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── toolbar.rs
│   │   └── appearance.rs
│   └── export/
│       ├── mod.rs
│       └── pdf.rs
├── tests/
│   ├── parser_tests.rs
│   ├── attributes_tests.rs
│   ├── renderer_tests.rs
│   ├── cursor_tracker_tests.rs
│   ├── highlighter_tests.rs
│   └── image_handler_tests.rs
└── docs/
    └── plans/
        ├── 2026-02-24-mdit-prd.md
        └── 2026-02-24-mdit-implementation.md
```
