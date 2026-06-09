# Server setup and CLI UX evaluation

Date: 2026-06-09

This is a clean re-evaluation of the current server setup and `taskmanager` CLI UX after the recent UX fixes. It intentionally distinguishes between **technical/developer UX** and **normal end-user product UX**.

## Summary

The project has improved substantially. The server setup is now solid for contributors, and the CLI no longer has several first-run trust issues. However, calling both areas a blanket “A” would overstate the current product polish.

Recommended UX readiness call:

- Server setup for developers: **A-**
- Server setup for non-developer/self-hosting users: **B+**
- CLI as developer/test harness: **A- / A**
- CLI as normal user product: **B+**

The codebase is now in good shape for a technical 1.0 audience. For a broader consumer-quality terminal product, the CLI still needs friendlier device pairing, richer date parsing, stronger semantics for global UX flags, and fewer hidden-but-present unfinished commands.

## Server setup UX

### Current flow

The server now supports a one-command local development path:

```sh
cd server
make dev
```

`make dev` starts PostgreSQL with Docker Compose, loads `.env` if present, runs migrations, and starts the API.

The manual flow is still documented:

```sh
cp .env.example .env
make docker-up
set -a; . ./.env; set +a
make run
```

Deployment is handled by:

```sh
cd server
./scripts/deploy.sh
```

The deploy script interactively prompts for configuration, writes `.env`, starts Docker Compose, and checks `/healthz`.

### What works well

- `make dev` provides a straightforward local startup path.
- `make check` is now non-mutating.
- `make fix` handles mutating formatting/module cleanup.
- Server requirements and expected ports are documented.
- The README now connects server setup to CLI setup:

  ```sh
  taskmanager configure --server-url http://localhost:8080
  ```

- The deploy script is interactive and preserves existing `.env` defaults.
- Secrets are generated automatically when missing.
- Deploy health checking is built in.
- Deploy-generated database URL components are URL-encoded, so reserved password characters are handled better.
- Defaults are sensible for a technical deployment.

### Remaining friction

1. **Port conflicts are not proactively diagnosed**

   The docs mention ports, but `make dev` does not preflight-check whether `5432` or `8080` are already in use.

2. **Self-hosting is still technical**

   The deploy script is good for developers/operators, but a normal self-hosting user still needs comfort with SSH, Docker Compose, environment variables, and logs.

3. **Deploy script depends on Python 3 for URL encoding**

   This is acceptable on most servers and is checked, but it is still an extra runtime requirement.

4. **API examples are intentionally minimal**

   The README provides health/auth examples, but blob sync is still best understood through the CLI/E2E suite.

### Server rating rationale

- **A- for developers** because local setup, validation, docs, and deploy flow are now coherent and low-friction.
- **B+ for non-developer self-hosting** because the workflow is still terminal/Docker/operator-oriented and lacks preflight diagnostics.

### Server recommendations

To reach a stronger A:

- Add preflight checks for required commands and occupied ports in `make dev` or a `scripts/dev.sh` wrapper.
- Print clearer next-step messages after `make dev` starts successfully.
- Add a concise self-hosting troubleshooting section for ports, Docker permissions, migrations, and health check failures.

## CLI UX

### Current visible command surface

Normal visible commands:

- `version`
- `configure`
- `task create|get|update|delete|list|search|complete|reopen`
- `sync status|retry|push|pull|run`
- `settings get|set|pull-plaintext|push-plaintext|migrate`
- `account init`
- `auth login|logout`
- `device init-keypair|wrap-key|unwrap-key`
- `generate completion|man`

Hidden but still callable:

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

### What works well

- Running `taskmanager` with no command now prints help.
- `configure` hides password input on TTYs.
- `auth login` now supports email/password login, not only token storage.
- Networked setup/login commands fail clearly when used with `--offline`.
- Developer/unfinished commands are hidden from default help, reducing user confusion.
- `configure` is a good first-run path: it initializes keys, saves server URL, registers/logs in, and stores tokens.
- JSON and JSONL output are strong for automation.
- Path overrides are excellent for tests and isolated profiles:
  - `--profile`
  - `--config`
  - `--db`
- Shell completions and man-page generation are available.
- The insecure file-backed key store is explicit opt-in.
- Due dates now accept simple human forms:

  ```sh
  --due today
  --due tomorrow
  --due 2026-06-10
  ```

### Remaining friction

1. **Device pairing is still not user-friendly**

   The current device workflow exposes public keys, ciphertext, and nonces. This is acceptable for diagnostics and tests, but not an A-level normal-user pairing experience.

   A friendlier target would be something like:

   ```sh
   taskmanager device pair
   taskmanager device pair --code ABCD-1234
   ```

2. **Date parsing is improved but limited**

   Supporting `today`, `tomorrow`, and `YYYY-MM-DD` is useful, but normal users may expect:

   ```sh
   --due "tomorrow 9am"
   --due "next friday"
   --due "in 2 days"
   ```

3. **Some unsupported commands still exist**

   Hiding unsupported commands is a good practical compromise, but a fully polished 1.0 CLI should avoid shipping visible-or-hidden command paths that intentionally return unsupported errors unless they are explicitly internal/testing-only.

4. **`--quiet` and `--yes` remain weak**

   These flags are accepted globally, but most commands do not visibly change behavior based on them. That can make the CLI feel more complete on paper than in practice.

5. **Local-only setup is not first-class**

   `--offline configure` now fails clearly, which is better than surprising behavior. But users may still reasonably want:

   ```sh
   taskmanager configure --local-only
   ```

6. **Normal-user docs could be more guided**

   The README is improved, but the product could benefit from a short “happy path” tutorial with expected output and troubleshooting.

### CLI rating rationale

- **A- / A as a developer/test harness** because the CLI is scriptable, deterministic, well-covered, and supports isolated state cleanly.
- **B+ as a normal user product** because core first-run issues are fixed, but device pairing, richer dates, global flag semantics, and hidden unsupported commands still hold it back from a confident A.

## Suggested first-run journey

This flow is now mostly supported:

```sh
# Start local server.
cd server
make dev

# Configure CLI profile and account.
taskmanager configure --server-url http://localhost:8080

# Create a task.
taskmanager task create "Buy milk" --due tomorrow

# Sync.
taskmanager sync run

# Inspect state.
taskmanager task list
```

For automation, the current flow is strong:

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

## Priority recommendations

Highest impact to reach a more defensible A for normal users:

1. Add friendly `device pair` / `device accept` flow.
2. Expand due-date parsing to include times and common relative phrases.
3. Add `configure --local-only`.
4. Give `--quiet` and `--yes` meaningful behavior or remove them until needed.
5. Decide whether hidden unsupported commands should be implemented, made test-only, or removed from release builds.
6. Add server dev preflight checks for ports and Docker availability.

## Overall conclusion

The current state is much better than the original evaluation. The server and CLI are genuinely strong for technical users, contributors, and automated environments.

However, they are **not unambiguously A-level for normal end users yet**. A fair current assessment is:

- **Server developer UX: A-**
- **Server self-hosting UX: B+**
- **CLI developer/test UX: A- / A**
- **CLI normal-user UX: B+**

The next UX milestone should focus less on setup basics and more on making advanced product workflows feel human: device pairing, natural dates, local-only setup, and removing or completing unfinished command paths.
