# Tasks

A local-first, end-to-end encrypted task manager.

## Documentation

The Markdown documentation site lives in [`docs/`](./docs/) and is built with VitePress.

```sh
npm install
npm run docs:dev
npm run docs:build
```

Start with the user-facing docs:

- [Overview](./docs/overview.md)
- [Get started](./docs/getting-started.md)
- [Client status](./docs/clients/index.md)
- [Linux app](./docs/clients/linux.md)
- [iOS app](./docs/clients/ios.md)
- [CLI guide](./docs/cli.md)
- [Server setup](./docs/server.md)
- [Known limitations](./docs/roadmap.md)

Architecture, security, release, and development details remain available from the docs site's Reference and Contributing sections.

## Structure

- `server/` — Go zero-knowledge sync server
- `core/` — Rust client core library
- `cli/` — Rust command-line client
- `ios/` — iOS app shell
- `android/` — Android app shell
- `windows/` — Windows app shell
- `macos/` — macOS app shell
- `linux/` — GTK/libadwaita Linux desktop app (`tsk-gui`)

## Linux desktop app

The Linux app is the GTK/libadwaita desktop client. The official Arch package is `tsk-linux`, published to the custom package repository at [repo.matthewyjiang.com](https://repo.matthewyjiang.com/) from `linux-app-v*` releases. Flatpak packaging is in progress.

Run from source with:

```sh
cargo run -p tsk-linux
```

The app stores account/device keys and auth tokens in the platform key store, syncs encrypted blobs through the server API, and automatically refreshes expired access tokens during sync when a valid refresh token is available.

## Command-line client

Prebuilt `tsk` binaries are in progress. From the repository root, install the Rust CLI with the repo helper:

```sh
make cli-install
```

Equivalent Cargo command:

```sh
cargo install --path cli --force
```

This installs the `tsk` binary into Cargo's bin directory, usually `~/.cargo/bin`. Ensure that directory is on your `PATH`:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
tsk --help
```

For local development without installing:

```sh
make cli-run -- --help
# or: cargo run -p taskmanager-cli -- --help
```

To uninstall:

```sh
make cli-uninstall
```

Optional shell completions and a man page can be generated after building/installing the CLI:

```sh
tsk generate completion bash > tsk.bash
tsk generate completion zsh > _tsk
tsk generate completion fish > tsk.fish
tsk generate completion powershell > tsk.ps1
tsk generate man > tsk.1
```

See the [CLI guide](./docs/cli.md) for user-facing examples. [`cli/README.md`](./cli/README.md) includes deeper CLI reference and development setup.

## Specification

See [`SPEC.md`](./SPEC.md) for the technical specification.
