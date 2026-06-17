# Linux app

The Linux app is the desktop client for `tsk`. It uses GTK/libadwaita and is intended to feel like a native Linux task manager while preserving the same local-first encrypted model as the other clients.

## Download

The official Linux package currently published by this project is the Arch package `tsk-linux`.

It is available from the custom package repository at [repo.matthewyjiang.com](https://repo.matthewyjiang.com/). Add that repository to your package manager, then install the `tsk-linux` package.

Flatpak packaging is in progress.

## What it provides

- Local task management on your desktop.
- Encrypted sync with a compatible `tsk` server.
- Account and device secrets stored in the Linux platform key store.
- Automatic access-token refresh during sync when a valid refresh token is available.
- Reminder support through the desktop environment.

## Why use it

Use the Linux app when you want a graphical, platform-native way to manage tasks on a Linux desktop. It is the preferred interactive Linux surface; the CLI remains available alongside it for terminal and automation workflows.

Contributor checks and implementation notes live in [Linux development](../development/linux.md) and the repository's `linux/README.md`.
