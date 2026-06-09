# Taskmanager CLI

Rust command-line client for the local-first encrypted task manager.

The CLI is intended for both human terminal use and deterministic integration testing. It is thin by design: command parsing, output formatting, path resolution, and platform/key-store adaptation live here; task and crypto behavior comes from `taskmanager-core`.

## Build and test

From the repository root:

```sh
cargo build -p taskmanager-cli
cargo test -p taskmanager-cli
cargo clippy -p taskmanager-cli --all-targets
```

Run locally:

```sh
cargo run -p taskmanager-cli -- --help
```

## Global flags

```text
--profile <name>           Profile name, defaults to default
--config <path>            Config path, reserved for settings support
--db <path>                Local SQLite DB path
--server <url>             Server URL, accepted but not wired to sync/auth yet
--output <table|json|jsonl> Output format, defaults to table
--quiet                    Suppress non-result messages
--yes                      Assume yes for future confirmations
--offline                  Force offline mode
--trace                    Write trace diagnostics to stderr
```

If `--db` is omitted, task commands use:

```text
~/.taskmanager/profiles/<profile>/tasks.db
```

## Output modes

Human output is the default:

```sh
taskmanager task list
```

Machine-readable JSON uses a stable envelope:

```sh
taskmanager --output json version
```

```json
{
  "result": {
    "name": "taskmanager-cli",
    "version": "0.1.0"
  }
}
```

Errors are written to stderr. With JSON/JSONL output selected, errors use:

```json
{"error":{"code":"input_error","message":"...","details":null}}
```

## Local task commands

Task commands use `taskmanager_core::TaskManagerCore` against the local DB.

```sh
taskmanager --db /tmp/tasks.db task create --title "write tests" --body "cover CLI" --due 1717603200000 --tag work --tag urgent

taskmanager --db /tmp/tasks.db task list

taskmanager --db /tmp/tasks.db task get <task_id>

taskmanager --db /tmp/tasks.db task update <task_id> --status in-progress --project-id <uuid>

taskmanager --db /tmp/tasks.db task complete <task_id>

taskmanager --db /tmp/tasks.db task reopen <task_id>

taskmanager --db /tmp/tasks.db task search "literal text"

taskmanager --db /tmp/tasks.db task delete <task_id>
```

`task delete` creates a tombstone through core; it does not hard-delete the row.

## Account, auth, and device key commands

These commands currently operate locally through the CLI platform key store. Server-backed auth/device registration is not wired yet.

For headless/dev/test usage, explicitly opt into the insecure file-backed key store:

```sh
export TASKMANAGER_INSECURE_KEY_DIR=/tmp/taskmanager-profile-a/keys
```

Initialize account keys:

```sh
taskmanager --output json account init
```

Initialize only a device keypair:

```sh
taskmanager --output json device init-keypair
```

Store/remove auth tokens locally:

```sh
taskmanager --output json auth login --access-token <token> --refresh-token <token>
taskmanager --output json auth logout
```

Wrap the account data key for another device public key:

```sh
taskmanager --output json device wrap-key --target <recipient_public_key_hex>
```

Unwrap and store an account data key from another device:

```sh
taskmanager --output json device unwrap-key \
  --from <sender_public_key_hex> \
  --ciphertext <wrapped_ciphertext_hex> \
  --nonce <nonce_hex>
```

Commands intentionally never print private key or account data key material.

## Settings commands

Plaintext settings are stored per profile, or at `--config <path>` when supplied. They can be read before opening the encrypted vault:

```sh
taskmanager --config /tmp/settings.json --output json settings get
taskmanager --config /tmp/settings.json --output json settings get server_url
taskmanager --config /tmp/settings.json --output json settings set server_url https://api.example.com
taskmanager --config /tmp/settings.json --output json settings set auth_method pin
taskmanager --config /tmp/settings.json --output json settings set language en
taskmanager --config /tmp/settings.json --output json settings set last_sync_cursor 42
```

Syncable plaintext settings exclude the device-local `last_sync_cursor`:

```sh
taskmanager --config /tmp/settings.json --output json settings pull-plaintext
taskmanager --config /tmp/settings.json --output json settings push-plaintext '{"schema_version":1,"server_url":"https://api.example.com","auth_method":"password","language":"en"}'
taskmanager --config /tmp/settings.json --output json settings migrate
```

## Crypto diagnostics

Development diagnostics use existing local account/device keys. Commands that can reveal raw secret material require `--dangerously-print-secrets`.

```sh
taskmanager crypto verify-local
taskmanager --output json crypto encrypt-task <task_id> > blob.json
taskmanager --output json crypto decrypt-blob blob.json
taskmanager --output json crypto wrap-data-key --target <peer_public_key>
taskmanager --dangerously-print-secrets --output json crypto unwrap-data-key \
  --from <peer_public_key> \
  --ciphertext <hex> \
  --nonce <hex>
```

## Server status

`--server` is accepted and stored in `CliContext`, but server connection UX is not implemented yet. The following remain future work:

- persistent server URL settings
- server auth refresh/login integration
- device register/list against the server
- sync push/pull/run

Local sync diagnostics are available with `sync status` and `sync retry`.

## Headless reminders

The CLI platform can persist scheduled reminders for headless tests when explicitly configured:

```sh
export TASKMANAGER_REMINDER_DIR=/tmp/taskmanager-reminders
```

Reminder commands are not exposed directly yet; this backs core/platform integration.

## Local CLI/server E2E suite

Run the black-box CLI pipeline from the repository root:

```sh
./scripts/cli_e2e.py
```

The suite:

1. Builds `taskmanager-cli`.
2. Starts a fresh PostgreSQL container with `docker compose down -v && docker compose up -d postgres` under `server/`.
3. Starts the Go server with test-only environment values on `http://127.0.0.1:18080`.
4. Runs the compiled CLI with isolated temp profiles and `--server http://127.0.0.1:18080`.
5. Exercises the currently implemented CLI interfaces and edge cases, including task create/get/update/delete/list/search/complete/reopen, output modes, account init idempotency, auth token storage/logout, device wrap/unwrap, malformed hex input, sync diagnostics/retry, and unsupported server-backed commands.

This test should be extended whenever new CLI features are added, especially once auth/device registration and sync push/pull start using the server.

Requirements:

- Docker with Compose v2
- Go
- Rust/Cargo

Optional environment overrides:

```sh
TASKMANAGER_E2E_SERVER_URL=http://127.0.0.1:18080 \
TASKMANAGER_E2E_DATABASE_URL='postgres://tasks:tasks@localhost:5432/tasks?sslmode=disable' \
TASKMANAGER_E2E_JWT_SECRET='taskmanager-cli-e2e-test-secret-change-me-32-bytes' \
./scripts/cli_e2e.py
```
