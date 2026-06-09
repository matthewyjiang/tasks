# CLI implementation plan

Goal: build the Rust `taskmanager` CLI incrementally as a complete terminal client for `core/` and as the canonical autonomous integration-test harness for the core ⇄ server pipeline. Each milestone should compile, expose a small tested interface, and avoid duplicating core business logic.

## Guiding rules

- Implement one command group at a time.
- Add tests in the same milestone as each feature.
- Keep the CLI thin: argument parsing, output formatting, platform adaptation, and orchestration only.
- Call public `core` APIs only; do not reach into private core modules or duplicate SQL/crypto logic.
- Prefer deterministic JSON output for tests and scripts; table/text output can follow once JSON is stable.
- Every command must be non-interactive when all required flags are provided.
- Every state path must be injectable for tests (`--profile`, `--config`, `--db`, env vars).
- Add CI early and keep it green before expanding command coverage.
- Update release automation when the CLI crate is introduced so it has its own artifact tag stream.

## Milestone 1: CLI crate foundation and CI

Create the crate, command skeleton, and CI before implementing business commands.

TODO:

- [x] Add `cli/Cargo.toml` as a Rust binary crate depending on `core = { path = "../core" }`.
- [x] Add `cli/src/main.rs` and `cli/src/lib.rs`.
- [x] Add module skeletons:
  - [x] `args.rs` for `clap` definitions.
  - [x] `output.rs` for JSON/table output.
  - [x] `error.rs` for CLI error mapping and exit codes.
  - [x] `context.rs` for resolved paths, profile, server URL, offline mode, and output mode.
  - [x] `platform.rs` for the CLI `Platform` implementation placeholder.
- [x] Add root workspace member `"cli"`.
- [x] Add `.github/workflows/cli.yml` running `cargo fmt`, `cargo clippy`, and `cargo test -p taskmanager-cli` on `cli/**`, `core/**`, `Cargo.toml`, and `Cargo.lock`.
- [x] Update `.github/workflows/release.yml` with a new `cli` artifact path.
- [x] Update `RELEASE.md` to document `cli-vX.Y.Z` tags and local dry-run command.

Interface tests to add before leaving this milestone:

- [x] `taskmanager --help` exits successfully.
- [x] `taskmanager --version` exits successfully.
- [x] Unknown command exits with code `1`.
- [x] `--output json` is accepted globally.
- [x] Invalid `--output` value exits with code `1`.
- [x] `--profile`, `--config`, `--db`, `--server`, and `--offline` resolve into `CliContext` correctly.
- [x] CLI errors serialize to the stable JSON error shape when JSON output is selected.
- [x] Exit-code mapping covers input, DB/key-store, crypto, network, conflict, and unsupported-platform errors.
- [ ] CI workflow syntax validates.
- [x] Release workflow dry-run includes `cli` without affecting `server`, `core`, or `app` artifacts.

## Milestone 2: Output contract and test harness utilities

Stabilize command output and integration-test helpers before adding many commands.

TODO:

- [x] Implement `OutputFormat::{Json, Jsonl, Table}`.
- [x] Implement stable `CommandResult<T>` JSON envelope where useful.
- [x] Implement stderr JSON errors for `--output json`.
- [x] Add test helpers for invoking the compiled binary with temp config, DB, and key directories.
- [x] Add fixture helpers for isolated profiles.
- [x] Add snapshot-style assertions for JSON command output.

Interface tests to add before leaving this milestone:

- [x] JSON output contains only deterministic fields for a fixed fixture.
- [x] JSONL output emits one valid JSON object per line.
- [x] Table output is human-readable and does not affect JSON tests.
- [x] `--quiet` suppresses non-result messages.
- [x] `--trace` writes logs to stderr, not stdout.
- [x] Temp-profile fixture creates isolated config, DB, and key-store paths.

## Milestone 3: CLI platform implementation

Provide the desktop/headless `Platform` implementation required by `core`.

TODO:

- [ ] Implement OS key-store selection for supported desktop platforms.
- [x] Implement explicit insecure file-backed key store selected only by `TASKMANAGER_INSECURE_KEY_DIR` or a test-only flag.
- [x] Implement `store_key`, `load_key`, and `delete_key`.
- [x] Implement headless reminder persistence for `schedule_notification` and `cancel_notification`.
- [x] Implement `network_available` honoring `--offline` first.
- [x] Add clear errors for unsupported platform capabilities.

