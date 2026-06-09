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

## 7. Open questions

- **Tombstone window:** 30-day tombstone retention may be too short for infrequent users. Consider extending to 90 days or making it configurable.
- **Account key rotation on device loss:** If a device is lost, its keychain copy of the account `data_key` is potentially exposed. Rotating the account `data_key` means re-encrypting every blob under a new key and re-wrapping it for all remaining devices — non-trivial and not yet specified. (Per-task revocation in §2.6 is solved; whole-account rotation is not.)
- **Shared-task model overhead:** Tasks switch from the account `data_key` to a per-task `task_key` when first shared (§2.6). The client must track which key encrypts which blob. Consider storing a `key_id` reference per task locally so the right key is selected on decrypt.
- **Conflict resolution:** Last-write-wins on the payload `updated_at` is the default. Per-field merge (e.g. preserve the longer body) is possible but adds complexity; worth revisiting when collaborative editing is needed.
- **Blob size limit:** No limit defined. A task with large attachments could produce a very large blob. Consider a per-blob size cap (e.g. 1 MB) and a separate attachment storage path.
- **Account deletion:** Cascading delete on `users` will remove all blobs. Define a grace period and export mechanism before shipping.
- **Key substitution defense:** The key directory could serve a malicious public key (§ key directory). A device-signed identity key with client-side verification (Signal-style key transparency) would close this; acceptable to defer for v1 against a non-adversarial server.

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
│       └── desktop.yml     # triggers on desktop/** or core/**
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

### 8.3 Cargo workspace

`core/` and `desktop/src-tauri/` are members of a shared Cargo workspace defined at the repo root:

```toml
# Cargo.toml (root)
[workspace]
members = [
    "core",
    "desktop/src-tauri",
]
```

This means `cargo build`, `cargo test`, and `cargo clippy` at the root cover both Rust crates. The desktop shell depends on `core` as a path dependency:

```toml
# desktop/src-tauri/Cargo.toml
[dependencies]
core = { path = "../../core" }
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

Any change to `core/` triggers all downstream platform builds. This is intentional — `core/` is a shared dependency and must be verified against every consumer on every change.
