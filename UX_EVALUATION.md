# Server setup and CLI UX re-evaluation

Date: 2026-06-09

This document re-evaluates the current user experience of the server setup flow and the `taskmanager` CLI after the second UX pass on `ux-evaluation-fixes`.

## Summary

Both the server setup and CLI now meet an **A-** UX bar for a technical 1.0 audience. The server has a one-command local development path, non-mutating checks, clearer runtime documentation, a CLI handoff, and safer deploy-time database URL construction. The CLI now has discoverable first-run help, hidden password prompts, explicit offline semantics, cleaner help output, first-class email/password `auth login`, and human-readable due-date input for common cases.

Recommended UX readiness call:

- Server setup: **A-** improved from B-
- CLI as developer/test harness: **A** improved from A-
- CLI as normal user product: **A-** improved from B-

Remaining gaps are no longer first-run blockers. The main opportunities are product expansion: guided multi-device pairing, broader natural-language date parsing, and implementing hidden future commands.

## Server setup UX

### Current flow

Local development now supports one-command startup:

```sh
cd server
make dev
```

`make dev` starts PostgreSQL with Docker Compose, loads `.env` if present, runs migrations, and starts the API.

Manual setup is still documented for contributors who want each step:

```sh
cp .env.example .env
make docker-up
set -a; . ./.env; set +a
make run
```

Deployment is handled by `server/scripts/deploy.sh`, which interactively prompts for configuration, writes `.env`, starts Docker Compose, and checks `/healthz`.

### What works well

- One-command local development exists via `make dev`.
- `make check` now verifies without mutating files.
- `make fix` performs the mutating cleanup/formatting path.
- Runtime requirements and expected ports are documented.
- Server docs now show the next CLI action:

  ```sh
  taskmanager configure --server-url http://localhost:8080
  ```

- The deploy script is interactive and approachable for technical users.
- Secrets are generated automatically when missing.
- Existing `.env` values are preserved and offered as defaults.
- Docker Compose deployment runs a health check after startup.
- Deploy-time database URL components are URL-encoded, so special characters in generated or user-entered Postgres passwords are safer.
- Server defaults remain sensible:
  - `PORT=8080`
  - `ACCESS_TOKEN_TTL=15m`
  - `REFRESH_TOKEN_TTL=720h`
  - `WRITE_RATE_LIMIT_PER_MIN=60`
  - `MAX_BLOB_BYTES=1048576`
  - `MAX_BATCH_BLOBS=100`
  - `TOMBSTONE_RETENTION=720h`

### Remaining friction points

1. **`make dev` assumes Docker/Postgres port availability**

   The docs list expected ports, but the command does not proactively diagnose port conflicts before Docker Compose reports them.

2. **Deploy script now depends on Python 3**

   This is reasonable on most servers and is checked up front, but it is one more deployment prerequisite.

3. **Sample API docs remain intentionally light**

   The README includes health and auth examples, but encrypted blob sync is still best exercised through the CLI.

### Server UX recommendations

Already resolved from the original must-fix list:

- Add a one-command local dev startup path.
- Make `make check` non-mutating and add `make fix`.
- Document how to connect the CLI to the local server.
- Fix raw DB password interpolation in deploy-generated `DATABASE_URL`.

Nice-to-have future improvements:

- Detect local port conflicts before `docker compose up`.
- Add a deploy preflight summary showing required commands and versions.
- Add deeper API examples for developers building direct integrations.

## CLI UX

### Current visible command surface

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

### Improvements now in place

1. **No-command behavior is discoverable**

   Running `taskmanager` with no command prints full help instead of returning silently.

2. **Password input is safer**

   `configure` and email/password `auth login` hide password input on TTYs while preserving stdin-compatible behavior for tests/headless automation.

3. **Unsupported/developer commands are hidden from default help**

   Developer and unfinished surfaces no longer make the product look broken to normal users, while still remaining available for diagnostics and E2E coverage.

4. **Offline semantics are explicit**

   `--offline configure` and email/password `--offline auth login` fail clearly before attempting server authentication.

5. **`auth login` now matches user expectations**

   Users can log in with email/password:

   ```sh
   taskmanager auth login --email you@example.com --server-url http://localhost:8080
   ```

   Token import still works for scripts and advanced workflows:

   ```sh
   taskmanager auth login --access-token ... --refresh-token ...
   ```

6. **Common human-readable due dates are supported**

   Task due-date arguments accept epoch milliseconds plus common date forms:

   ```sh
   taskmanager task create "Buy milk" --due tomorrow
   taskmanager task create "File taxes" --due 2026-04-15
   ```

7. **CLI docs match implementation**

   The README now describes the current configure/auth/sync behavior, hidden crypto diagnostics, human due-date examples, and the recommended first-run flow.

8. **Validation coverage improved**

   Tests cover no-command help, hidden unsupported/developer commands, offline configure behavior, and human due dates. The CLI/server E2E suite continues to exercise the integrated flow.

### What works well now

- The command hierarchy is understandable and maps well to product concepts.
- First-run behavior is discoverable.
- `taskmanager configure` is a strong setup path: it initializes local keys, saves the server URL, registers/logs in, and stores tokens.
- `auth login` now supports the expected email/password workflow.
- Default help is cleaner and less alarming.
- `--output json` and `--output jsonl` are strong features for scripts and tests.
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

1. **Device pairing remains low-level**

   Current key wrapping commands expose raw public keys, ciphertext, and nonces. This is useful for diagnostics, but normal users would benefit from a guided pairing flow such as:

   ```sh
   taskmanager device pair
   taskmanager device pair --code ABCD-1234
   ```

2. **Natural-language due dates are intentionally limited**

   `today`, `tomorrow`, `YYYY-MM-DD`, and epoch milliseconds are enough for an A- technical CLI. Broader forms like `next friday` or `tomorrow 9am` would be a polish improvement.

3. **Some global flags remain future-facing**

   `--quiet` and `--yes` are accepted, but they still have limited visible effect because most commands are already non-interactive or direct.

4. **Hidden unsupported commands still exist**

   Hiding unsupported commands is acceptable for this stage, but a strict consumer 1.0 could either implement them or exclude them from release builds.

## CLI UX recommendations

Already resolved from the original must-fix list:

- Print help when no command is supplied.
- Hide password input in `configure`.
- Make `auth login` support real email/password login.
- Hide unsupported/developer commands from default help.
- Update `cli/README.md` to match current auth/configure behavior.
- Define correct `--offline` semantics for networked setup/login commands.
- Add common human-readable due-date parsing.

Nice-to-have future improvements:

- Add friendly multi-device pairing.
- Expand date parsing to include times and relative weekday phrases.
- Give `--quiet` and `--yes` stronger semantics or remove them until needed.
- Normalize any remaining naming differences across args and output.
- Add `configure --local-only` for users who want explicitly offline setup.

## Suggested first-run user journey

The polished basic flow is now mostly available:

```sh
# Start local server for development.
cd server
make dev

# Configure local CLI profile and account.
taskmanager configure --server-url http://localhost:8080

# Create a task with a human due date.
taskmanager task create "Buy milk" --due tomorrow

# Sync.
taskmanager sync run

# Inspect state.
taskmanager task list
```

For CI or scripts, the non-interactive flow remains strong:

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

The server and CLI now both clear an A-level bar for technical users and automated environments. The remaining gaps are not basic trust or first-run blockers; they are advanced product polish items. The highest-leverage next UX improvement would be a friendly multi-device pairing workflow, followed by richer date parsing and a decision on hidden future commands.
