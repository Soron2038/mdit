# Sidebar Toggle-Formatting

**Datum:** 2026-03-03
**Status:** Approved

## Problem

Die Sidebar-Buttons sind reine Insert-Operationen. Ein Klick auf H1 fügt blind `# ` ein, auch wenn die Zeile bereits ein Heading ist. Wiederholte Klicks verändern den sichtbaren Text (`Blabla` → `# Blabla` → `## Blabla` …). Das gleiche Problem betrifft alle Block- und Inline-Buttons.

## Grundprinzip

Jeder Formatting-Button ist ein **Zustandsschalter**, keine Insert-Operation. Der sichtbare Text (nach Rendering) darf sich durch einen Klick **nie** ändern — nur die unsichtbaren Markdown-Marker werden hinzugefügt oder entfernt.

## 1. Block-Formate (H1, H2, H3, Blockquote, Normal)

**Funktion:** `set_block_format(tv, prefix)` ersetzt `prepend_line()`.

**Algorithmus:**

1. Finde die Zeile am Cursor (`lineRangeForRange`)
2. Erkenne den bestehenden Block-Prefix der Zeile (`### `, `## `, `# `, `> `)
3. Entscheide:
   - Bestehender Prefix == gewünschter Prefix → **Toggle off**: Prefix entfernen (→ Normal)
   - Bestehender Prefix != gewünschter Prefix → **Umschalten**: alten Prefix entfernen, neuen setzen
   - Kein Prefix vorhanden → neuen Prefix setzen

**Normal-Button:** Ruft dieselbe Funktion mit leerem Prefix auf — entfernt immer nur.

## 2. Inline-Formate (Bold, Italic, InlineCode, Strikethrough)

**Funktion:** `toggle_inline_wrap(tv, marker)` ersetzt `wrap_selection()`.

**Algorithmus für verschachtelte Marker:**

Marker-Zeichen bilden Schichten von außen nach innen:

```
**_`Blabla`_**
^^            ^^   Schicht 0: Bold (**)
  ^        ^       Schicht 1: Italic (_)
   ^      ^        Schicht 2: InlineCode (`)
    Blabla          Inhalt
```

1. Bestimme den Bereich: Selection oder, wenn leer, das Wort am Cursor
2. Expandiere den Bereich nach außen über alle angrenzenden bekannten Marker-Paare hinweg
3. Scanne die Marker-Schichten: Sammle alle Marker-Paare die den Inhalt umgeben (von außen nach innen)
4. Prüfe ob der gesuchte Marker (`**`, `_`, `` ` ``, `~~`) in den erkannten Schichten vorkommt:
   - **Ja** → entferne genau dieses Marker-Paar, lasse alle anderen Schichten intakt
   - **Nein** → füge den Marker als innerste Schicht hinzu (direkt um den Inhalt)

**Bekannte Marker-Paare** (symmetrisch):

| Format        | Marker |
|---------------|--------|
| Bold          | `**`   |
| Italic        | `_`    |
| InlineCode    | `` ` ``|
| Strikethrough | `~~`   |

## 3. Sonderfälle — unverändert

Diese Buttons bleiben Insert-Operationen ohne Toggle:

- **Code Block** (`` ``` ``): Mehrzeilig, kein Zeilen-Prefix
- **Link** (`[text](url)`): Asymmetrische Marker, Toggle nicht sinnvoll
- **HRule** (`---`): Reine Einfügung

## 4. Betroffene Code-Stellen

Alles in `src/app.rs`:

- `prepend_line()` (Z. 752) → wird zu `set_block_format()`
- `wrap_selection()` (Z. 738) → wird zu `toggle_inline_wrap()`
- `strip_line_prefix()` (Z. 774) → Logik wird in `set_block_format()` integriert, Funktion kann entfallen
- Die 9 Action-Methods (`apply_h1` bis `apply_strikethrough`) bleiben strukturell gleich, rufen nur die neuen Funktionen auf

## 5. Testbarkeit

Beide neuen Funktionen sind reine String-Transformationen:

- Block: "Zeile mit Prefix X → `set_block_format(Y)` → erwarteter Output"
- Inline: "Text mit Marker-Schichten → `toggle_inline_wrap(marker)` → erwarteter Output"
