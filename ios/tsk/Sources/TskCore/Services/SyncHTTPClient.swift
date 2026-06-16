import Foundation

public struct SyncHTTPRequest: Equatable, Sendable {
    public var method: String
    public var url: URL
    public var headers: [String: String]
    public var body: Data?

    public init(method: String, url: URL, headers: [String: String] = [:], body: Data? = nil) {
        self.method = method
        self.url = url
        self.headers = headers
        self.body = body
    }
}

public struct SyncHTTPResponse: Sendable {
    public var statusCode: Int
    public var body: Data

    public init(statusCode: Int, body: Data = Data()) {
        self.statusCode = statusCode
        self.body = body
    }
}

public protocol SyncHTTPTransport: Sendable {
    func send(_ request: SyncHTTPRequest) throws -> SyncHTTPResponse
}

public final class URLSessionSyncHTTPTransport: SyncHTTPTransport, @unchecked Sendable {
    public static let defaultRequestTimeout: TimeInterval = 30
    public static let defaultResourceTimeout: TimeInterval = 60

    private let session: URLSession
    private let requestTimeout: TimeInterval

    public convenience init(
        requestTimeout: TimeInterval = URLSessionSyncHTTPTransport.defaultRequestTimeout,
        resourceTimeout: TimeInterval = URLSessionSyncHTTPTransport.defaultResourceTimeout
    ) {
        let configuration = URLSessionConfiguration.default
        configuration.timeoutIntervalForRequest = requestTimeout
        configuration.timeoutIntervalForResource = resourceTimeout
        self.init(session: URLSession(configuration: configuration), requestTimeout: requestTimeout)
    }

    public init(session: URLSession, requestTimeout: TimeInterval = URLSessionSyncHTTPTransport.defaultRequestTimeout) {
        self.session = session
        self.requestTimeout = requestTimeout
    }

    public func send(_ request: SyncHTTPRequest) throws -> SyncHTTPResponse {
        var urlRequest = URLRequest(url: request.url, timeoutInterval: requestTimeout)
        urlRequest.httpMethod = request.method
        urlRequest.httpBody = request.body
        for (name, value) in request.headers { urlRequest.setValue(value, forHTTPHeaderField: name) }

        let semaphore = DispatchSemaphore(value: 0)
        var result: Result<SyncHTTPResponse, Error>!
        session.dataTask(with: urlRequest) { data, response, error in
            defer { semaphore.signal() }
            if let error {
                result = .failure(error)
                return
            }
            guard let http = response as? HTTPURLResponse else {
                result = .failure(SyncHTTPClientError.invalidResponse)
                return
            }
            result = .success(SyncHTTPResponse(statusCode: http.statusCode, body: data ?? Data()))
        }.resume()
        semaphore.wait()
        return try result.get()
    }
}

public enum SyncHTTPClientError: Error, Equatable, Sendable {
    case invalidBaseURL(String)
    case invalidResponse
}

public final class SyncHTTPAuthClient: FfiAuthClient, @unchecked Sendable {
    private let transport: any SyncHTTPTransport

    public init(transport: any SyncHTTPTransport = URLSessionSyncHTTPTransport()) {
        self.transport = transport
    }

    public func registerAccount(serverUrl: String, request: FfiRegisterRequest) throws -> FfiTokenResponse {
        let body = AuthRegisterRequest(email: request.email, password: request.password, pub_key: request.pubKey)
        return try sendJSON(serverUrl: serverUrl, method: "POST", path: "/auth/register", body: body, token: nil, response: AuthTokenResponse.self).ffi
    }

    public func login(serverUrl: String, request: FfiLoginRequest) throws -> FfiTokenResponse {
        let body = AuthLoginRequest(email: request.email, password: request.password)
        return try sendJSON(serverUrl: serverUrl, method: "POST", path: "/auth/login", body: body, token: nil, response: AuthTokenResponse.self).ffi
    }

    public func refresh(serverUrl: String, request: FfiRefreshTokenRequest) throws -> FfiTokenResponse {
        let body = AuthRefreshRequest(refresh_token: request.refreshToken)
        return try sendJSON(serverUrl: serverUrl, method: "POST", path: "/auth/refresh", body: body, token: nil, response: AuthTokenResponse.self).ffi
    }

