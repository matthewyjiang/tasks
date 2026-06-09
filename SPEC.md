# Task manager — technical specification

**Version:** 0.1  
**Stack:** Rust (client core), Go (server), PostgreSQL, AES-256-GCM, ECDH (P-256)  
**Architecture:** Local-first, end-to-end encrypted, cross-platform

---

## 1. Overview

A local-first task manager where all task content is encrypted on the client before it ever leaves the device. The server is a zero-knowledge blob relay — it stores ciphertext and routes it between a user's devices, but never has the ability to read task content.

### Core properties

- All reads and writes hit the local SQLite database first
- The network is only involved during sync; the app is fully functional offline
- The server never sees plaintext task content, titles, bodies, due dates, or tags
- Private keys never leave the device they were generated on
- Search, filtering, and reminders run entirely on-device against the local plaintext DB

---

## 2. Crypto & key management

### 2.1 Primitives

| Primitive | Algorithm | Notes |
|---|---|---|
| Key agreement | ECDH P-256 | One keypair per device; used only to wrap the data key for new devices |
| Key derivation | HKDF-SHA256 | Derives the wrap key from the ECDH shared secret |
| Symmetric encryption | AES-256-GCM | Encrypts task blobs |
| Password hashing | Argon2id | Server-side, for auth only |
| Nonce | Random 96-bit | Fresh nonce per encryption call |

### 2.2 Key model (envelope encryption)

There is **one data key per user account**. Every task blob is encrypted with this single 256-bit AES key. The data key is generated once, on the first device, and is shared to additional devices by wrapping it with an ECDH-derived key — it is never derived per-task and never transmitted in the clear.

```
User password
  └─ Argon2id ──────────────────────► auth_credential   (sent to server for login)
  └─ Argon2id (separate salt) ──────► master_secret     (never leaves device)

data_key (AES-256)                                       THE key that encrypts all blobs
  └─ generated once via OsRng on the first device
  └─ at rest on each device: encrypted with master_secret-derived wrap_key,
     stored in OS keychain alongside the device private key

Device keypair (ECDH P-256, generated on first launch of each device)
  └─ priv_key ──────────────────────► stored in OS keychain / secure enclave
  └─ pub_key ───────────────────────► registered with server key directory

Sharing data_key to a new device B:
  ECDH(priv_key_A, pub_key_B) = shared_secret
    └─ HKDF("dek-wrap") ────────────► wrap_key
         └─ AES-256-GCM(wrap_key, data_key) = wrapped_dek   (posted to server, opaque)
```

Key facts that the rest of the spec depends on:

- The `data_key` is the same value across all of a user's devices and across all their tasks.
- `task_id` is **not** an input to key derivation. It identifies a blob, nothing more.
- The ECDH exchange exists only to securely move the `data_key` from an existing device to a new one. It is not used to encrypt task content directly.
- On a brand-new account with a single device, `init_account` generates `data_key` locally with `OsRng`. No ECDH or second device is required to start encrypting tasks — this is the bootstrap path.

### 2.3 Encryption flow (encrypt_blob)

1. Serialise `Task` struct to JSON bytes
2. Generate 12-byte random nonce via `OsRng`
3. Encrypt with AES-256-GCM: `cipher.encrypt(nonce, plaintext)`
4. AES-GCM appends a 16-byte authentication tag to the ciphertext automatically
5. Store `{ ciphertext, nonce }` as the blob — this is what gets sent to the server

### 2.4 Decryption flow (decrypt_blob)

1. Validate key is 32 bytes
2. Construct `Aes256Gcm` cipher from key bytes
3. Call `cipher.decrypt(nonce, ciphertext)` — returns `Err` if auth tag fails
4. Deserialise plaintext bytes back to `Task` struct
5. Upsert into local SQLite DB

Authentication tag verification is built into AES-GCM — a wrong key, tampered ciphertext, or reused nonce all produce a decryption error before any plaintext is returned. There is no separate MAC step.

### 2.5 Adding a device

The `data_key` already exists on the first device. To grant a second device access:

1. New device generates its own ECDH keypair, registers `pub_key_B` with the server
2. Existing device fetches `pub_key_B` from the key directory
3. Existing device computes `ECDH(priv_key_A, pub_key_B)` → `shared_secret`
4. Derives `wrap_key` via `HKDF("dek-wrap")`, wraps the `data_key`: `wrapped_dek = AES-256-GCM(wrap_key, data_key)`
5. Posts `{ target_device, wrapped_dek, nonce }` to the server — server stores it opaquely
6. New device computes the same `shared_secret` via `ECDH(priv_key_B, pub_key_A)`, derives the same `wrap_key`, unwraps `data_key`, and stores it at rest in its own keychain

Both devices now hold the identical `data_key` and can decrypt every blob. This requires the existing device to be online at least once after the new device registers — the wrap happens device-to-device through the server, never on the server itself.

### 2.6 Shared tasks (collaboration)

