# Design: Sidebar im Viewer-Mode verbergen + Slide-Animation

**Datum:** 2026-03-18
**Status:** Approved

## Kontext

Die Sidebar (Formatierungs-Toolbar, 36pt breit) ist aktuell permanent sichtbar — unabhängig davon, ob sich die App im Viewer- oder Editor-Mode befindet. Im Viewer-Mode ist sie überflüssig und stört den Lesefluss. Beim Wechsel in den Editor-Mode soll sie mit einer sanften Animation erscheinen, beim Verlassen ebenso sanft verschwinden.

---

## Ziel

- Sidebar ist im Viewer-Mode **nicht sichtbar** (Breite = 0, Textbereich füllt das volle Fenster)
- Beim Wechsel **Viewer → Editor**: Sidebar gleitet in 0.35s von links herein, Textbereich zieht gleichzeitig zurück
- Beim Wechsel **Editor → Viewer**: Sidebar gleitet in 0.35s nach links heraus, Textbereich wächst gleichzeitig
- Neue Tabs starten im Viewer-Mode → Sidebar sofort versteckt (keine Eröffnungsanimation)
- Fenster-Resize respektiert den aktuellen Modus ohne Animation
- Tab-Wechsel snappt die Sidebar sofort auf den Ziel-Modus des neuen Tabs (keine Animation)

---

## Architektur

### Frame-Berechnung

Aktuell ist `SIDEBAR_W = 36.0` fest in `content_frame()` verdrahtet. Wir abstrahieren das:

```rust
/// Berechnet den Sidebar-Frame in Abhängigkeit vom Modus.
/// Viewer → width: 0.0  |  Editor → width: SIDEBAR_W
fn sidebar_target_frame(mode: ViewMode, h: f64) -> NSRect { … }

/// Berechnet den Content-Frame in Abhängigkeit vom Modus.
/// Viewer → x: 0.0, full width  |  Editor → x: SIDEBAR_W, reduced width
fn content_target_frame(mode: ViewMode, w: f64, h: f64) -> NSRect { … }
```

Beide Helfer ersetzen die bisherige hardcodierte `SIDEBAR_W`-Verwendung in `app.rs`.

### Animation

In `toggleMode()` wrappen wir die Frame-Zuweisung in `NSAnimationContext::runAnimationGroup_completionHandler`. Die korrekte Rust/objc2-Signatur:

```rust
// Achtung: Der Block-Parameter ist NonNull<NSAnimationContext>, nicht &NSAnimationContext.
// Zugriff erfolgt via unsafe { ctx.as_ref() }.
NSAnimationContext::runAnimationGroup_completionHandler(
    &block2::StackBlock::new(|ctx: NonNull<NSAnimationContext>| {
        let ctx = unsafe { ctx.as_ref() };
        ctx.setDuration(0.35);
        ctx.setTimingFunction(Some(
            &CAMediaTimingFunction::functionWithName(kCAMediaTimingFunctionEaseInEaseOut)
        ));
        sidebar.view().animator().setFrame(target_sidebar_frame);
        scroll_view.animator().setFrame(target_content_frame);
    }),
    None,
);
```

Die nicht-visuellen Mode-Änderungen (`setEditable`, `reapply`, `update_text_container_inset`) laufen **vor** dem Animation-Block — Inhalt und Inset wechseln sofort, die Geometrie gleitet dann sanft.

**Rapidwiederholung:** Wenn `toggleMode()` innerhalb von 0.35s erneut aufgerufen wird, startet AppKit einen neuen `NSAnimationContext`-Block. AppKit supersediert die laufende Animation korrekt. Kein zusätzlicher Guard nötig.

### Clipping

Um Buttons korrekt beim Ausgleiten abzuschneiden:

```rust
// In FormattingSidebar::new():
container.setWantsLayer(true);
if let Some(layer) = container.layer() {
    layer.setMasksToBounds(true);
}
```

`sidebar.view()` gibt eine Referenz auf den Container zurück (nicht `container_view` — dieses Feld existiert in der öffentlichen API nicht).

### `update_text_container_inset`

Diese Funktion (aktuell `src/app.rs` ~Zeile 782) berechnet `editor_width` als `win.frame().size.width - SIDEBAR_W`. Sie muss modusabhängig werden:

