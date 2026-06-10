# Tasks server

Go zero-knowledge sync backend for encrypted task blobs.

## Requirements

- Go 1.23 or newer
- Docker with Docker Compose v2
- `curl` for health checks and deploy verification
- Local ports:
  - `5432` for PostgreSQL in local development
  - `18080` for the API by default

## Local development

One-command development startup:

```sh
make dev
```

`make dev` starts PostgreSQL with Docker Compose, loads `.env` if present, applies development defaults for `DATABASE_URL` and `JWT_SECRET` when they are missing, runs database migrations, and starts the API.

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

Then configure the CLI against the local server:

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

## Sample API calls

```sh
curl http://localhost:18080/healthz

curl -sS http://localhost:18080/auth/register \
  -H 'content-type: application/json' \
  -d '{"email":"dev@example.com","password":"correct horse battery staple","pub_key":"BASE64_DEVICE_PUBLIC_KEY"}'
```

Blob sync endpoints require a bearer token returned by `/auth/register` or `/auth/login`; normal users should use a client instead of hand-crafting encrypted blob requests. Access tokens default to 15 minutes, refresh tokens default to 30 days, and clients should call `/auth/refresh` with the stored refresh token when an access token expires.

## Deploy on a server

SSH into the server, clone/update the repo, then run:

```sh
cd server
./scripts/deploy.sh
```

The script interactively asks for non-secret settings, automatically generates missing `POSTGRES_PASSWORD` and `JWT_SECRET`, writes `.env`, builds the `tasks-server-api:latest` image, starts PostgreSQL and the API with Docker Compose, and checks `/healthz`. `HOST_PORT` controls the public host port; the deploy script keeps the app container on the default `PORT=18080` internally and exposes `HOST_PORT=18080` by default.

For repeatable or CI-driven deploys, use non-interactive mode. Existing `.env` values are reused, missing secrets are generated automatically, and defaults are applied:

```sh
./scripts/deploy.sh --yes
./scripts/deploy.sh --yes --host-port 8080 --health-timeout 120
```

Operational helpers:

```sh
./scripts/deploy.sh status     # show Compose service status
./scripts/deploy.sh logs       # follow app and database logs
./scripts/deploy.sh backup     # write a gzipped Postgres dump to server/backups/
./scripts/deploy.sh undeploy   # stop and remove containers, preserving database data
```

To undeploy and delete the Postgres data volume too:

```sh
./scripts/deploy.sh undeploy --volumes
```

Use `./scripts/deploy.sh --help` for all options, including `--env-file` and `--skip-health`.

The deploy script URL-encodes database URL components, so generated or user-provided Postgres passwords may contain reserved URL characters such as `@`, `:`, `/`, `#`, and `?`.

## Checks and formatting

Verify without modifying the working tree:

```sh
make check
```

Apply formatting/module cleanup:

```sh
make fix
```

Other useful targets:

```sh
make test
make build
make docker-up
make docker-down
```

## Configuration

See `.env.example` for supported environment variables and defaults.
