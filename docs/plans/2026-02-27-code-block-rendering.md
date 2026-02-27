# Code Block Visual Rendering Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Render fenced code blocks as full-width, rounded-corner boxes with a subtle background and an always-visible SF Symbol "Copy" button in the bottom-right corner.

**Architecture:** Custom Drawing Overlay — identical pattern to the heading-separator feature. Code block ranges are collected after parsing, stored in `MditEditorDelegate`, and drawn in `MditTextView.drawRect:`. Click detection uses a `mouseDown:` override with stored hit rects.

**Tech Stack:** Rust, objc2 0.6.3, objc2-app-kit 0.3.2, AppKit (NSBezierPath, NSPasteboard, NSImage)

---

### Task 1: Add new Cargo.toml feature flags

Three new AppKit classes are required. Add them to `Cargo.toml`.

**Files:**
- Modify: `Cargo.toml:21-38`

**Step 1: Add features**

In `Cargo.toml`, add three entries to the `objc2-app-kit` features list:

```toml
    "NSBezierPath",
    "NSPasteboard",
    "NSImage",
```

Place them after `"NSBox",` (last current entry).

**Step 2: Verify build compiles**

```bash
cargo build 2>&1 | head -20
```

Expected: no errors (no new code uses these yet).

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add NSBezierPath, NSPasteboard, NSImage feature flags"
```

---

### Task 2: Add `code` field to `NodeKind::CodeBlock`

Store the code literal in the AST node so it flows through to the collection layer.

**Files:**
- Modify: `src/markdown/parser.rs:17` (NodeKind enum) and `:122-124` (construction)

**Step 1: Write the failing test**

Add to `tests/renderer_tests.rs`:

```rust
#[test]
fn code_block_code_content_captured() {
    let text = "```rust\nlet x = 1;\n```\n";
    let spans = mdit::markdown::parser::parse(text);
    let cb = spans.iter().find(|s| matches!(&s.kind, mdit::markdown::parser::NodeKind::CodeBlock { .. }));
    assert!(cb.is_some(), "expected a CodeBlock span");
    if let mdit::markdown::parser::NodeKind::CodeBlock { code, .. } = &cb.unwrap().kind {
        assert_eq!(code, "let x = 1;", "code content should be extracted without fences");
    }
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test code_block_code_content_captured 2>&1 | tail -20
```

Expected: compile error — `code` field doesn't exist yet on `NodeKind::CodeBlock`.

**Step 3: Update `NodeKind::CodeBlock` in the enum**

In `src/markdown/parser.rs`, change line 17:

```rust
// old
CodeBlock { language: String },
// new
CodeBlock { language: String, code: String },
```

**Step 4: Update the construction site**

In `src/markdown/parser.rs`, change lines 122-124:

```rust
// old
NodeValue::CodeBlock(cb) => NodeKind::CodeBlock {
    language: cb.info.trim().to_string(),
},
// new
NodeValue::CodeBlock(cb) => NodeKind::CodeBlock {
    language: cb.info.trim().to_string(),
    code: cb.literal.trim_end_matches('\n').to_string(),
},
```

**Step 5: Run test to verify it passes**

```bash
cargo test code_block_code_content_captured 2>&1 | tail -10
```

Expected: PASS. The `{ .. }` wildcard in `renderer.rs:164` still compiles without change.

**Step 6: Run all tests**

```bash
cargo test 2>&1 | tail -15
```

Expected: all 64 tests pass.

**Step 7: Commit**

```bash
git add src/markdown/parser.rs tests/renderer_tests.rs
git commit -m "feat: store code literal in NodeKind::CodeBlock"
```

---

### Task 3: Add `CodeBlockInfo` struct and collection function in `apply.rs`

**Files:**
- Modify: `src/editor/apply.rs`

**Step 1: Write the failing test**

Add to a new test file `tests/apply_tests.rs`:

```rust
use mdit::editor::apply::collect_code_block_infos;
use mdit::markdown::parser::parse;

#[test]
fn code_block_infos_collected() {
    let text = "```rust\nlet x = 1;\n```\n";
    let spans = parse(text);
    let infos = collect_code_block_infos(&spans, text);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].text, "let x = 1;");
    assert!(infos[0].start_utf16 < infos[0].end_utf16);
}

