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

From the repository root, install the Rust CLI with the repo helper:

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

See [`cli/README.md`](./cli/README.md) for usage examples and development setup.

## Specification

See [`SPEC.md`](./SPEC.md) for the technical specification.
