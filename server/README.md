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

## Checks

```sh
make check
```

## Configuration

See `.env.example` for supported environment variables and defaults.
