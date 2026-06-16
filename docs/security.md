# Security model

`tsk` uses client-side encryption so the sync server stores encrypted blobs rather than plaintext tasks.

## Key model

The core design uses envelope encryption:

- each account has account-level data key material used to protect task data;
- each device has its own public/private keypair;
- private keys and account data keys are stored through client platform key-store adapters;
- adding another device requires wrapping account data key material for that device public key.

Commands and UI must not print or persist raw private keys or account data keys except in explicit developer diagnostic paths that require opt-in flags.

## Client storage

Secrets are stored with platform-specific mechanisms:

- CLI: native platform key store by default; an explicitly configured file-backed key directory is available for headless development and tests through `TASKMANAGER_INSECURE_KEY_DIR`.
- Linux: Freedesktop Secret Service/libsecret-compatible storage.
- iOS: Keychain device-local items.

Plaintext settings such as server URL are separate from encrypted task content.

## Server trust boundary

The server handles account authentication, JWTs, refresh tokens, device public keys, and encrypted blobs. Deployments must not expose the plaintext Go HTTP port directly to the Internet; terminate HTTPS/TLS in a reverse proxy or managed load balancer and proxy to the local server port.

## Device enrollment

Existing-account enrollment is based on device public keys and wrapped account-data-key payloads. Manual low-level wrap/unwrap commands exist for diagnostics and recovery. Friendlier device-pairing workflows are still planned.
