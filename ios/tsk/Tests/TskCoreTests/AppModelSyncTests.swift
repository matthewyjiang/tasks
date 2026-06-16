import Foundation
import Testing
@testable import TskCore

private enum AppModelSyncTestError: LocalizedError {
    case deleteFailed

    var errorDescription: String? { "Delete failed" }
}

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

private final class AppModelSyncRecordingRepository: TaskRepository, @unchecked Sendable {
    private(set) var syncNowOnlineArguments: [Bool] = []
    var failDeletes = false

    func loadTasks(includeDeleted: Bool) async throws -> [TaskItem] { [] }
    func loadLists() async throws -> [TaskListItem] { [] }
    func createTask(title: String, body: String, dueAt: Date?, listID: UUID?, tags: [String]) async throws -> TaskItem {
        TaskItem(title: title, body: body, dueAt: dueAt, listID: listID, tags: tags)
    }
    func updateTask(_ task: TaskItem) async throws -> TaskItem { task }
    func deleteTask(id: UUID) async throws {
        if failDeletes { throw AppModelSyncTestError.deleteFailed }
    }
    func createList(name: String) async throws -> TaskListItem { TaskListItem(name: name) }
    func updateList(_ list: TaskListItem) async throws -> TaskListItem { list }
    func deleteList(id: UUID) async throws {}
    func syncSummary() async throws -> SyncSummary { SyncSummary(isOnline: false) }
    func syncNow(isOnline: Bool) async throws -> SyncSummary {
        syncNowOnlineArguments.append(isOnline)
        return SyncSummary(isOnline: isOnline)
    }
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

@Test @MainActor func backgroundSyncUsesPlatformReachabilityWhenUiReachabilityIsOffline() async throws {
    let suiteName = "tsk-appmodel-background-sync-\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suiteName)!
    defer { defaults.removePersistentDomain(forName: suiteName) }
    let bootstrap = generateLocalAccountBootstrap()
    let store = AppModelSyncSecretStore()
    store.values[SecureSecretID.devicePrivateKey] = Data(bootstrap.devicePrivateKey)
    store.values[SecureSecretID.accountDataKey] = Data(bootstrap.accountDataKey)
    store.values[SecureSecretID.accessToken] = Data("access".utf8)
    store.values[SecureSecretID.refreshToken] = Data("refresh".utf8)
    store.values[SecureSecretID.syncOriginID] = Data("https://example.com:443".utf8)
    let coordinator = SyncCoordinator(
        serverURL: "https://example.com",
        platform: CorePlatformAdapter(secretStore: store, networkAvailable: { true }),
        authClient: AppModelSyncAuthClient()
    )
    let repository = AppModelSyncRecordingRepository()
    let model = AppModel(repository: repository, syncCoordinator: coordinator, serverURLDefaults: defaults)
    model.updateReachability(.offline)
    repository.failDeletes = true
    _ = await model.deleteTask(id: UUID())
    #expect(model.errorMessage == "Delete failed")
    repository.failDeletes = false

    let success = await model.backgroundSyncNow()

    #expect(success)
    #expect(repository.syncNowOnlineArguments == [true])
    #expect(model.syncSummary.isOnline)
    #expect(model.errorMessage == "Delete failed")
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
