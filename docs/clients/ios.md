# iOS app

The iOS app is the mobile client for `tsk`. It uses SwiftUI and the same shared encrypted core as the Linux app and CLI.

## Download

Public TestFlight/App Store distribution is in progress.

Until an official iOS download is available, the app can be run from source in Xcode. Contributor setup details live in [iOS development](../development/ios.md) and the repository's `ios/README.md`.

## What it provides

- Local-first task management on iPhone and iPad.
- Foreground encrypted sync with a compatible `tsk` server.
- Device and account secrets stored in Keychain.
- Reminder behavior backed by shared task semantics.
- Background refresh foundations, subject to iOS scheduling limits.

## Why use it

Use the iOS app when you want your tasks on a mobile device without giving up the local-first and encrypted-sync model. Foreground launch, resume, and manual sync remain the dependable sync paths while background refresh support continues to mature.
