# Server development and deployment

The server is a Go zero-knowledge sync backend for encrypted task blobs.

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

`make dev` starts PostgreSQL with Docker Compose, loads `.env` if present, applies development defaults for `DATABASE_URL` and `JWT_SECRET` when missing, runs migrations, and starts the API.

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

## Deployment notes

The API handles passwords, JWTs, and refresh tokens. Do not expose the server's plaintext HTTP port directly to the Internet. Terminate HTTPS/TLS in a reverse proxy such as Caddy, Nginx, or a managed load balancer.

From `server/` on a deployment host:

```sh
./scripts/deploy.sh
```

Non-interactive deploy:

```sh
./scripts/deploy.sh --yes
./scripts/deploy.sh --yes --host-port 8080 --health-timeout 120
```

Operational helpers:

```sh
./scripts/deploy.sh status
./scripts/deploy.sh logs
./scripts/deploy.sh backup
./scripts/deploy.sh undeploy
```

## Checks

```sh
make check
make test
make build
```
