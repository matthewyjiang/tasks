import Foundation
import Testing
@testable import TskCore

private final class CoordinatorInMemorySecretStore: SecureSecretStoring, @unchecked Sendable {
    var values: [String: Data] = [:]

    func data(for id: String) throws -> Data? { values[id] }
    func setData(_ data: Data, for id: String) throws { values[id] = data }
    func removeData(for id: String) throws { values.removeValue(forKey: id) }
}

private final class RecordingSyncClient: FfiSyncClient, @unchecked Sendable {
    var pushCalls = 0
    var pullCalls = 0
    var failFirstPushWithAuthExpired = false

    func pushBlobs(blobs: [FfiBlobPush]) throws -> FfiPushResponse {
        pushCalls += 1
        if failFirstPushWithAuthExpired, pushCalls == 1 {
            throw FfiCoreError.SyncError(errorMessage: "auth expired")
        }
        return FfiPushResponse(acceptedTaskIds: blobs.map(\.taskId), failedTaskIds: [])
    }

    func deleteBlobs(taskIds: [String]) throws -> FfiPushResponse {
        FfiPushResponse(acceptedTaskIds: taskIds, failedTaskIds: [])
    }

    func pullBlobs(since: Int64) throws -> FfiPullResponse {
        pullCalls += 1
        return FfiPullResponse(blobs: [], cursor: since)
    }
}

private final class RecordingAuthClient: FfiAuthClient, @unchecked Sendable {
    var registerRequests: [(String, FfiRegisterRequest)] = []
    var loginRequests: [(String, FfiLoginRequest)] = []
    var refreshRequests: [(String, FfiRefreshTokenRequest)] = []
    var deleteRequests: [(String, FfiDeleteSessionRequest)] = []
    var putDeviceKeyRequests: [(String, String, FfiPutCurrentDeviceKeyRequest)] = []
    var registerResult = FfiTokenResponse(jwt: "registered-access", refreshToken: "registered-refresh", userId: "account-1")
    var refreshResult = FfiTokenResponse(jwt: "rotated-access", refreshToken: "rotated-refresh", userId: "account-1")
    var refreshError: Error?

    func registerAccount(serverUrl: String, request: FfiRegisterRequest) throws -> FfiTokenResponse {
        registerRequests.append((serverUrl, request))
        return registerResult
    }

    func login(serverUrl: String, request: FfiLoginRequest) throws -> FfiTokenResponse {
        loginRequests.append((serverUrl, request))
        return FfiTokenResponse(jwt: "login-access", refreshToken: "login-refresh", userId: "account-1")
    }

    func refresh(serverUrl: String, request: FfiRefreshTokenRequest) throws -> FfiTokenResponse {
        refreshRequests.append((serverUrl, request))
        if let refreshError { throw refreshError }
        return refreshResult
    }

    func deleteSession(serverUrl: String, request: FfiDeleteSessionRequest) throws {
        deleteRequests.append((serverUrl, request))
    }

    func putCurrentDeviceKey(serverUrl: String, accessToken: String, request: FfiPutCurrentDeviceKeyRequest) throws {
        putDeviceKeyRequests.append((serverUrl, accessToken, request))
    }
}

@Test func syncCoordinatorConfiguresAuthThroughSharedCoreHelpers() async throws {
    let store = CoordinatorInMemorySecretStore()
    let bootstrap = generateLocalAccountBootstrap()
    store.values[SecureSecretID.devicePrivateKey] = Data(bootstrap.devicePrivateKey)
    store.values[SecureSecretID.accountDataKey] = Data(bootstrap.accountDataKey)
    let platform = CorePlatformAdapter(secretStore: store)
    let authClient = RecordingAuthClient()
    let coordinator = SyncCoordinator(serverURL: "https://example.com/", platform: platform, authClient: authClient)

    let result = try await coordinator.configureSyncAuth(email: " user@example.com ", password: "secret")

    #expect(result.serverUrl == "https://example.com")
    #expect(result.syncOrigin == "https://example.com:443")
    #expect(result.accountId == "account-1")
    #expect(result.state == .syncReady)
    #expect(authClient.registerRequests.count == 1)
    #expect(authClient.registerRequests[0].0 == "https://example.com")
    #expect(authClient.registerRequests[0].1.email == "user@example.com")
    #expect(authClient.registerRequests[0].1.pubKey == Data(bootstrap.devicePublicKey).base64EncodedString())
    #expect(store.values[SecureSecretID.accessToken] == Data("registered-access".utf8))
    #expect(store.values[SecureSecretID.refreshToken] == Data("registered-refresh".utf8))
    #expect(store.values[SecureSecretID.accountID] == Data("account-1".utf8))
    #expect(store.values[SecureSecretID.syncOriginID] == Data("https://example.com:443".utf8))
    #expect(try await coordinator.loadAccessToken() == "registered-access")
    #expect(await coordinator.syncAuthState() == .syncReady)
}

