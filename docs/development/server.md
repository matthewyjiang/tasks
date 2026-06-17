# Server development

The server is a Go zero-knowledge sync backend for encrypted task blobs. This page is for local development. For self-hosted or production deployment, see [Server setup](../server.md).

## Requirements

- Go 1.23 or newer
- Docker with Docker Compose v2
- `curl` for health checks
- local PostgreSQL port `5432`
- local API port `18080` by default

## Local development

From `server/`:

```sh
make dev
```

`make dev` is a development-only convenience. It starts PostgreSQL with Docker Compose, loads `.env` if present, applies development defaults for `DATABASE_URL` and `JWT_SECRET` when missing, runs migrations, and starts the API.

Manual equivalent:

```sh
cp .env.example .env
make docker-up
set -a; . ./.env; set +a
make run
```

Health check:

```sh
curl http://localhost:18080/healthz
```

Then configure a client:

```sh
tsk configure --server-url http://localhost:18080
```

Non-interactive CLI setup for tests/scripts:

```sh
TASKMANAGER_INSECURE_KEY_DIR=/tmp/taskmanager/keys \
  tsk \
  --profile ci \
  --output json \
  configure \
  --server-url http://localhost:18080 \
  --email ci@example.com \
  --password "$TASKMANAGER_TEST_PASSWORD"
```

## Checks

From `server/`:

```sh
make check
make test
make build
```

Useful Docker helpers for local development:

```sh
make docker-up
make docker-down
```
