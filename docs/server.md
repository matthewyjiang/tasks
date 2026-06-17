# Server setup

The `tsk` server coordinates encrypted sync. It stores account, session, device, cursor, and encrypted blob metadata so enrolled clients can exchange changes. It is not the source of truth for plaintext tasks.

## When you need a server

You only need a server when you want sync across devices or a self-hosted encrypted task stream. Single-device task management works locally without one.

## Trust boundary

The server handles sensitive account infrastructure such as passwords, sessions, JWTs, refresh tokens, device public keys, and encrypted blobs. Deploy it behind HTTPS/TLS. Do not expose the plaintext Go HTTP port directly to the Internet.

A typical production deployment runs the Go server and PostgreSQL with Docker Compose, then puts a reverse proxy or managed load balancer such as Caddy, Nginx, or a cloud load balancer in front of the local server port to terminate TLS.

## Host requirements

Install these on the deployment host:

- Docker
- Docker Compose v2
- `curl` for deploy health checks

Production deployment does not require Go, `make`, or a local server image build.

## Production deployment

From a deployment host, clone or update the repository, then run the deploy script from `server/`:

```sh
cd server
./scripts/deploy.sh
```

The script writes `.env`, generates missing secrets, pulls the prebuilt server image from GHCR, starts PostgreSQL and the API with Docker Compose, and checks `/healthz` on the loopback interface.

For repeatable or CI-driven deploys, use non-interactive mode:

```sh
./scripts/deploy.sh --yes
./scripts/deploy.sh --yes --host-port 8080 --health-timeout 120
```

The API container listens internally on `PORT=18080`. `HOST_PORT` controls the local loopback port exposed on the host for your TLS-terminating reverse proxy. The default binding is `127.0.0.1:18080`.

## Configuration

The deploy script reads and writes `server/.env` by default. Use `--env-file PATH` to choose a different file.

Generated or managed values include:

- `POSTGRES_PASSWORD`: generated automatically when missing.
- `JWT_SECRET`: generated automatically when missing or still set to the example placeholder.
- `DATABASE_URL`: constructed from the Postgres settings. Components are URL-encoded, so generated or user-provided passwords may contain reserved URL characters such as `@`, `:`, `/`, `#`, and `?`.
- `HOST_PORT`: local host port for the reverse proxy, default `18080`.
- `TASKS_SERVER_IMAGE`: server image to deploy, default `ghcr.io/matthewyjiang/tasks-server:latest`.

To pin a specific server release image, set `TASKS_SERVER_IMAGE` in `.env` before deploying:

```sh
TASKS_SERVER_IMAGE=ghcr.io/matthewyjiang/tasks-server:server-v1.2.3
```

Subsequent deploy runs preserve that value when rewriting `.env`.

## Operations

Common operational helpers:

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

Use `./scripts/deploy.sh --help` for all options, including `--env-file`, `--host-port`, `--health-timeout`, and `--skip-health`.

## Local development

For local development with Go and the server Makefile, see [Server development](./development/server.md).

## Related pages

- [Security model](./security.md) explains what the server can and cannot read.
- [Architecture and sync model](./architecture.md) explains how local-first sync fits together.
