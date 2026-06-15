import Foundation

public struct LocalAccountBootstrapState: Equatable, Sendable {
    public var devicePublicKey: [UInt8]
    public var createdDeviceKey: Bool
    public var createdAccountDataKey: Bool

    public init(devicePublicKey: [UInt8], createdDeviceKey: Bool, createdAccountDataKey: Bool) {
        self.devicePublicKey = devicePublicKey
        self.createdDeviceKey = createdDeviceKey
        self.createdAccountDataKey = createdAccountDataKey
    }

    public var createdAnySecret: Bool {
        createdDeviceKey || createdAccountDataKey
    }

    public var publicKeyFingerprint: String {
        Self.fingerprint(for: devicePublicKey)
    }

    private static func fingerprint(for bytes: [UInt8]) -> String {
        bytes.prefix(6).map { String(format: "%02x", $0) }.joined(separator: ":")
    }
}

public struct LocalFirstBootstrapService: Sendable {
    private let secretStore: any SecureSecretStoring

    public init(secretStore: any SecureSecretStoring = KeychainSecretStore()) {
        self.secretStore = secretStore
    }

    @discardableResult
    public func ensureBootstrapped() throws -> LocalAccountBootstrapState {
        let devicePrivateKey = try secretStore.data(for: SecureSecretID.devicePrivateKey).map(Array.init)
        let accountDataKey = try secretStore.data(for: SecureSecretID.accountDataKey).map(Array.init)

        switch (devicePrivateKey, accountDataKey) {
        case let (.some(privateKey), .some(_)):
            let publicKey = try devicePublicKeyFromPrivateKey(privateKey: privateKey)
            return LocalAccountBootstrapState(devicePublicKey: publicKey, createdDeviceKey: false, createdAccountDataKey: false)

        case let (.some(privateKey), .none):
            let dataKey = generateAccountDataKey()
            try secretStore.setData(Data(dataKey), for: SecureSecretID.accountDataKey)
            let publicKey = try devicePublicKeyFromPrivateKey(privateKey: privateKey)
            return LocalAccountBootstrapState(devicePublicKey: publicKey, createdDeviceKey: false, createdAccountDataKey: true)

        case let (.none, .some(dataKey)):
            let keypair = generateFfiDeviceKeypair()
            try secretStore.setData(Data(keypair.privateKey), for: SecureSecretID.devicePrivateKey)
            try secretStore.setData(Data(dataKey), for: SecureSecretID.accountDataKey)
            return LocalAccountBootstrapState(devicePublicKey: keypair.publicKey, createdDeviceKey: true, createdAccountDataKey: false)

        case (.none, .none):
            let bootstrap = generateLocalAccountBootstrap()
            try secretStore.setData(Data(bootstrap.devicePrivateKey), for: SecureSecretID.devicePrivateKey)
            try secretStore.setData(Data(bootstrap.accountDataKey), for: SecureSecretID.accountDataKey)
            return LocalAccountBootstrapState(
                devicePublicKey: bootstrap.devicePublicKey,
                createdDeviceKey: true,
                createdAccountDataKey: true
            )
        }
    }
}