A user's own tasks all use the single account `data_key`. That key cannot be handed to a collaborator — it would expose every task. So a task that is shared gets its own dedicated per-task key (`task_key`), generated when the task is first shared:

1. Sharer generates a fresh `task_key` (AES-256) via `OsRng`, re-encrypts that task's blob under `task_key` instead of the account `data_key`
2. Sharer fetches the recipient's `pub_key` from the key directory
3. Sharer computes `ECDH(priv_key_sharer, pub_key_recipient)` → `shared_secret`, derives a `wrap_key`, and wraps the `task_key`: `wrapped_dek = AES-256-GCM(wrap_key, task_key)`
4. Posts `{ task_id, recipient_id, wrapped_dek, nonce }` to `/share/:task_id`
5. Recipient fetches their inbox, unwraps `wrapped_dek` with their own ECDH private key to recover `task_key`, and decrypts the blob

**Revoking access:** the sharer calls `DELETE /share/:task_id/:recipient_id`, which deletes the `wrapped_dek` row, and then **rotates the task** — generates a new `task_key`, re-encrypts the blob under it, and re-wraps it only for the remaining collaborators. Deleting the `wrapped_dek` row alone is not sufficient: a revoked recipient may have cached the old `task_key`, so the task must be re-keyed for revocation to be meaningful. Note that the revoked recipient can still read any copy of the task content they captured before revocation — forward secrecy is not retroactive.

### 2.7 Security constraints

- Nonces must never be reused with the same key — always generate fresh via `OsRng`
- Private keys are stored exclusively in the OS keychain (iOS Keychain, Android Keystore, macOS Keychain, Windows Credential Manager, Linux libsecret)
- `auth_credential` and `master_secret` are derived separately — the server never sees the master secret
- Decryption errors must not leak timing information about which part failed (wrong key vs. tampered ciphertext are indistinguishable by design)

---

## 3. Client core library

The core is a Rust library compiled to a native `.a` / `.so` and exposed to platform UI shells via UniFFI-generated bindings.

### 3.1 Types

```rust
pub struct Task {
    pub id:         Uuid,
    pub title:      String,
    pub body:       String,
    pub due_at:     Option<i64>,    // Unix timestamp (ms)
    pub status:     TaskStatus,
    pub project_id: Option<Uuid>,
    pub tags:       Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted:    bool,
    pub dirty:      bool,           // true = not yet synced to server
}

pub enum TaskStatus { Inbox, InProgress, Done }

pub struct Blob {
    pub ciphertext: Vec<u8>,
    pub nonce:      [u8; 12],
}
```

### 3.2 Task CRUD

| Function | Signature | Behaviour |
|---|---|---|
| `create_task` | `(title, body, due_at) → Result<Task>` | Inserts into local DB; sets `dirty=true`, generates UUID |
| `get_task` | `(task_id) → Result<Task>` | Local DB read only; never touches network |
| `update_task` | `(task_id, patch: TaskPatch) → Result<Task>` | Merges changed fields; bumps `updated_at`; sets `dirty=true` |
| `delete_task` | `(task_id) → Result<()>` | Sets `deleted=true`, `dirty=true` — tombstone, not hard delete |
| `list_tasks` | `(filter: TaskFilter, sort: TaskSort) → Result<Vec<Task>>` | SQL query over local DB; supports filter by status, project, due range |
| `search_tasks` | `(query: String) → Result<Vec<Task>>` | FTS5 full-text search over local plaintext; offline, instant |

### 3.3 Crypto functions

| Function | Signature | Behaviour |
|---|---|---|
| `init_account` | `() → Result<Vec<u8>>` | First-launch bootstrap: generates `data_key` via `OsRng`, wraps it at rest, stores in keychain; returns `pub_key` for the device |
| `init_device_keypair` | `() → Result<Vec<u8>>` | Generates the device's ECDH P-256 keypair; stores `priv_key` in keychain; returns `pub_key` |
| `wrap_data_key` | `(data_key, peer_pub_key, own_priv_key) → Result<Blob>` | ECDH → HKDF → AES-GCM-wrap the data key for a peer device. Returns wrapped blob |
| `unwrap_data_key` | `(wrapped: &Blob, peer_pub_key, own_priv_key) → Result<[u8; 32]>` | Reverse of `wrap_data_key`; recovers the shared data key |
| `encrypt_blob` | `(task: &Task, key: &[u8]) → Result<Blob>` | Serialise → AES-256-GCM encrypt with fresh nonce |
| `decrypt_blob` | `(blob: &Blob, key: &[u8]) → Result<Task>` | AES-256-GCM decrypt → deserialise |

### 3.4 Sync functions

