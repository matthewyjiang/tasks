# CLI installation and usage

The Rust CLI package is `taskmanager-cli`; the installed binary is `tsk`.

## Install

From the repository root:

```sh
make cli-install
```

Equivalent Cargo command:

```sh
cargo install --path cli --force
```

Ensure Cargo's bin directory is on your `PATH`:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
tsk --help
```

Run without installing:

```sh
make cli-run -- --help
# or
cargo run -p taskmanager-cli -- --help
```

Uninstall:

```sh
make cli-uninstall
```

## Common flags

```text
--profile <name>             Profile name, defaults to default
--config <path>              Plaintext settings path
--db <path>                  Local SQLite DB path
--server <url>               Override configured server URL for sync commands
--output <table|json|jsonl>  Output format, defaults to table
--quiet                      Suppress non-result messages
--yes                        Assume yes for future confirmations
--offline                    Force offline mode
--trace                      Write trace diagnostics to stderr
```

If `--db` is omitted, task commands use:

```text
~/.taskmanager/profiles/<profile>/tasks.db
```

## Local task commands

```sh
tsk task create --title "write tests" --body "cover CLI" --due tomorrow --tag work
tsk task list
tsk task get <task_id>
tsk task update <task_id> --status open
tsk task complete <task_id>
tsk task reopen <task_id>
tsk task search "literal text"
tsk task delete <task_id>
```

`task delete` creates a tombstone through core; it does not hard-delete the row.

## Configure and sync

`tsk configure` is the normal first-run command. It creates local account keys, saves the server URL, and registers/logs in with email and password.

```sh
tsk configure \
  --server-url http://127.0.0.1:18080 \
  --email you@example.com \
  --password "$TASKMANAGER_PASSWORD"
```

Then work locally and sync encrypted blobs:

```sh
tsk task create --title "Plan launch" --tag work
tsk sync status
tsk sync push
tsk sync pull
tsk sync run
```

## Generated artifacts

Shell completions and a man page can be generated from the CLI definition:

```sh
tsk generate completion bash > tsk.bash
tsk generate completion zsh > _tsk
tsk generate completion fish > tsk.fish
tsk generate completion powershell > tsk.ps1
tsk generate man > tsk.1
```

## Development checks

```sh
cargo fmt --check
cargo test -p taskmanager-cli
cargo clippy -p taskmanager-cli --all-targets
./scripts/cli_e2e.py
```
