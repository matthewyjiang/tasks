# Development

Use these pages for supported local workflows:

- [Linux app](./linux.md)
- [iOS app](./ios.md)
- [Server](./server.md)

Core behavior should be implemented in `taskmanager-core` first when it is platform-agnostic. UniFFI/FFI and platform apps should adapt native core APIs rather than containing separate business logic.
