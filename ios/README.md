# tsk iOS

Native SwiftUI iOS client for `tsk`, backed by the shared Rust `taskmanager-core` through UniFFI.

For the user-facing app status and run path, start with [`docs/clients/ios.md`](../docs/clients/ios.md). This README keeps implementation status, generated binding details, and validation notes close to the iOS source.

Current implementation scope covers Issue #92 Phase 5 background sync foundations and polish on top of the Phase 4 foreground sync/auth implementation.

## Structure

```text
ios/
  IMPLEMENTATION_TASKS.md        Phase-by-phase task tracking
  README.md
  tsk/
    Package.swift                Swift Package library/tests entry point
    tsk.xcodeproj                Runnable iOS app target for Simulator/device
    Frameworks/                  Generated UniFFI XCFramework output
      TaskmanagerCore.xcframework
    Scripts/generate-uniffi-bindings.sh
    Sources/
      tsk/                       SwiftUI app entry point
      TskCore/
        Generated/               UniFFI Swift source output target
        Models/
        Platform/
        Services/
        Views/
    Tests/TskCoreTests/
```

## Running in the iOS Simulator

Generate the bindings/XCFramework first, then open the Xcode project, not the Swift package executable.

From the repository root:

```sh
./ios/tsk/Scripts/generate-uniffi-bindings.sh
open ios/tsk/tsk.xcodeproj
```

From this `ios/` directory:

```sh
./tsk/Scripts/generate-uniffi-bindings.sh
open tsk/tsk.xcodeproj
```

In Xcode, select the `tsk` scheme from `tsk.xcodeproj` and an iOS Simulator destination, then run with Cmd+R. The `.xcodeproj` supplies the real iOS app bundle and `com.matthewyjiang.tsk` bundle identifier required by UIKit. Running a Swift package executable target directly in the simulator can fail with `__BKSHIDEvent__BUNDLE_IDENTIFIER_FOR_CURRENT_PROCESS_IS_NIL__` because it is not packaged as an iOS `.app` bundle.

## Development checks

Use the full Xcode developer directory when running SwiftPM tests from an environment where Command Line Tools are selected:

```sh
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --package-path ios/tsk
```

Generate UniFFI Swift bindings and the native core library after Rust tooling is installed:

```sh
./ios/tsk/Scripts/generate-uniffi-bindings.sh
```

The script now builds `taskmanager-core` for all Apple slices needed by local development and device deployment:

- `aarch64-apple-darwin` for SwiftPM tests on Apple Silicon Macs.
- `aarch64-apple-ios-sim` for the iOS Simulator.
- `aarch64-apple-ios` for physical iOS devices.

It installs missing Rust targets with `rustup`, prefers the full Xcode developer directory when available, caps Cargo build parallelism with `CARGO_BUILD_JOBS` (default `2`) to avoid memory pressure, runs a temporary `uniffi_bindgen = 0.31.1` runner, and creates `ios/tsk/Frameworks/TaskmanagerCore.xcframework`. The XCFramework carries the UniFFI C header and module map, while the generated Swift source stays in `ios/tsk/Sources/TskCore/Generated/`.

`Package.swift` consumes the generated XCFramework as the `taskmanager_coreFFI` binary target, so the same package can compile against the shared Rust core for SwiftPM tests, Simulator builds, and iOS device builds.

## Implemented offline UI

Phase 2 adds native SwiftUI flows for creating, editing, completing/reopening, and deleting tasks offline. Task detail editing persists title, notes, due date, list assignment, tags, and open/done status through the Rust core. The sidebar supports creating, renaming, deleting, and selecting user lists. Built-in views and `.searchable` remain local-first over local core-backed SQLite data.

## Implemented platform adapters

Phase 3 initializes local-first device/account keys on first launch before sign-in is required. The device private key and account data key are stored in device-only Keychain items available after first unlock. The app also has native `NWPathMonitor` reachability plumbing and `UNUserNotificationCenter` schedule/cancel hooks backed by shared core reminder semantics.