Interface tests to add before leaving this milestone:

- [x] File-backed test key store round-trips key bytes.
- [x] Missing key returns the expected key-store error.
- [x] `delete_key` removes only the selected key.
- [x] Insecure key store is never selected implicitly.
- [x] Reminder schedule/cancel persists expected records in headless mode.
- [x] `--offline` forces `network_available == false`.

## Milestone 4: Local task commands

Implement offline task management against the local DB through `core`.

TODO:

- [x] Add `task create`.
  - Note: create supports project and tags by creating through `TaskManagerCore::create_task`, then applying `TaskManagerCore::update_task` for fields not accepted by the current core create API.
- [x] Add `task get`.
- [x] Add `task update`.
- [x] Add `task delete` tombstone command.
- [x] Add `task list` with all `TaskFilter` and `TaskSort` variants.
- [x] Add `task search`.
- [x] Add `task complete` and `task reopen` status helpers.

Interface tests to add before leaving this milestone:

- [x] Creating a task returns a JSON `Task` with generated UUID and `dirty=true`.
- [x] Getting an existing task returns the same task.
- [x] Getting a missing task exits with a not-found error.
- [x] Updating each patchable field persists and marks `dirty=true`.
- [x] Deleting a task creates a tombstone, not a hard delete.
- [ ] Listing supports status, project, tag, due-range, deleted/include-deleted filters.
  - Note: tag list filtering remains blocked until core `TaskFilter` exposes a tag filter.
- [x] Sorting is stable for every supported sort variant.
- [x] Search returns matches from title and body.
- [x] Complete/reopen map to the correct status patches.
- [x] All task commands work with `--offline`.

## Milestone 5: Account, auth, and device commands

Add bootstrap and device-pairing command coverage.

TODO:

- [x] Add `account init`.
- [ ] Add `auth login`, `auth refresh`, and `auth logout`.
  - Note: `auth login` and `auth logout` are implemented for local token storage; `auth refresh` remains blocked until server auth is wired.
- [x] Add `device init-keypair`.
- [ ] Add `device register`.
  - Note: blocked until server auth/device directory commands are wired.
- [ ] Add `device list`.
  - Note: blocked until server auth/device directory commands are wired.
- [x] Add `device wrap-key --target <device_id>`.
- [x] Add `device unwrap-key --from <device_id>`.

Interface tests to add before leaving this milestone:

- [x] `account init` initializes local state and returns a device public key.
- [x] Re-running `account init` is idempotent or returns a stable `already_exists` error.
- [x] Auth token storage uses the platform key store.
- [x] `auth logout` removes stored tokens without deleting local tasks.
- [x] Device keypair generation stores private key and prints only public key.
- [x] Device commands never print secret key material by default.
- [x] Wrap on profile A and unwrap on profile B recovers the same data key.
- [x] Malformed peer public key fails with a crypto error exit code.

## Milestone 6: Sync commands

Expose the sync engine and local sync diagnostics.

TODO:

- [ ] Add `sync push`.
- [ ] Add `sync pull [--since <cursor>]`.
- [ ] Add `sync run`.
- [x] Add `sync status`.
- [x] Add `sync retry <task_id>`.
- [ ] Add `sync conflicts`.
- [ ] Add `sync resolve <task_id> --local|--remote|--json <patch>`.

Interface tests to add before leaving this milestone:

- [x] `sync status` reports dirty row count, retry queue depth, and cursor.
- [ ] `sync push` clears `dirty` only for server-confirmed blobs.
- [ ] Network failure preserves dirty rows and queues retries.
- [ ] `sync pull` advances cursor only after successful decrypt/upsert.
- [ ] `sync run` produces deterministic JSON summary.
- [x] `sync retry` updates queue state for the selected task.
- [ ] Conflict commands report and resolve conflicts through core APIs.

## Milestone 7: Settings commands

Add plaintext and vault settings management.

TODO:

- [x] Add `settings get [key]`.
- [x] Add `settings set <key> <value>`.
- [x] Add `settings pull-plaintext`.
- [x] Add `settings push-plaintext`.
- [x] Add `settings migrate`.

