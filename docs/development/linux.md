# Linux app

The Linux desktop client is a GTK/libadwaita app. Its Cargo package is `tsk-linux`; the GUI binary is `tsk-gui`.

## Run

From the repository root:

```sh
cargo run -p tsk-linux
```

## Sync and auth

The app uses the same zero-knowledge blob sync protocol as the CLI and server. Account/device keys and auth tokens are stored in the Linux platform key store.

During sync, the app uses the stored access token. If the server rejects it as expired, the app calls `/auth/refresh` with the stored refresh token, saves the rotated token pair, and retries sync once.

Failed outbound task changes stay dirty and are recorded in SQLite sync metadata with exponential backoff, so pending retries survive app restart. Conflicts are resolved automatically by core and surfaced in sync status text where implemented.

## Reminders

Reminder intent is stored in core as encrypted task data. The Linux app schedules open, non-deleted tasks at `due_at - reminder_offset_ms` and cancels reminders when tasks are done, deleted, missing due dates, or have reminders disabled.

Persistent scheduling uses per-task `systemd --user` timers. The timer calls `tsk-gui --emit-reminder <task-id>`, which reopens the local database and validates task state before showing a desktop notification.

## Checks

```sh
cargo fmt --check
cargo check -p tsk-linux
```
