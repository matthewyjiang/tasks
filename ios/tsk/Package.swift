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
    dependencies: [
        .package(url: "https://github.com/swiftlang/swift-testing.git", from: "0.12.0")
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
        .testTarget(
            name: "TskCoreTests",
            dependencies: [
                "TskCore",
                .product(name: "Testing", package: "swift-testing")
            ]
        )
    ]
)
