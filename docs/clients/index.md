# Clients

`tsk` shares task, sync, auth, crypto, conflict, and reminder behavior through the Rust `taskmanager-core` crate. Choose the client that fits the platform where you want to manage tasks; the graphical clients are the primary interactive surfaces, and the CLI is available for terminal, automation, diagnostics, and integration workflows.

## Current status

| Client | Status | Best for |
| --- | --- | --- |
| [Linux app](./linux.md) | Desktop client backed by shared core | GTK/libadwaita local-first task management, sync, and reminders |
| [iOS app](./ios.md) | Native SwiftUI client in active development | iOS local-first task management, foreground/background sync foundations, and reminders |
| [CLI](../cli.md) | Command-line client | Terminal task commands, account setup, diagnostics, scripting, machine-readable output, and encrypted sync |

The Linux and iOS apps use the same encrypted sync model as the CLI. Some clients may still require local development setup until broader packaging and distribution are available.

## Official packages

| Client | Package or binary | How to get it |
| --- | --- | --- |
| Linux app | Arch package: `tsk-linux` | Published from `linux-app-v*` releases to the custom package repository at [repo.matthewyjiang.com](https://repo.matthewyjiang.com/). Add that repository, then install `tsk-linux` with your package manager. Flatpak packaging is in progress. |
| iOS app | TestFlight / App Store distribution | In progress. Until public distribution is available, run the app from source in Xcode. |
| CLI | Prebuilt `tsk` binaries | In progress. Install from source with Cargo for now. |

## Shared behavior

All clients are designed around the same model:

1. Create and edit tasks against local storage first.
2. Keep working while offline.
3. Encrypt task blobs on the client before sync.
4. Push and pull encrypted blobs through the server.
5. Resolve supported conflicts deterministically in shared core.

See [Architecture and sync model](../architecture.md) for implementation details and [Known limitations](../roadmap.md) for workflows that are still evolving.
