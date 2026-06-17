# Clients

`tsk` clients share the same local-first, encrypted core. Choose the client that fits how you want to interact with your tasks.

## Client choices

| Client | Best for | Package status |
| --- | --- | --- |
| [Linux app](./linux.md) | Native desktop task management | Official Arch package: `tsk-linux` from [repo.matthewyjiang.com](https://repo.matthewyjiang.com/). Flatpak is in progress. |
| [iOS app](./ios.md) | Mobile task management | Public TestFlight/App Store distribution is in progress. |
| [CLI](../cli.md) | Terminal workflows, automation, diagnostics, and integrations | Prebuilt binaries are in progress; source install is available today. |

## Shared behavior

All clients use the same underlying model:

- Tasks are stored locally first.
- Sync is explicit infrastructure, not a requirement for using the app.
- Task contents are encrypted before they are uploaded.
- Supported conflicts are resolved consistently through shared core behavior.

For deeper design details, see [Architecture and sync model](../architecture.md). For current gaps, see [Known limitations](../roadmap.md).
