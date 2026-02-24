# mdit — Projektstatus

**Letzte Aktualisierung:** 2026-02-24

---

## Überblick

`mdit` ist ein nativer macOS-Markdown-Editor mit In-Space-Rendering (Typora-Stil) in Rust + AppKit.

**Branch:** `feat/implementation`  
**Worktree:** `.worktrees/implementation`

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

---

## Ausstehende Tasks

### Task 11 — Listen, Blockquotes, Tabellen, Fußnoten 🔜
- `TextAttribute`: `ListMarker`, `BlockquoteBar`, `ParagraphSpacing` ergänzen
- `renderer.rs`: NodeKind::List, Item, BlockQuote, Table, FootnoteDefinition behandeln
- Tabellen: Monospace-Fallback (Phase 1)
- Tests in `renderer_tests.rs`

### Task 12 — Math-Rendering (KaTeX via WKWebView)
- `src/editor/math_view.rs`
- `$...$` und `$$...$$` → WKWebView als NSTextAttachment

### Task 13 — Bild-Handling (Inline + Paste-to-Embed)
- `src/editor/image_handler.rs` (Stub existiert bereits in `mod.rs`)
- `generate_image_path()`, `save_image_from_clipboard()`
- NSPasteboard-Integration, NSTextAttachment für Inline-Bilder

### Task 14 — NSDocument-Integration
- `src/document.rs` — NSDocument-Subklasse
- Autosave + Versionshistorie via macOS
- Öffnen / Speichern von `.md`-Dateien

### Task 15 — Floating Formatting Toolbar
- `src/ui/toolbar.rs` — NSPanel mit NSVisualEffectView
- Erscheint bei Textauswahl über der Selektion
- Buttons: Bold, Italic, Code, Strikethrough, Link, H1/H2/H3

### Task 16 — Light/Dark Mode + Typografie
- `src/ui/appearance.rs` — `ColorScheme` (light/dark)
- SF Pro Body/Heading, Monospace für Code
- Zentrierte Textfläche, max. 700pt breit

### Task 17 — PDF-Export
- `src/export/pdf.rs` — NSPrintOperation
- Menüeintrag `File > Export as PDF…` (Cmd+Shift+E)

### Task 18 — Keyboard Shortcuts + Menüstruktur
- `src/menu.rs` — vollständige NSMenu-Struktur
- File / Edit / View / Help

### Task 19 — Finales Hardening
- Alle Erfolgs-Kriterien aus PRD prüfen
- Performance: App-Start < 200ms
- Release-Build verifizieren

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
| renderer_tests          | 7     | ✅ grün |
| **Gesamt**              | **26**| ✅      |

---

## Nächster Schritt

**Task 11: Listen, Blockquotes, Tabellen, Fußnoten** — `attributes.rs` erweitern, Renderer für Block-Elemente implementieren, Tests grün machen.
