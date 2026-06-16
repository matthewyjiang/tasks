import Foundation
import Testing
@testable import TskCore

private final class RecordingTransport: SyncHTTPTransport, @unchecked Sendable {
    var requests: [SyncHTTPRequest] = []
    var responses: [SyncHTTPResponse]
    var error: Error?

    init(_ responses: [SyncHTTPResponse], error: Error? = nil) {
        self.responses = responses
        self.error = error
    }

    func send(_ request: SyncHTTPRequest) throws -> SyncHTTPResponse {
        requests.append(request)
        if let error { throw error }
        return responses.removeFirst()
    }
}

@Test func syncHTTPAuthClientUsesLinuxAuthWireProtocol() throws {
    let transport = RecordingTransport([
        SyncHTTPResponse(statusCode: 200, body: Data(#"{"jwt":"access","refresh_token":"refresh","user_id":"user-1"}"#.utf8)),
        SyncHTTPResponse(statusCode: 200, body: Data(#"{"jwt":"login-access","refresh_token":"login-refresh"}"#.utf8)),
        SyncHTTPResponse(statusCode: 200, body: Data(#"{"jwt":"rotated","refresh_token":"rotated-refresh","user_id":"user-1"}"#.utf8)),
        SyncHTTPResponse(statusCode: 204),
        SyncHTTPResponse(statusCode: 204)
    ])
    let client = SyncHTTPAuthClient(transport: transport)

    let registered = try client.registerAccount(serverUrl: "https://example.com/", request: FfiRegisterRequest(email: "a@b.com", password: "pw", pubKey: "pub"))
    let loggedIn = try client.login(serverUrl: "https://example.com", request: FfiLoginRequest(email: "a@b.com", password: "pw"))
    let refreshed = try client.refresh(serverUrl: "https://example.com", request: FfiRefreshTokenRequest(refreshToken: "old-refresh"))
    try client.deleteSession(serverUrl: "https://example.com", request: FfiDeleteSessionRequest(refreshToken: "rotated-refresh"))
    try client.putCurrentDeviceKey(serverUrl: "https://example.com", accessToken: "access", request: FfiPutCurrentDeviceKeyRequest(pubKey: "pub"))

    #expect(registered.jwt == "access")
    #expect(loggedIn.refreshToken == "login-refresh")
    #expect(refreshed.jwt == "rotated")
    #expect(transport.requests.map { $0.method } == ["POST", "POST", "POST", "DELETE", "PUT"])
    #expect(transport.requests.map { $0.url.path } == ["/auth/register", "/auth/login", "/auth/refresh", "/auth/session", "/keys/me"])
    #expect(transport.requests[4].headers["Authorization"] == "Bearer access")
    let registerJSON = try JSONSerialization.jsonObject(with: transport.requests[0].body ?? Data()) as? [String: String]
    #expect(registerJSON?["pub_key"] == "pub")
    let refreshJSON = try JSONSerialization.jsonObject(with: transport.requests[2].body ?? Data()) as? [String: String]
    #expect(refreshJSON?["refresh_token"] == "old-refresh")
}

@Test func syncHTTPBlobClientUsesLinuxBlobWireProtocol() throws {
    let taskID = "00000000-0000-0000-0000-000000000001"
    let transport = RecordingTransport([
        SyncHTTPResponse(statusCode: 200, body: Data(#"{"results":[{"task_id":"00000000-0000-0000-0000-000000000001","status":"ok"},{"task_id":"00000000-0000-0000-0000-000000000002","status":"failed"}]}"#.utf8)),
        SyncHTTPResponse(statusCode: 204),
        SyncHTTPResponse(statusCode: 200, body: Data(#"{"blobs":[{"task_id":"00000000-0000-0000-0000-000000000001","ciphertext":"AQID","nonce":"BAUG","updated_at":42,"deleted":false},{"task_id":"00000000-0000-0000-0000-000000000003","updated_at":43,"deleted":true}],"cursor":99}"#.utf8))
    ])
    let client = SyncHTTPBlobClient(serverURL: "https://example.com/", accessToken: "access", transport: transport)

    let push = try client.pushBlobs(blobs: [FfiBlobPush(taskId: taskID, blob: FfiBlob(ciphertext: [1, 2, 3], nonce: [4, 5, 6]))])
    let delete = try client.deleteBlobs(taskIds: [taskID])
    let pull = try client.pullBlobs(since: 7)

    #expect(push.acceptedTaskIds == [taskID])
    #expect(push.failedTaskIds == ["00000000-0000-0000-0000-000000000002"])
    #expect(delete.acceptedTaskIds == [taskID])
    #expect(pull.cursor == 99)
    #expect(pull.blobs.count == 1)
    #expect(pull.blobs[0].blob.ciphertext == [1, 2, 3])
    #expect(pull.blobs[0].blob.nonce == [4, 5, 6])
    #expect(transport.requests.map { $0.url.path } == ["/blobs/batch", "/blobs/\(taskID)", "/blobs"])
    #expect(transport.requests[2].url.query == "since=7")
    #expect(transport.requests.allSatisfy { $0.headers["Authorization"] == "Bearer access" })
    let pushJSON = try JSONSerialization.jsonObject(with: transport.requests[0].body ?? Data()) as? [String: Any]
    let blobs = pushJSON?["blobs"] as? [[String: String]]
    #expect(blobs?.first?["ciphertext"] == "AQID")
    #expect(blobs?.first?["nonce"] == "BAUG")
}

@Test func syncHTTPClientsMapUnauthorizedAndMalformedJSONToCoreSyncErrors() throws {
    let unauthorized = RecordingTransport([SyncHTTPResponse(statusCode: 401)])
    #expect(throws: FfiCoreError.self) {
        _ = try SyncHTTPBlobClient(serverURL: "https://example.com", accessToken: "expired", transport: unauthorized).pullBlobs(since: 0)
    }

    let malformed = RecordingTransport([SyncHTTPResponse(statusCode: 200, body: Data("not-json".utf8))])
    #expect(throws: FfiCoreError.self) {
        _ = try SyncHTTPAuthClient(transport: malformed).refresh(serverUrl: "https://example.com", request: FfiRefreshTokenRequest(refreshToken: "refresh"))
    }
}

@Test func syncHTTPBlobClientRejectsMalformedNonDeletedPullBlob() throws {
    let missingCiphertext = RecordingTransport([
        SyncHTTPResponse(statusCode: 200, body: Data(#"{"blobs":[{"task_id":"00000000-0000-0000-0000-000000000001","nonce":"BAUG","updated_at":42,"deleted":false}],"cursor":99}"#.utf8))
    ])
    do {
        _ = try SyncHTTPBlobClient(serverURL: "https://example.com", accessToken: "access", transport: missingCiphertext).pullBlobs(since: 0)
        Issue.record("Expected malformed non-deleted pull blob to throw")
    } catch FfiCoreError.SyncError(let message) {
        #expect(message == "malformed blob payload")
    }

    let invalidBase64 = RecordingTransport([
        SyncHTTPResponse(statusCode: 200, body: Data(#"{"blobs":[{"task_id":"00000000-0000-0000-0000-000000000001","ciphertext":"not base64","nonce":"BAUG","updated_at":42,"deleted":false}],"cursor":100}"#.utf8))
    ])
    do {
        _ = try SyncHTTPBlobClient(serverURL: "https://example.com", accessToken: "access", transport: invalidBase64).pullBlobs(since: 0)
        Issue.record("Expected invalid base64 pull blob to throw")
    } catch FfiCoreError.SyncError(let message) {
        #expect(message == "malformed blob payload")
    }
}

@Test func syncHTTPClientsMapTransportFailuresToNetworkUnavailableSyncErrors() throws {
    let authTransport = RecordingTransport([], error: URLError(.timedOut))
    do {
        _ = try SyncHTTPAuthClient(transport: authTransport).refresh(serverUrl: "https://example.com", request: FfiRefreshTokenRequest(refreshToken: "refresh"))
        Issue.record("Expected auth transport failure to map to sync error")
    } catch FfiCoreError.SyncError(let message) {
        #expect(message == "network unavailable")
    }

    let blobTransport = RecordingTransport([], error: URLError(.cannotConnectToHost))
    do {
        _ = try SyncHTTPBlobClient(serverURL: "https://example.com", accessToken: "access", transport: blobTransport).pullBlobs(since: 0)
        Issue.record("Expected blob transport failure to map to sync error")
    } catch FfiCoreError.SyncError(let message) {
        #expect(message == "network unavailable")
    }
}
