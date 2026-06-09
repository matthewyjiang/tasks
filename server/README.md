# Tasks server

Go zero-knowledge sync backend for encrypted task blobs.

## Local development

```sh
cp .env.example .env
make docker-up
set -a; . ./.env; set +a
make run
```

Health check:

```sh
curl http://localhost:8080/healthz
```

## Deploy on a server

SSH into the server, clone/update the repo, then run:

```sh
cd server
./scripts/deploy.sh
```

The script interactively asks for required settings, writes `.env`, builds the app image, starts PostgreSQL and the API with Docker Compose, and checks `/healthz`. `HOST_PORT` controls the public host port; the app container always listens on `PORT=8080`.

## Checks

```sh
make check
```

## Configuration

See `.env.example` for supported environment variables and defaults.
