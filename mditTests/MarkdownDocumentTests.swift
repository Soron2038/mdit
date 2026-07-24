import Foundation
import Testing
@testable import mdit

struct MarkdownDocumentTests {
    @Test func decodesValidUTF8() {
        let data = Data("# Hällo wörld 🎉".utf8)
        #expect(MarkdownDocument.decode(data) == "# Hällo wörld 🎉")
    }

    @Test func decodesInvalidBytesLossily() {
        var data = Data("ok".utf8)
        data.append(contentsOf: [0xFF, 0xFE])
        let result = MarkdownDocument.decode(data)
        #expect(result.hasPrefix("ok"))
        #expect(result.contains("\u{FFFD}"))
    }

    @Test func decodesEmptyData() {
        #expect(MarkdownDocument.decode(Data()) == "")
    }
}