## Implemented foreground sync/auth

Phase 4 adds foreground sync/auth parity through shared Rust core APIs and thin iOS platform adapters:

- Settings provides a native SwiftUI sync setup form for server URL, email, and password. Credentials are transient view state and are not persisted.
- The server URL is local plaintext app metadata stored in `UserDefaults` under `tsk.sync.serverURL`.
- Access and refresh tokens are stored in Keychain using the same core-generated secret IDs consumed by Rust (`auth_access_token` and `auth_refresh_token`).
- The iOS HTTP auth adapter uses the same server endpoints as Linux/shared core: `/auth/register`, `/auth/login`, `/auth/refresh`, `/auth/session`, and `/keys/me`.
- Foreground sync uses the same encrypted blob protocol as Linux (`/blobs/batch`, `/blobs/{task_id}`, and `/blobs?since=...`) via the shared core `sync_run` orchestration API.
- Expired access tokens trigger a shared-core refresh flow, store the rotated token pair, and retry the sync once.
- Existing-account enrollment supports a basic manual wrapped account-data-key import path. Paste JSON with base64 `sender_public_key`, `recipient_public_key`, `ciphertext`, and `nonce`; the shared core unwraps and stores the account data key only if it is addressed to this device and no account data key already exists.
- Settings and the sidebar surface online/offline state, auth/enrollment state, dirty count, retry queue depth, cursor, and the last failed/conflict count.

## Implemented background sync and polish

Phase 5 registers iOS background refresh at app launch using `BGAppRefreshTask` with the permitted identifier `com.matthewyjiang.tsk.refresh` and the `fetch` background mode in the app `Info.plist`. Background refresh is best-effort: the app schedules refreshes on launch and when entering the background, then runs the same shared-core sync path used by foreground sync when iOS grants execution time. Foreground launch/resume/manual sync remains authoritative because iOS can defer or skip background work.

After foreground or background sync pulls task changes, the app reloads tasks/lists and reconciles local notifications through shared core reminder semantics, so changed/deleted synced tasks update or cancel their stable task-UUID notification requests.

Polish added in this phase:

- The Xcode app target now includes all Phase 4/5 Swift sources used by the runnable iOS app, not just SwiftPM tests.
- Settings documents background refresh status alongside notification platform behavior.
- Unit coverage verifies background scheduler registration/scheduling and that failed background sync does not replace the user's visible error state.
- Xcode simulator builds cover the app bundle, custom `Info.plist` metadata/background configuration, and full source target membership.

Manual Phase 5 validation:

1. Build the app with `xcodebuild -project ios/tsk/tsk.xcodeproj -scheme tsk -destination 'generic/platform=iOS Simulator' build`.
2. Launch the app, configure sync in Settings, and verify foreground **Sync Now** still succeeds.
3. Background the app and verify a `BGAppRefreshTaskRequest` can be submitted without crashing. Device-level execution timing is controlled by iOS and is not guaranteed in Simulator.
4. Change a task reminder on another device, sync, and verify the local notification request is updated or cancelled after the pull.
5. Use Dynamic Type, dark mode, and VoiceOver rotor/navigation to smoke-test the native `NavigationStack`, `NavigationSplitView`, `List`, `Form`, labels, and row accessibility actions.

Manual Phase 4 validation:

1. Start a compatible `tsk` server.
2. Launch the iOS app and open Settings.
3. Enter the server URL, email, and password, then use **Sign In or Register**.
4. For an existing account, copy the displayed device public key to an already-enrolled device, create a wrapped account-data-key payload, and paste the JSON into the enrollment field.
5. Create or edit tasks offline, return online, then use **Sync Now**.
6. Force an expired access token and verify foreground sync refreshes tokens and retries once.
7. Verify retry queue/dirty counts remain visible and survive app restart when sync cannot complete.
