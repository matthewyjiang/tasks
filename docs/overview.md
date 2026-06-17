# Overview

`tsk` is a local-first, end-to-end encrypted task manager.

## Goals

- Keep task creation, editing, search, completion, and deletion usable while offline.
- Store task data locally in client databases and sync encrypted blobs when a server is available.
- Keep server-side sync zero-knowledge: the server stores accounts, sessions, device public keys, cursors, and encrypted blobs, but not plaintext task contents.
- Share task, sync, auth, crypto, conflict, and enrollment semantics through the native Rust `taskmanager-core` crate.

## Choose your path

- [Choose a client](./getting-started.md) for the platform where you want to manage tasks.
- [Compare clients](./clients/index.md) to see the current Linux, iOS, and CLI support status.
- [Use the CLI](./cli.md) for command examples, common flags, output formats, scripted workflows, diagnostics, and sync commands.
- [Run a sync server](./server.md) when you want self-hosted encrypted blob sync.
- [Review known limitations](./roadmap.md) before depending on a workflow that is still evolving.
- [Read the architecture](./architecture.md) or [security model](./security.md) when you want implementation and trust-boundary details.

## Current client status

The Linux and iOS apps provide platform-native task management on top of the shared core. The CLI provides the command-line surface for local tasks, account setup, auth diagnostics, low-level device key operations, scripting, machine-readable output, and encrypted sync.

## Repository structure

- `core/` — Rust client core library, local data model, crypto, sync orchestration, and UniFFI exports.
- `cli/` — Rust command-line client named `tsk`.
- `linux/` — GTK/libadwaita Linux desktop app, binary `tsk-gui`.
- `ios/` — SwiftUI iOS client backed by generated UniFFI bindings.
- `server/` — Go zero-knowledge sync backend.
- `packaging/` — platform packaging metadata.
- `scripts/` — release and integration-test automation.
