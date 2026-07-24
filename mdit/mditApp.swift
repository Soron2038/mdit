import SwiftUI

@main
struct MditApp: App {
    var body: some Scene {
        DocumentGroup(viewing: MarkdownDocument.self) { configuration in
            MarkdownViewerView(
                document: configuration.document,
                fileURL: configuration.fileURL
            )
        }
    }
}
