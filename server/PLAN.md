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
4. ☐ Add configuration model
5. ☐ Define initial PostgreSQL migrations
6. ☐ Implement server startup path
7. ☐ Define common API response helpers
8. ☐ Implement auth domain
9. ☐ Implement access tokens
10. ☐ Implement refresh tokens
11. ☐ Implement auth endpoints
12. ☐ Implement wire encoding for binary fields
13. ☐ Implement blob repository
14. ☐ Implement blob validation
15. ☐ Implement blob endpoints
16. ☐ Implement key directory
17. ☐ Implement multi-device key model
18. ☐ Implement shared tasks
19. ☐ Implement plaintext settings
20. ☐ Add rate limiting
21. ☐ Add tombstone cleanup job
22. ☐ Security hardening
23. ☐ Add tests in layers
24. ☐ Add local development tooling
25. ☐ Add CI for server
26. ☐ Final implementation order
27. ☐ Resolve remaining spec issues before coding
