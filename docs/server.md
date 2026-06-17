# Server setup

The `tsk` server coordinates encrypted sync. It is not the source of truth for plaintext tasks.

Clients keep local task data, encrypt task changes, and upload encrypted blobs. The server stores the account, session, device, cursor, and blob metadata needed to let enrolled clients find and exchange those encrypted changes.

## When you need a server

You only need a server when you want sync across devices or a self-hosted encrypted task stream. Single-device task management works locally without one.

## Trust boundary

The server handles sensitive account infrastructure such as passwords, sessions, JWTs, refresh tokens, device public keys, and encrypted blobs. It should be deployed behind HTTPS/TLS. Do not expose the plaintext Go HTTP port directly to the Internet.

## Operating model

A typical deployment places the Go server and PostgreSQL behind a reverse proxy or managed load balancer that terminates TLS. Clients connect to the public HTTPS endpoint, authenticate, and exchange encrypted blob data.

The repository includes a deploy script and Docker Compose setup for running the server. Detailed commands and maintenance notes live in the repository's `server/README.md`.

## Related pages

- [Security model](./security.md) explains what the server can and cannot read.
- [Architecture and sync model](./architecture.md) explains how local-first sync fits together.
