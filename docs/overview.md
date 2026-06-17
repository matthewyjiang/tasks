# Overview

`tsk` is a local-first, end-to-end encrypted task manager.

It is designed around a simple idea: your tasks should feel fast and dependable on the device in front of you, and sync should be an added capability rather than a requirement for basic use.

## Philosophy

- **Local first.** Task creation, editing, search, completion, and deletion happen against local storage first, so the app remains useful offline.
- **Private by default.** Task contents are encrypted on the client before they are sent to a sync server.
- **One shared model.** The Linux app, iOS app, and CLI share the same core task and sync semantics so behavior stays consistent across clients.
- **Thin platform clients.** Each client should feel native to its platform while relying on shared core behavior for task data, encryption, sync, and conflict handling.

## Why it works

`tsk` separates the parts of task management that must be trusted from the parts that only need to move data around.

Your device owns plaintext task data and cryptographic keys. The sync server stores accounts, sessions, device public keys, cursors, and encrypted blobs, but not readable task titles, notes, tags, or due dates. That keeps the server useful for coordination without making it the source of truth for your private content.

## Choose your path

- [Get started](./getting-started.md) by choosing the client for your platform.
- [Compare clients](./clients/index.md) to see current package and distribution status.
- [Run a sync server](./server.md) when you want self-hosted encrypted sync.
- [Review known limitations](./roadmap.md) before relying on a workflow that is still evolving.
- [Read the architecture](./architecture.md) or [security model](./security.md) for a deeper explanation of the design.