| Function | Signature | Behaviour |
|---|---|---|
| `sync_push` | `() → Result<SyncResult>` | Finds `dirty=true` rows; encrypts each; sends via `PUT /blobs/:id` (or `POST /blobs/batch`); clears `dirty` only for blobs the response confirms succeeded — never clears optimistically |
| `sync_pull` | `(since: i64) → Result<SyncResult>` | `GET /blobs?since=<cursor>`; decrypts each blob; upserts local DB; advances cursor |
| `resolve_conflict` | `(local: &Task, remote: &Task) → Task` | Compares the `updated_at` inside the decrypted payload (not the server column); last-write-wins by default; pluggable strategy |
| `queue_retry` | `(task_id: Uuid) → ()` | On network failure, re-queues with exponential backoff; queue persisted across restarts |

### 3.5 Platform trait

The core library never imports platform SDKs directly. Each shell provides a concrete implementation of this trait at initialisation:

```rust
pub trait Platform: Send + Sync {
    fn store_key(&self, id: &str, bytes: &[u8]) -> Result<()>;
    fn load_key(&self, id: &str) -> Result<Vec<u8>>;
    fn delete_key(&self, id: &str) -> Result<()>;
    fn schedule_notification(&self, task_id: Uuid, fire_at: i64, title: &str) -> Result<()>;
    fn cancel_notification(&self, task_id: Uuid) -> Result<()>;
    fn network_available(&self) -> bool;
}
```

Platform implementations:

| Platform | Keychain | Notifications |
|---|---|---|
| iOS | `SecItemAdd` / Keychain Services | `UNUserNotificationCenter` |
| Android | Android Keystore | `AlarmManager` |
| macOS | Keychain Services | `UNUserNotificationCenter` |
| Windows | Windows Credential Manager | Windows Task Scheduler |
| Linux | libsecret | `notify-send` / D-Bus |

### 3.6 Local DB schema (SQLite)

```sql
CREATE TABLE tasks (
    id          TEXT PRIMARY KEY,   -- UUID as text
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    due_at      INTEGER,            -- Unix ms, nullable
    status      TEXT NOT NULL DEFAULT 'inbox',
    project_id  TEXT,
    tags        TEXT NOT NULL DEFAULT '[]',  -- JSON array
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0,
    dirty       INTEGER NOT NULL DEFAULT 1
);

CREATE VIRTUAL TABLE tasks_fts USING fts5(
    title, body,
    content='tasks', content_rowid='rowid'
);

CREATE TABLE sync_cursor (
    id          INTEGER PRIMARY KEY DEFAULT 1,
    last_pull   INTEGER NOT NULL DEFAULT 0   -- updated_at of last pulled blob
);

CREATE TABLE sync_queue (
    task_id     TEXT NOT NULL,
    queued_at   INTEGER NOT NULL,
    attempt     INTEGER NOT NULL DEFAULT 0,
    next_retry  INTEGER NOT NULL DEFAULT 0
);
```

---

## 4. Server API

### 4.1 Overview

The server is a zero-knowledge blob relay. It stores encrypted blobs identified by `task_id` and `owner_id`, and serves them back on request. It has no knowledge of task content, structure, or meaning.

**Stack:** Go, `net/http` stdlib + `chi` router, `pgx` PostgreSQL driver  
**Auth:** JWT (short-lived, 15 min) + refresh token (30 days, rotated on use)  
**All blob routes require:** `Authorization: Bearer <jwt>`

Binary JSON fields (`pub_key`, `ciphertext`, `nonce`, `wrapped_dek`) are standard base64 strings on the wire.

### 4.2 Endpoints

#### Auth

| Method | Path | Request body | Response | Notes |
|---|---|---|---|---|
| `POST` | `/auth/register` | `{ email, password, pub_key }` | `{ jwt, refresh_token, user_id }` | Stores argon2id hash; stores raw `pub_key` bytes |
| `POST` | `/auth/login` | `{ email, password }` | `{ jwt, refresh_token }` | Verifies argon2id hash |
| `POST` | `/auth/refresh` | `{ refresh_token }` | `{ jwt, refresh_token }` | Rotates refresh token; old token invalidated |
| `DELETE` | `/auth/session` | `{ refresh_token }` | `204` | Invalidates the supplied refresh token |

#### Blobs

| Method | Path | Request body | Response | Notes |
|---|---|---|---|---|
| `GET` | `/blobs` | — | `{ blobs: [...], cursor }` | Query param `?since=<unix_ms>`; returns blobs where `updated_at > since` for authenticated user. `cursor` in the response is the max `updated_at` (Unix ms) of the returned set |
| `PUT` | `/blobs/:task_id` | `{ ciphertext, nonce }` | `{ task_id, updated_at }` | Upsert; idempotent; sets `updated_at = now()` |
| `DELETE` | `/blobs/:task_id` | — | `204` | Sets `deleted=true`, bumps `updated_at`; tombstone returned on next pull |
| `POST` | `/blobs/batch` | `{ blobs: [{ task_id, ciphertext, nonce }] }` | `{ results: [{ task_id, status, updated_at }] }` | Up to 100 blobs per request. Each result reports per-blob success/failure; the client clears `dirty` only for entries with a success status. Partial failure is expected and safe |