#[test]
fn two_code_blocks_both_collected() {
    let text = "```\nfoo\n```\n\nsome text\n\n```\nbar\n```\n";
    let spans = parse(text);
    let infos = collect_code_block_infos(&spans, text);
    assert_eq!(infos.len(), 2);
    assert_eq!(infos[0].text, "foo");
    assert_eq!(infos[1].text, "bar");
}

#[test]
fn no_code_blocks_returns_empty() {
    let text = "Just a paragraph.\n";
    let spans = parse(text);
    let infos = collect_code_block_infos(&spans, text);
    assert!(infos.is_empty());
}
```

Register the test file: add to `Cargo.toml` under `[[test]]` or just create the file (Cargo auto-discovers tests in `tests/`).

**Step 2: Run to verify failure**

```bash
cargo test --test apply_tests 2>&1 | tail -20
```

Expected: compile error — `collect_code_block_infos` not found, `CodeBlockInfo` not found.

**Step 3: Add `CodeBlockInfo` struct and `collect_code_block_infos`**

Add at the top of `src/editor/apply.rs` (after the existing `use` statements, before the public entry point comment):

```rust
use crate::markdown::parser::{MarkdownSpan, NodeKind};
```

Add after the `use` block:

```rust
// ---------------------------------------------------------------------------
// Code-block info collection
// ---------------------------------------------------------------------------

/// Metadata about a fenced code block, computed once per edit from the AST.
/// Used by MditTextView to draw the visual box and handle copy-to-clipboard.
#[derive(Debug, Clone)]
pub struct CodeBlockInfo {
    /// UTF-16 code-unit offset of the code block's first character.
    pub start_utf16: usize,
    /// UTF-16 code-unit offset one past the code block's last character.
    pub end_utf16: usize,
    /// The raw code content (without fences, trailing newline stripped).
    pub text: String,
}

/// Walk `spans` to find all `CodeBlock` nodes, convert their byte offsets
/// to UTF-16, and return the list.  Call this after every re-parse.
pub fn collect_code_block_infos(spans: &[MarkdownSpan], text: &str) -> Vec<CodeBlockInfo> {
    let mut result = Vec::new();
    collect_recursive(spans, text, &mut result);
    result
}

