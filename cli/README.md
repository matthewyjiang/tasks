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

## Server status

`--server` is accepted and stored in `CliContext`, but server connection UX is not implemented yet. The following remain future work:

- persistent server URL settings
- server auth refresh/login integration
- device register/list against the server
- sync push/pull/run/status

## Headless reminders

The CLI platform can persist scheduled reminders for headless tests when explicitly configured:

```sh
export TASKMANAGER_REMINDER_DIR=/tmp/taskmanager-reminders
```

Reminder commands are not exposed directly yet; this backs core/platform integration.
