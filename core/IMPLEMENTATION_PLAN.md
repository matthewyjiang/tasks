# Core implementation plan

Goal: build the Rust client core incrementally, with readable, well-tested modules. Each milestone should compile and have focused interface tests before moving on.

## Guiding rules

- Implement one subsystem at a time.
- Keep public APIs small and close to `SPEC.md`.
- Prefer explicit types and clear error names over clever abstractions.
- Treat every public function, trait, and data type as an interface that needs tests.
- Add tests with each subsystem, including success paths, failure paths, and edge cases.
- Do not start UniFFI bindings until the Rust API has stabilized.

## Milestone 1: Core crate foundation

Create the Rust crate structure and shared domain types.

TODO:

- [x] Add `core/src/lib.rs` with module declarations.
- [x] Add `core/src/error.rs` for core, crypto, DB, and sync errors.
- [x] Add `core/src/types.rs` containing:
  - [x] `Task`
  - [x] `TaskStatus`
  - [x] `TaskPatch`
  - [x] `TaskFilter`
  - [x] `TaskSort`
  - [x] `Blob`
  - [x] `SyncResult`
- [x] Add serde support for task JSON encryption/decryption.

Interface tests to add before leaving this milestone:

- [x] `Task` JSON round-trip preserves every field.
- [x] `TaskStatus` serializes/deserializes to stable lowercase wire values.
- [x] Optional fields round-trip correctly when `None` and `Some`.
- [x] Empty tag lists and multi-tag lists round-trip correctly.
- [x] `Blob` preserves ciphertext bytes and 12-byte nonce through clone/debug/equality where applicable.
- [x] Public constructors/defaults, if added, produce spec-compliant values.

## Milestone 2: Crypto primitives

Implement encryption/decryption and ECDH wrapping from the spec.

TODO:

- [x] Add `core/src/crypto.rs`.
- [x] Implement `generate_data_key() -> [u8; 32]`.
- [x] Implement `encrypt_blob(task, key) -> Result<Blob>`.
- [x] Implement `decrypt_blob(blob, key) -> Result<Task>`.
- [x] Implement device keypair generation.
- [x] Implement `wrap_data_key` with ECDH P-256 + HKDF-SHA256 + AES-256-GCM.
- [x] Implement `unwrap_data_key`.

Interface tests to add before leaving this milestone:

- [x] `generate_data_key` returns exactly 32 bytes.
- [x] Consecutive generated data keys differ.
- [x] `encrypt_blob` returns non-empty ciphertext and exactly 12-byte nonce.
- [x] Encrypting the same task twice with the same key produces different nonces/ciphertext.
- [x] `decrypt_blob(encrypt_blob(task, key), key)` returns the original task.
- [x] `encrypt_blob` rejects keys that are not 32 bytes.
- [x] `decrypt_blob` rejects keys that are not 32 bytes.
- [x] Decrypting with the wrong key fails as `DecryptFailed`.
- [x] Tampered ciphertext fails as `DecryptFailed`.
- [x] Tampered nonce fails as `DecryptFailed`.
- [x] Device public keys are stable encoded bytes accepted by the peer unwrap path.
- [x] `DeviceKeypair` debug output redacts private key bytes.
- [x] A data key wrapped by device A for device B unwraps correctly on device B.
- [x] Unwrap with the wrong private key fails.
- [x] Unwrap with malformed public key bytes fails cleanly.

## Milestone 3: Local SQLite database

Implement local-first task persistence.

TODO:

- [x] Add `core/src/db.rs`.
- [x] Create schema initialization for:
  - [x] `tasks`
  - [x] `tasks_fts`
  - [x] `sync_cursor`
  - [x] `sync_queue`
- [x] Add task row serialization helpers for status, tags, UUIDs, and booleans.
- [x] Implement local CRUD:
  - [x] create task
  - [x] get task
  - [x] update task
  - [x] tombstone delete
  - [x] list tasks
  - [x] search tasks

Interface tests to add before leaving this milestone:

- [x] Opening a new DB initializes all required tables and indexes.
- [x] Opening an already-initialized DB is idempotent.
- [x] `create_task` writes a task with generated UUID, timestamps, and `dirty=true`.
- [x] `get_task` returns exactly the created task.
- [x] `update_task` changes only patched fields, bumps `updated_at`, and sets `dirty=true`.
- [x] `delete_task` tombstones instead of hard deleting.
- [x] `list_tasks` excludes deleted tasks by default unless explicitly requested.
- [x] `list_tasks` filters by status.
- [x] `list_tasks` filters by project.
- [x] `list_tasks` filters by due-date range.
- [x] `list_tasks` sort modes are deterministic.
- [x] `search_tasks` finds title matches.
- [x] `search_tasks` finds body matches.
- [x] FTS rows stay updated after create, update, and delete.
- [x] Tags serialize and deserialize as JSON arrays.
- [x] Invalid UUID/status/tag JSON rows produce clear DB errors.

## Milestone 4: Core facade

Expose a simple high-level Rust API that platform shells can call.