@Test func syncCoordinatorRefreshAndLogoutUseCoreTokenIDs() async throws {
    let store = CoordinatorInMemorySecretStore()
    store.values[SecureSecretID.refreshToken] = Data("old-refresh".utf8)
    store.values[SecureSecretID.accessToken] = Data("old-access".utf8)
    let platform = CorePlatformAdapter(secretStore: store)
    let authClient = RecordingAuthClient()
    let coordinator = SyncCoordinator(serverURL: "https://example.com", platform: platform, authClient: authClient)

    let refreshed = try await coordinator.refreshAuth()
    #expect(refreshed.jwt == "rotated-access")
    #expect(authClient.refreshRequests.count == 1)
    #expect(authClient.refreshRequests[0].1.refreshToken == "old-refresh")
    #expect(store.values[SecureSecretID.accessToken] == Data("rotated-access".utf8))
    #expect(store.values[SecureSecretID.refreshToken] == Data("rotated-refresh".utf8))

    try await coordinator.logout()
    #expect(authClient.deleteRequests.count == 1)
    #expect(authClient.deleteRequests[0].1.refreshToken == "rotated-refresh")
    #expect(store.values[SecureSecretID.accessToken] == nil)
    #expect(store.values[SecureSecretID.refreshToken] == nil)
}

@Test func syncCoordinatorEnrollmentScaffoldCallsSharedCoreHelpers() async throws {
    let store = CoordinatorInMemorySecretStore()
    let bootstrap = generateLocalAccountBootstrap()
    store.values[SecureSecretID.devicePrivateKey] = Data(bootstrap.devicePrivateKey)
    store.values.removeValue(forKey: SecureSecretID.accountDataKey)
    let platform = CorePlatformAdapter(secretStore: store)
    let coordinator = SyncCoordinator(serverURL: "https://example.com", platform: platform, authClient: RecordingAuthClient())

    #expect(await coordinator.enrollmentState() == .existingAccountPending)
    #expect(try await coordinator.beginExistingAccountEnrollment() == .existingAccountPending)
    #expect(try await coordinator.devicePublicKeyBase64() == Data(bootstrap.devicePublicKey).base64EncodedString())
}

@Test func syncCoordinatorForegroundSyncClearsDirtyWork() async throws {
    let databaseURL = FileManager.default.temporaryDirectory.appending(path: "tsk-sync-coordinator-\(UUID().uuidString).sqlite3")
    defer { try? FileManager.default.removeItem(at: databaseURL) }
    let core = try FfiTaskManagerCore(databasePath: databaseURL.path)
    _ = try core.createTask(title: "Sync me", body: "", dueAt: nil)

    let store = CoordinatorInMemorySecretStore()
    let bootstrap = generateLocalAccountBootstrap()
    store.values[SecureSecretID.devicePrivateKey] = Data(bootstrap.devicePrivateKey)
    store.values[SecureSecretID.accountDataKey] = Data(bootstrap.accountDataKey)
    store.values[SecureSecretID.accessToken] = Data("access".utf8)
    store.values[SecureSecretID.refreshToken] = Data("refresh".utf8)
    store.values[SecureSecretID.syncOriginID] = Data("https://example.com:443".utf8)
    let platform = CorePlatformAdapter(secretStore: store)
    let syncClient = RecordingSyncClient()
    let coordinator = SyncCoordinator(serverURL: "https://example.com", platform: platform, authClient: RecordingAuthClient()) { _, token in
        #expect(token == "access")
        return syncClient
    }

    let before = try core.syncStatus()
    #expect(before.dirtyCount == 1)
    let result = try await coordinator.foregroundSync(core: core, isOnline: true)
    let summary = try await coordinator.syncSummary(core: core, isOnline: true)

    #expect(result.pushed == 1)
    #expect(syncClient.pushCalls == 1)
    #expect(summary.dirtyCount == 0)
    #expect(summary.retryQueueDepth == 0)
}

