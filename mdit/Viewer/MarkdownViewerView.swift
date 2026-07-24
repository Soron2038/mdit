import SwiftUI
import MarkdownEngine
import MarkdownEngineCodeBlocks

/// Read-only markdown rendering for one document window.
struct MarkdownViewerView: View {
    @State private var model: ViewerModel

    private let configuration: MarkdownEditorConfiguration
    private let documentId: String

    init(document: MarkdownDocument, fileURL: URL?) {
        _model = State(initialValue: ViewerModel(initialText: document.text, fileURL: fileURL))
        configuration = Self.makeConfiguration(fileURL: fileURL)
        documentId = fileURL?.path ?? "untitled"
    }

    var body: some View {
        Group {
            if model.fileDeleted {
                ContentUnavailableView(
                    "File Deleted",
                    systemImage: "doc.questionmark",
                    description: Text("The file was removed from disk.")
                )
            } else {
                NativeTextViewWrapper(
                    text: Binding(get: { model.text }, set: { _ in }),
                    configuration: configuration,
                    documentId: documentId,
                    isEditable: false
                )
            }
        }
        .frame(minWidth: 360, minHeight: 240)
    }

    private static func makeConfiguration(fileURL: URL?) -> MarkdownEditorConfiguration {
        var config = MarkdownEditorConfiguration.default
        config.extensions = [StrikethroughExtension()]
        config.services = MarkdownEditorServices(
            images: RelativeImageProvider(baseDir: fileURL?.deletingLastPathComponent()),
            syntaxHighlighter: HighlighterSwiftBridge()
        )
        return config
    }
}
