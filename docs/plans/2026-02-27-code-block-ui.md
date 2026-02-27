# Code-Block UI Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Hide fence markers (reveal-on-focus), show language tag in the border, and add green-checkmark copy feedback.

**Architecture:** Feature 1 is a pure renderer change (split one `AttributeRun` into three). Feature 2 adds a `language` field to `CodeBlockInfo` and draws text in a gap in the border using AppKit string-drawing via `msg_send!`. Feature 3 adds a timer-based feedback state to `MditTextViewIvars` and swaps the copy icon for a green checkmark for 1.5 seconds.

**Tech Stack:** Rust, comrak, objc2 0.6.3, AppKit (NSBezierPath, NSFont, NSColor, NSAttributedString drawing via msg_send!)

---

### Task 1: Renderer — split CodeBlock into fence + content + fence runs

**Files:**
- Modify: `src/editor/renderer.rs` (the `NodeKind::CodeBlock` branch of `collect_runs()`)
- Test: `tests/renderer_tests.rs`

**Step 1: Write the failing tests**

Add these three tests to `tests/renderer_tests.rs`:

```rust
#[test]
fn code_block_fences_hidden_without_cursor() {
    // "```rust\n" = bytes 0..8, "let x = 1;\n" = bytes 8..19, "```\n" = bytes 19..23
    let text = "```rust\nlet x = 1;\n```\n";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);

    let opening = runs.iter().find(|r| r.range.0 == 0)
        .expect("no run starting at byte 0");
    assert!(opening.attrs.contains(&TextAttribute::Hidden),
        "opening fence must be hidden; got: {:?}", opening.attrs.attrs());

    let content = runs.iter().find(|r| r.range.0 == 8)
        .expect("no run starting at byte 8");
    assert!(content.attrs.contains(&TextAttribute::Monospace),
        "code content must be Monospace");
    assert!(!content.attrs.contains(&TextAttribute::Hidden),
        "code content must not be hidden");

    let closing = runs.iter().find(|r| r.range.0 == 19)
        .expect("no run starting at byte 19");
    assert!(closing.attrs.contains(&TextAttribute::Hidden),
        "closing fence must be hidden");
}

#[test]
fn opening_fence_visible_when_cursor_on_it() {
    let text = "```rust\nlet x = 1;\n```\n";
    let spans = parse(text);
    // cursor at byte 2 — inside the opening fence (bytes 0..8)
    let runs = compute_attribute_runs(text, &spans, Some(2));
    let opening = runs.iter().find(|r| r.range.0 == 0)
        .expect("no run at byte 0");
    assert!(!opening.attrs.contains(&TextAttribute::Hidden),
        "opening fence must be visible when cursor is on it");
}

#[test]
fn closing_fence_visible_when_cursor_on_it() {
    let text = "```rust\nlet x = 1;\n```\n";
    let spans = parse(text);
    // cursor at byte 20 — inside the closing fence (bytes 19..23)
    let runs = compute_attribute_runs(text, &spans, Some(20));
    let closing = runs.iter().find(|r| r.range.0 == 19)
        .expect("no run at byte 19");
    assert!(!closing.attrs.contains(&TextAttribute::Hidden),
        "closing fence must be visible when cursor is on it");
}
```

**Step 2: Run tests — they must fail**

```
cargo test code_block_fences_hidden_without_cursor opening_fence_visible closing_fence_visible -- --nocapture
```

Expected: FAIL ("no run starting at byte 0" or similar — the current code emits one run for the whole range).

**Step 3: Replace the `NodeKind::CodeBlock` branch in `collect_runs()`**

In `src/editor/renderer.rs`, replace:

```rust
NodeKind::CodeBlock { .. } => {
    runs.push(AttributeRun {
        range: (start, end),
        attrs: AttributeSet::for_code_block(),
    });
}
```

with:

```rust
NodeKind::CodeBlock { .. } => {
    let slice = &text[start..end];
    if let Some(open_nl) = slice.find('\n') {
        let open_end = start + open_nl + 1; // includes the \n

        // Start of closing fence = start of last line in text[open_end..end].
        // "Last line" starts right after the second-to-last \n.
        let suffix = &text[open_end..end];
        let close_start = open_end + if suffix.len() > 1 {
            suffix[..suffix.len() - 1]
                .rfind('\n')
                .map(|p| p + 1)
                .unwrap_or(0)
        } else {
            0
        };

        // Opening fence: hidden/visible based on cursor position ON that line.
        runs.push(AttributeRun {
            range: (start, open_end),
            attrs: syntax_attrs(cursor_pos, (start, open_end)),
        });
        // Code content (may be empty for a block with no body).
        if open_end < close_start {
            runs.push(AttributeRun {
                range: (open_end, close_start),
                attrs: AttributeSet::for_code_block(),
            });
        }
        // Closing fence.
        if close_start < end {
            runs.push(AttributeRun {
                range: (close_start, end),
                attrs: syntax_attrs(cursor_pos, (close_start, end)),
            });
        }
    } else {
        // Degenerate: no newline — treat whole span as code content.
        runs.push(AttributeRun {
            range: (start, end),
            attrs: AttributeSet::for_code_block(),
        });
    }
}
```

