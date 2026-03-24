# mdit — Roadmap to v1.0

## Kontext für neue Sessions

mdit ist ein nativer macOS Markdown-Editor in Rust + AppKit.
Kernfeature: Typora-artiges In-Space-Rendering (Syntax versteckt sich wenn Cursor den Span verlässt).
Vertrieb: Open Source, kostenlos auf GitHub (MIT-Lizenz).
Aktueller Stand: v0.1.0 — Kern funktioniert, Feinschliff fehlt noch.

**Wie diese Datei benutzen:**
Den nächsten noch offenen Punkt (erste ungetickte Checkbox) in einer neuen Session als Ziel angeben.
Erledigte Punkte bleiben abgehakt als Fortschritts-Dokumentation.

---

## Priorität 1 — Kern-Funktionen die fehlen

- [x] **Find & Replace (Cmd+F)**
  NSTextView hat ein eingebautes Find-Panel, das nur aktiviert werden muss (`usesFindPanel = true`).
  Zusätzlich: Menüeintrag unter `Edit > Find` verdrahten, `performFindPanelAction` korrekt weiterleiten.
  Relevante Dateien: `src/menu.rs`, `src/editor/text_view.rs`, `src/app.rs`

- [x] **Settings-Persistenz (UserDefaults)**
  Theme-Auswahl (Light/Dark/System) überlebt aktuell keinen Neustart — wird nicht gespeichert.
  Fix: `NSUserDefaults` beim Theme-Wechsel schreiben, beim App-Start auslesen.
  Relevante Dateien: `src/ui/appearance.rs`, `src/app.rs`

---

## Priorität 2 — UX-Verbesserungen

- [x] **Schriftgröße konfigurierbar (Cmd++ / Cmd+–)**
  Aktuell hardcoded 16pt in mehreren Dateien. Globale Konstante extrahieren,
  dann über Tastenkürzel und/oder Menüeintrag änderbar machen + in UserDefaults persistieren.
  Relevante Dateien: `src/ui/appearance.rs`, `src/editor/renderer.rs`, `src/menu.rs`

- [x] **Wordcount in der Path-Bar**
  Wörter und Zeichen live zählen, in der Path-Bar neben dem Dateipfad anzeigen.
  Nur im Editor-Modus sinnvoll; im Viewer-Modus optional ausblenden.
  Relevante Dateien: `src/ui/path_bar.rs`, `src/editor/document_state.rs`

- [x] **Feature-Discoverability: Cmd+E Hinweis**
  Das Viewer/Editor-Toggle (Cmd+E) ist das Kernfeature, aber vollständig unsichtbar.
  Einmaligen Tooltip oder Overlay beim ersten Start einblenden (via UserDefaults-Flag "hasSeenModeHint").
  Alternativ: Permanenter Hinweis im leeren Dokument-Zustand ("Drücke Cmd+E für Viewer-Modus").
  Relevante Dateien: `src/app.rs`, `src/ui/` (neues Overlay oder Erweiterung bestehender Views)

---

## Priorität 3 — Code-Qualität (bereits geplant)

- [ ] **Refactoring: Duplizierten Code extrahieren**
  ~475 Zeilen Duplikat über 5 Dateien. Utility-Funktionen extrahieren, überlange Funktionen aufteilen,
  englische Doc-Kommentare ergänzen. Keine Verhaltensänderungen.
  Plan liegt bereits unter: `docs/superpowers/plans/2026-03-18-code-quality-refactoring.md`

---

## Priorität 4 — GitHub Release vorbereiten

- [ ] **README.md ausbauen**
  Aktuell minimales README. Ergänzen: Screenshots, Feature-Liste, Installation (DMG + `cargo build`),
  Tastenkürzel-Übersicht, Contribution-Guidelines.

- [ ] **Erstes GitHub Release taggen (v1.0.0)**
  DMG unter `dist/` als Release-Asset hochladen. Release Notes schreiben.
  Tag: `v1.0.0`, Branch: `main`.

- [ ] **GitHub Repository Topics & Description setzen**
  Topics: `markdown`, `editor`, `macos`, `rust`, `appkit`, `typora`
  Description: "Native macOS Markdown editor. No Electron. 4 MB."

---

## Erledigte Punkte (Archiv)

*(Abgehakte Items hier sammeln für Überblick)*