    public func deleteSession(serverUrl: String, request: FfiDeleteSessionRequest) throws {
        let body = AuthRefreshRequest(refresh_token: request.refreshToken)
        _ = try sendJSON(serverUrl: serverUrl, method: "DELETE", path: "/auth/session", body: body, token: nil, response: EmptyResponse.self)
    }

    public func putCurrentDeviceKey(serverUrl: String, accessToken: String, request: FfiPutCurrentDeviceKeyRequest) throws {
        let body = PutDeviceKeyRequest(pub_key: request.pubKey)
        _ = try sendJSON(serverUrl: serverUrl, method: "PUT", path: "/keys/me", body: body, token: accessToken, response: EmptyResponse.self)
    }

    private func sendJSON<Body: Encodable, Response: Decodable>(serverUrl: String, method: String, path: String, body: Body, token: String?, response: Response.Type) throws -> Response {
        var headers = ["Content-Type": "application/json", "Accept": "application/json"]
        if let token { headers["Authorization"] = "Bearer \(token)" }
        let request = try SyncHTTPRequest(method: method, url: endpoint(serverUrl, path), headers: headers, body: JSONEncoder().encode(body))
        do {
            return try decode(try transport.send(request), as: response)
        } catch let error as FfiCoreError {
            throw error
        } catch {
            throw syncTransportError(error)
        }
    }

    private func endpoint(_ serverUrl: String, _ path: String) throws -> URL {
        guard var components = URLComponents(string: serverUrl.trimmingCharacters(in: .whitespacesAndNewlines).trimmingCharacters(in: CharacterSet(charactersIn: "/"))) else { throw FfiCoreError.PlatformError(errorMessage: "invalid server URL: \(serverUrl)") }
        components.path += path
        guard let url = components.url else { throw FfiCoreError.PlatformError(errorMessage: "invalid server URL: \(serverUrl)") }
        return url
    }

    private func decode<Response: Decodable>(_ response: SyncHTTPResponse, as type: Response.Type) throws -> Response {
        if response.statusCode == 401 { throw FfiCoreError.SyncError(errorMessage: "auth expired") }
        guard (200..<300).contains(response.statusCode) else { throw FfiCoreError.SyncError(errorMessage: "server error \(response.statusCode)") }
        if type == EmptyResponse.self { return EmptyResponse() as! Response }
        do { return try JSONDecoder().decode(type, from: response.body) } catch { throw FfiCoreError.SyncError(errorMessage: error.localizedDescription) }
    }
}

public final class SyncHTTPBlobClient: FfiSyncClient, @unchecked Sendable {
    private let serverURL: String
    private let accessToken: String
    private let transport: any SyncHTTPTransport

    public init(serverURL: String, accessToken: String, transport: any SyncHTTPTransport = URLSessionSyncHTTPTransport()) {
        self.serverURL = serverURL
        self.accessToken = accessToken
        self.transport = transport
    }

    public func pushBlobs(blobs: [FfiBlobPush]) throws -> FfiPushResponse {
        let body = BatchRequest(blobs: blobs.map { BlobRequest(task_id: $0.taskId, ciphertext: Data($0.blob.ciphertext).base64EncodedString(), nonce: Data($0.blob.nonce).base64EncodedString()) })
        let response: BatchResponse = try send(method: "POST", path: "/blobs/batch", body: body, response: BatchResponse.self)
        let accepted = response.results.filter { $0.status == "ok" }.map(\.task_id)
        let failed = response.results.filter { $0.status != "ok" }.map(\.task_id)
        return FfiPushResponse(acceptedTaskIds: accepted, failedTaskIds: failed)
    }

    public func deleteBlobs(taskIds: [String]) throws -> FfiPushResponse {
        var accepted: [String] = []
        for taskId in taskIds {
            _ = try send(method: "DELETE", path: "/blobs/\(taskId)", body: Optional<EmptyRequest>.none, response: EmptyResponse.self)
            accepted.append(taskId)
        }
        return FfiPushResponse(acceptedTaskIds: accepted, failedTaskIds: [])
    }

