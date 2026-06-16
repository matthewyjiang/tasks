# Architecture and sync model

`tsk` is organized around a shared Rust core with thin platform clients.

## Local-first clients

Client apps use `taskmanager-core` for task CRUD, local SQLite persistence, encryption, sync queue management, conflict handling, and shared platform-independent behavior. Platform layers own command parsing or UI, key-store adapters, HTTP transport, notifications, reachability, and OS integration.

Task commands operate against a local database first. Sync is an explicit or scheduled pull/push of encrypted task blobs between the local database and the server.

## Zero-knowledge server

The Go server provides:

- account registration and login;
- short-lived access tokens and longer-lived refresh tokens;
- device public-key directory APIs;
- encrypted blob upload, download, and cursor-based listing;
- health checks and deployment tooling.

The server should not receive plaintext task titles, notes, tags, due dates, or list metadata. Normal users should use a client rather than hand-crafting encrypted blob requests.

## Sync flow

A typical user flow is:

1. Configure a client with a server URL, email, and password.
2. Initialize or load local account/device keys.
3. Create and edit tasks offline against local SQLite.
4. Push dirty local rows as encrypted blobs.
5. Pull remote blobs newer than the local cursor.
6. Decrypt and apply remote changes locally.

Linux and iOS clients can refresh expired access tokens during sync when a valid refresh token is available. The CLI currently documents auth-token refresh as future work.

## Conflict behavior

Core resolves conflicts with a deterministic last-write-wins policy using `updated_at`, then a stable id tie-breaker. Linux and iOS surface sync status including failed counts, retry queue depth, dirty counts, cursor information, and automatically resolved conflicts where implemented.
