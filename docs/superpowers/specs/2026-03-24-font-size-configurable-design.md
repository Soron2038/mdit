# Design: Konfigurierbare Schriftgröße

**Datum:** 2026-03-24
**Status:** Approved
**Bereich:** UX — Priorität 2 aus FINISHING.md

---

## Problem

Die Basisschriftgröße in mdit ist mit 16pt hardcoded (in `apply.rs` und `attributes.rs`). Überschriftengrößen sind ebenfalls absolute Werte. Benutzer können die Schriftgröße nicht anpassen, und die Einstellung wird nicht gespeichert.

---

## Lösung

Schriftgröße konfigurierbar machen über Cmd++/Cmd+–/Cmd+0, persistent über `NSUserDefaults` — identisch zum bestehenden Theme-Pattern.

---

## Entscheidungen

| Thema | Entscheidung |
|-------|-------------|
| Bereich | 12–24pt, 1pt pro Schritt |
| Default | 16pt |
| Headings | Proportional skalieren (Faktoren, nicht absolute Werte) |
| H3-Größe | ×1.0 (= Body; Unterschied nur durch Farbe/Bold) — **bewusste Änderung** gegenüber bisherigen 15pt |
| State | `body_font_size: f64` als AppDelegate-Ivar + `base_size: Cell<f64>` in `MditEditorDelegate` |
| Persistenz | `NSUserDefaults`, Key `"mditFontSize"` |
| Architektur | Parameter durchreichen (kein globaler State) |

---

## Heading-Faktoren

| Level | Faktor | Bei 16pt | Bei 20pt |
|-------|--------|----------|----------|
| H1    | ×1.375 | 22pt     | 28pt     |
| H2    | ×1.125 | 18pt     | 23pt     |
| H3    | ×1.0   | 16pt     | 20pt     |
| Body  | ×1.0   | 16pt     | 20pt     |

Berechnung: `(base_size * factor).round() as u8`

**Hinweis H3:** Bisher war H3 = 15pt (kleiner als Body). Die neue ×1.0-Regel ist eine bewusste Verbesserung — H3 wird von Body ausschließlich durch Farbe/Bold unterschieden, nicht durch Verkleinerung.

---

## Tastenkürzel & Menü

Im View-Menü nach der Appearance-Sektion (Separator davor):

| Menüitem | Shortcut | Selector |
|----------|----------|----------|
| Schrift vergrößern | Cmd++ | `increaseFontSize:` |
| Schrift verkleinern | Cmd+– | `decreaseFontSize:` |
| Standardgröße | Cmd+0 | `resetFontSize:` |

---

## Betroffene Dateien

### `src/markdown/attributes.rs`

`AttributeSet::for_heading(level: u8)` → `for_heading(level: u8, base_size: f64)`

Bisher:
```rust
fn for_heading(level: u8) -> Self {
    let size = match level { 1 => 22, 2 => 18, 3 => 15, _ => 16 };
    // ...
}
```

Neu:
```rust
fn for_heading(level: u8, base_size: f64) -> Self {
    let size = match level {
        1 => (base_size * 1.375).round() as u8,
        2 => (base_size * 1.125).round() as u8,
        _ => base_size as u8,
    };
    // ...
}
```

Alle Aufrufstellen von `for_heading()` anpassen (Markdown-Parser, Tests).

### `src/editor/apply.rs`

1. `reset_to_body_style(storage, base_size: f64)` — ersetzt hardcoded `16.0` in Zeile 142

2. `apply_attribute_runs(storage, cursor, base_size: f64)` — neuer dritter Parameter; reicht `base_size` an `reset_to_body_style` und `build_font` weiter

3. `build_font` / Monospace-Anpassung (Zeile 563):

   ```rust
   // Alt (kaputt bei base_size ≠ 16):
   let code_size = if size == 16.0 { 14.0 } else { size };
   // Neu (relativ):
   let code_size = size - 2.0;
   ```

   Inline-Code ist immer 2pt kleiner als die jeweils aktuelle Größe.

