# Tasks server

Go zero-knowledge sync backend for encrypted task blobs.

## Documentation

- Production and self-hosted operation: [`docs/server.md`](../docs/server.md)
- Local server development: [`docs/development/server.md`](../docs/development/server.md)

## Quick local development

From this directory:

```sh
make dev
```

Then check the local API:

```sh
curl http://localhost:18080/healthz
```

The Makefile is for local development only. Production deployment uses the deploy script described in the dedicated server docs and pulls a prebuilt GHCR image.
