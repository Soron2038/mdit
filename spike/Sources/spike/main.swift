// Spike: verify swift-markdown-engine viability as a read-only viewer.
// Renders fixture.md in a window, introspects the text view, snapshots a PNG,
// prints machine-readable findings, then exits.

import AppKit
import SwiftUI
import MarkdownEngine
import MarkdownEngineCodeBlocks

let spikeDir = URL(fileURLWithPath: CommandLine.arguments.count > 1
    ? CommandLine.arguments[1]
    : FileManager.default.currentDirectoryPath)
let fixtureURL = spikeDir.appendingPathComponent("fixture.md")
let renderURL = spikeDir.appendingPathComponent("render.png")
let imgURL = spikeDir.appendingPathComponent("img.png")

// Generate the fixture image (red/blue gradient block) if missing.
if !FileManager.default.fileExists(atPath: imgURL.path) {
    let size = NSSize(width: 200, height: 100)
    let img = NSImage(size: size)
    img.lockFocus()
    NSGradient(starting: .systemRed, ending: .systemBlue)?
        .draw(in: NSRect(origin: .zero, size: size), angle: 0)
    img.unlockFocus()
    if let tiff = img.tiffRepresentation,
       let rep = NSBitmapImageRep(data: tiff),
       let png = rep.representation(using: .png, properties: [:]) {
        try? png.write(to: imgURL)
    }
}

let fixture: String = {
    guard let s = try? String(contentsOf: fixtureURL, encoding: .utf8) else {
        print("FINDING fixture=MISSING (\(fixtureURL.path))")
        exit(2)
    }
    return s
}()

struct SpikeImageProvider: EmbeddedImageProvider {
    let baseDir: URL
    func image(for reference: EmbeddedImageRequest) -> NSImage? {
        let url = URL(fileURLWithPath: reference.name, relativeTo: baseDir)
        let img = NSImage(contentsOf: url)
        print("FINDING imageProviderCalled name=\(reference.name) resolved=\(img != nil)")
        return img
    }
    func fingerprint() -> AnyHashable { 1 }
}

func findViews<T: NSView>(_ type: T.Type, in root: NSView) -> [T] {
    var result: [T] = []
    var queue: [NSView] = [root]
    while let v = queue.popLast() {
        if let t = v as? T { result.append(t) }
        queue.append(contentsOf: v.subviews)
    }
    return result
}

final class SpikeDelegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!

    func applicationDidFinishLaunching(_ notification: Notification) {
        var config = MarkdownEditorConfiguration.default
        config.extensions = [StrikethroughExtension()]
        config.services = MarkdownEditorServices(
            images: SpikeImageProvider(baseDir: spikeDir),
            syntaxHighlighter: HighlighterSwiftBridge()
        )

        let viewer = NativeTextViewWrapper(
            text: .constant(fixture),
            configuration: config,
            isEditable: false,
            onLinkClick: { url in print("FINDING onLinkClick url=\(url)") }
        )

        window = NSWindow(
            contentRect: NSRect(x: 100, y: 100, width: 800, height: 1100),
            styleMask: [.titled, .closable, .resizable],
            backing: .buffered, defer: false)
        window.title = "mdit spike"
        window.contentView = NSHostingView(rootView: viewer.frame(width: 800, height: 1100))
        window.orderFrontRegardless()

        // Engine styling + HighlighterSwift (JSCore) are async — give them time.
        DispatchQueue.main.asyncAfter(deadline: .now() + 3.5) { self.inspect() }
    }

    func inspect() {
        guard let content = window.contentView else { exit(3) }

        let textViews = findViews(NSTextView.self, in: content)
        print("FINDING textViewCount=\(textViews.count)")
        for tv in textViews {
            print("FINDING isEditable=\(tv.isEditable) isSelectable=\(tv.isSelectable)")
            if let storage = tv.textStorage {
                var links: [String] = []
                var attachments = 0
                let full = NSRange(location: 0, length: storage.length)
                storage.enumerateAttribute(.link, in: full) { value, _, _ in
                    if let v = value { links.append(String(describing: v)) }
                }
                storage.enumerateAttribute(.attachment, in: full) { value, _, _ in
                    if value != nil { attachments += 1 }
                }
                var colors = Set<String>()
                storage.enumerateAttribute(.foregroundColor, in: full) { value, _, _ in
                    if let c = value as? NSColor { colors.insert(c.description) }
                }
                print("FINDING linkAttributes=\(links)")
                print("FINDING attachmentCount=\(attachments)")
                print("FINDING distinctForegroundColors=\(colors.count)")
            }
        }
        let scrollViews = findViews(NSScrollView.self, in: content)
        print("FINDING scrollViewCount=\(scrollViews.count)")

        // Snapshot the whole content view.
        if let rep = content.bitmapImageRepForCachingDisplay(in: content.bounds) {
            content.cacheDisplay(in: content.bounds, to: rep)
            if let png = rep.representation(using: .png, properties: [:]) {
                try? png.write(to: renderURL)
                print("FINDING renderPNG=\(renderURL.path)")
            }
        }
        print("FINDING done=true")
        exit(0)
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = SpikeDelegate()
app.delegate = delegate
app.run()
