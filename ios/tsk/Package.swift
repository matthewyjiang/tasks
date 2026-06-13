// swift-tools-version: 5.9
import PackageDescription

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
        .target(name: "TskCore"),
        .executableTarget(name: "tsk", dependencies: ["TskCore"]),
        .testTarget(name: "TskCoreTests", dependencies: ["TskCore"])
    ]
)
