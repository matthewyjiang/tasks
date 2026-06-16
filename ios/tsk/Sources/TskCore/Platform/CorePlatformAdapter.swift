import Foundation

public final class CorePlatformAdapter: FfiPlatform, @unchecked Sendable {
    private let secretStore: any SecureSecretStoring
    private let isNetworkAvailable: @Sendable () -> Bool

    public init(
        secretStore: any SecureSecretStoring,
        networkAvailable: @escaping @Sendable () -> Bool = { true }
    ) {
        self.secretStore = secretStore
        self.isNetworkAvailable = networkAvailable
    }

    public func storeKey(id: String, bytes: [UInt8]) throws {
        do {
            try secretStore.setData(Data(bytes), for: id)
        } catch {
            throw Self.platformError("failed to store key \(id): \(error)")
        }
    }

    public func loadKey(id: String) throws -> [UInt8] {
        do {
            guard let data = try secretStore.data(for: id) else {
                throw Self.platformError("missing key \(id)")
            }
            return Array(data)
        } catch let error as FfiCoreError {
            throw error
        } catch {
            throw Self.platformError("failed to load key \(id): \(error)")
        }
    }

    public func deleteKey(id: String) throws {
        do {
            try secretStore.removeData(for: id)
        } catch {
            throw Self.platformError("failed to delete key \(id): \(error)")
        }
    }

    public func networkAvailable() -> Bool {
        isNetworkAvailable()
    }

    private static func platformError(_ message: String) -> FfiCoreError {
        .PlatformError(errorMessage: message)
    }
}
