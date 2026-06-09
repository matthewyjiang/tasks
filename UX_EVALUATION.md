# Server setup and CLI UX re-evaluation

Date: 2026-06-09

This document re-evaluates the current user experience of the server setup flow and the `taskmanager` CLI after the first CLI UX fixes in PR #27 (`ux-evaluation-fixes`).

## Summary

The project remains technically strong, especially for deterministic tests and developer workflows. The latest CLI fixes remove several high-trust first-run problems: running `taskmanager` with no subcommand now shows help, `configure` hides password input on TTYs, `--offline configure` fails clearly, stale CLI docs were corrected, and developer/unsupported commands are no longer shown in default help.

Recommended UX readiness call:

- Server setup: **B-** unchanged
- CLI as developer/test harness: **A-** improved from B+
- CLI as normal user product: **B- / closer, but not 1.0-ready** improved from C+

The CLI now feels much less unfinished on first contact. Remaining 1.0 blockers are mostly product-level workflows: real email/password `auth login`, friendly device pairing, human-readable due dates, and either implementing or fully retiring unsupported command surfaces.

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

### Remaining friction points

1. **Local setup is still too manual**

   A contributor still has to copy `.env`, start Postgres, source env vars, and then run the API. This is acceptable for contributors, but not ideal for a polished first-run experience.

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

   The current `check` target runs `go mod tidy` and `gofmt -w`, which rewrite files. A command named `check` should verify without modifying the working tree. Split into:

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

5. **Server docs still need a stronger CLI handoff**

   After starting the server, the README should show the next user action, for example:

   ```sh
   taskmanager configure --server-url http://localhost:8080
   ```

6. **Runtime requirements are underdocumented**

   `server/README.md` should mention required Go, Docker, and Docker Compose versions, plus expected local ports.

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

The normal visible CLI namespaces are now:

- `version`
- `configure`
- `task create|get|update|delete|list|search|complete|reopen`
- `sync status|retry|push|pull|run`
- `settings get|set|pull-plaintext|push-plaintext|migrate`
- `account init`
- `auth login|logout`
- `device init-keypair|wrap-key|unwrap-key`
- `generate completion|man`

Hidden but still callable for tests/diagnostics:

- `crypto ...`
- `auth refresh`
- `device register|list`
- `sync conflicts|resolve`

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

### Improvements since the original evaluation

1. **No-command behavior is fixed**

   Running `taskmanager` with no command now prints full help instead of returning silently. This is a major first-run UX improvement.

2. **Password input is hidden on TTYs**

   `configure` now uses hidden password input when a terminal is available. It falls back to stdin-compatible input for tests/headless flows, preserving automation support.

3. **Unsupported commands are hidden from default help**

   Developer/unfinished surfaces no longer appear in normal help:

   - `crypto ...`
   - `auth refresh`
   - `device register|list`
   - `sync conflicts|resolve`

   Keeping them callable preserves E2E coverage and diagnostics without making the product look broken to normal users.

4. **`--offline configure` semantics are explicit**

   `configure` now fails clearly when invoked with `--offline`, instead of prompting and then trying server auth anyway.

5. **CLI docs were reconciled with implementation**

   `cli/README.md` now describes the current `configure`/auth/sync behavior more accurately, including configured server URL usage and the hidden crypto diagnostics surface.

6. **Validation coverage improved**

   Tests cover no-command help, hidden unsupported/developer commands, and offline configure behavior. The full CLI E2E suite still passes.

### What works well now

- The command hierarchy is understandable and maps well to product concepts.
- First-run behavior is discoverable because no-command prints help.
- `taskmanager configure` is the right main setup path. It initializes local keys, saves the server URL, registers/logs in, and stores tokens.
- Password prompting is safer for interactive users.
- Default help is cleaner and less alarming.
- `--output json` and `--output jsonl` remain strong features for scripts and tests.
- Path overrides are excellent for integration testing:
  - `--profile`
  - `--config`
  - `--db`
- Per-profile local state is a good default:
  - `~/.taskmanager/profiles/<profile>/tasks.db`
  - `~/.taskmanager/profiles/<profile>/settings.json`
- Shell completion and man-page generation are available.
- The insecure file-backed key store is explicitly opted in, which is good for safety.

### Remaining friction points

1. **`auth login` is still misleading**

   Users expect email/password credentials. The current command stores already-issued tokens:

   ```sh
   taskmanager auth login --access-token ... --refresh-token ...
   ```

   Better options:

   - make `auth login` perform real email/password login, or
   - rename the current command to `auth store-token` / `auth import-token`.

2. **Hidden unsupported commands still exist**

   Hiding unsupported commands improves UX, but for a strict 1.0 product the hidden commands should either be implemented, renamed as internal/test-only, or removed from release builds.

3. **Device pairing is too low-level**

   Current key wrapping commands expose raw public keys, ciphertext, and nonces. This is useful for diagnostics, but normal users need a guided pairing flow such as:

   ```sh
   taskmanager device pair
   taskmanager device pair --code ABCD-1234
   ```

4. **Due dates require epoch milliseconds**

   Examples still use values like:

   ```sh
   --due 1717603200000
   ```

   Normal users need human-readable parsing:

   ```sh
   --due "tomorrow 9am"
   --due "2026-06-10"
   --due "next friday"
   ```

5. **Some global flags appear unused**

   `--quiet` and `--yes` are accepted, but they do not appear to materially change command behavior yet. This can still make the CLI feel unfinished.

6. **Naming consistency could improve**

   Status values and sort names should be consistent across help text, accepted arguments, table output, and JSON output. Mixed forms like `in-progress` vs `in_progress` can confuse users.

7. **`configure` has no local-only setup mode**

   The explicit `--offline configure` error is good, but users may reasonably want local-only setup. Consider adding:

   ```sh
   taskmanager configure --local-only
   ```

## CLI UX recommendations

Resolved from the original must-fix list:

- Print help when no command is supplied.
- Hide password input in `configure` for TTY users.
- Hide unsupported/developer commands from default help.
- Update `cli/README.md` to match current auth/configure behavior.
- Define `--offline configure` semantics clearly.

Remaining must-fix before 1.0 UX:

- Make `auth login` perform real email/password login, or rename the current token-storage behavior.
- Provide a normal-user multi-device pairing workflow.
- Add human-friendly date parsing for task due dates.
- Decide whether hidden unsupported commands ship in 1.0, become internal-only, or get implemented.
- Ensure `--quiet` and `--yes` either work or are removed until needed.

Strongly recommended:

- Add `configure --local-only` for offline/local-only first use.
- Normalize naming across args and output.
- Add more examples in CLI docs for the polished basic journey.

## Suggested first-run user journey

The intended polished basic flow is now closer to this:

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

Current gaps in that journey:

- `server/make dev` does not exist yet.
- `--due "tomorrow"` is not supported yet.
- First-class `auth login --email ...` is not available yet.
- Multi-device setup still requires low-level key commands.

For CI or scripts, the current non-interactive flow remains strong:

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

The CLI improved materially after the first UX pass. It now has a much better first-run posture: help appears by default, password entry is safer, offline configure behavior is explicit, docs are more accurate, and normal help no longer advertises commands that immediately fail.

The server setup remains the larger setup-flow weakness. For the CLI, the remaining work is less about polish bugs and more about completing normal-user product workflows: email/password `auth login`, friendly device pairing, human-readable dates, meaningful `--quiet`/`--yes`, and a decision on hidden unsupported commands.
