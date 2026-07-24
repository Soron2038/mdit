// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "spike",
    platforms: [.macOS(.v14)],
    dependencies: [
        .package(url: "https://github.com/nodes-app/swift-markdown-engine", exact: "0.10.1"),
    ],
    targets: [
        .executableTarget(
            name: "spike",
            dependencies: [
                .product(name: "MarkdownEngine", package: "swift-markdown-engine"),
                .product(name: "MarkdownEngineCodeBlocks", package: "swift-markdown-engine"),
            ]
        ),
    ]
)
