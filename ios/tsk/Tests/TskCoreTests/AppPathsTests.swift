import Testing
@testable import TskCore

@Test func appPathsUseSandboxApplicationSupportAndStableFileNames() throws {
    let paths = try AppPaths(bundleIdentifier: "tsk-tests")

    #expect(paths.bundleIdentifier == "tsk-tests")
    #expect(paths.applicationSupport.path.contains("Application Support"))
    #expect(paths.applicationSupport.path.hasSuffix("tsk-tests"))
    #expect(paths.databaseURL.lastPathComponent == "tasks.sqlite3")
    #expect(paths.plaintextSettingsURL.lastPathComponent == "settings.json")
    #expect(!paths.databaseURL.path.contains("Documents"))
}
