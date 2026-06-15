# AGENTS.md

## Release and commit rules

Follow `RELEASE.md` for path-scoped semantic release behavior, artifact tag naming, Conventional Commit rules, release-impacting commit types, examples, and local dry-run commands.

Key reminder: this monorepo uses artifact-prefixed tags only (`server-vX.Y.Z`, `core-vX.Y.Z`, `app-vX.Y.Z`). Do not create unscoped `vX.Y.Z` tags.

## Core API layering

When adding app-facing behavior, implement it in native `core` first. UniFFI/FFI should expose or adapt native core functionality, not contain standalone business logic or FFI-only convenience workflows. Linux should call native `TaskManagerCore` APIs directly; iOS should use generated bindings for the same core APIs.

Core APIs must be platform-agnostic and useful to all client platforms unless a platform-specific abstraction is explicitly unavoidable. Do not add iOS-only, Linux-only, or FFI-only business logic to `core/src/core.rs`, `core/src/ffi.rs`, or `core/uniffi/core.udl`. Put shared task, sync, auth, crypto, conflict, enrollment, and server-protocol semantics in native Rust core, then expose them through UniFFI as needed. Platform apps should provide only thin adapters for platform concerns such as Keychain/secret storage, URLSession or HTTP transport, SwiftUI/GTK UI, reachability, notifications, sandbox paths, and background execution.

Generated bindings under paths such as `ios/tsk/Sources/TskCore/Generated/` are generated artifacts only. Do not hand-edit generated Swift bindings; update the Rust/UDL source and regenerate them.

## CLI validation

When changing files under `cli/`, run the normal CLI checks before committing:

```sh
cargo fmt --check
cargo test -p taskmanager-cli
cargo clippy -p taskmanager-cli --all-targets
```

Also run the local CLI/server E2E suite when CLI behavior, CLI output, server flags, sync/auth/device flows, task commands, or CLI documentation/examples change:

```sh
./scripts/cli_e2e.py
```

The E2E suite starts a fresh test server/Postgres instance and exercises the compiled CLI against it. Extend `scripts/cli_e2e.py` whenever new CLI features are added so the suite remains comprehensive.