@Test func syncCoordinatorRefreshesExpiredAccessTokenAndRetriesExactlyOnce() async throws {
    let databaseURL = FileManager.default.temporaryDirectory.appending(path: "tsk-sync-refresh-\(UUID().uuidString).sqlite3")
    defer { try? FileManager.default.removeItem(at: databaseURL) }
    let core = try FfiTaskManagerCore(databasePath: databaseURL.path)
    _ = try core.createTask(title: "Retry me", body: "", dueAt: nil)

    let store = CoordinatorInMemorySecretStore()
    let bootstrap = generateLocalAccountBootstrap()
    store.values[SecureSecretID.devicePrivateKey] = Data(bootstrap.devicePrivateKey)
    store.values[SecureSecretID.accountDataKey] = Data(bootstrap.accountDataKey)
    store.values[SecureSecretID.accessToken] = Data("expired-access".utf8)
    store.values[SecureSecretID.refreshToken] = Data("old-refresh".utf8)
    store.values[SecureSecretID.syncOriginID] = Data("https://example.com:443".utf8)
    let platform = CorePlatformAdapter(secretStore: store)
    let authClient = RecordingAuthClient()
    let syncClient = RecordingSyncClient()
    syncClient.failFirstPushWithAuthExpired = true
    var tokens: [String] = []
    let coordinator = SyncCoordinator(serverURL: "https://example.com", platform: platform, authClient: authClient) { _, token in
        tokens.append(token)
        return syncClient
    }

    let result = try await coordinator.foregroundSync(core: core, isOnline: true)

    #expect(result.pushed == 1)
    #expect(syncClient.pushCalls == 2)
    #expect(authClient.refreshRequests.count == 1)
    #expect(authClient.refreshRequests[0].1.refreshToken == "old-refresh")
    #expect(tokens == ["expired-access", "rotated-access"])
    #expect(store.values[SecureSecretID.accessToken] == Data("rotated-access".utf8))
    #expect(try core.syncStatus().dirtyCount == 0)
}

@Test func syncCoordinatorRefreshFailureLeavesDirtyWork() async throws {
    let databaseURL = FileManager.default.temporaryDirectory.appending(path: "tsk-sync-refresh-fails-\(UUID().uuidString).sqlite3")
    defer { try? FileManager.default.removeItem(at: databaseURL) }
    let core = try FfiTaskManagerCore(databasePath: databaseURL.path)
    _ = try core.createTask(title: "Keep dirty", body: "", dueAt: nil)

    let store = CoordinatorInMemorySecretStore()
    let bootstrap = generateLocalAccountBootstrap()
    store.values[SecureSecretID.devicePrivateKey] = Data(bootstrap.devicePrivateKey)
    store.values[SecureSecretID.accountDataKey] = Data(bootstrap.accountDataKey)
    store.values[SecureSecretID.accessToken] = Data("expired-access".utf8)
    store.values[SecureSecretID.refreshToken] = Data("old-refresh".utf8)
    store.values[SecureSecretID.syncOriginID] = Data("https://example.com:443".utf8)
    let authClient = RecordingAuthClient()
    authClient.refreshError = FfiCoreError.SyncError(errorMessage: "auth expired")
    let syncClient = RecordingSyncClient()
    syncClient.failFirstPushWithAuthExpired = true
    let coordinator = SyncCoordinator(serverURL: "https://example.com", platform: CorePlatformAdapter(secretStore: store), authClient: authClient) { _, _ in syncClient }

    await #expect(throws: FfiCoreError.self) {
        _ = try await coordinator.foregroundSync(core: core, isOnline: true)
    }

    #expect(syncClient.pushCalls == 1)
    #expect(authClient.refreshRequests.count == 1)
    #expect(try core.syncStatus().dirtyCount == 1)
}

@Test func syncCoordinatorOfflineQueuesRetryWithoutClearingDirtyWork() async throws {
    let databaseURL = FileManager.default.temporaryDirectory.appending(path: "tsk-sync-offline-\(UUID().uuidString).sqlite3")
    defer { try? FileManager.default.removeItem(at: databaseURL) }
    let core = try FfiTaskManagerCore(databasePath: databaseURL.path)
    _ = try core.createTask(title: "Offline", body: "", dueAt: nil)

    let store = CoordinatorInMemorySecretStore()
    let bootstrap = generateLocalAccountBootstrap()
    store.values[SecureSecretID.accountDataKey] = Data(bootstrap.accountDataKey)
    store.values[SecureSecretID.accessToken] = Data("access".utf8)
    let coordinator = SyncCoordinator(serverURL: "https://example.com", platform: CorePlatformAdapter(secretStore: store, networkAvailable: { false }), authClient: RecordingAuthClient()) { _, _ in RecordingSyncClient() }

    await #expect(throws: FfiCoreError.self) {
        _ = try await coordinator.foregroundSync(core: core, isOnline: false)
    }

    let status = try core.syncStatus()
    #expect(status.dirtyCount == 1)
    #expect(status.retryQueueDepth == 1)
}
