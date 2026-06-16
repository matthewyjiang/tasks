import Foundation
import Testing
@testable import TskCore

private final class PlatformAdapterInMemorySecretStore: SecureSecretStoring, @unchecked Sendable {
    var values: [String: Data] = [:]
    var failingLoadIDs: Set<String> = []

    func data(for id: String) throws -> Data? {
        if failingLoadIDs.contains(id) { throw TestSecretStoreError.forcedFailure }
        return values[id]
    }

    func setData(_ data: Data, for id: String) throws {
        values[id] = data
    }

    func removeData(for id: String) throws {
        values.removeValue(forKey: id)
    }
}

private enum TestSecretStoreError: Error {
    case forcedFailure
}

@Test func corePlatformAdapterStoresLoadsAndDeletesCoreSecretIDs() throws {
    let store = PlatformAdapterInMemorySecretStore()
    let platform = CorePlatformAdapter(secretStore: store)
    let ids = [
        SecureSecretID.accessToken,
        SecureSecretID.refreshToken,
        SecureSecretID.accountDataKey,
        SecureSecretID.devicePrivateKey
    ]

    for (index, id) in ids.enumerated() {
        let bytes = [UInt8(index), UInt8(index + 1), UInt8(index + 2)]
        try platform.storeKey(id: id, bytes: bytes)
        #expect(store.values[id] == Data(bytes))
        #expect(try platform.loadKey(id: id) == bytes)

        try platform.deleteKey(id: id)
        #expect(store.values[id] == nil)
    }
}

@Test func corePlatformAdapterReportsMissingKeysAsPlatformError() throws {
    let platform = CorePlatformAdapter(secretStore: PlatformAdapterInMemorySecretStore())

    do {
        _ = try platform.loadKey(id: SecureSecretID.accessToken)
        Issue.record("expected missing key to throw")
    } catch let error as FfiCoreError {
        guard case .PlatformError(let message) = error else {
            Issue.record("expected PlatformError, got \(error)")
            return
        }
        #expect(message.contains("missing key"))
        #expect(message.contains(SecureSecretID.accessToken))
    }
}

@Test func corePlatformAdapterDistinguishesUnexpectedSecretStoreFailures() throws {
    let store = PlatformAdapterInMemorySecretStore()
    store.failingLoadIDs.insert(SecureSecretID.refreshToken)
    let platform = CorePlatformAdapter(secretStore: store)

    do {
        _ = try platform.loadKey(id: SecureSecretID.refreshToken)
        Issue.record("expected load failure to throw")
    } catch let error as FfiCoreError {
        guard case .PlatformError(let message) = error else {
            Issue.record("expected PlatformError, got \(error)")
            return
        }
        #expect(message.contains("failed to load key"))
        #expect(message.contains(SecureSecretID.refreshToken))
    }
}

@Test func corePlatformAdapterUsesInjectedReachability() {
    var online = false
    let platform = CorePlatformAdapter(secretStore: PlatformAdapterInMemorySecretStore()) { online }

    #expect(!platform.networkAvailable())
    online = true
    #expect(platform.networkAvailable())
}
