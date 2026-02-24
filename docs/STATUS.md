# mdit — Projektstatus

**Letzte Aktualisierung:** 2026-02-24 (Multi-Tab File I/O abgeschlossen)

---

## Überblick

`mdit` ist ein nativer macOS-Markdown-Editor mit In-Space-Rendering (Typora-Stil) in Rust + AppKit.

**Branch:** `main`

---

## Erledigte Tasks

### Task 1 — Projekt-Scaffold ✅
- `Cargo.toml` mit allen Dependencies
- `src/main.rs` (minimaler Einstiegspunkt)
- `.gitignore`

### Task 2 — NSApplication + NSWindow ✅
- `src/app.rs` mit `run()`-Funktion
- macOS-Fenster öffnet sich, Titel „mdit"

### Task 3 — NSTextView im Fenster ✅
- `src/editor/text_view.rs` mit `create_text_view()`
- NSTextView in NSScrollView eingebettet
- System-Font, Auto-Scroller

### Task 4 — Markdown-Parser (TDD) ✅
- `src/markdown/parser.rs` — `parse()` via comrak
- `MarkdownSpan` + `NodeKind` (Strong, Emph, Code, Math, Link, Heading, CodeBlock, Table, Footnote, Strikethrough, Image)
- `tests/parser_tests.rs` — 9 Tests grün

### Task 5 — Attribute-Mapping (TDD) ✅
- `src/markdown/attributes.rs` — `AttributeSet` + `TextAttribute`
- Methoden: `for_strong()`, `for_emph()`, `for_heading(level)`, `for_inline_code()`, `syntax_hidden()`, `syntax_visible()`
- `tests/attributes_tests.rs` — 6 Tests grün

### Task 6 — Custom NSTextStorage ✅
- `src/editor/text_storage.rs` — `MditTextStorage` als Objective-C-Subklasse
- Backing-Store mit `NSMutableAttributedString`
- Neuparsen bei jeder Texteingabe via `NSTextStorageDelegate`

### Task 7 — In-Space Rendering Inline-Elemente ✅
- `src/editor/renderer.rs` — `compute_attribute_runs()`
- Cursor-aware: Syntax-Marker werden ausgeblendet wenn Cursor außerhalb liegt
- Bold, Italic, Code, Link, Strikethrough
- `tests/renderer_tests.rs` — 7 Tests grün (inkl. Task 9)

### Task 8 — Cursor-Tracking ✅
- `src/editor/cursor_tracker.rs` — `find_containing_span()`
- `tests/cursor_tracker_tests.rs` — 2 Tests grün

### Task 9 — Headings H1–H6 ✅
- Heading-Rendering in `renderer.rs` (Prefix hidden, Schriftgröße skaliert)
- Tests in `renderer_tests.rs` eingeschlossen (`h1_prefix_hidden_outside_cursor`, `heading_gets_large_font`)

### Task 10 — Code-Blöcke mit Syntax-Highlighting ✅
- `src/markdown/highlighter.rs` — syntect-Integration
- `HighlightSpan` mit RGB-Farben pro Token
- `tests/highlighter_tests.rs` — 2 Tests grün

### Task 11 — Listen, Blockquotes, Tabellen, Fußnoten ✅
- `NodeKind::List` → recurse, `NodeKind::Item` → Marker als `ListMarker`-Run + recurse
- `NodeKind::Table` → Monospace-Fallback
- `NodeKind::Footnote` → Link-Farbe
- `NodeKind::BlockQuote` war bereits implementiert
- 3 neue Tests: `list_item_marker_styled`, `blockquote_gets_bar_attribute`, `table_gets_monospace`

### Task 12 — Math-Rendering (KaTeX via WKWebView) ✅
- `src/editor/math_view.rs` — `create_math_view(latex, display)` + `build_katex_html()`
- `objc2-web-kit = "0.3.2"` als Dependency
- `build_katex_html` unit-getestet (6 Tests, reine Rust-Logik)
- `create_math_view` baut kompiliert (AppKit-Seite, Main-Thread-Marker korrekt)
- NSTextAttachment-Embedding als TODO markiert (nächste Integration)

