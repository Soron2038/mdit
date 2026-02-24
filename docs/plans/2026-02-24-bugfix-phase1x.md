# Phase 1.x Bug Fix Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix three confirmed runtime bugs in mdit Phase 1: dark-mode background colour, setext-heading display corruption, and toolbar button wiring.

**Architecture:** All fixes are surgical — no new subsystems needed. The existing renderer/apply/toolbar pipeline is correct in principle; only specific edge cases are wrong.

**Tech Stack:** Rust, objc2 + objc2-appkit, comrak

---

## Bug Inventory

### BUG-1 — Dark Mode: background stays white

**Root cause:** `apply_scheme()` in `src/app.rs` calls `ed.reapply()` (text attributes only) but never calls `text_view.setBackgroundColor()`. The initial colour `NSColor::textBackgroundColor()` is a semantic colour that tracks the *system* appearance, not our app-level scheme switch. So the NSTextView background stays white after a manual Dark Mode toggle.

**Affected files:**
- `src/app.rs` — `apply_scheme()`, `did_finish_launching()`
- `src/editor/text_view.rs` — initial `setBackgroundColor` call

**Fix summary:** After updating the scheme, explicitly set `tv.setBackgroundColor()` from `scheme.background`. Do the same during the initial startup when the system is already dark.

---

### BUG-2 — Setext heading misparse corrupts italic display

**Root cause:** CommonMark allows *setext headings*: a paragraph followed by a line of `---` (H2) or `===` (H1). When the user writes:

```
*kursiv*
-
```

comrak correctly parses this as a level-2 setext heading whose content is `*kursiv*`. The renderer in `src/editor/renderer.rs` (branch `NodeKind::Heading`) **always** assumes ATX style (`# `) and blindly hides the first `level + 1` bytes as a "syntax marker". For a setext heading that means:

- `prefix_len = 2 + 1 = 3` → hides bytes 0–3 = `*ku` (foreground → clear)
- bytes 3–end = `rsiv*\n-` rendered with H2 font size

Result: only `rsiv*` visible, in heading size.

**Affected files:**
- `src/editor/renderer.rs` — `collect_runs`, `NodeKind::Heading` branch

**Fix summary:** Detect ATX vs setext by checking whether `text[start]` is `#`.
- **ATX**: keep existing logic (hide `level+1` char prefix).
- **Setext**: find the last `\n` inside the span; apply heading style to everything *before* it; treat the underline line (`-…` or `=…`) as a syntax marker (shown/hidden based on cursor).

---

### BUG-3 — Floating toolbar buttons are no-ops

**Root cause:** `FloatingToolbar::new()` has an explicit TODO: buttons are created without `setTarget:` / `setAction:`. The AppDelegate already has `applyBold:`, `applyItalic:`, `applyInlineCode:`, `applyStrikethrough:`, `applyH1:`, `applyH2:`, `applyH3:` implemented.

**Affected files:**
- `src/ui/toolbar.rs` — `FloatingToolbar::new()`
- `src/app.rs` — call site `FloatingToolbar::new(mtm)`

**Fix summary:** Add a `target: &AnyObject` parameter to `FloatingToolbar::new()`. Use a static map from button label to selector string; call `NSControl::setTarget` / `NSControl::setAction` for each button. Pass the AppDelegate (`self`) as target in `app.rs`.

---

## Task 1 — BUG-1: Fix dark-mode background colour

**Files:**
- Modify: `src/app.rs` (lines ~232–242, `apply_scheme`; lines ~44–86, `did_finish_launching`)
- Modify: `src/editor/text_view.rs` (initial `setBackgroundColor` call, line ~46)

**Step 1: Write the failing (manual) test**

No automated test is possible for AppKit painting, but write a unit test that verifies `ColorScheme::dark().background` is distinct from `ColorScheme::light().background` — this documents the expected values and catches future regressions.

Add to `tests/appearance_tests.rs`:

```rust
#[test]
fn dark_background_differs_from_light() {
    let dark = ColorScheme::dark();
    let light = ColorScheme::light();
    assert_ne!(dark.background, light.background);
    // dark background must be darker than 0.5 luminance
    let (r, g, b) = dark.background;
    assert!(r < 0.5 && g < 0.5 && b < 0.5, "dark bg should be dark: {:?}", dark.background);
}
```

Run: `cargo test appearance_tests`
Expected: PASS (values are already correct, test is a guard)

**Step 2: Fix `text_view.rs` initial background**

In `create_editor_view()`, remove the semantic-colour call and use a plain colour matching the default light scheme:

```rust
// Before
text_view.setBackgroundColor(&NSColor::textBackgroundColor());

// After — use explicit sRGB matching ColorScheme::light().background (0.98, 0.98, 0.98)
let bg = NSColor::colorWithRed_green_blue_alpha(0.98, 0.98, 0.98, 1.0);
text_view.setBackgroundColor(&bg);
```

**Step 3: Fix `app.rs` — add background update to `apply_scheme`**

In `apply_scheme()`, after `ed.set_scheme(scheme)` and before `ed.reapply()`:

