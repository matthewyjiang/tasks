# Server implementation plan

## Phase 0 — Scope and decisions

1. ☑ Define server implementation scope for v0.1

   The v0.1 server is a Go-based zero-knowledge sync backend for the task manager. It is responsible for authentication, device/key-directory metadata, opaque encrypted blob storage, sharing metadata, plaintext settings sync, and operational safeguards. It never decrypts or interprets task/vault blob contents.

   ### In scope

   - Go HTTP service under `server/`
   - `net/http` server with `chi` routing
   - PostgreSQL persistence via `pgxpool`
   - SQL migrations under `server/migrations/`
   - Argon2id password hashing for server-side auth
   - JWT access tokens with 15-minute default TTL
   - Rotated refresh tokens with 30-day default TTL
   - Auth endpoints:
     - `POST /auth/register`
     - `POST /auth/login`
     - `POST /auth/refresh`
     - `DELETE /auth/session`
   - Blob sync endpoints:
     - `GET /blobs?since=`
     - `PUT /blobs/:task_id`
     - `DELETE /blobs/:task_id`
     - `POST /blobs/batch`
   - Key-directory/device endpoints:
     - `GET /keys/:user_id`
     - `PUT /keys/me`
   - Shared-task endpoints:
     - `POST /share/:task_id`
     - `GET /share/inbox`
     - `DELETE /share/:task_id/:recipient_id`
   - Plaintext settings endpoints:
     - `GET /settings/plaintext`
     - `PUT /settings/plaintext`
   - Per-user ownership checks on all protected resources
   - Base64 JSON representation for binary wire fields
   - Write rate limiting: 60 writes/min/user by default
   - Tombstone cleanup job for deleted blobs older than 30 days
   - Tests for auth, repositories, handlers, and JSON contracts
   - Local development tooling and CI

   ### Out of scope

   - Server-side encryption/decryption of task blobs
   - Server-side task search, filtering, reminders, or conflict resolution
   - Server-side inspection of ciphertext, wrapped keys, or task content
   - Key transparency / Signal-style identity verification
   - Account-wide data-key rotation
   - Attachment storage
   - Multi-node/distributed rate limiting
   - Production deployment manifests

   ### v0.1 implementation decisions

   - `task_id` is stored as `TEXT`, not `UUID`, so the reserved `vault_settings` blob ID is valid.
   - Binary fields in JSON are base64 strings: `pub_key`, `ciphertext`, `nonce`, and `wrapped_dek`.
   - The server adds a `devices` table because the spec requires one ECDH keypair per device.
   - Auth endpoints accept `password` for v0.1; the server stores only an Argon2id hash.
   - `last_sync_cursor` is device-local and must not be persisted by `PUT /settings/plaintext`.
   - Max blob size is `1 MiB` by default.
   - Max batch size is `100` blobs.

2. ☑ Create server project skeleton under `server/`

   Created the initial implementation layout:

   ```text
   server/
   ├── cmd/server/          # executable entrypoint
   ├── internal/auth/       # auth service, password hashing, JWT/refresh logic
   ├── internal/blobs/      # encrypted blob repository and handlers
   ├── internal/config/     # env configuration loading
   ├── internal/db/         # PostgreSQL connection and migration helpers
   ├── internal/http/       # router/server assembly
   ├── internal/keys/       # device/key-directory handlers
   ├── internal/middleware/ # auth, logging, rate-limit middleware
   ├── internal/respond/    # JSON response/error helpers
   ├── internal/settings/   # plaintext settings handlers
   ├── internal/share/      # shared-task handlers
   ├── migrations/          # numbered SQL migrations
   └── test/                # integration/contract test helpers
   ```
3. ☑ Choose core Go dependencies

   Added `server/go.mod` with the v0.1 dependency set:

   - `github.com/go-chi/chi/v5` for routing
   - `github.com/jackc/pgx/v5` / `pgxpool` for PostgreSQL
   - `github.com/golang-jwt/jwt/v5` for JWT access tokens
   - `golang.org/x/crypto` for Argon2id password hashing
   - `github.com/google/uuid` for UUID parsing/generation
   - `github.com/pressly/goose/v3` for SQL migrations
   - `github.com/stretchr/testify` for tests

   Note: Go is not installed in the current environment, so `go mod tidy`/`go.sum` generation must run once Go is available.
4. ☑ Add configuration model

   Implemented `internal/config.Config` with environment-based loading and validation.

   Supported variables:

   - `PORT` default `8080`
   - `DATABASE_URL` required
   - `JWT_SECRET` required
   - `JWT_ISSUER` default `tasks-server`
   - `ACCESS_TOKEN_TTL` default `15m`
   - `REFRESH_TOKEN_TTL` default `720h`
   - `WRITE_RATE_LIMIT_PER_MIN` default `60`
   - `MAX_BLOB_BYTES` default `1048576`
   - `MAX_BATCH_BLOBS` default `100`
   - `TOMBSTONE_RETENTION` default `720h`
5. ☑ Define initial PostgreSQL migrations

   Added numbered Goose migrations:

   - `001_init.sql`
     - `pgcrypto` extension
     - `users`
     - `devices`
     - `blobs`
     - `refresh_tokens`
   - `002_add_settings.sql`
     - `plaintext_settings`
   - `003_add_shared_blobs.sql`
     - `shared_blobs`

   Schema decisions applied:

   - `blobs.task_id` and `shared_blobs.task_id` are `TEXT`, allowing normal UUID task IDs and the reserved `vault_settings` ID.
   - Blob tombstones may have `NULL` ciphertext/nonce.
   - Nonce length is constrained to 12 bytes where present.
   - Device keys are stored per-device in `devices`, not as a single user key.
