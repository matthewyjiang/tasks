import Foundation

public struct AppPaths: Equatable, Sendable {
    public var applicationSupport: URL
    public var caches: URL
    public var temporary: URL

    public init(fileManager: FileManager = .default, bundleIdentifier: String = "tsk") throws {
        let supportRoot = try fileManager.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
        let cacheRoot = try fileManager.url(for: .cachesDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
        applicationSupport = supportRoot.appending(path: bundleIdentifier, directoryHint: .isDirectory)
        caches = cacheRoot.appending(path: bundleIdentifier, directoryHint: .isDirectory)
        temporary = fileManager.temporaryDirectory.appending(path: bundleIdentifier, directoryHint: .isDirectory)
    }

    public var databaseURL: URL {
        applicationSupport.appending(path: "tasks.sqlite3")
    }

    public var plaintextSettingsURL: URL {
        applicationSupport.appending(path: "settings.json")
    }

    public func createDirectories(fileManager: FileManager = .default) throws {
        try fileManager.createDirectory(at: applicationSupport, withIntermediateDirectories: true)
        try fileManager.createDirectory(at: caches, withIntermediateDirectories: true)
        try fileManager.createDirectory(at: temporary, withIntermediateDirectories: true)
    }
}
