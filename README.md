# Tasks

A local-first, end-to-end encrypted task manager.

## Structure

- `server/` — Go zero-knowledge sync server
- `core/` — Rust client core library
- `cli/` — Rust command-line client
- `ios/` — iOS app shell
- `android/` — Android app shell
- `windows/` — Windows app shell
- `macos/` — macOS app shell
- `linux/` — Linux app shell

## CLI installation

From the repository root, install the Rust CLI with Cargo:

```sh
cargo install --path cli --bin taskmanager
```

This installs the `taskmanager` binary into Cargo's bin directory, usually `~/.cargo/bin`. Ensure that directory is on your `PATH`:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
taskmanager --help
```

For local development without installing:

```sh
cargo run -p taskmanager-cli -- --help
```

Optional shell completions and a man page can be generated after building/installing the CLI:

```sh
taskmanager generate completion bash > taskmanager.bash
taskmanager generate completion zsh > _taskmanager
taskmanager generate completion fish > taskmanager.fish
taskmanager generate completion powershell > taskmanager.ps1
taskmanager generate man > taskmanager.1
```

See [`cli/README.md`](./cli/README.md) for usage examples and development setup.

## Specification

See [`SPEC.md`](./SPEC.md) for the technical specification.
