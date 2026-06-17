# Server setup

The `tsk` server is a Go zero-knowledge sync backend for encrypted task blobs. Normal task content is encrypted by clients before upload; the server stores accounts, sessions, device public keys, cursors, and encrypted blobs.

## Local server for testing

From the `server/` directory, start the development stack:

```sh
cd server
make dev
```

`make dev` starts PostgreSQL with Docker Compose, loads `.env` when present, applies development defaults for missing `DATABASE_URL` and `JWT_SECRET`, runs migrations, and starts the API.

Health check:

```sh
curl http://localhost:18080/healthz
```

Configure the CLI against the local server:

```sh
tsk configure --server-url http://localhost:18080
```

## Deploy safely

The API handles passwords, JWTs, refresh tokens, device public keys, and encrypted blobs. Do not expose the Go server's plaintext HTTP port directly to the Internet. Terminate HTTPS/TLS in a reverse proxy such as Caddy, Nginx, or a managed load balancer, then proxy to the local server port.

On the server host:

```sh
cd server
./scripts/deploy.sh
```

The deploy script asks for non-secret settings, generates missing `POSTGRES_PASSWORD` and `JWT_SECRET`, writes `.env`, builds the Docker image, starts PostgreSQL and the API with Docker Compose, and checks `/healthz` on the loopback interface.

For repeatable deploys:

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

## More details

- [Security model](./security.md) explains the trust boundary and key model.
- [Architecture and sync model](./architecture.md) explains the encrypted blob sync flow.
- The repository's `server/README.md` contains local development commands, sample API calls, and full server maintenance notes.
