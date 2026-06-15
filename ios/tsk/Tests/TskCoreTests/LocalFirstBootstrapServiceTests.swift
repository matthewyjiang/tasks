import Foundation
import Testing
@testable import TskCore

private final class InMemorySecretStore: SecureSecretStoring, @unchecked Sendable {
    var values: [String: Data] = [:]

    func data(for id: String) throws -> Data? {
        values[id]
    }

    func setData(_ data: Data, for id: String) throws {
        values[id] = data
    }

    func removeData(for id: String) throws {
        values.removeValue(forKey: id)
    }
}

@Test func localFirstBootstrapCreatesAndPersistsDeviceAndAccountKeys() throws {
    let store = InMemorySecretStore()
    let service = LocalFirstBootstrapService(secretStore: store)

    let created = try service.ensureBootstrapped()
    #expect(created.createdDeviceKey)
    #expect(created.createdAccountDataKey)
    #expect(!created.devicePublicKey.isEmpty)
    #expect(store.values[SecureSecretID.devicePrivateKey] != nil)
    #expect(store.values[SecureSecretID.accountDataKey] != nil)

    let existing = try service.ensureBootstrapped()
    #expect(!existing.createdDeviceKey)
    #expect(!existing.createdAccountDataKey)
    #expect(existing.devicePublicKey == created.devicePublicKey)
}

@Test func localFirstBootstrapRepairsMissingAccountDataKeyWithoutReplacingDeviceKey() throws {
    let store = InMemorySecretStore()
    let bootstrap = generateLocalAccountBootstrap()
    store.values[SecureSecretID.devicePrivateKey] = Data(bootstrap.devicePrivateKey)

    let state = try LocalFirstBootstrapService(secretStore: store).ensureBootstrapped()

    #expect(!state.createdDeviceKey)
    #expect(state.createdAccountDataKey)
    #expect(state.devicePublicKey == bootstrap.devicePublicKey)
    #expect(store.values[SecureSecretID.accountDataKey] != nil)
}

@Test func taskNotificationIdentifiersUseStableTaskUUIDs() {
    let id = UUID(uuidString: "00000000-0000-0000-0000-000000000092")!

    #expect(LocalNotificationRequest.identifier(forTaskID: id) == "task.00000000-0000-0000-0000-000000000092")
}