#### Key directory

| Method | Path | Request body | Response | Notes |
|---|---|---|---|---|
| `GET` | `/keys/:user_id` | — | `{ user_id, keys: [{ device_id, pub_key }] }` | Returns all registered device ECDH public keys for any user; used before sharing a task |
| `PUT` | `/keys/me` | `{ pub_key }` | `204` | Registers a new device's public key |

#### Shared tasks

| Method | Path | Request body | Response | Notes |
|---|---|---|---|---|
| `POST` | `/share/:task_id` | `{ recipient_id, wrapped_dek, nonce }` | `201` | Stores the per-task key wrapped for recipient; server never unwraps it |
| `GET` | `/share/inbox` | — | `{ shared: [...] }` | Returns blobs shared with current user, including `wrapped_dek` for each |
| `DELETE` | `/share/:task_id/:recipient_id` | — | `204` | Revokes access; deletes `wrapped_dek` row |

### 4.3 DB schema (PostgreSQL)

```sql
CREATE TABLE users (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email       TEXT UNIQUE NOT NULL,
    password_h  TEXT NOT NULL,          -- argon2id hash
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE devices (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    pub_key       BYTEA NOT NULL,       -- ECDH public key for one device
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX devices_user_created ON devices(user_id, created_at);

CREATE TABLE blobs (
    task_id     TEXT NOT NULL,          -- UUID text or reserved IDs such as "vault_settings"
    owner_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ciphertext  BYTEA,
    nonce       BYTEA,                  -- 12 bytes when present
    updated_at  BIGINT NOT NULL,        -- Unix ms; set by server on write, matches wire cursor
    deleted     BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (task_id, owner_id),
    CONSTRAINT blobs_nonce_len CHECK (nonce IS NULL OR octet_length(nonce) = 12),
    CONSTRAINT blobs_deleted_payload CHECK (
        (deleted = true) OR (ciphertext IS NOT NULL AND nonce IS NOT NULL)
    )
);

CREATE INDEX blobs_owner_updated ON blobs(owner_id, updated_at);

CREATE TABLE shared_blobs (
    task_id         TEXT NOT NULL,
    owner_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    wrapped_dek     BYTEA NOT NULL,     -- per-task task_key, AES-GCM wrapped for recipient
    nonce           BYTEA NOT NULL,     -- 12 bytes, for the wrap
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (task_id, recipient_id),
    CONSTRAINT shared_blobs_nonce_len CHECK (octet_length(nonce) = 12)
);

CREATE TABLE refresh_tokens (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_h     TEXT NOT NULL,          -- hashed token
    expires_at  TIMESTAMPTZ NOT NULL,
    revoked     BOOLEAN NOT NULL DEFAULT false
);
```

### 4.4 Server constraints

- The server queries blobs only by `task_id`, `owner_id`, `updated_at`, and `deleted` — all opaque identifiers or timestamps. It never queries or indexes ciphertext.
- `updated_at` exists in two places: as a plaintext `BIGINT` column the server sets on every write (drives the `?since=` sync cursor) and as a field inside the encrypted payload (drives client-side conflict resolution). The server column orders sync; the payload field decides which version wins a conflict. They are written together but only the payload field is authoritative for `resolve_conflict`.
- All blob endpoints enforce ownership by filtering on `owner_id` derived from the JWT claim. A user cannot read or write another user's blobs.
- Tombstones (`deleted=true`) are retained for 30 days by default, then hard-deleted by a background job. Clients that have not synced within the retention window may miss deletions.
- Tombstone rows keep `task_id`, `owner_id`, `updated_at`, and `deleted=true`; `ciphertext` and `nonce` may be `NULL`.
- `PUT /blobs/:task_id` is idempotent. Retried pushes after network failure are safe.
- Rate limit: 60 write requests/min per user by default. No read rate limit.
- Maximum blob size is 1 MiB by default; maximum batch size is 100 blobs by default.
- JWT expiry: 15 minutes. Refresh token expiry: 30 days, rotated on every use.

---

## 5. Error types

### Client (Rust)

```rust
pub enum CryptoError {
    DecryptFailed,              // Wrong key, tampered ciphertext, or bad nonce
    BadKeyLength(usize),        // Key was not 32 bytes
    DeserFailed(serde_json::Error),
}

pub enum SyncError {
    NetworkUnavailable,
    AuthExpired,                // JWT expired; re-login required
    BlobConflict(Uuid),         // Conflict that could not be auto-resolved
    ServerError(u16, String),   // HTTP status + body
}
```

### Server (Go)

Go uses sentinel errors and structured error values rather than enums:

```go
// Sentinel errors for handler logic
var (
    ErrNotFound      = errors.New("not found")
    ErrUnauthorized  = errors.New("unauthorized")
    ErrBadRequest    = errors.New("bad request")
)

// ErrorResponse is returned as JSON to the client
type ErrorResponse struct {
    Error string `json:"error"`
}

// BlobResult is used in batch responses for per-blob success/failure
type BlobResult struct {
    TaskID    string `json:"task_id"`
    Status    string `json:"status"`     // "ok" | "error"
    UpdatedAt int64  `json:"updated_at"` // Unix ms, zero on error
    Error     string `json:"error,omitempty"`
}
```

### Type duplication note

The server and client core are in different languages, so shared types (`Task`, `Blob`, `ErrorResponse`) are defined twice — once in Rust for the client core, once in Go for the server. This is an accepted tradeoff for this project. To prevent drift:

- The wire format for all request/response bodies is JSON with a documented schema (see §4.2)
- Any field rename or addition must be applied to both codebases and increments the spec version
- Integration tests should validate the JSON contract at the boundary

---

## 6. Settings

Settings are split into two documents based on whether they are needed before or after the vault is unlocked.

### 6.1 Plaintext settings (`settings.json`)

Stored locally as a flat JSON file on disk (not in SQLite) and synced to the server as a dedicated plaintext row. Contains only what the app needs before decryption can occur. Never encrypted.

```json
{
  "schema_version": 1,
  "server_url": "https://api.example.com",
  "auth_method": "biometric",
  "language": "en",
  "last_sync_cursor": 1717603200000
}
```

| Field | Type | Description |
|---|---|---|
| `schema_version` | `int` | Incremented on breaking field changes |
| `server_url` | `string` | Sync server base URL; must be set before first sync |
| `auth_method` | `string` | `"biometric"` / `"pin"` / `"password"` |
| `language` | `string` | BCP-47 locale code |
| `last_sync_cursor` | `int` | Unix ms of last successful pull; device-local, never overwritten by a pull |

**Storage:** written to a platform-appropriate app data directory (`~/.config/taskmanager/settings.json` on Linux, `NSApplicationSupportDirectory` on macOS, etc.). Not in SQLite — must be readable before the DB is opened.

**Sync:** `PUT /settings/plaintext` on change. Server stores as a single row keyed by `owner_id`. `last_sync_cursor` is device-local state and must not be sent to or overwritten from the server.

### 6.2 Vault settings (`vault_settings.json`)

Personal preferences that are private or user-facing. Encrypted with the account `data_key` and synced as a blob, identical in every way to task blobs. Stored in the local SQLite DB as a single row with the well-known `task_id` value of `"vault_settings"`.

```json
{
  "schema_version": 1,
  "theme": "system",
  "default_sort": "due_at_asc",
  "show_completed": false,
  "default_reminder_minutes": 30,
  "tag_colors": {
    "work": "#4A90D9",
    "personal": "#7ED321"
  },
  "display_density": "comfortable",
  "first_day_of_week": 1,
  "notification_sound": "default"
}
```

| Field | Type | Description |
|---|---|---|
| `schema_version` | `int` | Incremented on breaking field changes |
| `theme` | `string` | `"light"` / `"dark"` / `"system"` |
| `default_sort` | `string` | Default `list_tasks` sort order |
| `show_completed` | `bool` | Whether done tasks appear in default list view |
| `default_reminder_minutes` | `int` | Minutes before `due_at` to fire a reminder; `0` = disabled |
| `tag_colors` | `object` | Map of tag name to hex colour string |
| `display_density` | `string` | `"compact"` / `"comfortable"` / `"spacious"` |
| `first_day_of_week` | `int` | ISO weekday: `1` = Monday, `7` = Sunday |
| `notification_sound` | `string` | Platform notification sound identifier |

**Encryption:** encrypted with `encrypt_blob` using the account `data_key`, identical to any task blob. Decrypted with `decrypt_blob`.

**Sync:** participates in the standard push/pull cycle via the existing blob endpoints — `PUT /blobs/vault_settings` on change, returned in `GET /blobs?since=` like any other blob. No special server handling required; the server sees an opaque blob with a fixed well-known ID.

**Conflict resolution:** last-write-wins on `updated_at`. The settings document is small and edited infrequently — full-document overwrite on conflict is acceptable.

**Schema migration:** when `schema_version` increments, the client reads the old blob, applies a migration function in the core library, and writes the new version back. Migrations run on first open after an app update.

### 6.3 Server endpoints for settings

#### Plaintext settings

| Method | Path | Request body | Response | Notes |
|---|---|---|---|---|
| `GET` | `/settings/plaintext` | — | `{ settings }` | Returns current plaintext settings for the authenticated user |
| `PUT` | `/settings/plaintext` | `{ settings }` | `204` | Upserts; sets `updated_at = now()` server-side |

#### Vault settings

No dedicated endpoint. Vault settings sync through the existing blob endpoints as `task_id = "vault_settings"`. No server changes required.

### 6.4 Server DB addition

```sql
CREATE TABLE plaintext_settings (
    owner_id    UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    settings    JSONB NOT NULL,
    updated_at  BIGINT NOT NULL      -- Unix ms
);
```

