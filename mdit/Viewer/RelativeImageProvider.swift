import AppKit
import MarkdownEngine

/// Resolves standard `![alt](path)` image URLs against the directory of the
/// opened markdown file. The engine passes the raw URL string as the
/// request's `name`. Remote URLs are skipped (loading is synchronous inside
/// the styling pass and would block).
struct RelativeImageProvider: EmbeddedImageProvider {
    let baseDir: URL?

    func image(for reference: EmbeddedImageRequest) -> NSImage? {
        let raw = reference.name.removingPercentEncoding ?? reference.name
        if raw.hasPrefix("http://") || raw.hasPrefix("https://") { return nil }
        if raw.hasPrefix("file://"), let url = URL(string: reference.name) {
            return NSImage(contentsOf: url)
        }
        if raw.hasPrefix("/") { return NSImage(contentsOfFile: raw) }
        guard let baseDir else { return nil }
        return NSImage(contentsOf: URL(fileURLWithPath: raw, relativeTo: baseDir))
    }

    func fingerprint() -> AnyHashable { baseDir?.path ?? "" }
}
