import Foundation
import Observation

/// Owns the displayed markdown text and the file watcher that keeps it in
/// sync with the file on disk.
@MainActor
@Observable
final class ViewerModel {
    private(set) var text: String
    private(set) var fileDeleted = false

    let fileURL: URL?
    @ObservationIgnored private var watcher: FileWatcher?

    init(initialText: String, fileURL: URL?) {
        self.text = initialText
        self.fileURL = fileURL
        startWatching()
    }

    private func startWatching() {
        guard let fileURL else { return }
        watcher = FileWatcher(
            url: fileURL,
            onChange: { [weak self] in self?.reload() },
            onDisappear: { [weak self] in self?.fileDeleted = true }
        )
    }

    func reload() {
        guard let fileURL else { return }
        guard let data = try? Data(contentsOf: fileURL) else {
            fileDeleted = true
            return
        }
        fileDeleted = false
        text = MarkdownDocument.decode(data)
    }
}