**Step 4: Run tests — they must pass**

```
cargo test
```

Expected: all 69 + 3 = 72 tests pass.

**Step 5: Commit**

```bash
git add tests/renderer_tests.rs src/editor/renderer.rs
git commit -m "feat: hide fence markers; reveal on cursor focus"
```

---

### Task 2: Data — add `language` to `CodeBlockInfo`

**Files:**
- Modify: `src/editor/apply.rs` (`CodeBlockInfo` struct + `collect_recursive()`)
- Test: `tests/apply_tests.rs`

**Step 1: Write the failing tests**

Add to `tests/apply_tests.rs`:

```rust
#[test]
fn code_block_language_captured() {
    let text = "```rust\nlet x = 1;\n```\n";
    let spans = parse(text);
    let infos = collect_code_block_infos(&spans, text);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].language, "rust");
}

#[test]
fn code_block_without_language_has_empty_language() {
    let text = "```\nplain text\n```\n";
    let spans = parse(text);
    let infos = collect_code_block_infos(&spans, text);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].language, "");
}
```

**Step 2: Run tests — they must fail**

```
cargo test code_block_language
```

Expected: FAIL — `CodeBlockInfo` has no `language` field.

**Step 3: Add `language` field to `CodeBlockInfo` and populate it**

In `src/editor/apply.rs`:

```rust
// Add the field:
pub struct CodeBlockInfo {
    pub start_utf16: usize,
    pub end_utf16: usize,
    pub text: String,
    pub language: String,   // ← new
}
```

In `collect_recursive()`, update the match arm:

```rust
if let NodeKind::CodeBlock { code, language } = &span.kind {
    out.push(CodeBlockInfo {
        start_utf16: byte_to_utf16(text, span.source_range.0),
        end_utf16:   byte_to_utf16(text, span.source_range.1),
        text:        code.clone(),
        language:    language.clone(),   // ← new
    });
}
```

**Step 4: Run tests — they must pass**

```
cargo test
```

Expected: all 74 tests pass. (The compiler will flag the unused `language` field in `text_view.rs` at most as a warning — ignore it for now.)

**Step 5: Commit**

```bash
git add tests/apply_tests.rs src/editor/apply.rs
git commit -m "feat: add language field to CodeBlockInfo"
```

---

### Task 3: Copy feedback — state, timer, `clearCopyFeedback`, icon swap

**Files:**
- Modify: `src/editor/text_view.rs`

No unit tests are possible for AppKit drawing code; verification is visual (Step 7).

**Step 1: Add `copy_feedback` to `MditTextViewIvars`**

In `MditTextViewIvars`:

```rust
pub struct MditTextViewIvars {
    delegate: RefCell<Option<Retained<MditEditorDelegate>>>,
    copy_button_rects: RefCell<Vec<(NSRect, String)>>,
    copy_feedback: RefCell<Option<(usize, std::time::Instant)>>,  // ← new
}
```

In `MditTextView::new()`, initialise it:

```rust
let this = Self::alloc(mtm).set_ivars(MditTextViewIvars {
    delegate:           RefCell::new(None),
    copy_button_rects:  RefCell::new(Vec::new()),
    copy_feedback:      RefCell::new(None),    // ← new
});
```

**Step 2: Add `clearCopyFeedback` method to `define_class!`**

Inside the `impl MditTextView { … }` block in `define_class!`, add:

```rust
/// Called by the timer scheduled in mouseDown: — clears the copy-icon
/// feedback state and triggers a redraw to show the normal copy icon.
#[unsafe(method(clearCopyFeedback))]
fn clear_copy_feedback(&self) {
    *self.ivars().copy_feedback.borrow_mut() = None;
    let _: () = unsafe { msg_send![self, setNeedsDisplay: true] };
}
```

**Step 3: Update `mouseDown:` to set feedback state and schedule timer**

Replace the entire `mouseDown:` method body with:

