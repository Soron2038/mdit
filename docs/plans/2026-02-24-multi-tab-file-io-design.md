# Multi-Tab File I/O — Design Document

**Datum:** 2026-02-24
**Status:** Approved

## Ziel

Bestehende `.md`-Dateien öffnen, bearbeiten und speichern. Mehrere Dokumente gleichzeitig als Tabs im selben Fenster. Kein Autosave.

## Entscheidungen

- **Ansatz:** Custom Tab-Leiste + ein `NSScrollView/NSTextView`-Paar **pro Tab** (kein Content-Swap). Jeder Tab behält seine eigene Undo-History, Cursor-Position und Scroll-Position.
- **Kein NSDocumentController:** File-I/O wird manuell über `NSOpenPanel`/`NSSavePanel` und direkte UTF-8-Lese-/Schreiboperationen umgesetzt. Kein Autosave, keine Versionshistorie.
- **Kein Autosave:** Nutzer entscheidet explizit per Cmd+S. Schützt vor versehentlichen Änderungen bei falschem Fenster-Fokus.

## Datenmodell

```
DocumentState {
    scroll_view:     Retained<NSScrollView>
    text_view:       Retained<NSTextView>
    editor_delegate: Retained<MditEditorDelegate>
    url:             Option<PathBuf>   // None = "Untitled"
    is_dirty:        bool
}
```

`AppDelegate` hält:
- `tabs: RefCell<Vec<DocumentState>>`
- `active_index: Cell<usize>`
- `tab_bar: OnceCell<TabBar>`
- `path_bar: OnceCell<PathBar>`

## Layout

```
┌─────────────────────────────────────────┐
│  notes.md  × │ • todo.md  × │     [+]   │  TabBar  (32pt)
├─────────────────────────────────────────┤
│                                         │
│         NSScrollView (aktiver Tab)      │
│                                         │
├─────────────────────────────────────────┤
│  /Users/witt/Documents/notes.md         │  PathBar (22pt)
└─────────────────────────────────────────┘
```

## Tab-Leiste

- Tab-Buttons: Filename + `•`-Prefix wenn dirty, `×`-Close-Button per Tab
- `+`-Button rechts: neues Dokument
- Button-Tags codieren den Tab-Index (für `switchToTab:` / `closeTab:`)

## Path-Bar

- `NSTextField`, nicht editierbar, 11pt, `secondaryLabelColor`
- Inhalt: voller Pfad oder `"Untitled — not saved"`

## File Operations

| Aktion | Verhalten |
|---|---|
| `openDocument:` (Cmd+O) | NSOpenPanel → UTF-8 laden → neuer Tab (oder zu bestehendem Tab wechseln falls bereits offen) |
| `saveDocument:` (Cmd+S) | Hat URL → direkt schreiben; kein URL → NSSavePanel |
| `newDocument:` (Cmd+N) | Leerer Tab |
| Tab schließen `×` | Dirty → NSAlert (Save / Don't Save / Cancel); letzter Tab → Inhalt leeren statt Tab entfernen |

## Dirty Tracking

`NSTextDelegate::textDidChange:` auf `AppDelegate` → aktiver Tab `is_dirty = true` → Tab-Label mit `•` aktualisieren. Nach Save: `is_dirty = false`, `•` entfernen.

## Kompatibilität bestehender Features

- Formatting-Actions (`applyBold:` etc.) und Toolbar-Buttons: routen über `active_text_view()` — kein Change
- `apply_scheme()`: iteriert alle Tabs statt nur den aktiven
- `update_text_container_inset()`: wirkt auf aktiven Tab
- `textViewDidChangeSelection:`: Toolbar show/hide bleibt unverändert