fn collect_recursive(spans: &[MarkdownSpan], text: &str, out: &mut Vec<CodeBlockInfo>) {
    for span in spans {
        if let NodeKind::CodeBlock { code, .. } = &span.kind {
            out.push(CodeBlockInfo {
                start_utf16: byte_to_utf16(text, span.source_range.0),
                end_utf16: byte_to_utf16(text, span.source_range.1),
                text: code.clone(),
            });
        }
        collect_recursive(&span.children, text, out);
    }
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test --test apply_tests 2>&1 | tail -15
```

Expected: all 3 new tests pass.

**Step 5: Run all tests**

```bash
cargo test 2>&1 | tail -10
```

Expected: all 65 tests pass (64 + 3 new - apply_tests now a separate file).

**Step 6: Commit**

```bash
git add src/editor/apply.rs tests/apply_tests.rs
git commit -m "feat: add CodeBlockInfo struct and collect_code_block_infos()"
```

---

### Task 4: Remove `BackgroundColor` from `for_code_block()`

The background will now be drawn by `NSBezierPath` in `drawRect:`, so the NSAttributedString background is removed to avoid a double-background.

**Files:**
- Modify: `src/markdown/attributes.rs:91-97`
- Modify: `tests/attributes_tests.rs`

**Step 1: Write a test to pin the new behavior**

Add to `tests/attributes_tests.rs`:

```rust
#[test]
fn code_block_gets_monospace_no_bg_color() {
    let attrs = AttributeSet::for_code_block();
    assert!(attrs.contains(&TextAttribute::Monospace));
    // Background color is now drawn via NSBezierPath overlay, not NSAttributedString.
    assert!(!attrs.contains(&TextAttribute::BackgroundColor("code_block_bg")));
}
```

**Step 2: Run to verify it fails (attribute still has BackgroundColor)**

```bash
cargo test code_block_gets_monospace_no_bg_color 2>&1 | tail -10
```

Expected: FAIL.

**Step 3: Remove `BackgroundColor` from `for_code_block()`**

In `src/markdown/attributes.rs`, change `for_code_block()`:

```rust
// old
pub fn for_code_block() -> Self {
    Self::new(vec![
        TextAttribute::Monospace,
        TextAttribute::BackgroundColor("code_block_bg"),
        TextAttribute::ForegroundColor("code_fg"),
    ])
}

// new
pub fn for_code_block() -> Self {
    Self::new(vec![
        TextAttribute::Monospace,
        TextAttribute::ForegroundColor("code_fg"),
    ])
}
```

**Step 4: Run tests**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass.

**Step 5: Commit**

```bash
git add src/markdown/attributes.rs tests/attributes_tests.rs
git commit -m "feat: remove BackgroundColor from for_code_block() — drawn via overlay"
```

---

### Task 5: Wire `code_block_infos` into `MditEditorDelegate`

Store code block info in the delegate after every re-parse, parallel to `heading_sep_positions`.

**Files:**
- Modify: `src/editor/text_storage.rs`

**Step 1: Update ivars and the `did_process_editing` handler**

Add to `src/editor/text_storage.rs` at the top:

```rust
use crate::editor::apply::{apply_attribute_runs, collect_code_block_infos, CodeBlockInfo};
```

(Replace the current `use crate::editor::apply::apply_attribute_runs;` line.)

Add to `MditEditorDelegateIvars` struct (after `heading_sep_positions`):

```rust
/// Code block metadata updated after every re-parse.
/// Read by MditTextView to draw the visual overlay and copy-to-clipboard.
code_block_infos: RefCell<Vec<CodeBlockInfo>>,
```

In `MditEditorDelegate::new`, add to the `set_ivars` call:

```rust
code_block_infos: RefCell::new(Vec::new()),
```

In `did_process_editing`, after the `*self.ivars().heading_sep_positions...` line (line 81), add:

```rust
let new_spans_ref = self.ivars().spans.borrow();
let infos = collect_code_block_infos(&new_spans_ref, &text);
drop(new_spans_ref);
*self.ivars().code_block_infos.borrow_mut() = infos;
```

In `reapply`, after the `*self.ivars().heading_sep_positions...` line (line 147), add:

```rust
let text_str = storage.string().to_string();
let spans_ref = self.ivars().spans.borrow();
let infos = collect_code_block_infos(&spans_ref, &text_str);
drop(spans_ref);
*self.ivars().code_block_infos.borrow_mut() = infos;
```

Add a public accessor method at the end of `impl MditEditorDelegate`:

```rust
/// Returns the code block metadata for all fenced code blocks in the document.
/// Used by `MditTextView` to draw the visual overlay.
pub fn code_block_infos(&self) -> Vec<CodeBlockInfo> {
    self.ivars().code_block_infos.borrow().clone()
}
```

**Step 2: Build and run all tests**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass.

**Step 3: Commit**

```bash
git add src/editor/text_storage.rs
git commit -m "feat: collect and store code block infos in MditEditorDelegate"
```

---

### Task 6: Implement `draw_code_blocks()` in `MditTextView`

Draw the rounded-rect background, border, and copy icon for each code block.

**Files:**
- Modify: `src/editor/text_view.rs`

**Step 1: Update imports**

Add to the `use objc2_app_kit::{...}` block:

```rust
NSBezierPath, NSImage, NSPasteboard, NSPasteboardTypeString,
```

Add to `use objc2_foundation::{...}`:

```rust
NSString,
```

**Step 2: Add `copy_button_rects` ivar**

In `MditTextViewIvars`, add after the `delegate` field:

```rust
/// Copy-button rects computed each draw cycle: (icon_rect, code_text).
/// Populated in draw_code_blocks(), read in mouseDown:.
copy_button_rects: RefCell<Vec<(NSRect, String)>>,
```

In `MditTextView::new`, add to `set_ivars`:

```rust
copy_button_rects: RefCell::new(Vec::new()),
```

**Step 3: Call `draw_code_blocks()` from `draw_rect`**

Update the `drawRect:` override (keep heading separators after code blocks):

```rust
#[unsafe(method(drawRect:))]
fn draw_rect(&self, dirty_rect: NSRect) {
    let _: () = unsafe { msg_send![super(self), drawRect: dirty_rect] };
    self.draw_code_blocks();
    self.draw_heading_separators();
}
```

**Step 4: Implement `draw_code_blocks()`**

Add this method to `impl MditTextView` (after `draw_heading_separators`):

```rust
fn draw_code_blocks(&self) {
    // Clear previous frame's hit rects.
    self.ivars().copy_button_rects.borrow_mut().clear();

    let delegate_ref = self.ivars().delegate.borrow();
    let delegate = match delegate_ref.as_ref() {
        Some(d) => d,
        None => return,
    };
    let infos = delegate.code_block_infos();
    if infos.is_empty() {
        return;
    }

    let layout_manager = match unsafe { self.layoutManager() } {
        Some(lm) => lm,
        None => return,
    };
    let text_container = match unsafe { self.textContainer() } {
        Some(tc) => tc,
        None => return,
    };

    let tc_origin = self.textContainerOrigin();
    let container_width = text_container.containerSize().width;
    let null_ptr = std::ptr::null_mut::<objc2_foundation::NSRange>();

    for info in &infos {
        if info.start_utf16 >= info.end_utf16 {
            continue;
        }

        // ── Map UTF-16 offsets to glyph indices ──────────────────────────
        let first_glyph: usize = unsafe {
            msg_send![&*layout_manager,
                glyphIndexForCharacterAtIndex: info.start_utf16]
        };
        // Use end_utf16 - 1 to get the last character's glyph.
        let last_char = info.end_utf16.saturating_sub(1);
        let last_glyph: usize = unsafe {
            msg_send![&*layout_manager,
                glyphIndexForCharacterAtIndex: last_char]
        };
        if first_glyph == usize::MAX || last_glyph == usize::MAX {
            continue;
        }

        // ── Get line fragment rects ───────────────────────────────────────
        let top_frag: NSRect = unsafe {
            msg_send![&*layout_manager,
                lineFragmentRectForGlyphAtIndex: first_glyph,
                effectiveRange: null_ptr]
        };
        let bot_frag: NSRect = unsafe {
            msg_send![&*layout_manager,
                lineFragmentRectForGlyphAtIndex: last_glyph,
                effectiveRange: null_ptr]
        };
        if top_frag.size.height == 0.0 || bot_frag.size.height == 0.0 {
            continue;
        }

        // ── Build full-width block rect (8pt vertical padding) ────────────
        let block_y = top_frag.origin.y + tc_origin.y - 8.0;
        let block_bottom = bot_frag.origin.y + bot_frag.size.height + tc_origin.y + 8.0;
        let block_rect = NSRect::new(
            NSPoint::new(tc_origin.x, block_y),
            NSSize::new(container_width, block_bottom - block_y),
        );

        // ── Fill background ───────────────────────────────────────────────
        let bg_color = NSColor::controlBackgroundColor();
        unsafe {
            let path: Retained<NSBezierPath> = msg_send![
                NSBezierPath::class(),
                bezierPathWithRoundedRect: block_rect
                xRadius: 6.0_f64
                yRadius: 6.0_f64
            ];
            bg_color.setFill();
            path.fill();
        }

        // ── Draw border ───────────────────────────────────────────────────
        let border_color = NSColor::separatorColor();
        unsafe {
            let border_path: Retained<NSBezierPath> = msg_send![
                NSBezierPath::class(),
                bezierPathWithRoundedRect: block_rect
                xRadius: 6.0_f64
                yRadius: 6.0_f64
            ];
            border_path.setLineWidth(0.5);
            border_color.setStroke();
            border_path.stroke();
        }

        // ── Draw SF Symbol copy icon (14×14pt, 6pt from bottom-right) ────
        let icon_x = block_rect.origin.x + block_rect.size.width - 20.0;
        let icon_y = block_rect.origin.y + 6.0;
        let icon_rect = NSRect::new(
            NSPoint::new(icon_x, icon_y),
            NSSize::new(14.0, 14.0),
        );
        unsafe {
            let name = NSString::from_str("doc.on.doc");
            if let Some(icon) = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &name, None
            ) {
                // Tint the icon with secondaryLabelColor for a muted appearance.
                let tint = NSColor::secondaryLabelColor();
                tint.set();
                icon.drawInRect(icon_rect);
            }
        }

        // Store rect for hit-testing in mouseDown:.
        self.ivars().copy_button_rects
            .borrow_mut()
            .push((icon_rect, info.text.clone()));
    }
}
```

**Step 5: Build**

```bash
cargo build 2>&1 | grep -E "^error" | head -20
```

Fix any compile errors (likely import issues — check exact objc2-app-kit method names using `cargo doc --open` if needed).

**Step 6: Run all tests**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass.

**Step 7: Commit**

```bash
git add src/editor/text_view.rs
git commit -m "feat: draw code block overlay — rounded rect + copy icon"
```

---

### Task 7: Add `mouseDown:` override for copy-to-clipboard

**Files:**
- Modify: `src/editor/text_view.rs`

**Step 1: Add `mouseDown:` to the `define_class!` block**

Inside the `impl MditTextView` block within `define_class!` (alongside `drawRect:`), add:

```rust
#[unsafe(method(mouseDown:))]
fn mouse_down(&self, event: &objc2_app_kit::NSEvent) {
    // Convert window coords → view coords.
    let window_point = unsafe { event.locationInWindow() };
    let view_point: NSPoint = unsafe {
        self.convertPoint_fromView(window_point, None)
    };

    // Check if click landed on any copy-button icon.
    let rects = self.ivars().copy_button_rects.borrow();
    for (rect, code_text) in rects.iter() {
        let in_rect = view_point.x >= rect.origin.x
            && view_point.x <= rect.origin.x + rect.size.width
            && view_point.y >= rect.origin.y
            && view_point.y <= rect.origin.y + rect.size.height;
        if in_rect {
            // Copy to clipboard.
            unsafe {
                let pb = NSPasteboard::generalPasteboard();
                pb.clearContents();
                let ns_str = NSString::from_str(code_text);
                pb.setString_forType(&ns_str, NSPasteboardTypeString);
            }
            return; // Consume event — don't pass to text editing.
        }
    }
    drop(rects);

    // Not a copy-button click — pass to standard text-view handling.
    let _: () = unsafe { msg_send![super(self), mouseDown: event] };
}
```

**Step 2: Build**

```bash
cargo build 2>&1 | grep -E "^error" | head -20
```

**Step 3: Run all tests**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass.

**Step 4: Commit**

```bash
git add src/editor/text_view.rs
git commit -m "feat: mouseDown: copies code block content to clipboard"
```

---

### Task 8: Manual end-to-end verification

**Step 1: Build and launch**

```bash
cargo build && ./target/debug/mdit
```

**Step 2: Test rendering**

Type or paste this into mdit:

```
# My Document