```rust
#[unsafe(method(mouseDown:))]
fn mouse_down(&self, event: &objc2_app_kit::NSEvent) {
    // Convert window coords → view coords.
    let window_point = unsafe { event.locationInWindow() };
    let view_point: NSPoint = unsafe {
        self.convertPoint_fromView(window_point, None)
    };

    // Find which copy-button (if any) was clicked.
    let click_result = {
        let rects = self.ivars().copy_button_rects.borrow();
        rects.iter().enumerate().find_map(|(idx, (rect, code_text))| {
            let in_rect = view_point.x >= rect.origin.x
                && view_point.x <= rect.origin.x + rect.size.width
                && view_point.y >= rect.origin.y
                && view_point.y <= rect.origin.y + rect.size.height;
            if in_rect { Some((idx, code_text.clone())) } else { None }
        })
    };

    if let Some((block_idx, code_text)) = click_result {
        // Copy content to clipboard.
        unsafe {
            let pb = NSPasteboard::generalPasteboard();
            pb.clearContents();
            let ns_str = NSString::from_str(&code_text);
            pb.setString_forType(&ns_str, NSPasteboardTypeString);
        }
        // Set feedback state: show green checkmark for 1.5s.
        *self.ivars().copy_feedback.borrow_mut() =
            Some((block_idx, std::time::Instant::now()));
        unsafe {
            // Schedule clearCopyFeedback after 1.5s (NSObject method).
            let _: () = msg_send![
                self,
                performSelector: objc2::sel!(clearCopyFeedback)
                withObject: std::ptr::null::<objc2::runtime::AnyObject>()
                afterDelay: 1.5f64
            ];
            // Redraw immediately to show the checkmark.
            let _: () = msg_send![self, setNeedsDisplay: true];
        }
        return; // Consume event.
    }

    // Not a copy-button click — pass to standard text-view handling.
    let _: () = unsafe { msg_send![super(self), mouseDown: event] };
}
```

**Step 4: Update `draw_code_blocks()` to swap icon based on feedback state**

The current for loop iterates `(block_rect, icon_rect, code_text)`. After Task 4 this becomes a 4-tuple, but for now update the icon drawing logic (currently a 3-tuple):

Find the section in `draw_code_blocks()` that draws the SF Symbol and replace:

```rust
unsafe {
    let name = NSString::from_str("doc.on.doc");
    if let Some(icon) = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &name, None,
    ) {
        NSColor::secondaryLabelColor().set();
        icon.drawInRect(icon_rect);
    }
}
```

with:

```rust
// Check whether this block should show the copy-feedback checkmark.
let show_checkmark = {
    let fb = self.ivars().copy_feedback.borrow();
    matches!(&*fb, Some((i, t)) if *i == index && t.elapsed().as_secs_f64() < 1.5)
};
unsafe {
    let icon_name = if show_checkmark { "checkmark" } else { "doc.on.doc" };
    let name = NSString::from_str(icon_name);
    if let Some(icon) = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &name, None,
    ) {
        if show_checkmark {
            NSColor::systemGreenColor().set();
        } else {
            NSColor::secondaryLabelColor().set();
        }
        icon.drawInRect(icon_rect);
    }
}
```

Also update the for loop to use `enumerate()` so `index` is available:

```rust
for (index, (block_rect, icon_rect, code_text)) in rects.into_iter().enumerate() {
```

**Step 5: Add `NSColor::systemGreenColor` to imports if needed**

`NSColor::systemGreenColor()` is a standard AppKit method. If the compiler complains it's not in scope, it's already covered by `use objc2_app_kit::NSColor` — no new feature flags needed.

**Step 6: Build and verify it compiles**

```
cargo build
```

Expected: builds cleanly. (Tests still at 74.)

**Step 7: Visual verification**

Run `./target/debug/mdit`, click any copy icon:
- Icon switches to green checkmark ✓
- After 1.5s, reverts to copy icon ✓

**Step 8: Commit**

```bash
git add src/editor/text_view.rs
git commit -m "feat: copy button shows green checkmark for 1.5s"
```

---

### Task 4: Drawing — language tag in the top border (fieldset style)

**Files:**
- Modify: `src/editor/text_view.rs`

**Step 1: Extend `code_block_rects()` to include language**

Change the return type from `Vec<(NSRect, NSRect, String)>` to `Vec<(NSRect, NSRect, String, String)>` (adding language as the fourth element).

In `collect_code_block_infos` loop, change:

```rust
result.push((block_rect, icon_rect, info.text.clone()));
```

to:

```rust
result.push((block_rect, icon_rect, info.text.clone(), info.language.clone()));
```

Update every call-site destructuring:
- `draw_code_block_fills()`: change `for (block_rect, _, _) in rects` → `for (block_rect, _, _, _) in rects`
- `draw_code_blocks()`: change the `for (index, (block_rect, icon_rect, code_text)) in ...` → `for (index, (block_rect, icon_rect, code_text, language)) in ...`
  - Also update `copy_button_rects.push((icon_rect, code_text))` — unchanged.

**Step 2: Add language-tag drawing in `draw_code_blocks()`**

