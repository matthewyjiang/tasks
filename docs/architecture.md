# Architecture and sync model

`tsk` is organized around a shared local-first core with thin platform clients.

The goal is to make task behavior consistent across platforms without forcing every client to reimplement encryption, sync, conflict handling, and local persistence rules.

## Shape of the system

- **Clients** provide the user interface or command-line surface for a platform.
- **Shared core** owns platform-independent task behavior, local persistence, encryption, sync orchestration, and conflict rules.
- **Platform adapters** handle OS-specific concerns such as key stores, HTTP transport, notifications, reachability, and UI.
- **Server** coordinates encrypted blob sync without reading task contents.

## Why this structure works

Local task operations are fast and resilient because they happen on the device first. Sync can fail, pause, or retry without making the client unusable.

Encryption belongs to the client side because clients are the only place plaintext task data should be needed. The server can still coordinate devices and blobs, but it does not need to understand the task content it stores.

Shared core keeps behavior predictable. A task created from the CLI, edited on Linux, and synced to iOS should follow the same rules for storage, encryption, sync, and conflict handling.

## Sync flow

At a high level, sync works like this:

1. A client changes local task data.
2. The client encrypts the change into a blob.
3. The server stores and indexes the encrypted blob.
4. Another enrolled client asks for newer blobs.
5. That client downloads, decrypts, and applies the change locally.

## Conflict behavior

When multiple clients change the same task, core resolves supported conflicts deterministically. The current policy favors the newest update and uses a stable tie-breaker when timestamps are equal.

This is intentionally simple and predictable. Richer conflict inspection and resolution workflows are tracked as future work.
