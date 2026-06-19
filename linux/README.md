# Tasks Linux app

GTK/libadwaita desktop client for the local-first encrypted task manager.

For the user-facing app page, start with [`docs/clients/linux.md`](../docs/clients/linux.md). This README keeps package-specific run, sync, reminder, and development notes close to the Linux source.

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

## Settings

Local plaintext settings store the sync server URL, account email, theme choice, and last sync status. Encrypted vault settings store cross-device task preferences: default sort (`due_at_asc` by default), completed-task visibility, display density (`comfortable`), first day of week (`1`/Monday), notification sound (`default`), default reminder (`30` minutes before due), and keybindings.

## Reminders

Task reminder intent is stored in core as an encrypted `reminder_offset_ms` value on each task. The Linux app schedules open, non-deleted tasks at `due_at - reminder_offset_ms` and cancels reminders when a task is done, deleted, has no due date, or has reminders disabled.

Persistent scheduling uses per-task `systemd --user` timer units. The timer calls the hidden helper mode `tsk-gui --emit-reminder <task-id>`, which reopens the local database and validates the task state before showing a desktop notification with `notify-rust`/the desktop notification service. Only the task id is stored in the unit file; the task title is read at fire time.

## Development checks

```sh
cargo fmt --check
cargo check -p tsk-linux
```
