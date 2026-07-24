import Foundation
import Testing
@testable import mdit

private final class Counter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    var value: Int { lock.withLock { count } }
    func increment() { lock.withLock { count += 1 } }
}

/// Polls `condition` until it holds or `timeout` elapses.
private func eventually(
    timeout: Duration = .seconds(5),
    _ condition: @escaping () -> Bool
) async -> Bool {
    let clock = ContinuousClock()
    let deadline = clock.now.advanced(by: timeout)
    while clock.now < deadline {
        if condition() { return true }
        try? await Task.sleep(for: .milliseconds(50))
    }
    return condition()
}

struct FileWatcherTests {
    private func makeTempFile(_ content: String = "initial") throws -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("mdit-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent("watched.md")
        try content.write(to: url, atomically: false, encoding: .utf8)
        return url
    }

    @Test func inPlaceWriteFiresOnChange() async throws {
        let url = try makeTempFile()
        let changes = Counter()
        let watcher = try #require(FileWatcher(url: url, onChange: { changes.increment() }))
        defer { watcher.cancel() }

        try "changed".write(to: url, atomically: false, encoding: .utf8)
        #expect(await eventually { changes.value >= 1 })
    }

    @Test func atomicReplaceFiresAndRearms() async throws {
        let url = try makeTempFile()
        let changes = Counter()
        let watcher = try #require(FileWatcher(url: url, onChange: { changes.increment() }))
        defer { watcher.cancel() }

        // Atomic save: write a sibling temp file, then rename it over the original.
        let temp = url.deletingLastPathComponent().appendingPathComponent("temp.md")
        try "replaced".write(to: temp, atomically: false, encoding: .utf8)
        _ = try FileManager.default.replaceItemAt(url, withItemAt: temp)
        #expect(await eventually { changes.value >= 1 }, "replace should fire onChange")

        // The watcher must have re-armed on the new inode: a subsequent
        // in-place write still fires.
        let baseline = changes.value
        try "changed again".write(to: url, atomically: false, encoding: .utf8)
        #expect(await eventually { changes.value > baseline }, "write after replace should fire (re-arm)")
    }

    @Test func deleteWithoutReplacementReportsDisappear() async throws {
        let url = try makeTempFile()
        let disappearances = Counter()
        let watcher = try #require(FileWatcher(
            url: url,
            onChange: {},
            onDisappear: { disappearances.increment() }
        ))
        defer { watcher.cancel() }

        try FileManager.default.removeItem(at: url)
        #expect(await eventually { disappearances.value >= 1 })
    }

    @Test func writeBurstIsDebounced() async throws {
        let url = try makeTempFile()
        let changes = Counter()
        let watcher = try #require(FileWatcher(url: url, onChange: { changes.increment() }))
        defer { watcher.cancel() }

        for i in 0..<10 {
            try "burst \(i)".write(to: url, atomically: false, encoding: .utf8)
        }
        try await Task.sleep(for: .seconds(1))
        let count = changes.value
        #expect(count >= 1 && count <= 3, "10 rapid writes should coalesce, got \(count)")
    }

    @Test func initReturnsNilForMissingFile() {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("does-not-exist-\(UUID().uuidString).md")
        #expect(FileWatcher(url: url, onChange: {}) == nil)
    }
}
