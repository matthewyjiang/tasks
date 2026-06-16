import Foundation
import Testing
@testable import TskCore

private final class AppModelSyncSecretStore: SecureSecretStoring, @unchecked Sendable {
    var values: [String: Data] = [:]
    func data(for id: String) throws -> Data? { values[id] }
    func setData(_ data: Data, for id: String) throws { values[id] = data }
    func removeData(for id: String) throws { values.removeValue(forKey: id) }
}

private final class AppModelSyncAuthClient: FfiAuthClient, @unchecked Sendable {
    var registerRequests: [FfiRegisterRequest] = []

    func registerAccount(serverUrl: String, request: FfiRegisterRequest) throws -> FfiTokenResponse {
        registerRequests.append(request)
        return FfiTokenResponse(jwt: "access", refreshToken: "refresh", userId: "account-1")
    }

    func login(serverUrl: String, request: FfiLoginRequest) throws -> FfiTokenResponse {
        FfiTokenResponse(jwt: "login-access", refreshToken: "login-refresh", userId: "account-1")
    }

    func refresh(serverUrl: String, request: FfiRefreshTokenRequest) throws -> FfiTokenResponse {
        FfiTokenResponse(jwt: "access-2", refreshToken: "refresh-2", userId: "account-1")
    }

    func deleteSession(serverUrl: String, request: FfiDeleteSessionRequest) throws {}
    func putCurrentDeviceKey(serverUrl: String, accessToken: String, request: FfiPutCurrentDeviceKeyRequest) throws {}
}

@Test @MainActor func appModelConfiguresSyncAndPersistsOnlyServerURL() async throws {
    let suiteName = "tsk-appmodel-sync-\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suiteName)!
    defer { defaults.removePersistentDomain(forName: suiteName) }
    let store = AppModelSyncSecretStore()
    let bootstrap = generateLocalAccountBootstrap()
    store.values[SecureSecretID.devicePrivateKey] = Data(bootstrap.devicePrivateKey)
    store.values[SecureSecretID.accountDataKey] = Data(bootstrap.accountDataKey)
    let authClient = AppModelSyncAuthClient()
    let coordinator = SyncCoordinator(serverURL: "", platform: CorePlatformAdapter(secretStore: store), authClient: authClient)
    let model = AppModel(repository: PreviewTaskRepository(), syncCoordinator: coordinator, serverURLDefaults: defaults)

    await model.updateSyncServerURL(" https://example.com/ ")
    await model.configureSync(email: " user@example.com ", password: "secret-password")

    #expect(model.syncServerURL == "https://example.com/")
    #expect(defaults.string(forKey: AppModel.syncServerURLDefaultsKey) == "https://example.com/")
    #expect(defaults.string(forKey: "password") == nil)
    #expect(authClient.registerRequests.count == 1)
    #expect(authClient.registerRequests[0].email == "user@example.com")
    #expect(store.values[SecureSecretID.accessToken] == Data("access".utf8))
    #expect(store.values[SecureSecretID.refreshToken] == Data("refresh".utf8))
    #expect(model.syncAuthState == .syncReady)
    #expect(model.canSyncNow)
}

@Test @MainActor func appModelAcceptsWrappedEnrollmentPayloadWithoutReplacingExistingKey() async throws {
    let suiteName = "tsk-appmodel-enrollment-\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suiteName)!
    defer { defaults.removePersistentDomain(forName: suiteName) }
    let recipient = generateLocalAccountBootstrap()
    let sender = generateLocalAccountBootstrap()
    let store = AppModelSyncSecretStore()
    store.values[SecureSecretID.devicePrivateKey] = Data(recipient.devicePrivateKey)
    let coordinator = SyncCoordinator(serverURL: "https://example.com", platform: CorePlatformAdapter(secretStore: store), authClient: AppModelSyncAuthClient())
    let model = AppModel(repository: PreviewTaskRepository(), syncCoordinator: coordinator, serverURLDefaults: defaults)
    await model.refreshSyncState()
    #expect(model.enrollmentState == .existingAccountPending)

    let payload = try createFfiWrappedAccountDataKeyPayload(
        accountDataKey: sender.accountDataKey,
        recipientPublicKey: recipient.devicePublicKey,
        senderPrivateKey: sender.devicePrivateKey
    )
    let json = """
    {
      "sender_public_key": "\(Data(payload.senderPublicKey).base64EncodedString())",
      "recipient_public_key": "\(Data(payload.recipientPublicKey).base64EncodedString())",
      "ciphertext": "\(Data(payload.wrappedAccountDataKey.ciphertext).base64EncodedString())",
      "nonce": "\(Data(payload.wrappedAccountDataKey.nonce).base64EncodedString())"
    }
    """

    await model.acceptWrappedAccountDataKeyPayload(json: json)
    #expect(model.errorMessage == nil)
    #expect(model.enrollmentState == .syncReady)
    #expect(store.values[SecureSecretID.accountDataKey] == Data(sender.accountDataKey))

    await model.acceptWrappedAccountDataKeyPayload(json: json)
    #expect(store.values[SecureSecretID.accountDataKey] == Data(sender.accountDataKey))
    #expect(model.errorMessage != nil)
}
