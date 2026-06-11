# Tasks Linux app

GTK/libadwaita desktop client for the local-first encrypted task manager.

## Build and run

From the repository root:

```sh
cargo run -p tsk-linux
```

The Cargo package is `tsk-linux`; the GUI binary is `tsk-gui`.

## Sync and auth

The app uses the same zero-knowledge blob sync protocol as the CLI and server. Account/device keys and auth tokens are stored in the Linux platform key store.

During sync, the app uses the stored access token. If the server rejects it as expired, the app calls `/auth/refresh` with the stored refresh token, saves the returned rotated token pair, and retries the sync once. Manual sign-in is only required when refresh fails, such as an expired or revoked refresh token.

Failed outbound task changes stay dirty and are recorded in the SQLite `sync_queue` with exponential backoff metadata, so pending retries survive app restart and are retried by later sync runs. Conflicts are resolved automatically with the core last-write-wins policy (`updated_at`, then a deterministic id tie-breaker); the Linux UI surfaces the number of automatically resolved conflicts and pending retry entries in sync status text.

## Development checks

```sh
cargo fmt --check
cargo check -p tsk-linux
```
