import Foundation

public actor SyncCoordinator {
    private let platform: CorePlatformAdapter
    private let authClient: any FfiAuthClient
    private let syncClientFactory: @Sendable (String, String) -> any FfiSyncClient
    private var serverURL: String
    private var lastSyncFailedCount: UInt64 = 0

    public init(
        serverURL: String,
        platform: CorePlatformAdapter,
        authClient: any FfiAuthClient,
        syncClientFactory: @escaping @Sendable (String, String) -> any FfiSyncClient = { serverURL, accessToken in
            SyncHTTPBlobClient(serverURL: serverURL, accessToken: accessToken)
        }
    ) {
        self.serverURL = serverURL
        self.platform = platform
        self.authClient = authClient
        self.syncClientFactory = syncClientFactory
    }

    public func updateServerURL(_ serverURL: String) {
        self.serverURL = serverURL
    }

    public func syncAuthState() -> FfiSyncAuthState {
        ffiSyncAuthState(platform: platform, serverUrl: serverURL)
    }

    public func enrollmentState() -> FfiEnrollmentState {
        ffiExistingAccountEnrollmentState(platform: platform)
    }

    public func devicePublicKeyBase64() throws -> String {
        try ffiDevicePublicKeyBase64FromPlatform(platform: platform)
    }

    public func configureSyncAuth(email: String, password: String) throws -> FfiConfigureSyncAuthResult {
        let publicKey = try devicePublicKeyBase64()
        return try ffiConfigureSyncAuth(
            platform: platform,
            authClient: authClient,
            serverUrl: serverURL,
            credentials: FfiAuthCredentials(email: email, password: password),
            registerPublicKeyBase64: publicKey
        )
    }

    @discardableResult
    public func refreshAuth() throws -> FfiTokenResponse {
        try ffiRefreshAuth(platform: platform, authClient: authClient, serverUrl: serverURL)
    }

    public func logout() throws {
        try ffiLogoutSyncAuth(platform: platform, authClient: authClient, serverUrl: serverURL)
    }

    public func beginExistingAccountEnrollment() throws -> FfiEnrollmentState {
        try ffiBeginExistingAccountEnrollment(platform: platform)
    }

    public func acceptWrappedAccountDataKeyPayload(_ payload: FfiWrappedAccountDataKeyPayload) throws -> FfiEnrollmentState {
        try acceptFfiWrappedAccountDataKeyPayload(platform: platform, payload: payload)
    }

    public func loadAccessToken() throws -> String {
        try ffiLoadAccessToken(platform: platform)
    }

    public func foregroundSync(core: FfiTaskManagerCore, isOnline: Bool) throws -> FfiSyncResult {
        guard isOnline, platform.networkAvailable() else {
            let dataKey = try platform.loadKey(id: SecureSecretID.accountDataKey)
            let token = (try? loadAccessToken()) ?? ""
            _ = try? core.syncRun(networkAvailable: false, client: syncClientFactory(serverURL, token), dataKey: dataKey)
            throw FfiCoreError.SyncError(errorMessage: "network unavailable")
        }
        guard syncAuthState() == .syncReady else {
            throw FfiCoreError.SyncError(errorMessage: "sync auth required")
        }
        let dataKey = try platform.loadKey(id: SecureSecretID.accountDataKey)
        do {
            let result = try runSync(core: core, dataKey: dataKey)
            lastSyncFailedCount = result.failed
            return result
        } catch {
            guard Self.isAuthExpired(error) else { throw error }
            do {
                _ = try refreshAuth()
                let result = try runSync(core: core, dataKey: dataKey)
                lastSyncFailedCount = result.failed
                return result
            } catch {
                lastSyncFailedCount = 0
                throw error
            }
        }
    }

    public func syncSummary(core: FfiTaskManagerCore, isOnline: Bool) throws -> SyncSummary {
        let status = try core.syncStatus()
        return SyncSummary(dirtyCount: status.dirtyCount, retryQueueDepth: status.retryQueueDepth, conflictCount: lastSyncFailedCount, cursor: status.cursor, isOnline: isOnline)
    }

    public func networkAvailable() -> Bool {
        platform.networkAvailable()
    }

    private func runSync(core: FfiTaskManagerCore, dataKey: [UInt8]) throws -> FfiSyncResult {
        let token = try loadAccessToken()
        return try core.syncRun(networkAvailable: platform.networkAvailable(), client: syncClientFactory(serverURL, token), dataKey: dataKey)
    }

    private static func isAuthExpired(_ error: Error) -> Bool {
        if case let FfiCoreError.SyncError(message) = error {
            return message.localizedCaseInsensitiveContains("auth expired")
        }
        if case let FfiCoreError.PlatformError(message) = error {
            return message.localizedCaseInsensitiveContains("auth expired")
        }
        return String(describing: error).localizedCaseInsensitiveContains("auth expired")
            || error.localizedDescription.localizedCaseInsensitiveContains("auth expired")
    }
}
