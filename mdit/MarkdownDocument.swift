import SwiftUI
import UniformTypeIdentifiers

extension UTType {
    /// Matches the UTImportedTypeDeclarations entry in Info.plist.
    static let markdownDocument = UTType(importedAs: "net.daringfireball.markdown", conformingTo: .plainText)
}

/// Read-only document model for a Markdown file.
struct MarkdownDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.markdownDocument, .plainText] }

    var text: String

    init(text: String = "") {
        self.text = text
    }

    init(configuration: ReadConfiguration) throws {
        guard let data = configuration.file.regularFileContents else {
            throw CocoaError(.fileReadCorruptFile)
        }
        text = Self.decode(data)
    }

    /// Lossy UTF-8 decode: invalid byte sequences become U+FFFD instead of failing.
    static func decode(_ data: Data) -> String {
        String(decoding: data, as: UTF8.self)
    }

    // Required by FileDocument; never reached — DocumentGroup(viewing:) has no save UI.
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}