Interface tests to add before leaving this milestone:

- [x] Plaintext settings can be read before unlocking/opening the encrypted vault.
- [x] Setting `server_url`, `auth_method`, `language`, and `last_sync_cursor` validates types.
- [ ] Vault settings update marks the vault settings blob dirty.
- [ ] Vault settings encrypt/decrypt through the normal blob path.
- [x] `last_sync_cursor` remains device-local and is not overwritten by server settings pull.
- [x] Schema migration writes current defaults when no file exists.

## Milestone 8: Sharing commands

Expose shared-task workflows.

TODO:

- [ ] Add `share create <task_id> --recipient <user_or_device>`.
- [ ] Add `share inbox`.
- [ ] Add `share accept <share_id>`.
- [ ] Add `share revoke <task_id> --recipient <id>`.
- [ ] Add `share list <task_id>`.

Interface tests to add before leaving this milestone:

- [ ] Sharing a task switches it to a per-task key and re-encrypts the blob.
- [ ] Recipient can unwrap the task key and decrypt the shared task.
- [ ] Share inbox output is stable JSON.
- [ ] Revocation deletes recipient access and rotates the task key.
- [ ] Remaining collaborators receive re-wrapped access after rotation.
- [ ] Revoked recipient cannot decrypt newly synced ciphertext.

## Milestone 9: Crypto diagnostic commands

Add development/test diagnostics with strict secret-output controls.

TODO:

- [x] Add `crypto encrypt-task <task_id>`.
- [x] Add `crypto decrypt-blob <file>`.
- [x] Add `crypto wrap-data-key`.
- [x] Add `crypto unwrap-data-key`.
- [x] Add `crypto verify-local`.
- [x] Add `--dangerously-print-secrets` gate for commands that can reveal secret material.

Interface tests to add before leaving this milestone:

- [x] Encrypt/decrypt diagnostics round-trip a fixture task.
- [x] Diagnostics reject malformed blobs and keys cleanly.
- [x] Secret material is redacted by default.
- [x] Secret material is printed only when `--dangerously-print-secrets` is supplied.
- [x] `crypto verify-local` detects missing key material and verifies encrypt/decrypt.

## Milestone 10: Black-box core ⇄ server integration suite

Use the CLI as the autonomous end-to-end test harness.

TODO:

- [ ] Add test support to start a disposable server and database.
- [ ] Add helpers to create multiple isolated CLI profiles.
- [ ] Add `sync run --until-quiescent --timeout <duration>` if needed for reliable tests.
- [ ] Add CI job or matrix entry for CLI integration tests against the server.
- [ ] Store logs/artifacts on failure without leaking plaintext secrets unless explicitly configured.

Integration tests to add before leaving this milestone:

- [ ] Account bootstrap: init account, create task offline, push, verify server stores only opaque ciphertext.
- [ ] Device pairing: second profile registers, first wraps data key, second unwraps, second pulls and decrypts tasks.
- [ ] Conflict path: two profiles edit the same task offline, sync both, verify configured resolution.
- [ ] Tombstone path: delete task locally, sync tombstone, pull deletion on another profile.
- [ ] Settings path: update plaintext and vault settings, sync, pull on another profile.
- [ ] Sharing path: share a task, accept on recipient profile, revoke, rotate task key, verify revoked recipient fails on new ciphertext.

## Milestone 11: Packaging and release polish

Prepare the CLI as an independently shipped artifact.

TODO:

- [ ] Add release build profile checks for the CLI binary.
- [ ] Add generated shell completions for bash, zsh, fish, and PowerShell.
- [ ] Add man page or markdown command reference generated from `clap`.
- [ ] Add install/archive steps to release workflow if binary artifacts are desired.
- [ ] Confirm artifact-prefixed tags use only `cli-vX.Y.Z` for CLI releases.

Tests/checks to add before leaving this milestone:

- [ ] Release dry-run creates only `cli-vX.Y.Z` for CLI-only releasable commits.
- [ ] CLI-only commits do not trigger `server-v*`, `core-v*`, or `app-v*` releases.
- [ ] Completion generation succeeds for every supported shell.
- [ ] Packaged binary prints the same version as the release tag.