`JSONB` allows the server to read `schema_version` if needed (e.g. to prompt outdated clients), but the server must not use any other field for business logic.


---

## 7. Rust CLI

The project includes a first-party Rust command-line application, `taskmanager`, built on top of the same `core/` crate as the GUI shells. The CLI is not a reduced admin tool: it must expose every user-facing and integration-facing capability of the core library so it can be used as a complete terminal client and as an autonomous end-to-end test driver for the core ⇄ server pipeline.

### 7.1 Goals

- Provide full task-manager functionality without any GUI dependency
- Exercise every public core interface from an executable client
- Support deterministic JSON output for scripts, CI, and integration tests
- Support human-friendly table/text output for interactive terminal use
- Use the same local-first, encrypted, offline-capable data path as all GUI apps
- Avoid duplicate business logic; all task, crypto, sync, settings, reminder, and sharing behavior lives in `core/`

### 7.2 Binary and configuration

**Binary:** `taskmanager`

Global flags:

| Flag | Description |
|---|---|
| `--profile <name>` | Select local profile/config directory; default `default` |
| `--config <path>` | Override plaintext settings path |
| `--db <path>` | Override SQLite DB path, useful for tests |
| `--server <url>` | Override configured server URL for this invocation |
| `--output <table|json|jsonl>` | Output format; `json` is required for all commands |
| `--quiet` | Suppress non-result messages |
| `--yes` | Assume yes for destructive confirmations |
| `--offline` | Refuse network access even if a command would normally sync |
| `--trace` | Enable structured debug logs on stderr |

The CLI uses platform app-data directories by default, but every path must be overridable so tests can run in temporary isolated directories.

### 7.3 Platform implementation

The CLI provides a concrete `Platform` implementation for desktop/server environments:

| Trait method | CLI behavior |
|---|---|
| `store_key` / `load_key` / `delete_key` | Use OS keychain where available (`libsecret`, macOS Keychain, Windows Credential Manager); in CI or with `TASKMANAGER_INSECURE_KEY_DIR`, use an explicitly opted-in file-backed test key store |
| `schedule_notification` / `cancel_notification` | Use the same Linux/macOS/Windows desktop notification backends as the desktop shell when available; in headless mode persist scheduled reminders and expose them through CLI inspection commands |
| `network_available` | Check explicit `--offline` first, then perform a lightweight network/backend availability check |

The file-backed key store is only for local development and CI. It must be clearly named insecure and must never be selected implicitly for production profiles.

### 7.4 Command surface

Every command that changes local state must go through the core library and preserve the same dirty/tombstone/sync semantics as GUI apps.

#### Account, auth, and device commands

| Command | Core/API coverage |
|---|---|
| `taskmanager account init` | `init_account`, local DB/bootstrap settings, device public-key registration |
| `taskmanager auth login` / `auth refresh` / `auth logout` | Server auth endpoints, token storage through platform key store |
| `taskmanager device init-keypair` | `init_device_keypair` |
| `taskmanager device list` | Server key directory |
| `taskmanager device register` | Register this device public key with server |
| `taskmanager device wrap-key --target <device_id>` | `wrap_data_key`, wrapped-key upload |
| `taskmanager device unwrap-key --from <device_id>` | Wrapped-key fetch, `unwrap_data_key`, local key storage |

#### Task commands

| Command | Core coverage |
|---|---|
| `taskmanager task create --title ... [--body ...] [--due ...] [--tag ...] [--project ...]` | `create_task` |
| `taskmanager task get <task_id>` | `get_task` |
| `taskmanager task update <task_id> [fields...]` | `update_task` |
| `taskmanager task delete <task_id>` | `delete_task` tombstone |
| `taskmanager task list [filters...] [--sort ...]` | `list_tasks` with all `TaskFilter` and `TaskSort` variants |
| `taskmanager task search <query>` | `search_tasks` |
| `taskmanager task complete <task_id>` / `reopen <task_id>` | `update_task` status patch |

#### Sync and server-pipeline commands

| Command | Core/API coverage |
|---|---|
| `taskmanager sync push` | `sync_push` |
| `taskmanager sync pull [--since <cursor>]` | `sync_pull` |
| `taskmanager sync run` | Pull then push, or configured bidirectional order |
| `taskmanager sync status` | Local dirty rows, queue depth, retry state, cursor |
| `taskmanager sync retry <task_id>` | `queue_retry` |
| `taskmanager sync conflicts` | Inspect pending conflicts when pluggable conflict handling is enabled |
| `taskmanager sync resolve <task_id> --local|--remote|--json <patch>` | `resolve_conflict` / configured strategy |

#### Settings commands

| Command | Coverage |
|---|---|
| `taskmanager settings get [key]` | Plaintext and vault settings read paths |
| `taskmanager settings set <key> <value>` | Plaintext/vault settings write paths and sync dirty marking |
| `taskmanager settings pull-plaintext` / `push-plaintext` | `/settings/plaintext` endpoints |
| `taskmanager settings migrate` | Vault settings schema migration functions |

