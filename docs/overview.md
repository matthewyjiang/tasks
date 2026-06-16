# Overview

`tsk` is a local-first, end-to-end encrypted task manager.

## Goals

- Keep task creation, editing, search, completion, and deletion usable while offline.
- Store task data locally in client databases and sync encrypted blobs when a server is available.
- Keep server-side sync zero-knowledge: the server stores accounts, sessions, device public keys, cursors, and encrypted blobs, but not plaintext task contents.
- Share task, sync, auth, crypto, conflict, and enrollment semantics through the native Rust `taskmanager-core` crate.

## Repository structure

- `core/` — Rust client core library, local data model, crypto, sync orchestration, and UniFFI exports.
- `cli/` — Rust command-line client named `tsk`.
- `linux/` — GTK/libadwaita Linux desktop app, binary `tsk-gui`.
- `ios/` — SwiftUI iOS client backed by generated UniFFI bindings.
- `server/` — Go zero-knowledge sync backend.
- `packaging/` — platform packaging metadata.
- `scripts/` — release and integration-test automation.

## Current client status

The CLI is the primary supported command-line surface for local tasks, account setup, auth diagnostics, low-level device key operations, and encrypted sync. The Linux and iOS apps use the shared core for local-first task storage, encrypted sync, token refresh, and reminders through platform-specific adapters.
