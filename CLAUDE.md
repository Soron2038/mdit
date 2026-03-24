# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                  # debug build
cargo build --release        # optimized release (LTO, ~799 KB)
cargo run                    # run debug build
cargo test                   # run all tests
cargo test <test_name>       # run a single test (e.g. cargo test test_bold)
./scripts/build-dmg.sh       # build distributable DMG → dist/mdit-0.1.0.dmg
```

## Architecture

**mdit** is a native macOS Markdown editor built in Rust with AppKit bindings (`objc2` / `objc2-app-kit`).

**Core concept:** In-space rendering — Markdown syntax hides when the cursor leaves a span (Typora-style). No split view or preview pane; the document is the UI.

**Dual modes** (toggle Cmd+E):
- **Viewer** — read-only, full typography/color rendering, sidebar hidden
- **Editor** — editable, monospace with syntax visible, formatting sidebar shown

### Key modules

| Path | Role |
|------|------|
| `src/app.rs` | `AppDelegate` — central coordinator (~1,577 lines); owns all state, handles all actions, wires all components |
| `src/editor/tab_manager.rs` | Pure-Rust tab list; manages `Vec<DocumentState>` with safe index correction |
| `src/editor/document_state.rs` | Per-tab state: NSScrollView, NSTextView, editor delegate, file URL, dirty flag, view mode |
| `src/editor/renderer.rs` | Core rendering: `compute_attribute_runs()` applies cursor-aware show/hide logic to every Markdown span |
| `src/editor/apply.rs` | Converts Rust attribute runs to `NSAttributedString` attributes (handles UTF-8 → UTF-16 offsets) |
| `src/markdown/` | Comrak-based parser + attribute mapping + Syntect syntax highlighting |
| `src/ui/` | Self-contained UI components: `find_bar`, `sidebar`, `tab_bar`, `path_bar`, `appearance` |

### Rendering pipeline

```
Text change
  → MditEditorDelegate (NSTextStorageDelegate)
  → Comrak parse → MarkdownSpan tree
  → renderer.rs: cursor-aware attribute runs (show syntax if cursor inside span, hide otherwise)
  → apply.rs: map to NSFont / NSColor / NSParagraphStyle / NSBackgroundColor
  → NSTextView redraws
```

### Layout

Frame-based (no AutoLayout). All components are re-laid out in `windowDidResize:`.

Key height constants: `TAB_H=32`, `PATH_H=22`, `FIND_H_COMPACT=30`, `FIND_H_EXPANDED=56`.

Tab switching: only the active tab's NSScrollView is added to the window content view; switching removes the old one and inserts the new one.

### Tests

Test files live in `tests/`, one file per module (e.g. `tests/renderer_tests.rs`). The rendering logic in `renderer.rs` and `tab_manager.rs` is pure Rust and fully testable without AppKit.

## Key documents

- `FINISHING.md` — v1.0 remaining tasks and priorities
- `docs/plans/2026-02-24-mdit-prd.md` — full PRD (features, non-goals, architecture decisions)