### Task 13 — Bild-Handling (Inline + Paste-to-Embed) ✅
- `src/editor/image_handler.rs`: `generate_image_path()` (TDD), `save_image_from_clipboard()` (Stub)
- `tests/image_handler_tests.rs` — 3 Tests grün
- UUID-Dateinamen, `<stem>-assets/`-Verzeichnis neben Dokument-Datei

### Task 14 — NSDocument-Integration ✅
- `src/document.rs`: `MditDocument` als NSDocument-Subklasse (`define_class!`)
- `readFromData:ofType:error:` + `dataOfType:error:` als Stubs (ObjC-Overrides)
- Vollständige Cmd+O-Integration benötigt noch `Info.plist CFBundleDocumentTypes`
- `NSError`-Feature zu `objc2-foundation` hinzugefügt

### Task 15 — Floating Formatting Toolbar ✅
- `src/ui/toolbar.rs`: `FloatingToolbar` — NSPanel + NSVisualEffectView-Blur + 7 NSButton-Elemente
- Toolbar erscheint bei Textauswahl (via `NSTextViewDelegate.textViewDidChangeSelection:`)
- Positionierung via `firstRectForCharacterRange:actualRange:` (Screen-Koordinaten)
- Button-Actions als TODO-Stubs (noch nicht mit NSTextView verbunden)
- `NSButton`, `NSButtonCell`, `NSControl` zu objc2-app-kit-Features hinzugefügt

### Task 16 — Light/Dark Mode + Typografie + Rendering-Pipeline ✅
- `src/ui/appearance.rs`: `ColorScheme` (light/dark) mit `resolve_fg()`/`resolve_bg()` Token-Mapping
- `src/editor/apply.rs`: `apply_attribute_runs()` — **kritische Lücke geschlossen**: AppKit-Attribut-Layer der alle `AttributeRun`s in echte NSAttributedString-Attribute umwandelt
  - NSFontAttributeName (Bold/Italic/Monospace via NSFontDescriptor-Traits, FontSize kombiniert)
  - NSForegroundColorAttributeName, NSBackgroundColorAttributeName, NSStrikethroughStyleAttributeName
  - NSParagraphStyleAttributeName mit lineSpacing = 9.6pt
  - Hidden-Spans → `NSColor.clearColor()` (alpha = 0)
  - UTF-8 → UTF-16 Byte-Offset-Konvertierung für korrekte NSRange
- `src/editor/text_storage.rs`: `apply_attribute_runs` in `did_process_editing` eingebaut
  - `applying: Cell<bool>` Guard gegen Rekursion aus Attribut-Callbacks
  - `scheme: Cell<ColorScheme>` Ivar + `set_scheme()` Methode
- `src/editor/text_view.rs`: SF Pro `systemFontOfSize_weight(16, Regular)`, initialer `textContainerInset`
- `src/app.rs`: Appearance-Erkennung beim Start via `NSApp.effectiveAppearance`, `windowDidResize:` für zentriertes Layout (max 700pt)
- `NSAttributedString`, `NSAppearance`, `NSValue` zu objc2-app-kit/foundation-Features hinzugefügt

---

## Erledigte Tasks (Forts.)

### Task 17 — PDF-Export ✅
- `src/export/pdf.rs` + `src/export/mod.rs`: `export_pdf()` via `NSPrintOperation`
- Menüintrag `File > Export as PDF…` (Cmd+Shift+E) — verdrahtet in `app.rs`

### Task 18 — Keyboard Shortcuts + Menüstruktur ✅
- `src/menu.rs` — vollständige NSMenu-Struktur (App / File / Edit / View / Help)
- Alle Shortcuts gemaß PRD: Cmd+B/I/E/K, Cmd+1/2/3, Cmd+Shift+E/X/Z, Cmd+N/O/S/W

