import Foundation

public protocol SecureSecretStoring: Sendable {
    func data(for id: String) throws -> Data?
    func setData(_ data: Data, for id: String) throws
    func removeData(for id: String) throws
}

public enum SecureSecretID: Sendable {
    public static let devicePrivateKey = devicePrivateKeyId()
    public static let accountDataKey = accountDataKeyId()
    public static let accessToken = authAccessTokenId()
    public static let refreshToken = authRefreshTokenId()
    public static let accountID = authAccountIdId()
    public static let syncOriginID = authSyncOriginId()
}
