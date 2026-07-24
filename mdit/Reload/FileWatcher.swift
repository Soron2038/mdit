import Foundation

/// Watches a single file for changes via a DispatchSource on an O_EVTONLY
/// file descriptor. Survives atomic saves (write-temp-then-rename replaces
/// the inode) by re-arming on `.delete`/`.rename`. Callbacks fire on the
/// main queue; write bursts are debounced.
final class FileWatcher {
    private let url: URL
    private let queue = DispatchQueue(label: "mdit.filewatcher", qos: .utility)
    private let onChange: () -> Void
    private let onDisappear: () -> Void

    private var source: DispatchSourceFileSystemObject?
    private var debounceWork: DispatchWorkItem?
    private var cancelled = false

    private static let debounceInterval: DispatchTimeInterval = .milliseconds(100)
    private static let rearmRetries = 5
    private static let rearmRetryDelay: DispatchTimeInterval = .milliseconds(100)

    /// Returns nil when the file cannot be opened for watching.
    init?(url: URL, onChange: @escaping () -> Void, onDisappear: @escaping () -> Void = {}) {
        self.url = url
        self.onChange = onChange
        self.onDisappear = onDisappear
        guard makeSource() else { return nil }
    }

    deinit {
        cancel()
    }

    /// Synchronous so it is safe to call from deinit (must not capture self
    /// asynchronously during deallocation). Never call from the watcher queue.
    func cancel() {
        queue.sync {
            cancelled = true
            debounceWork?.cancel()
            source?.cancel()
            source = nil
        }
    }

    // MARK: - Internals (all on `queue` unless noted)

    /// Thread-safe: only called from init (before any events) and from `queue`.
    private func makeSource() -> Bool {
        let fd = open(url.path, O_EVTONLY)
        guard fd >= 0 else { return false }

        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd,
            eventMask: [.write, .extend, .delete, .rename],
            queue: queue
        )
        source.setEventHandler { [weak self] in
            guard let self else { return }
            self.handle(events: source.data)
        }
        source.setCancelHandler { close(fd) }
        source.resume()
        self.source = source
        return true
    }

    private func handle(events: DispatchSource.FileSystemEvent) {
        guard !cancelled else { return }
        if events.contains(.delete) || events.contains(.rename) {
            source?.cancel()
            source = nil
            rearm(remainingRetries: Self.rearmRetries)
        } else {
            scheduleChange()
        }
    }

    /// After an atomic save the path may be briefly absent while the editor
    /// swaps files — retry with a short backoff before giving up.
    private func rearm(remainingRetries: Int) {
        guard !cancelled else { return }
        if makeSource() {
            scheduleChange()
        } else if remainingRetries > 0 {
            queue.asyncAfter(deadline: .now() + Self.rearmRetryDelay) { [weak self] in
                self?.rearm(remainingRetries: remainingRetries - 1)
            }
        } else {
            DispatchQueue.main.async(execute: onDisappear)
        }
    }

    private func scheduleChange() {
        debounceWork?.cancel()
        let work = DispatchWorkItem { [weak self] in
            guard let self, !self.cancelled else { return }
            DispatchQueue.main.async(execute: self.onChange)
        }
        debounceWork = work
        queue.asyncAfter(deadline: .now() + Self.debounceInterval, execute: work)
    }
}
