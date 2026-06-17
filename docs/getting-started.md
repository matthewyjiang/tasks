# Get started

Start by choosing the client for the device where you want to use `tsk` day to day. The graphical clients are the natural entry points for interactive task management, while the CLI remains available for terminal workflows, scripting, diagnostics, and integrations.

## Choose a client

| Platform | Start here | Notes |
| --- | --- | --- |
| Linux desktop | [Linux app](./clients/linux.md) | GTK/libadwaita desktop client with local-first tasks, encrypted sync, token refresh, and reminders. Official Arch package: `tsk-linux`; Flatpak is in progress. |
| iPhone / iPad | [iOS app](./clients/ios.md) | SwiftUI client with local-first tasks, foreground sync, background refresh foundations, and reminders. TestFlight/App Store distribution is in progress. |
| Terminal / automation | [CLI guide](./cli.md) | Command-line client for task commands, sync, machine-readable output, diagnostics, and scripted workflows. Prebuilt binaries are in progress. |

See [Client status](./clients/index.md) for the current support matrix and package/download notes.

## Local-first basics

All clients are designed around the same model:

1. Create and edit tasks against local storage first.
2. Keep working while offline.
3. Encrypt task blobs on the client before sync.
4. Push and pull encrypted blobs through the server when one is configured.

## Add encrypted sync

Run or deploy a compatible `tsk` server, then configure your preferred client with the server URL and account credentials. For local testing, the server usually runs at:

```text
http://localhost:18080
```

The CLI setup flow is useful for scripted or terminal-first setup:

```sh
tsk configure \
  --server-url http://127.0.0.1:18080 \
  --email you@example.com \
  --password "$TASKMANAGER_PASSWORD"
```

See [Server setup](./server.md) for local and deployed server options.

## Next steps

- [Linux app](./clients/linux.md) for desktop usage.
- [iOS app](./clients/ios.md) for mobile usage.
- [CLI guide](./cli.md) for command examples and output modes.
- [Security model](./security.md) for the zero-knowledge sync boundary.
- [Known limitations](./roadmap.md) for workflows that are still evolving.