TODO:

- [x] Add `core/src/core.rs` with a `TaskManagerCore` struct.
- [x] Add constructor that accepts DB path.
- [x] Wire CRUD methods through the DB layer.
- [x] Keep network and sync out of the facade for now.

Interface tests to add before leaving this milestone:

- [x] Constructor opens/creates the configured DB path.
- [x] Constructor fails clearly on invalid DB path.
- [x] Facade `create_task`, `get_task`, `update_task`, `delete_task`, `list_tasks`, and `search_tasks` delegate correctly.
- [x] Facade methods preserve the same error semantics as the DB layer.
- [x] Multiple facade instances over the same DB path observe consistent state.

## Milestone 5: Platform trait

Define platform boundary without importing platform SDKs.

TODO:

- [x] Add `core/src/platform.rs`.
- [x] Define the `Platform` trait from the spec.
- [x] Add a test/mock platform implementation.
- [x] Implement `init_device_keypair` storing private key through `Platform`.
- [x] Implement `init_account` generating and storing the account data key.

Interface tests to add before leaving this milestone:

- [x] Mock platform can store, load, and delete keys.
- [x] Loading a missing key returns a clear error.
- [x] `init_device_keypair` stores private key and returns public key.
- [x] `init_device_keypair` does not expose private key bytes in its return value.
- [x] `init_account` stores the data key through the platform.
- [x] `init_account` returns the device public key.
- [x] Notification scheduling and cancellation calls are forwarded with correct task ID, time, and title.
- [x] `network_available` result is observable through the platform interface.

## Milestone 6: Settings

Implement plaintext and encrypted vault settings locally.

TODO:

- [x] Add `core/src/settings.rs`.
- [x] Define `PlaintextSettings`.
- [x] Define `VaultSettings`.
- [x] Implement local JSON file read/write for plaintext settings.
- [x] Implement vault settings as the reserved local blob/task id `vault_settings`.
- [x] Add migration hooks for future schema versions.

Interface tests to add before leaving this milestone:

- [x] Plaintext settings serialize to the documented JSON shape.
- [x] Plaintext settings deserialize from the documented JSON shape.
- [x] Missing plaintext settings file returns defaults or a clear first-run state.
- [x] Saving plaintext settings excludes device-local `last_sync_cursor` from server sync payloads.
- [x] Vault settings serialize to the documented JSON shape.
- [x] Vault settings encrypt/decrypt with the account data key.
- [x] Vault settings use reserved ID `vault_settings`.
- [x] Vault settings conflict resolution uses last-write-wins.
- [x] Unknown future schema version produces a migration/error path, not silent corruption.

## Milestone 7: Sync interfaces

Add sync abstractions before concrete HTTP implementation.

TODO:

- [ ] Add `core/src/sync.rs`.
- [ ] Define an HTTP/client trait for server interaction.
- [ ] Implement `sync_push` using dirty local rows.
- [ ] Clear dirty flags only after confirmed success.
- [ ] Implement `sync_pull` using decrypted remote blobs.
- [ ] Implement last-write-wins conflict resolution.
- [ ] Implement retry queue persistence.

Interface tests to add before leaving this milestone:

- [ ] `sync_push` sends only dirty, non-deleted tasks as encrypted blobs.
- [ ] `sync_push` sends tombstones through the delete path.
- [ ] Successful push clears `dirty` only for confirmed task IDs.
- [ ] Partial batch failure leaves failed tasks dirty.
- [ ] Network unavailable returns `NetworkUnavailable` and queues retry.
- [ ] Auth failure returns `AuthExpired` without clearing dirty flags.
- [ ] Server error returns status and body.
- [ ] `sync_pull` decrypts remote blobs and upserts local tasks.
- [ ] `sync_pull` advances cursor only after successful processing.
- [ ] Pull decryption failure does not advance cursor.
- [ ] Last-write-wins chooses higher payload `updated_at`.
- [ ] Equal timestamp conflict is deterministic.
- [ ] Retry queue persists across DB reopen.
- [ ] Exponential backoff increases attempts and next retry time.

## Milestone 8: UniFFI boundary

Expose only stable core APIs to platform shells.

TODO:

- [ ] Add `core/uniffi/core.udl`.
- [ ] Map Rust types into UniFFI-compatible records/enums.
- [ ] Add exported constructor and CRUD methods.

Interface tests to add before leaving this milestone:

- [ ] UDL contains every public API intended for Swift/Kotlin.
- [ ] UDL does not expose internal-only APIs.
- [ ] Generated Swift bindings compile.
- [ ] Generated Kotlin bindings compile.
- [ ] FFI constructors and CRUD methods round-trip representative values.
- [ ] FFI error mapping preserves meaningful error names/messages.
- [ ] UUID, timestamp, nullable fields, byte arrays, and string lists cross the FFI boundary correctly.

## Current immediate next step

Start with **Milestone 1** only: crate foundation and readable shared types. Do not implement DB, crypto, sync, or UniFFI yet.
