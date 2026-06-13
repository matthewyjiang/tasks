# tsk iOS

Native SwiftUI iOS client for `tsk`, backed by the shared Rust `taskmanager-core` through UniFFI.

Current implementation scope is Phase 2 from issue #92: offline task UI backed by the shared core. Platform adapter, sync/auth, and background sync phases are intentionally not started yet.

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