    public func pullBlobs(since: Int64) throws -> FfiPullResponse {
        let response: PullWireResponse = try send(method: "GET", path: "/blobs", queryItems: [URLQueryItem(name: "since", value: String(since))], body: Optional<EmptyRequest>.none, response: PullWireResponse.self)
        let blobs = try response.blobs.compactMap { wire -> FfiRemoteBlob? in
            if wire.deleted { return nil }
            guard
                let ciphertext = wire.ciphertext,
                let nonce = wire.nonce,
                let ciphertextData = Data(base64Encoded: ciphertext),
                let nonceData = Data(base64Encoded: nonce)
            else {
                throw FfiCoreError.SyncError(errorMessage: "malformed blob payload")
            }
            return FfiRemoteBlob(taskId: wire.task_id, blob: FfiBlob(ciphertext: Array(ciphertextData), nonce: Array(nonceData)), updatedAt: wire.updated_at)
        }
        return FfiPullResponse(blobs: blobs, cursor: response.cursor)
    }

    private func send<Body: Encodable, Response: Decodable>(method: String, path: String, queryItems: [URLQueryItem] = [], body: Body?, response: Response.Type) throws -> Response {
        var headers = ["Accept": "application/json", "Authorization": "Bearer \(accessToken)"]
        var bodyData: Data?
        if let body {
            headers["Content-Type"] = "application/json"
            bodyData = try JSONEncoder().encode(body)
        }
        let request = try SyncHTTPRequest(method: method, url: endpoint(path, queryItems), headers: headers, body: bodyData)
        do {
            return try decode(try transport.send(request), as: response)
        } catch let error as FfiCoreError {
            throw error
        } catch {
            throw syncTransportError(error)
        }
    }

    private func endpoint(_ path: String, _ queryItems: [URLQueryItem]) throws -> URL {
        guard var components = URLComponents(string: serverURL.trimmingCharacters(in: .whitespacesAndNewlines).trimmingCharacters(in: CharacterSet(charactersIn: "/"))) else { throw FfiCoreError.PlatformError(errorMessage: "invalid server URL: \(serverURL)") }
        components.path += path
        components.queryItems = queryItems.isEmpty ? nil : queryItems
        guard let url = components.url else { throw FfiCoreError.PlatformError(errorMessage: "invalid server URL: \(serverURL)") }
        return url
    }

    private func decode<Response: Decodable>(_ response: SyncHTTPResponse, as type: Response.Type) throws -> Response {
        if response.statusCode == 401 { throw FfiCoreError.SyncError(errorMessage: "auth expired") }
        guard (200..<300).contains(response.statusCode) else { throw FfiCoreError.SyncError(errorMessage: "server error \(response.statusCode)") }
        if type == EmptyResponse.self { return EmptyResponse() as! Response }
        do { return try JSONDecoder().decode(type, from: response.body) } catch { throw FfiCoreError.SyncError(errorMessage: error.localizedDescription) }
    }
}

private func syncTransportError(_ error: Error) -> FfiCoreError {
    FfiCoreError.SyncError(errorMessage: "network unavailable")
}

private struct EmptyRequest: Encodable {}
private struct EmptyResponse: Codable {}
private struct AuthRegisterRequest: Encodable { var email: String; var password: String; var pub_key: String }
private struct AuthLoginRequest: Encodable { var email: String; var password: String }
private struct AuthRefreshRequest: Encodable { var refresh_token: String }
private struct PutDeviceKeyRequest: Encodable { var pub_key: String }
private struct AuthTokenResponse: Decodable { var jwt: String; var refresh_token: String; var user_id: String?; var ffi: FfiTokenResponse { FfiTokenResponse(jwt: jwt, refreshToken: refresh_token, userId: user_id) } }
private struct BatchRequest: Encodable { var blobs: [BlobRequest] }
private struct BlobRequest: Encodable { var task_id: String; var ciphertext: String; var nonce: String }
private struct BatchResponse: Decodable { var results: [BatchResult] }
private struct BatchResult: Decodable { var task_id: String; var status: String }
private struct PullWireResponse: Decodable { var blobs: [PullWireBlob]; var cursor: Int64 }
private struct PullWireBlob: Decodable { var task_id: String; var ciphertext: String?; var nonce: String?; var updated_at: Int64; var deleted: Bool }
