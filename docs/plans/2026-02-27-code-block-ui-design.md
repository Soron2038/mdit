# Design: Code-Block UI Improvements

**Date:** 2026-02-27
**Status:** Approved

## Context

Code block visual rendering is now working (box, border, copy icon). Three improvements
were requested after seeing the first working version:

1. Hide the fence markers (` ``` ` lines) — Typora-style reveal-on-focus
2. Show the language tag visibly — fieldset/legend style in the top border
3. Copy button visual feedback — green checkmark for 1.5 seconds after click

---

## Feature 1 — Hide Fence Markers

### Behaviour

- Opening and closing fence lines (` ```rust\n `, ` ```\n `) are **always hidden** by
  default, exactly like `# ` prefixes for ATX headings.
- When the cursor moves **onto a fence line**, that fence becomes visible (same
  "reveal on focus" pattern used for `**`, `*`, `` ` `` markers).
- The code content between the fences keeps `for_code_block()` styling unchanged.

### Implementation

**File:** `src/editor/renderer.rs`, `NodeKind::CodeBlock` branch in `collect_runs()`.

Replace the current single-run approach with three runs:

```
[opening fence line]  → syntax_attrs(cursor_pos, opening_fence_range)
[code content]        → for_code_block()
[closing fence line]  → syntax_attrs(cursor_pos, closing_fence_range)
```

Fence boundary detection (byte offsets within `text[start..end]`):
- **Opening fence end:** first `\n` after `start` (inclusive)
- **Closing fence start:** last `\n` before `end` (exclusive) + 1

The `cursor_in_span` check uses the **fence-line range** for each fence (not the
whole block range), so the cursor must be on that specific line to reveal it.

Edge cases: if the source range has no inner `\n` (degenerate block), fall back to
applying `for_code_block()` to the entire range.

---

## Feature 2 — Language Tag in Border (Fieldset Style)

### Behaviour

- When a code block has a language tag (e.g., `rust`, `python`), a small label
  appears **in the top border line** — like an HTML `<fieldset>` with `<legend>`.
- Blocks without a language tag show a plain, unbroken top border.
- Font: ~10pt monospace, color: `secondaryLabelColor` (adapts to light/dark mode).

### Visual Layout

```
┌─ rust ──────────────────────────── 📋 ─┐
│  fn main() {                            │
│      let x = 1 + 1;                    │
└─────────────────────────────────────────┘
```

### Implementation

**`src/editor/apply.rs`**, `CodeBlockInfo`: add `language: String` field.

**`src/editor/text_view.rs`**:

- `code_block_rects()`: return type extended to
  `Vec<(NSRect, NSRect, String, String)>` — `(block_rect, icon_rect, code_text, language)`.
- `draw_code_blocks()`: after drawing the full rounded-rect border, if `language`
  is non-empty:
  1. Measure tag text width via `NSString::sizeWithAttributes:` (10pt monospace).
  2. Erase the gap in the border by drawing a filled rect in the view's background
     color — `self.backgroundColor()` — at the top border position.
     Rect: height = 2pt (covers 1pt border + anti-aliasing), width = tag_width + 8pt padding,
     origin: `(block_rect.x + 14pt, block_rect.y - 1pt)`.
  3. Draw the tag text string inside the gap with `drawInRect:withAttributes:`.

---

## Feature 3 — Copy Button Visual Feedback

### Behaviour

- After clicking the copy icon, it **switches to a green checkmark** for **1.5 seconds**.
- After 1.5 seconds the checkmark reverts to the normal copy icon automatically.
- No fade animation — instant icon swap is sufficient and simpler.

### Implementation

**`MditTextViewIvars`**: add
`copy_feedback: RefCell<Option<(usize, std::time::Instant)>>`.
- `usize` = index of the block whose icon shows the checkmark
- `Instant` = moment the copy was triggered

**`mouseDown:`**: on successful copy, set the feedback state for the copied block
index, then schedule a delayed self-message:
```rust
let sel = objc2::sel!(clearCopyFeedback);
let _: () = unsafe {
    msg_send![self, performSelector: sel withObject: ptr::null::<AnyObject>() afterDelay: 1.5f64]
};
self.setNeedsDisplay_(true);
```

**New method `clearCopyFeedback`** (in `define_class!`):
```rust
#[unsafe(method(clearCopyFeedback))]
fn clear_copy_feedback(&self) {
    *self.ivars().copy_feedback.borrow_mut() = None;
    self.setNeedsDisplay_(true);
}
```

**`draw_code_blocks()`**: for each block at `index`, check feedback state:
```rust
let show_checkmark = matches!(
    &*self.ivars().copy_feedback.borrow(),
    Some((i, t)) if *i == index && t.elapsed().as_secs_f64() < 1.5
);
let icon_name = if show_checkmark { "checkmark" } else { "doc.on.doc" };
let icon_color = if show_checkmark {
    NSColor::systemGreenColor()
} else {
    NSColor::secondaryLabelColor()
};
icon_color.set();
icon.drawInRect(icon_rect);
```

---

## Files Modified

| File | Change |
|---|---|
| `src/editor/apply.rs` | Add `language: String` to `CodeBlockInfo`; populate in `collect_recursive` |
| `src/editor/renderer.rs` | Split `CodeBlock` into 3 runs (fence-open, content, fence-close) |
| `src/editor/text_view.rs` | Extend `code_block_rects()` return type; add language-tag drawing; add feedback state + `clearCopyFeedback` method |

---

## Test Plan

1. `cargo test` — all existing tests must pass
2. Build debug binary: `cargo build`
3. Open `test.md` in the app:
   - Fence markers invisible by default ✓
   - Move cursor onto ` ```rust ` line → fence becomes visible ✓
   - Language tag "rust" / "python" appears in top border ✓
   - Blocks without language tag show unbroken border ✓
   - Click copy icon → green checkmark appears ✓
   - After 1.5s → copy icon returns ✓
4. Test in dark mode: all colors adapt correctly