#### Sharing commands

| Command | Coverage |
|---|---|
| `taskmanager share create <task_id> --recipient <user_or_device>` | Per-task key generation, task re-encryption, recipient key wrap, `/share/:task_id` |
| `taskmanager share inbox` | Shared-task inbox fetch |
| `taskmanager share accept <share_id>` | Wrapped task-key unwrap and local import |
| `taskmanager share revoke <task_id> --recipient <id>` | Share deletion, task-key rotation, remaining-recipient rewrap |
| `taskmanager share list <task_id>` | Current collaborators and key state |

#### Crypto diagnostic commands

These commands exist for development/test profiles and must never print secret key material unless `--dangerously-print-secrets` is supplied.

| Command | Coverage |
|---|---|
| `taskmanager crypto encrypt-task <task_id>` | `encrypt_blob` |
| `taskmanager crypto decrypt-blob <file>` | `decrypt_blob` |
| `taskmanager crypto wrap-data-key ...` / `unwrap-data-key ...` | Direct wrap/unwrap coverage |
| `taskmanager crypto verify-local` | Validate local key availability and decryptability without network |

### 7.5 Output and exit-code contract

- `--output json` returns stable machine-readable JSON for every command.
- Errors are written to stderr as `{ "error": { "code", "message", "details" } }` when JSON output is selected.
- Exit codes:
  - `0`: success
  - `1`: user/input error
  - `2`: local DB or key-store error
  - `3`: crypto/decryption/authentication failure
  - `4`: network/server error
  - `5`: conflict or partial sync failure
  - `6`: unsupported platform capability

### 7.6 Autonomous integration testing mode

The CLI is the canonical black-box integration-test harness for core ⇄ server behavior. Test suites should be able to start a disposable server, create two or more isolated CLI profiles, and verify the complete encrypted sync path without GUI automation.

Required test-friendly capabilities:

- All local state paths injectable via flags or environment variables
- Insecure file-backed key store available only by explicit opt-in
- JSON output for every command
- Idempotent commands where practical, or clear `already_exists` errors
- Fixtures for generating users, devices, tasks, shares, and settings
- Ability to run push/pull loops until quiescent with a timeout
- Ability to inspect local dirty rows, cursors, retry queue, and scheduled reminders
- No command may require interactive input when all required flags are supplied

Minimum end-to-end scenarios covered through the CLI:

1. Account bootstrap: init account, create task offline, push, verify opaque blob on server.
2. Device pairing: second profile registers device, first wraps data key, second unwraps, second pulls and decrypts tasks.
3. Conflict path: two profiles edit the same task offline, sync both, verify configured resolution.
4. Tombstone path: delete task locally, sync tombstone, pull deletion on another profile.
5. Settings path: update plaintext and vault settings, sync, pull on another profile.
6. Sharing path: share a task, accept on recipient profile, revoke and rotate task key.

### 7.7 Implementation requirements

- `cli/` is a Rust binary crate in the root Cargo workspace and depends on `core` by path.
- Use `clap` for argument parsing and `serde_json` for JSON output.
- The CLI must call public core APIs only; it must not reach into private core modules or duplicate SQL/crypto logic.
- Any new core capability required by the CLI must first be added to the core public API and, when needed by GUI apps, to `core/uniffi/core.udl`.
- CLI integration tests run against the same server API documented in §4 and should be included in CI.

---

## 8. Repository structure

The project uses a monorepo. All code, migrations, and the spec itself live in one repository. CI pipelines are path-filtered so only affected sub-projects build on any given change.

### 8.1 Directory layout

```
taskmanager/
├── .github/
│   └── workflows/
│       ├── core.yml        # triggers on core/**
│       ├── server.yml      # triggers on server/** or core/**
│       ├── ios.yml         # triggers on ios/** or core/**
│       ├── android.yml     # triggers on android/** or core/**
│       ├── desktop.yml     # triggers on desktop/** or core/**
│       └── cli.yml         # triggers on cli/**, core/**, or server/**
├── core/
│   ├── src/
│   ├── Cargo.toml
│   └── uniffi/
│       └── core.udl        # UniFFI interface definition — source of truth for FFI boundary
├── server/
│   ├── cmd/server/
│   ├── internal/
│   │   ├── auth/
│   │   ├── blobs/
│   │   ├── keys/
│   │   └── settings/
│   ├── migrations/         # numbered SQL files, never edited after running in production
│   └── go.mod
├── ios/
│   └── TaskManager.xcodeproj
├── android/
│   └── app/
├── desktop/
│   ├── src-tauri/          # Rust Tauri backend, imports core as a crate dependency
│   └── src/                # Web UI (React or Svelte)
├── cli/
│   ├── src/
│   └── Cargo.toml          # Rust CLI binary, imports core as a crate dependency
└── README.md
```