6. ☑ Implement server startup path

   Added a runnable `cmd/server` entrypoint that:

   - loads environment configuration
   - runs Goose migrations by default (`-migrate=false` to skip)
   - opens and pings a PostgreSQL `pgxpool`
   - builds the HTTP router
   - serves with `ReadHeaderTimeout`
   - handles SIGINT/SIGTERM graceful shutdown

   Added `internal/db` helpers for PostgreSQL pool creation and Goose migration execution.

7. ☑ Define common API response helpers

   Added `internal/respond` with JSON response, JSON error, and strict request decode helpers.

   Added initial router assembly in `internal/http` with standard middleware, `/healthz`, JSON 404, and JSON 405 responses.
8. ☑ Implement auth domain

   Added `internal/auth.Service` with registration and login flows backed by PostgreSQL. Passwords are normalized by email and stored using Argon2id hashes only.

9. ☑ Implement access tokens

   Added HS256 JWT access-token issuance with configured issuer and TTL. Tokens use the user UUID as `sub`.

10. ☑ Implement refresh tokens

   Added cryptographically random refresh tokens, SHA-256 token hashes in the database, expiry checks, revocation, and rotation-on-refresh semantics.

11. ☑ Implement auth endpoints

   Wired auth routes into the router:

   - `POST /auth/register`
   - `POST /auth/login`
   - `POST /auth/refresh`
   - `DELETE /auth/session`

   Auth JSON uses base64 `pub_key` input and returns `{ jwt, refresh_token, user_id }` for registration.
12. ☑ Implement wire encoding for binary fields

   Added `internal/wire.Base64Bytes`, a JSON helper for standard base64 wire encoding/decoding of opaque binary fields such as `pub_key`, `ciphertext`, `nonce`, and `wrapped_dek`.

   Added `internal/middleware.RequireAuth` to validate Bearer JWTs for upcoming protected routes and attach the authenticated user UUID to request context.
13. ☑ Implement blob repository

   Added `internal/blobs.Repository` with per-owner list, upsert, and tombstone operations over the `blobs` table.

14. ☑ Implement blob validation

   Added validation for non-empty task IDs, max task ID length, required ciphertext, max blob size, and 12-byte nonce length.

15. ☑ Implement blob endpoints

   Added protected blob routes:

   - `GET /blobs?since=`
   - `PUT /blobs/{task_id}`
   - `DELETE /blobs/{task_id}`
   - `POST /blobs/batch`

   Endpoints use authenticated user ownership from JWT context and base64 JSON binary fields. Added tests for blob validation.
16. ☑ Implement key directory

   Added `internal/keys` repository and protected handlers for key directory operations:

   - `GET /keys/{user_id}` returns all registered device public keys for a user
   - `PUT /keys/me` registers another public key for the authenticated user

   Public keys use base64 JSON encoding and validation rejects missing/oversized keys.

17. ☑ Implement multi-device key model

   Key registration stores one row per device in the existing `devices` table. The key lookup response returns a list of device keys so clients can wrap account/task keys for each target device.
18. ☑ Implement shared tasks

   Added `internal/share` repository and protected handlers:

   - `POST /share/{task_id}` stores opaque wrapped task keys for recipients
   - `GET /share/inbox` lists shares for the authenticated recipient
   - `DELETE /share/{task_id}/{recipient_id}` revokes share metadata owned by the authenticated user

   Share payload validation covers task ID, recipient UUID, wrapped key size, and 12-byte nonce.

19. ☑ Implement plaintext settings

   Added `internal/settings` repository and protected handlers:

   - `GET /settings/plaintext`
   - `PUT /settings/plaintext`

   Settings are stored as JSONB, must be a JSON object, and reject `last_sync_cursor` so device-local cursors are not persisted.
20. ☑ Add rate limiting

   Added in-memory per-user write rate limiting middleware with the configured default of 60 writes/min/user. Applied it to mutating protected routes while leaving reads unrestricted.

21. ☑ Add tombstone cleanup job

   Added tombstone cleanup for deleted blobs older than the configured retention period and starts the periodic cleanup job from server startup.

22. ☑ Security hardening

   Current hardening includes strict JSON decoding, JWT issuer/signing-method validation, request `ReadHeaderTimeout`, per-user resource scoping on protected repositories, max blob/key/share payload sizes, refresh-token hashing at rest, and write rate limiting.
23. ☑ Add tests in layers

   Added package-level tests alongside each implemented layer: config, response helpers, auth primitives, auth middleware, wire encoding, blob validation, key validation, share validation, settings validation, rate limiting, router behavior, cleanup startup behavior, and tooling/CI file checks.

24. ☑ Add local development tooling

   Added local server tooling:

   - `server/Makefile` with `tidy`, `fmt`, `test`, `build`, `check`, `run`, `migrate`, `docker-up`, and `docker-down`
   - `server/docker-compose.yml` for local PostgreSQL
   - `server/.env.example`
   - `server/README.md`

25. ☑ Add CI for server

   Added `.github/workflows/server.yml` to run Go module tidiness checks, formatting checks, tests, and build for server changes.

26. ☑ Final implementation order

   Implemented the server in the planned dependency order: configuration and migrations first, startup/router foundations, auth, wire/auth middleware, blob sync, keys/devices, sharing, settings, safeguards, tests, tooling, and CI.
27. ☑ Resolve remaining spec issues before coding

   Updated `SPEC.md` to match the implemented v0.1 server contract:

   - binary fields are base64 JSON strings
   - `DELETE /auth/session` requires `{ refresh_token }`
   - key directory returns per-device keys as `{ user_id, keys: [{ device_id, pub_key }] }`
   - PostgreSQL schema documents `devices`, text blob IDs, nullable tombstone payloads, and nonce constraints
   - server errors use `{ error }`
   - default blob/batch limits and tombstone retention are documented
