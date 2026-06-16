# iOS app

The iOS client is a native SwiftUI app backed by shared Rust core through UniFFI.

## Project layout

```text
ios/tsk/Package.swift
ios/tsk/tsk.xcodeproj
ios/tsk/Scripts/generate-uniffi-bindings.sh
ios/tsk/Sources/tsk/
ios/tsk/Sources/TskCore/
ios/tsk/Tests/TskCoreTests/
```

Generated Swift bindings live under `ios/tsk/Sources/TskCore/Generated/` and should not be hand-edited.

## Run in Simulator

Generate bindings/XCFramework first, then open the Xcode project:

```sh
./ios/tsk/Scripts/generate-uniffi-bindings.sh
open ios/tsk/tsk.xcodeproj
```

In Xcode, select the `tsk` scheme from `tsk.xcodeproj` and an iOS Simulator destination. The Xcode project supplies the real app bundle and bundle identifier required by UIKit.

## Development checks

```sh
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --package-path ios/tsk
./ios/tsk/Scripts/generate-uniffi-bindings.sh
xcodebuild -project ios/tsk/tsk.xcodeproj -scheme tsk -destination 'generic/platform=iOS Simulator' build
```

## Sync, auth, and background refresh

The app stores server URL in `UserDefaults` and access/refresh tokens in Keychain. Foreground sync uses the shared core sync orchestration path and server endpoints. Expired access tokens trigger refresh, token rotation, and one retry.

Background refresh is registered with identifier `com.matthewyjiang.tsk.refresh`. iOS controls when background refresh actually runs, so foreground launch/resume/manual sync remains authoritative.