### 8.2 Sub-project responsibilities

| Directory | Language | Role |
|---|---|---|
| `core/` | Rust | Crypto, local SQLite DB, sync engine, reminder scheduler. Compiled to a native library; exposed via UniFFI bindings to iOS and Android |
| `server/` | Go | Zero-knowledge blob relay. Auth, blob store, key directory, plaintext settings. No business logic |
| `ios/` | Swift | SwiftUI shell. Thin UI layer + Platform trait implementation for iOS/macOS |
| `android/` | Kotlin | Jetpack Compose shell. Thin UI layer + Platform trait implementation for Android |
| `desktop/` | Rust + Web | Tauri shell for Windows, macOS, Linux. `src-tauri/` imports `core/` as a Cargo workspace member; `src/` is the web UI |
| `cli/` | Rust | Full-featured terminal client and autonomous integration-test harness. Imports `core/` as a Cargo workspace member and exercises the same local-first encrypted sync path as GUI apps |

### 8.3 Cargo workspace

`core/`, `desktop/src-tauri/`, and `cli/` are members of a shared Cargo workspace defined at the repo root:

```toml
# Cargo.toml (root)
[workspace]
members = [
    "core",
    "desktop/src-tauri",
    "cli",
]
```

This means `cargo build`, `cargo test`, and `cargo clippy` at the root cover both Rust crates. The desktop shell and CLI depend on `core` as a path dependency:

```toml
# desktop/src-tauri/Cargo.toml
[dependencies]
core = { path = "../../core" }

# cli/Cargo.toml
[dependencies]
core = { path = "../core" }
```

### 8.4 UniFFI bindings

Generated bindings for Swift and Kotlin are **not** checked into version control. Each platform's build step generates them fresh from `core/uniffi/core.udl`:

```bash
# iOS build step (run before xcodebuild)
uniffi-bindgen generate core/uniffi/core.udl --language swift --out-dir ios/Generated/

# Android build step (run before gradle)
uniffi-bindgen generate core/uniffi/core.udl --language kotlin --out-dir android/app/src/main/java/com/taskmanager/core/
```

`core/uniffi/core.udl` is the authoritative definition of the FFI boundary. Any change to the public API of `core/` that crosses the FFI must be reflected in `core.udl` first.

### 8.5 Database migrations

Server migrations live in `server/migrations/` as numbered SQL files:

```
server/migrations/
├── 001_init.sql
├── 002_add_settings.sql
└── 003_add_shared_blobs.sql
```

Rules:
- Files are named `NNN_description.sql` with zero-padded three-digit numbers
- A migration that has been applied to any environment (dev, staging, production) is **never edited** — add a new migration instead
- Migrations are run automatically on server startup via `golang-migrate` or `goose`
- Down migrations are optional but recommended for development environments

### 8.6 CI path filters

Each workflow triggers only on relevant paths to avoid unnecessary builds:

| Workflow | Triggers on changes to |
|---|---|
| `core.yml` | `core/**` |
| `server.yml` | `server/**`, `core/**` |
| `ios.yml` | `ios/**`, `core/**` |
| `android.yml` | `android/**`, `core/**` |
| `desktop.yml` | `desktop/**`, `core/**` |
| `cli.yml` | `cli/**`, `core/**`, `server/**` |

Any change to `core/` triggers all downstream platform builds. This is intentional — `core/` is a shared dependency and must be verified against every consumer on every change. The CLI workflow also triggers on `server/**` because it owns black-box core ⇄ server integration coverage.

---

## 9. Open questions

- **Tombstone window:** 30-day tombstone retention may be too short for infrequent users. Consider extending to 90 days or making it configurable.
- **Account key rotation on device loss:** If a device is lost, its keychain copy of the account `data_key` is potentially exposed. Rotating the account `data_key` means re-encrypting every blob under a new key and re-wrapping it for all remaining devices — non-trivial and not yet specified. (Per-task revocation in §2.6 is solved; whole-account rotation is not.)
- **Shared-task model overhead:** Tasks switch from the account `data_key` to a per-task `task_key` when first shared (§2.6). The client must track which key encrypts which blob. Consider storing a `key_id` reference per task locally so the right key is selected on decrypt.
- **Conflict resolution:** Last-write-wins on the payload `updated_at` is the default. Per-field merge (e.g. preserve the longer body) is possible but adds complexity; worth revisiting when collaborative editing is needed.
- **Blob size limit:** No limit defined. A task with large attachments could produce a very large blob. Consider a per-blob size cap (e.g. 1 MB) and a separate attachment storage path.
- **Account deletion:** Cascading delete on `users` will remove all blobs. Define a grace period and export mechanism before shipping.
- **Key substitution defense:** The key directory could serve a malicious public key (§ key directory). A device-signed identity key with client-side verification (Signal-style key transparency) would close this; acceptable to defer for v1 against a non-adversarial server.