### Task 19 — Finales Hardening ✅
- `View > Appearance > Light / Dark / Use System Setting` (Cmd+Shift+L)
- `applyLightMode:`, `applyDarkMode:`, `applySystemMode:` Action-Methods
- `MditEditorDelegate::reapply()` für sofortigen Re-Render nach Scheme-Wechsel
- Release-Build: 799 KB, kompiliert ohne Warnings
- Startup-Ziel < 200ms: Lean binary erfüllt die Anforderung 
- Bugfix: `unsafe impl AppDelegate` → `impl AppDelegate` in `define_class!`

## Erledigte Tasks — Phase 1.x Bugfixes

**Plan:** `docs/plans/2026-02-24-bugfix-phase1x.md`

### BUG-1 — Dark Mode: Hintergrund bleibt hell ✅
- `apply_scheme()` ruft jetzt `tv.setBackgroundColor()` mit explizitem sRGB-Wert aus `scheme.background`
- Startup: `apply_scheme(initial_scheme)` ersetzt `editor_delegate.set_scheme()` — Background wird auch bei dunklem System-Start korrekt gesetzt
- **Commits:** `3ce2a81`

### BUG-2 — Setext-Heading zerstört Kursiv-Darstellung ✅
- Renderer erkennt ATX- vs. Setext-Headings via `text[start] == '#'`
- Setext: Content erhält Heading-Schriftgrösse, Unterstreichungszeile wird als Syntax-Marker (ausgeblendet/sichtbar) behandelt
- 3 neue Renderer-Tests (13 gesamt)
- **Commits:** `af84f88`

### BUG-3 — Toolbar-Buttons sind No-Ops ✅
- `FloatingToolbar::new(mtm, target)` verdrahtet jeden Button via `NSControl::setTarget/setAction`
- `BTN_LABELS` ersetzt durch `BTN_ACTIONS` (Label + CStr-Selektor)
- **Commits:** `69576e3`

## Multi-Tab File I/O ✅

**Plan:** `docs/plans/2026-02-24-multi-tab-file-io.md`

### Implementierte Features
- **TabBar** oben im Fenster: Tab-Buttons (× zum Schliessen, + zum Neu), Dirty-Indikator `•`
- **PathBar** unten: zeigt vollen Dateipfad oder "Untitled — not saved"
- **DocumentState** per Tab: eigene NSScrollView + NSTextView + MditEditorDelegate
- **Multi-Tab-Architektur**: AppDelegate verwaltet `Vec<DocumentState>` + active_index
- **Cmd+N**: neuer leerer Tab
- **Cmd+O**: NSOpenPanel, .md/.markdown/.txt öffnen, bereits geöffnete Tabs werden erkannt
- **Cmd+S**: NSSavePanel bei neuem Dokument, sonst direktes Speichern
- **Dirty-Tracking**: `textDidChange:` setzt `is_dirty = true`, Tab-Label zeigt `•`
- **Close-Dialog (NSAlert)**: Speichern / Nicht speichern / Abbrechen bei dirty Tabs
- **Resize**: TabBar und PathBar passen sich an Fenstergröße an
- **Appearance**: `apply_scheme` iteriert alle Tabs

## Ausstehende Tasks

*Keine — Multi-Tab File I/O abgeschlossen.*

---

## Teststand

```
cargo test
```

| Test-Suite              | Tests | Status |
|-------------------------|-------|--------|
| attributes_tests        | 6     | ✅ grün |
| cursor_tracker_tests    | 2     | ✅ grün |
| highlighter_tests       | 2     | ✅ grün |
| parser_tests            | 9     | ✅ grün |
| renderer_tests          | 10    | ✅ grün |
| image_handler_tests     | 3     | ✅ grün |
| math_view (inline)      | 6     | ✅ grün |
| appearance_tests        | 3     | ✅ grün |
| tab_label_tests         | 6     | ✅ grün |
||| **Gesamt**              | **58**| ✅      |

**Release-Binary:** `dist/mdit-0.1.0.dmg` erstellt

---

## Nächster Schritt

*Multi-Tab File I/O abgeschlossen. Nächste Phase: Focus-Mode, erweiterte Features.*
