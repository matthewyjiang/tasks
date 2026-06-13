// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "tsk-ios",
    platforms: [
        .iOS(.v17),
        .macOS(.v14)
    ],
    products: [
        .library(name: "TskCore", targets: ["TskCore"])
    ],
    targets: [
        .binaryTarget(
            name: "taskmanager_coreFFI",
            path: "Frameworks/TaskmanagerCore.xcframework"
        ),
        .target(
            name: "TskCore",
            dependencies: ["taskmanager_coreFFI"],
            linkerSettings: [
                .linkedLibrary("iconv")
            ]
        ),
        .testTarget(name: "TskCoreTests", dependencies: ["TskCore"])
    ]
)
