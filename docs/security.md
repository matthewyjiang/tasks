# Security model

`tsk` uses client-side encryption so the sync server stores encrypted task blobs rather than plaintext tasks.

## The basic promise

Your clients need to read and edit task contents. The server does not.

That boundary shapes the system: clients hold the keys needed for task data, and the server coordinates accounts, devices, sessions, and encrypted blobs.

## What the server can see

The server necessarily handles operational metadata, including account records, sessions, device public keys, cursors, and encrypted blob records. It also handles authentication material such as access and refresh tokens.

The server should not receive plaintext task titles, notes, tags, due dates, or list metadata during normal sync.

## What clients protect

Clients protect account and device key material with platform storage mechanisms such as the Linux key store, iOS Keychain, or the CLI platform key store. Plaintext settings, such as a server URL, are separate from encrypted task content.

## Device enrollment

Adding a device is a trust decision. An enrolled device receives the key material it needs to decrypt synced task data. Current low-level enrollment tools exist for diagnostics and recovery; friendlier pairing workflows are planned.

## Deployment expectations

The server handles sensitive account infrastructure even though it should not see plaintext task contents. Deploy it behind HTTPS/TLS and avoid exposing the plaintext application port directly to the Internet.
