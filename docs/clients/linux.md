# Linux app

The Linux desktop client is a GTK/libadwaita app for the local-first encrypted task manager. Its Cargo package is `tsk-linux`; the GUI binary is `tsk-gui`.

## Official packages

The official Linux package currently published by this project is the Arch package `tsk-linux`. It is built from `linux-app-v*` releases and published to the custom package repository at [repo.matthewyjiang.com](https://repo.matthewyjiang.com/).

After adding that repository to your package manager, install `tsk-linux`:

```sh
sudo pacman -S tsk-linux
```

Flatpak packaging is in progress.

## Run from source

From the repository root:

```sh
cargo run -p tsk-linux
```

The app stores account/device keys and auth tokens in the Linux platform key store.

## Sync and auth

The Linux app uses the same zero-knowledge blob sync protocol as the CLI and server. During sync, it uses the stored access token. If the server rejects it as expired, the app calls `/auth/refresh` with the stored refresh token, saves the rotated token pair, and retries sync once.

Failed outbound task changes stay dirty with retry metadata, so pending retries survive app restart. Conflicts are resolved automatically by shared core and surfaced in sync status text where implemented.

## Reminders

Reminder intent is stored in core as encrypted task data. The Linux app schedules open, non-deleted tasks at `due_at - reminder_offset_ms` and cancels reminders when tasks are done, deleted, missing due dates, or have reminders disabled.

Persistent scheduling uses per-task `systemd --user` timers. Only the task id is stored in the timer; the task title is read from the local database when the reminder fires.

## Development notes

Contributor checks and lower-level implementation notes live in [Linux development](../development/linux.md) and the repository's `linux/README.md`.