```rust
fn apply_scheme(&self, scheme: ColorScheme) {
    if let Some(ed) = self.ivars().editor_delegate.get() {
        ed.set_scheme(scheme);
        if let Some(tv) = self.ivars().text_view.get() {
            // Update window background to match colour scheme.
            let (r, g, b) = scheme.background;
            let bg = NSColor::colorWithRed_green_blue_alpha(r, g, b, 1.0);
            tv.setBackgroundColor(&bg);
            if let Some(storage) = unsafe { tv.textStorage() } {
                ed.reapply(&storage);
            }
        }
    }
}
```

`NSColor::colorWithRed_green_blue_alpha` is already used in `apply.rs`; `NSColor` is already imported in `app.rs`.

**Step 4: Fix initial dark startup in `did_finish_launching`**

After `editor_delegate.set_scheme(initial_scheme)` is called, invoke `apply_scheme` for real (which now also updates the background). Replace the standalone `set_scheme` call with a call to the full helper:

```rust
// Remove: editor_delegate.set_scheme(initial_scheme);
// After storing all ivars, call:
self.apply_scheme(initial_scheme);
```

This requires the ivars to be stored first (they already are in the current code).

**Step 5: Run tests and build**

```bash
cargo test
cargo build
```

Expected: all 48 tests green, 0 warnings.

**Step 6: Commit**

```bash
git add src/app.rs src/editor/text_view.rs tests/appearance_tests.rs
git commit -m "fix(dark-mode): set NSTextView background colour on scheme change

apply_scheme() now calls tv.setBackgroundColor() with the explicit sRGB
value from ColorScheme.background, so the view background updates when
the user switches between Light/Dark manually from the menu.

Also fixes initial startup in already-dark system mode.

Co-Authored-By: Warp <agent@warp.dev>"
```

---

## Task 2 — BUG-2: Fix setext-heading display corruption

**Files:**
- Modify: `src/editor/renderer.rs` (`collect_runs`, `NodeKind::Heading` branch, lines ~105–114)
- Test: `tests/renderer_tests.rs`

**Step 1: Write failing tests**

Add to `tests/renderer_tests.rs`:

```rust
#[test]
fn setext_h2_does_not_hide_content_prefix() {
    // "kursiv\n-\n" — a setext H2. The heading content ("kursiv") must NOT
    // have its first characters hidden. Only the underline line should be
    // treated as a syntax marker.
    let text = "kursiv\n-\n";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);

    // Find the run covering "kursiv" — must be heading style, NOT hidden
    let content_run = runs.iter().find(|r| r.range == (0, 6)).expect("content run");
    assert!(content_run.attrs.contains(&TextAttribute::FontSize(28.0)),
        "setext H2 content must have H2 font size");
    assert!(!content_run.attrs.contains(&TextAttribute::Hidden),
        "setext H2 content must not be hidden");
}

#[test]
fn setext_h2_underline_is_syntax_marker() {
    // The "-" underline line must be rendered as a syntax marker.
    let text = "kursiv\n-\n";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);

    // "\n-\n" starts at byte 6; underline "-" at byte 7
    // When cursor is None, syntax markers must be hidden
    let underline_run = runs.iter()
        .find(|r| r.range.0 >= 6 && r.range.1 <= 9)
        .expect("underline region run");
    assert!(underline_run.attrs.contains(&TextAttribute::Hidden),
        "setext underline must be hidden when cursor is outside");
}

#[test]
fn atx_heading_prefix_still_hidden() {
    // Regression: ATX headings must still hide the "## " prefix.
    let text = "## Hello\n";
    let spans = parse(text);
    let runs = compute_attribute_runs(text, &spans, None);

    let prefix_run = runs.iter().find(|r| r.range == (0, 3)).expect("ATX prefix run");
    assert!(prefix_run.attrs.contains(&TextAttribute::Hidden),
        "ATX prefix '## ' must be hidden");
}
```

Run: `cargo test renderer_tests`
Expected: the two new setext tests FAIL, the atx regression test PASSES.

**Step 2: Fix `renderer.rs`**

Replace the `NodeKind::Heading` branch in `collect_runs`:

```rust
NodeKind::Heading { level } => {
    // Distinguish ATX headings ("# …") from setext headings (underline style).
    let is_atx = text.as_bytes().get(start).copied() == Some(b'#');

    if is_atx {
        // ATX: hide the "# " (or "## " etc.) prefix.
        let prefix_len = (*level as usize + 1).min(end - start);
        runs.push(AttributeRun { range: (start, start + prefix_len), attrs: syn });
        if start + prefix_len < end {
            runs.push(AttributeRun {
                range: (start + prefix_len, end),
                attrs: AttributeSet::for_heading(*level),
            });
        }
    } else {
        // Setext: find the underline (last line of the span).
        // Everything before the final newline is content; the last line is
        // the underline marker (`---` / `===`), rendered as a syntax token.
        let span_slice = &text[start..end];
        if let Some(nl_rel) = span_slice.rfind('\n') {
            let nl_abs = start + nl_rel;
            if start < nl_abs {
                runs.push(AttributeRun {
                    range: (start, nl_abs),
                    attrs: AttributeSet::for_heading(*level),
                });
            }
            if nl_abs < end {
                runs.push(AttributeRun {
                    range: (nl_abs, end),
                    attrs: syn, // shown/hidden based on cursor
                });
            }
        } else {
            // No newline found (degenerate span) — treat whole range as content.
            runs.push(AttributeRun {
                range: (start, end),
                attrs: AttributeSet::for_heading(*level),
            });
        }
    }
}
```