```rust
let effective_sidebar_w = if mode == ViewMode::Editor { SIDEBAR_W } else { 0.0 };
let editor_width = win.frame().size.width - effective_sidebar_w;
```

Sonst ist der Text-Container im Viewer-Mode 36pt zu schmal.

### `set_height` während Animation

`FormattingSidebar::set_height` setzt Frames direkt (nicht über Animator). Falls ein Resize während einer laufenden 0.35s-Animation eintrifft: Das ist akzeptabel. Nur der Container-Frame wird animiert; die internen Sub-Views passen sich direkt an. Da Resize ausschließlich durch Nutzerdrag ausgelöst wird (nicht programmatisch), ist kein Konflikt zu erwarten.

### Initiale Darstellung

In `setup_content_views()` wird die Sidebar bei Viewer-Mode (Default) sofort mit `width: 0.0` angelegt — kein Animations-Block, da es kein sichtbares Gleiten geben soll.

### Tab-Wechsel

`switch_to_tab` muss nach dem Tab-Wechsel auch den Sidebar-Frame des neuen Tabs direkt (ohne Animation) setzen:

```rust
// Nach dem Aktivieren des neuen Tabs:
let new_mode = tab.mode.get();
let (w, h) = self.window_size();
self.sidebar.set_frame_direct(sidebar_target_frame(new_mode, h));
self.scroll_view.setFrame(content_target_frame(new_mode, w, h));
```

### Tab-Schliessen

`close_tab` (LastTab-Pfad, `src/app.rs` ~Zeile 688) positioniert den Scrollview via `self.content_frame()`. Da `content_frame` nach der Änderung modusabhängig ist, muss der Sidebar-Frame hier ebenfalls aktualisiert werden — direkt, ohne Animation.

---

## Abhängigkeiten (Cargo.toml)

Folgende Änderungen an `Cargo.toml` sind nötig:

```toml
[dependencies]
block2 = "0.6"
objc2-quartz-core = { version = "0.3", features = ["CAMediaTimingFunction"] }

# In objc2-app-kit features ergänzen:
objc2-app-kit = { ..., features = [
    ...,
    "NSAnimation",
    "NSAnimationContext",
]}
```

---

## Betroffene Dateien

| Datei | Änderung |
| ----- | -------- |
| `Cargo.toml` | `block2`, `objc2-quartz-core` Deps; `NSAnimation`, `NSAnimationContext` Features in `objc2-app-kit` |
| `src/app.rs` | `content_target_frame(mode, w, h)` + `sidebar_target_frame(mode, h)` Helfer; `toggleMode` mit NSAnimationContext; `update_text_container_inset` mode-aware; `switch_to_tab` Sidebar-Frame-Update; `close_tab` Sidebar-Frame-Update; `setup_content_views` initial state; `windowDidResize` mode-aware |
| `src/ui/sidebar.rs` | `wantsLayer = true` + `masksToBounds = true` auf Container; ggf. `set_frame_direct()`-Methode für direkte (nicht-animierte) Frame-Setzung von außen |

---

## Animation-Parameter

| Parameter | Wert |
| --------- | ---- |
| Dauer | 0.35s |
| Easing | `kCAMediaTimingFunctionEaseInEaseOut` |
| Richtung Ein | Links → rechts (width 0 → 36pt) |
| Richtung Aus | Rechts → links (width 36pt → 0) |

---

## Verifikation

1. App starten → Sidebar ist **nicht sichtbar**, Textbereich füllt das volle Fenster
2. Cmd+E → Sidebar gleitet sanft von links herein, Textbereich zieht gleichzeitig zurück — 0.35s
3. Cmd+E erneut → Sidebar gleitet sanft nach links heraus, Textbereich wächst gleichzeitig — 0.35s
4. **Text-Inset:** Im Viewer-Mode ist die Textspalte korrekt zentriert (nicht 36pt zu schmal) — bei verschiedenen Fensterbreiten prüfen
5. Fenster größer/kleiner ziehen → beide Layouts respektieren den Modus korrekt (kein Flackern)
6. Mehrere Tabs: Pro-Tab-State korrekt — Tab-Wechsel snappt Sidebar ohne Animation in den richtigen Zustand
7. Tab schliessen (letzter Tab) → Sidebar-State korrekt
8. Rapidtest: Cmd+E mehrfach schnell hintereinander drücken → keine hängenden Animations-Artefakte
