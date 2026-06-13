import Foundation

public protocol SecureSecretStoring: Sendable {
    func data(for id: String) throws -> Data?
    func setData(_ data: Data, for id: String) throws
    func removeData(for id: String) throws
}

public enum SecureSecretID: Sendable {
    public static let devicePrivateKey = devicePrivateKeyId()
    public static let accountDataKey = accountDataKeyId()
    public static let accessToken = "auth.access-token"
    public static let refreshToken = "auth.refresh-token"
    public static let accountID = "auth.account-id"
    public static let syncOriginID = "sync.origin-id"
}