4. `font_size()` Fallback: Body-Runs tragen kein `FontSize`-Attribut und erhalten ihre Größe aus `reset_to_body_style`. Der `unwrap_or`-Fallback in `font_size()` wird nie für Body-Text getroffen. Er bleibt als Sicherheitsnetz auf `base_size` (aus Parameter in `build_font`):

   ```rust
   fn build_font(attrs: &AttributeSet, base_size: f64) -> Retained<NSFont> {
       let size = attrs.font_size().unwrap_or(base_size);
       // ...
   }
   ```

   Signatur von `font_size()` selbst bleibt unverändert.

### `src/editor/text_storage.rs` (MditEditorDelegate)

**Kritisch: `base_size` muss hier gelagert und in die Pipeline eingefädelt werden.**

Neues Ivar:
```rust
base_size: Cell<f64>,
```

Neue Accessor-Methoden:
```rust
fn base_size(&self) -> f64 { self.ivars().base_size.get() }
fn set_base_size(&self, size: f64) { self.ivars().base_size.set(size); }
```

In `did_process_editing` und `reapply`: `apply_attribute_runs(storage, cursor, self.base_size())` statt des bisherigen Aufrufs ohne `base_size`.

Initialisierung: `base_size` auf `DEFAULT_FONT_SIZE` (16.0) beim Erstellen des Delegates setzen.

### `src/app.rs`

Neue Konstanten:
```rust
const FONT_SIZE_PREF_KEY: &str = "mditFontSize";
const DEFAULT_FONT_SIZE: f64 = 16.0;
const MIN_FONT_SIZE: f64 = 12.0;
const MAX_FONT_SIZE: f64 = 24.0;
```

Neues Ivar in `AppDelegateIvars`:
```rust
body_font_size: Cell<f64>,
```

Neue Hilfsfunktionen (analog zu `save_theme_pref` / `load_theme_pref`):
```rust
fn save_font_size_pref(size: f64) { /* defaults.setDouble:forKey: */ }
fn load_font_size_pref() -> f64 { /* defaults.doubleForKey:, default DEFAULT_FONT_SIZE */ }
```

Startup (`applicationDidFinishLaunching`):
```rust
let size = load_font_size_pref();
self.ivars().body_font_size.set(size);
// Alle beim Start geladenen Tabs: delegate.set_base_size(size)
```

Neue Action-Methods (registriert mit `sel!(...)`, exposed via `#[method]`):
```rust
fn increase_font_size(&self, _sender: Option<&AnyObject>);
fn decrease_font_size(&self, _sender: Option<&AnyObject>);
fn reset_font_size(&self, _sender: Option<&AnyObject>);
```

Jede Action:

1. Wert anpassen (±1 oder DEFAULT_FONT_SIZE)
2. Auf MIN_FONT_SIZE..=MAX_FONT_SIZE clampen
3. In `body_font_size`-Ivar und UserDefaults speichern
4. Für alle offenen Tabs: `editor_delegate.set_base_size(size)` → `editor_delegate.reapply(storage)`

Pattern analog zu `apply_scheme` in `app.rs`.

### `src/menu.rs`

Nach dem Appearance-Separator im View-Menü:
```rust
separator(),
item("Schrift vergrößern").with_cmd('+').action(sel!(increaseFontSize:)),
item("Schrift verkleinern").with_cmd('-').action(sel!(decreaseFontSize:)),
item("Standardgröße").with_cmd('0').action(sel!(resetFontSize:)),
```

---

## Verifikation

1. `cargo build` — kein Compilefehler
2. `cargo test` — alle Tests grün (insbesondere `tests/renderer_tests.rs`)
3. **Editor-Modus (Cmd+E):** Cmd++ → Monospace-Fließtext wird größer
4. **Viewer-Modus:** Cmd++ → Proportionalschrift und Überschriften skalieren mit
5. Beide Modi: Code-Spans sind immer 2pt kleiner als Body
6. App neu starten → eingestellte Größe bleibt erhalten
7. Cmd+0 → Reset auf 16pt
8. Grenzen: 11× Cmd+– ab 16pt → bleibt bei 12pt; 8× Cmd++ ab 16pt → bleibt bei 24pt
