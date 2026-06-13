// swift-tools-version: 5.9
import Foundation
import PackageDescription

let packageDirectory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let repositoryRoot = packageDirectory.deletingLastPathComponent().deletingLastPathComponent()
let coreDebugLibraryDirectory = repositoryRoot.appending(path: "target/debug").path

let package = Package(
    name: "tsk-ios",
    platforms: [
        .iOS(.v17),
        .macOS(.v14)
    ],
    products: [
        .library(name: "TskCore", targets: ["TskCore"]),
        .executable(name: "tsk", targets: ["tsk"])
    ],
    targets: [
        .systemLibrary(name: "taskmanager_coreFFI", path: "Sources/TskCore/Generated"),
        .target(
            name: "TskCore",
            dependencies: ["taskmanager_coreFFI"],
            linkerSettings: [
                .unsafeFlags(["-L\(coreDebugLibraryDirectory)", "-ltaskmanager_core"])
            ]
        ),
        .executableTarget(name: "tsk", dependencies: ["TskCore"]),
        .testTarget(name: "TskCoreTests", dependencies: ["TskCore"])
    ]
)
