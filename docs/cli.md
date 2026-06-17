# CLI

The `tsk` command-line client is the terminal surface for the local-first encrypted task manager.

It is useful for scripted workflows, machine-readable output, diagnostics, local setup, and direct task operations from a shell. It uses the same shared core behavior as the graphical clients.

## Download

Prebuilt `tsk` binaries are in progress.

For now, the CLI can be installed from source with Cargo from this repository. The package is `taskmanager-cli`, and the installed binary is `tsk`.

## What it provides

- Local task commands for creating, listing, searching, updating, completing, reopening, and deleting tasks.
- Profile, configuration, and database path controls for isolated environments.
- Human-readable and machine-readable output modes.
- Account setup, auth diagnostics, device-key workflows, and encrypted sync commands.
- Generated shell completions and a man page for local installations.

## How it fits

The CLI is not a separate task system. It is another client over the same local-first encrypted core. That makes it a good companion to the graphical clients when you want terminal access, automation, reproducible diagnostics, or agent-driven workflows.

Detailed command examples and CLI development checks live in the repository's `cli/README.md`.
