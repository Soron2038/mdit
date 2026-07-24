# mdit

A slim, native Markdown viewer for macOS. Read-only by design — open a
`.md` file, read it beautifully rendered, and let the view follow the file
as it changes on disk.

Rendering is 100% native TextKit 2 via
[swift-markdown-engine](https://github.com/nodes-app/swift-markdown-engine) —
no WebView, no Electron, no JavaScript bundle.

## Features

- CommonMark + GFM: tables, task checkboxes, strikethrough
- Syntax-highlighted code blocks (atom-one light/dark, follows system appearance)
- Relative images: `![alt](img.png)` resolves next to the opened file
- Clickable links (open in your default browser)
- Live reload: the view updates when the file changes on disk — including
  atomic saves from editors like vim or VS Code
- One window per file; text is selectable and copyable, never editable

## Requirements

- macOS 14+
- To build: Xcode 15+ and [XcodeGen](https://github.com/yonaskolb/XcodeGen)
  (`brew install xcodegen`)

## Build

```sh
xcodegen generate
xcodebuild -project mdit.xcodeproj -scheme mdit -configuration Release build
```

The app lands in Xcode's DerivedData products folder; copy `mdit.app` to
`/Applications` to register it with Launch Services (enables Finder
"Open with" and double-click).

Handy shell alias:

```sh
alias mdit='open -a mdit'
```

### Package as DMG

```sh
xcodebuild -project mdit.xcodeproj -scheme mdit -configuration Release \
  -derivedDataPath build/DerivedData build
mkdir -p dist build/dmg-staging
cp -R build/DerivedData/Build/Products/Release/mdit.app build/dmg-staging/
ln -s /Applications build/dmg-staging/Applications
hdiutil create -volname "mdit" -srcfolder build/dmg-staging -ov -format UDZO \
  dist/mdit-$(date +%Y%m%d).dmg
```

## Notes

- The app is **not sandboxed**: it must read images referenced relative to
  the opened file. It is also unsigned in v1 — build it yourself or expect
  a Gatekeeper prompt.
- Remote images (`http/https`) are intentionally not loaded in v1.
- The rendering engine is pre-1.0 and pinned exactly (`0.10.1` in
  `project.yml`); treat any bump as its own verification pass (see
  `spike/NOTES.md`).

## Manual smoke checklist (release builds)

- [ ] Finder double-click opens the file (app in /Applications)
- [ ] Finder "Open with" lists mdit for `.md`, `.markdown`, `.mdown`, `.mkd`
- [ ] ⌘O open panel works; File → Open Recent populated
- [ ] Drop a `.md` on the Dock icon opens it
- [ ] `open -a mdit file.md` works
- [ ] Two files → two windows; reopening the same file focuses its window
- [ ] Edit the file in another editor → view updates in place
- [ ] Atomic-save editor (vim, TextEdit) → view still updates afterwards
- [ ] Delete the file on disk → "File Deleted" state appears
- [ ] Toggle system dark mode → theme and code highlighting follow
- [ ] Click a web link → default browser opens
- [ ] Code block is highlighted; GFM table renders as a table; checkboxes render
- [ ] Relative image next to the file renders
- [ ] No caret, typing does nothing, select + ⌘C works
