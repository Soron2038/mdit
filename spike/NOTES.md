# Spike findings — swift-markdown-engine 0.10.1 viability (2026-07-24)

Verdict: **GO** — all four gate criteria pass. Verified with a minimal SwiftPM
executable (`spike/`) rendering `fixture.md` off-screen, introspecting the
NSTextView, and snapshotting the result (`render.png`).

Note: the plan referenced engine version 0.1.0; the actual latest release is
**0.10.1** — pin that.

## Gate criteria

| # | Criterion | Result | Evidence / how |
|---|-----------|--------|----------------|
| a | Read-only | **PASS** | `NativeTextViewWrapper(isEditable: false)` is a first-class init parameter ("renders read-only with no caret"). Introspection confirms `isEditable=false`, `isSelectable=true` (⌘C works). |
| b | Relative images `![](img.png)` | **PASS** | Standard syntax is parsed; the URL string is passed as `EmbeddedImageRequest.name` to the app's `EmbeddedImageProvider`. A provider resolving against the file's directory renders the image (confirmed visually). Images are drawn as layout fragments, not NSTextAttachments. |
| c | Web links | **PASS** | `.link` attribute present; `clickedOnLink` delegate returns false for URL links → AppKit opens the browser. Source comment: "read-only links must stay navigable". There is also an optional `onLinkClick: (String) -> Void` callback. |
| d | GFM tables | **PASS** | Parser has a dedicated `.table` block kind; renders as a real table with borders/header (confirmed visually). |

## Additional findings for the real integration (M3)

- **Strikethrough `~~x~~` is opt-in**: `config.extensions = [StrikethroughExtension()]` — without it the tildes render literally. Task-list completion strikethrough works regardless.
- Services are injected via `config.services = MarkdownEditorServices(images:, syntaxHighlighter:, ...)`.
- Code highlighting: `HighlighterSwiftBridge()` from the `MarkdownEngineCodeBlocks` product works out of the box (atom-one light/dark, auto-switches with system appearance; JSCore-based, so first render is async — allow for it in tests).
- Task checkboxes, GFM tables, headings, inline code all render correctly with `.default` configuration.
- Engine styling is asynchronous; the spike waits 3.5 s before snapshotting.
- One `NSScrollView` in the hierarchy → scroll-position preservation via introspection looks feasible.
