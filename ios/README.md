# tsk iOS

Native SwiftUI iOS client for `tsk`, backed by the shared Rust `taskmanager-core` through UniFFI.

Current implementation scope is Phase 1 from issue #92: app shell and core binding surface. Later offline UI, platform adapter, sync/auth, and background sync phases are intentionally not started yet.

## Structure

```text
ios/
  IMPLEMENTATION_TASKS.md        Phase-by-phase task tracking
  README.md
  tsk/
    Package.swift                Swift Package app shell
    Scripts/generate-uniffi-bindings.sh
    Sources/
      tsk/                       SwiftUI app entry point
      TskCore/
        Generated/               UniFFI Swift output target
        Models/
        Platform/
        Services/
        Views/
    Tests/TskCoreTests/
```

## Phase 1 checks

Use the full Xcode developer directory when running SwiftPM tests from an environment where Command Line Tools are selected:

```sh
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --package-path ios/tsk
```

Generate UniFFI Swift bindings after Rust tooling is installed:

```sh
./ios/tsk/Scripts/generate-uniffi-bindings.sh
```

The script builds `taskmanager-core`, runs a temporary `uniffi_bindgen = 0.31.1` runner, and writes Swift bindings under `ios/tsk/Sources/TskCore/Generated/`. The Swift package links against the local debug core dylib for Phase 1 development and tests.
