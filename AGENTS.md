# AGENTS.md

## Release and commit rules

Follow `RELEASE.md` for path-scoped semantic release behavior, artifact tag naming, Conventional Commit rules, release-impacting commit types, examples, and local dry-run commands.

Key reminder: this monorepo uses artifact-prefixed tags only (`server-vX.Y.Z`, `core-vX.Y.Z`, `app-vX.Y.Z`). Do not create unscoped `vX.Y.Z` tags.

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
