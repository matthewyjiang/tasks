# Server setup and CLI UX evaluation

Date: 2026-06-09

This document evaluates the current user experience of the server setup flow and the `taskmanager` CLI, based on the implementation and docs in this repository.

## Summary

The project has a strong technical foundation, especially for developer and automated-test workflows. The server setup is workable for technical users, and the CLI has a clear command structure with good JSON support. However, the user-facing UX is not yet polished enough to call the CLI a 1.0-quality terminal product.

Recommended UX readiness call:

- Server setup: **B-**
- CLI as developer/test harness: **B+**
- CLI as normal user product: **C+ / not yet 1.0-ready**

## Server setup UX

### Current flow

Local development is documented in `server/README.md`:

```sh
cp .env.example .env
make docker-up
set -a; . ./.env; set +a
make run
```

Deployment is handled by `server/scripts/deploy.sh`, which interactively prompts for configuration, writes `.env`, starts Docker Compose, and checks `/healthz`.

### What works well

- The deploy script is interactive and approachable for technical users.
- Secrets are generated automatically when missing.
- Existing `.env` values are preserved and offered as defaults.
- Docker Compose deployment runs a health check after startup.
- Server defaults are sensible:
  - `PORT=8080`
  - `ACCESS_TOKEN_TTL=15m`
  - `REFRESH_TOKEN_TTL=720h`
  - `WRITE_RATE_LIMIT_PER_MIN=60`
  - `MAX_BLOB_BYTES=1048576`
  - `MAX_BATCH_BLOBS=100`
  - `TOMBSTONE_RETENTION=720h`
- `make check` is easy to discover and currently passes.

### Friction points

1. **Local setup is too manual**

   The developer has to copy `.env`, start Postgres, source env vars, and then run the API. This is fine for contributors, but not ideal for a polished first-run experience.

2. **No one-command local dev flow**

   A first-time user should be able to run something like:

   ```sh
   make dev
   ```

   or:

   ```sh
   docker compose up --build
   ```

   and get both Postgres and the API.

3. **`make check` mutates files**

   The current `check` target runs `go mod tidy` and `gofmt -w`, which rewrite files. A command named `check` should usually verify without modifying the working tree. Consider splitting into:

   ```sh
   make fix
   make check
   ```

4. **Deploy script may mishandle special DB password characters**

   `server/scripts/deploy.sh` interpolates the Postgres password directly into `DATABASE_URL`:

   ```sh
   DATABASE_URL=postgres://$POSTGRES_USER:$POSTGRES_PASSWORD@postgres:5432/$POSTGRES_DB?sslmode=disable
   ```

   Passwords containing characters like `@`, `:`, `/`, `#`, or `?` may break the URL unless encoded.

5. **Server docs do not connect to CLI setup**

   After starting the server, the README should show the next user action, for example:

   ```sh
   taskmanager configure --server-url http://localhost:8080
   ```

6. **Runtime requirements are underdocumented**

   `server/README.md` should mention required Go, Docker, and Docker Compose versions, plus the expected local ports.

### Server UX recommendations

Must-fix before a polished 1.0 UX:

- Add a one-command local dev startup path.
- Make `make check` non-mutating and add a separate mutating `make fix`.
- Document how to connect the CLI to the local server.
- Fix or avoid raw URL interpolation for DB passwords in the deploy script.

Nice-to-have:

- Add sample `curl` calls for `/healthz`, auth registration, and blob sync.
- Document required tool versions.
- Detect local port conflicts, especially Postgres on `5432`.

## CLI UX

### Current command surface

The main CLI namespaces are:

- `version`
- `configure`
- `task create|get|update|delete|list|search|complete|reopen`
- `sync status|retry|push|pull|run|conflicts|resolve`
- `settings get|set|pull-plaintext|push-plaintext|migrate`
- `account init`
- `auth login|refresh|logout`
- `device init-keypair|register|list|wrap-key|unwrap-key`
- `crypto ...`
- `generate completion|man`

Global flags include:

- `--profile`
- `--config`
- `--db`
- `--server`
- `--output <table|json|jsonl>`
- `--quiet`
- `--yes`
- `--offline`
- `--trace`
- `--dangerously-print-secrets`

### What works well

- The command hierarchy is understandable and maps well to the product concepts.
- `--output json` and `--output jsonl` are strong features for scripts and tests.
- Path overrides are excellent for integration testing:
  - `--profile`
  - `--config`
  - `--db`
- `taskmanager configure` is the right direction. It initializes local keys, saves the server URL, registers or logs in, and stores tokens.
- Per-profile local state is a good default:
  - `~/.taskmanager/profiles/<profile>/tasks.db`
  - `~/.taskmanager/profiles/<profile>/settings.json`