Some introductory text.

```rust
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
```

And some follow-up text here.
```

Expected:
- Code block spans full document width
- Rounded corners (6pt radius) with 0.5pt separator-colored border
- Background (`controlBackgroundColor`) slightly different from window background
- SF Symbol copy icon visible in bottom-right corner of block

**Step 3: Test copy button**

Click the copy icon. Open TextEdit and ⌘V.

Expected: the code content (`fn greet(name: &str) -> String {\n    format!("Hello, {}!", name)\n}`) is pasted.

**Step 4: Test multiple blocks**

Add a second code block. Verify each has its own box and working copy button.

**Step 5: Test light/dark mode**

Toggle macOS appearance (System Preferences → Appearance). Verify colors adapt.

**Step 6: Test window resize**

Drag to resize the window. Verify code block redraws at new width.

**Step 7: Test clicking inside text**

Click inside the code block text (not on the icon). Verify cursor is placed normally (not consumed).

---

## Troubleshooting

**`NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius` not found:**
Use `msg_send!` as shown in the plan. The selector is `bezierPathWithRoundedRect:xRadius:yRadius:`.

**Icon not appearing:**
`NSImage::imageWithSystemSymbolName_accessibilityDescription` requires macOS 11+. Verify deployment target. If needed, fall back to drawing a simple filled circle.

**Coordinates off:**
Remember: line fragment rects are in text container space. Add `tc_origin.y` for the vertical offset. `tc_origin.x` is the horizontal inset.

**`convertPoint_fromView` not found:**
Ensure "NSView" feature is in Cargo.toml (it is). Try `msg_send![self, convertPoint: p fromView: None::<&NSView>]`.
