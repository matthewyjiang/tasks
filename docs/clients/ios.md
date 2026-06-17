# iOS app

The iOS client is a native SwiftUI app backed by the shared Rust `taskmanager-core` through UniFFI-generated bindings.

## Official packages

Public TestFlight/App Store distribution is in progress. Until an official iOS download is available, run the app from source in Xcode.

## Run in Simulator

Generate the bindings/XCFramework first, then open the Xcode project:

```sh
./ios/tsk/Scripts/generate-uniffi-bindings.sh
open ios/tsk/tsk.xcodeproj
```

In Xcode, select the `tsk` scheme from `tsk.xcodeproj` and an iOS Simulator destination, then run with Cmd+R.

## Current user-visible behavior

The app supports local-first task flows for creating, editing, completing/reopening, deleting, searching, and organizing tasks. It uses shared core for local storage, encrypted sync, token refresh, and reminder semantics.

Foreground sync is the authoritative sync path. Background refresh is registered with iOS, but execution timing is best-effort and controlled by the system.

## Sync and enrollment

Settings provides sync setup for server URL, email, and password. Credentials are transient view state and are not persisted. Access and refresh tokens are stored in Keychain.

Existing-account enrollment currently uses a manual wrapped account-data-key import path. Friendlier device pairing workflows are still planned; see [Known limitations](../roadmap.md).

## Development notes

Contributor checks, generated binding details, and validation steps live in [iOS development](../development/ios.md) and the repository's `ios/README.md`.