**Step 3: Run tests to verify green**

Run: `cargo test renderer_tests`
Expected: all renderer tests PASS (10 original + 3 new = 13 total)

**Step 4: Commit**

```bash
git add src/editor/renderer.rs tests/renderer_tests.rs
git commit -m "fix(renderer): handle setext headings correctly

ATX headings start with '#' and have a prefix to hide.
Setext headings use an underline (--- / ===) and have no prefix.

Previously the renderer applied ATX logic to setext headings, hiding
the first `level+1` bytes of content — causing 'kursiv' to appear as
'rsiv*' in heading size.

Now setext headings correctly:
- Apply heading font size to the content (everything before the underline)
- Treat the underline line as a syntax marker (hidden when cursor is away)

Co-Authored-By: Warp <agent@warp.dev>"
```

---

## Task 3 — BUG-3: Wire floating toolbar buttons to AppDelegate actions

**Files:**
- Modify: `src/ui/toolbar.rs` — `FloatingToolbar::new()` signature and button setup
- Modify: `src/app.rs` — call site in `did_finish_launching`

**Step 1: No automated test exists for AppKit button wiring, document instead**

Note in code comments that the correct selectors are verified at runtime. The `applyBold:` etc. actions are already tested implicitly via keyboard shortcuts.

**Step 2: Update `toolbar.rs`**

Change `new()` to accept a `target: &AnyObject`:

```rust
use objc2::runtime::{AnyObject, Sel};
use objc2_app_kit::NSControl;

/// Mapping: button label → ObjC selector name (matches AppDelegate action methods).
const BTN_ACTIONS: &[(&str, &str)] = &[
    ("B",    "applyBold:"),
    ("I",    "applyItalic:"),
    ("Code", "applyInlineCode:"),
    ("~~",   "applyStrikethrough:"),
    ("H1",   "applyH1:"),
    ("H2",   "applyH2:"),
    ("H3",   "applyH3:"),
];
```

Replace `BTN_LABELS` iteration with `BTN_ACTIONS` and wire each button:

```rust
pub fn new(mtm: MainThreadMarker, target: &AnyObject) -> Self {
    // …existing panel + blur setup unchanged…

    if let Some(content) = panel.contentView() {
        let blur = /* …unchanged… */;
        content.addSubview(&blur);

        let y = (PANEL_H - BTN_H) / 2.0;
        for (i, (label, action_name)) in BTN_ACTIONS.iter().enumerate() {
            let x = BTN_MARGIN + i as f64 * (BTN_W + BTN_GAP);
            let btn = NSButton::initWithFrame(
                NSButton::alloc(mtm),
                NSRect::new(NSPoint::new(x, y), NSSize::new(BTN_W, BTN_H)),
            );
            btn.setTitle(&NSString::from_str(label));
            btn.setButtonType(NSButtonType::MomentaryPushIn);
            btn.setBezelStyle(NSBezelStyle::Toolbar);
            unsafe {
                NSControl::setTarget(&btn, Some(target));
                NSControl::setAction(&btn, Some(Sel::register(action_name)));
            }
            content.addSubview(&btn);
        }
    }

    Self { panel }
}
```

**Step 3: Update call site in `app.rs`**

In `did_finish_launching`, pass `self` (the AppDelegate) as target:

```rust
// Before
let toolbar = FloatingToolbar::new(mtm);

// After
let toolbar = FloatingToolbar::new(mtm, AnyObject::from_ref(self));
// or, if AnyObject::from_ref is not available:
let toolbar = FloatingToolbar::new(mtm, unsafe { &*(self as *const AppDelegate as *const AnyObject) });
```

Add `use objc2::runtime::AnyObject;` to imports in `app.rs` if not already present.

**Step 4: Build to verify compilation**

```bash
cargo build
```

Expected: compiles with 0 errors, 0 warnings.

**Step 5: Commit**

```bash
git add src/ui/toolbar.rs src/app.rs
git commit -m "fix(toolbar): wire formatting buttons to AppDelegate actions

FloatingToolbar::new() now accepts an AnyObject target. Each button
is wired via NSControl::setTarget/setAction to the corresponding
AppDelegate action method (applyBold:, applyItalic:, etc.).

Removes the TODO stub. Buttons now wrap selected text in Markdown
syntax identically to keyboard shortcuts.

Co-Authored-By: Warp <agent@warp.dev>"
```

---

## Final verification

After all three tasks:

```bash
cargo test
cargo build --release
```

Expected:
- All tests green (48 + 3 new renderer tests = 51 total)
- Release binary builds without warnings
- Manual smoke test: dark mode background is dark, setext headings render correctly, toolbar buttons apply formatting
