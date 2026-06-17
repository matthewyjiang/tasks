# Get started

Start by choosing the client for the device where you want to manage tasks day to day.

The graphical clients are the natural entry points for interactive task management. The CLI is available for terminal workflows, automation, diagnostics, and integrations.

## Pick a client

| Platform | Client | Package status |
| --- | --- | --- |
| Linux desktop | [Linux app](./clients/linux.md) | Official Arch package: `tsk-linux` from [repo.matthewyjiang.com](https://repo.matthewyjiang.com/). Flatpak is in progress. |
| iPhone / iPad | [iOS app](./clients/ios.md) | Public TestFlight/App Store distribution is in progress. |
| Terminal / automation | [CLI](./cli.md) | Prebuilt binaries are in progress; source install is available today. |

## What to expect

Whichever client you choose, `tsk` follows the same local-first model:

1. Your device keeps its own local task database.
2. You can keep working without a network connection.
3. When sync is configured, task contents are encrypted before leaving the device.
4. Other enrolled clients can pull and decrypt the same encrypted task history.

## Add sync when you need it

Sync is optional for single-device use. Add it when you want multiple devices or a backed-up encrypted task stream.

The server is intentionally not a general-purpose task database. It coordinates accounts, devices, sessions, and encrypted blobs. Your clients remain responsible for plaintext task data and keys.

Continue with [Server setup](./server.md) when you are ready to run a sync server.