- Shell completion and man-page generation are available.
- The insecure file-backed key store is explicitly opted in, which is good for safety.

### Friction points

1. **Running `taskmanager` with no command prints nothing**

   Current behavior returns no output. Most users expect help text. `taskmanager` should behave like `taskmanager --help`.

2. **Password prompt is not hidden**

   `configure` currently prompts for passwords through normal stdin, so the password is echoed in the terminal. This is a significant UX and security issue.

3. **`auth login` is misleading**

   Users expect this to accept email/password credentials. Instead, it stores already-issued tokens:

   ```sh
   taskmanager auth login --access-token ... --refresh-token ...
   ```

   Actual email/password login is hidden inside `taskmanager configure`. Consider changing `auth login` to perform real login and renaming the token-storage command to something explicit, such as `auth store-token`.

4. **Visible unimplemented commands reduce trust**

   Several commands are visible but return unsupported errors:

   - `auth refresh`
   - `device register`
   - `device list`
   - `sync conflicts`
   - `sync resolve`

   For a 1.0 CLI, visible commands should work or be hidden/marked experimental.

5. **Device pairing is too low-level**

   Current key wrapping commands expose raw public keys, ciphertext, and nonces. That is useful for diagnostics, but not friendly for normal users. A better UX would guide users through pairing profiles/devices with a short code, URL, or explicit `device pair` flow.

6. **Due dates require epoch milliseconds**

   Examples use values like:

   ```sh
   --due 1717603200000
   ```

   Normal users need human-readable parsing:

   ```sh
   --due "tomorrow 9am"
   --due "2026-06-10"
   --due "next friday"
   ```

7. **Developer crypto commands are prominent**

   The `crypto` namespace is valuable for diagnostics and E2E tests, but it may distract or worry normal users. Consider hiding it from normal help or labeling it clearly as advanced/developer-only.

8. **Docs contain stale/conflicting statements**

   `cli/README.md` says server-backed auth/device registration is not wired yet, while `configure` now performs server register/login. This should be reconciled.

9. **`--offline` behavior is surprising for `configure`**

   `--offline` is global, but `configure` still attempts server auth. It should either:

   - perform local-only setup, or
   - fail clearly with a message like `configure requires network unless --local-only is supplied`.

10. **Some global flags appear unused**

   `--quiet` and `--yes` are accepted, but they do not appear to materially change command behavior yet. This can make the CLI feel unfinished.

11. **Naming consistency could improve**

   Status values and sort names should be consistent across help text, accepted arguments, table output, and JSON output. Mixed forms like `in-progress` vs `in_progress` can confuse users.

### CLI UX recommendations

Must-fix before 1.0 UX:

- Print help when no command is supplied.
- Hide password input in `configure`.
- Make `auth login` perform real email/password login, or rename the current token-storage behavior.
- Hide, remove, or clearly mark unimplemented commands.
- Update `cli/README.md` to match current auth/configure behavior.
- Define correct `--offline` semantics for `configure`.

Strongly recommended:

- Add human-friendly date parsing for task due dates.
- Add a friendly multi-device pairing workflow.
- Move or hide developer crypto commands from default help.
- Ensure `--quiet` and `--yes` either work or are removed until needed.
- Normalize naming across args and output.

## Suggested first-run user journey

A polished basic flow should be this simple:

```sh
# Start local server for development.
cd server
make dev

# Configure local CLI profile and account.
taskmanager configure --server-url http://localhost:8080

# Create a task.
taskmanager task create "Buy milk" --due "tomorrow"

# Sync.
taskmanager sync run

# Inspect state.
taskmanager task list
```

For CI or scripts, the equivalent should remain fully non-interactive:

```sh
TASKMANAGER_INSECURE_KEY_DIR=/tmp/taskmanager/keys \
  taskmanager \
  --profile ci \
  --db /tmp/taskmanager/tasks.db \
  --config /tmp/taskmanager/settings.json \
  --output json \
  configure \
  --server-url http://localhost:8080 \
  --email ci@example.com \
  --password "$TASKMANAGER_TEST_PASSWORD"
```

## Overall conclusion

The server setup is usable for developers and close to acceptable for a technical 1.0 audience, but it needs smoother local startup and clearer CLI handoff docs.

The CLI is technically useful and especially strong as a deterministic test harness, but it is not yet a polished 1.0 user experience. The most important issues are the visible unimplemented commands, misleading `auth login`, unhidden password prompt, no-output no-command behavior, and low-level device-pairing flow.