After the `border_path.stroke()` call, add:

```rust
// ── Draw language tag in fieldset style (gap in top border) ──────────────
if !language.is_empty() {
    use objc2_foundation::{NSMutableAttributedString, NSRange as NSRangeF};
    use objc2_app_kit::{NSFontAttributeName, NSForegroundColorAttributeName};

    let ns_lang = NSString::from_str(&language);

    // Build an attributed string with 10pt monospace + secondaryLabelColor.
    let mattr: Retained<objc2::runtime::AnyObject> = unsafe {
        let cls = objc2::runtime::AnyClass::get(c"NSMutableAttributedString")
            .expect("NSMutableAttributedString class not found");
        let alloc: *mut objc2::runtime::AnyObject = msg_send![cls, alloc];
        let init: *mut objc2::runtime::AnyObject =
            msg_send![alloc, initWithString: &*ns_lang];
        Retained::from_raw(init).expect("initWithString failed")
    };

    let tag_len = language.encode_utf16().count();
    let tag_range = objc2_foundation::NSRange { location: 0, length: tag_len };
    let tag_font = unsafe {
        NSFont::monospacedSystemFontOfSize_weight(10.0, NSFontWeightRegular)
    };
    let tag_color = NSColor::secondaryLabelColor();
    unsafe {
        let _: () = msg_send![&*mattr,
            addAttribute: NSFontAttributeName
            value: tag_font.as_ref()
            range: tag_range];
        let _: () = msg_send![&*mattr,
            addAttribute: NSForegroundColorAttributeName
            value: tag_color.as_ref()
            range: tag_range];
    }

    // Measure text so we know how wide to make the gap.
    let tag_size: NSSize = unsafe { msg_send![&*mattr, size] };

    // Gap geometry: starts 14pt from left edge of block, 4pt padding each side.
    let gap_x   = block_rect.origin.x + 14.0;
    let gap_w   = tag_size.width + 8.0;
    let gap_y   = block_rect.origin.y - tag_size.height / 2.0 - 1.0;
    let gap_h   = tag_size.height + 2.0;

    // Erase the border line in the gap region using the view's background color.
    let bg = unsafe { self.backgroundColor() };
    bg.setFill();
    NSRectFill(NSRect::new(
        NSPoint::new(gap_x, gap_y),
        NSSize::new(gap_w, gap_h),
    ));

    // Draw the attributed string inside the gap.
    let text_rect = NSRect::new(
        NSPoint::new(gap_x + 4.0, gap_y + 1.0),
        NSSize::new(tag_size.width, tag_size.height),
    );
    let _: () = unsafe { msg_send![&*mattr, drawInRect: text_rect] };
}
```

**Step 3: Build**

```
cargo build
```

If the compiler reports an ambiguous or missing `NSRange` import (there are two — `objc2_foundation::NSRange` and the local alias), use the fully qualified form throughout Task 4 additions: `objc2_foundation::NSRange`.

If `NSFontAttributeName` / `NSForegroundColorAttributeName` are already imported at the top of the file, remove the duplicate `use` statements from the block above. If they aren't imported (they're currently only in `apply.rs`), add them to the top-level imports in `text_view.rs`:

```rust
use objc2_app_kit::{
    // ... existing imports ...
    NSFontAttributeName, NSForegroundColorAttributeName,
};
```

**Step 4: Run all tests**

```
cargo test
```

Expected: all 74 tests pass.

**Step 5: Visual verification**

Run `./target/debug/mdit`, open `test.md` or type a code block:
- `rust` / `python` / no-language blocks render correctly
- Language tag appears in gap in top border ✓
- Plain blocks (no language tag) show unbroken border ✓
- Dark mode: tag color adapts via `secondaryLabelColor` ✓

**Step 6: Commit**

```bash
git add src/editor/text_view.rs
git commit -m "feat: show language tag in code block border (fieldset style)"
```

---

### Task 5: Final verification and wrap-up

**Step 1: Run full test suite**

```
cargo test
```

Expected: 74 tests pass, 0 failures.

**Step 2: Build release**

```
cargo build --release
```

**Step 3: Visual smoke test with `test.md`**

Open `test.md` in the debug binary (`./target/debug/mdit`):

| Feature | Expected |
|---|---|
| Fence markers (```rust, ```) | Hidden by default |
| Cursor on fence line | Fence becomes visible |
| Rust/Python blocks | "rust"/"python" label in top border |
| Plain code block (no lang) | Unbroken border |
| Click copy icon | Green checkmark appears |
| After 1.5s | Copy icon returns |
| Dark mode | All colors adapt |

**Step 4: Run finishing-a-development-branch skill**

After all checks pass, use the `superpowers:finishing-a-development-branch` skill to decide how to integrate the work.
