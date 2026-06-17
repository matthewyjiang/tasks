# Tasks

A local-first, end-to-end encrypted task manager.

`tsk` is designed so task management stays fast and useful on the device in front of you. Sync is optional infrastructure for sharing encrypted task changes across enrolled clients, not a requirement for basic use.

## Start here

The documentation site lives in [`docs/`](./docs/) and is built with VitePress.

User-facing entry points:

- [Overview](./docs/overview.md)
- [Get started](./docs/getting-started.md)
- [Client status](./docs/clients/index.md)
- [Linux app](./docs/clients/linux.md)
- [iOS app](./docs/clients/ios.md)
- [CLI](./docs/cli.md)
- [Server setup](./docs/server.md)
- [Known limitations](./docs/roadmap.md)

Architecture, security, release, and development details remain available from the docs site's Reference and Contributing sections.

## Clients

- **Linux app** — GTK/libadwaita desktop client. The official Arch package is `tsk-linux`, available from [repo.matthewyjiang.com](https://repo.matthewyjiang.com/). Flatpak packaging is in progress.
- **iOS app** — SwiftUI mobile client. Public TestFlight/App Store distribution is in progress.
- **CLI** — terminal client for automation, diagnostics, integrations, and command-line task workflows. Prebuilt binaries are in progress; source install is available today.

## Repository structure

- `core/` — shared Rust client core
- `linux/` — Linux desktop app
- `ios/` — iOS app
- `cli/` — command-line client
- `server/` — zero-knowledge sync server
- `docs/` — VitePress documentation site
- `packaging/` — package metadata
- `scripts/` — release and integration-test automation

## Specification

See [`SPEC.md`](./SPEC.md) for the technical specification.
