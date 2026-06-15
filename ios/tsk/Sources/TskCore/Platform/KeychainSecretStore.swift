import Foundation
import Security

public struct KeychainSecretStore: SecureSecretStoring {
    public let service: String
    private let accessible: String

    public init(service: String = Bundle.main.bundleIdentifier ?? "com.matthewyjiang.tsk", accessible: String = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly as String) {
        self.service = service
        self.accessible = accessible
    }

    public func data(for id: String) throws -> Data? {
        var query = baseQuery(for: id)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        switch status {
        case errSecSuccess:
            guard let data = item as? Data else { throw KeychainSecretStoreError.unexpectedItemType }
            return data
        case errSecItemNotFound:
            return nil
        default:
            throw KeychainSecretStoreError.unhandledStatus(status)
        }
    }

    public func setData(_ data: Data, for id: String) throws {
        let query = baseQuery(for: id)
        let attributes: [String: Any] = [
            kSecValueData as String: data,
            kSecAttrAccessible as String: accessible
        ]

        let updateStatus = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
        switch updateStatus {
        case errSecSuccess:
            return
        case errSecItemNotFound:
            var addQuery = query
            addQuery.merge(attributes) { _, new in new }
            let addStatus = SecItemAdd(addQuery as CFDictionary, nil)
            guard addStatus == errSecSuccess else { throw KeychainSecretStoreError.unhandledStatus(addStatus) }
        default:
            throw KeychainSecretStoreError.unhandledStatus(updateStatus)
        }
    }

    public func removeData(for id: String) throws {
        let status = SecItemDelete(baseQuery(for: id) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw KeychainSecretStoreError.unhandledStatus(status)
        }
    }

    private func baseQuery(for id: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: id
        ]
    }
}

public enum KeychainSecretStoreError: Error, Equatable, Sendable {
    case unexpectedItemType
    case unhandledStatus(OSStatus)
}
