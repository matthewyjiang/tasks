# Issue 92 iOS implementation task list

Branch: `issue-92-ios-app`

Rule for this branch: implement and validate **one phase at a time**. Current active phase: **Phase 1**.

## Phase 1: iOS shell and core binding

- [x] Create implementation branch.
- [x] Add this task list to track implementation phases and completed work.
- [x] Extend Rust UniFFI surface for task lists, plaintext settings, encrypted vault settings, sync status, retry queue, and account/device key helpers needed by iOS.
- [x] Add Rust FFI unit tests covering the new iOS-facing surface.
- [x] Add Swift Package based iOS app shell under `ios/tsk`.
- [x] Add a runnable Xcode iOS app target with a real app bundle identifier for Simulator/device launches.
- [x] Add UniFFI binding generation script and README instructions.
- [x] Add basic native SwiftUI shell with sidebar, task list, task detail placeholder, and settings placeholder.
- [x] Add iOS sandbox path helper for the local SQLite database path.
- [x] Add first Swift unit tests for shell/path/filtering helpers.
- [x] Generate and consume UniFFI Swift bindings for iOS.
- [x] Fix the binding generation/build script to produce a `TaskmanagerCore.xcframework` with macOS, iOS Simulator, and physical iOS device slices.
- [x] Move the UniFFI header/module map into the XCFramework and keep only generated Swift source under `Sources/TskCore/Generated/`.
- [x] Update `Package.swift` to link `TskCore` against the generated `taskmanager_coreFFI` binary target instead of a local debug dylib.
- [x] Add build-script hardening: automatic Rust target installation, full-Xcode selection, and capped Cargo parallelism for cross-build memory usage.
- [x] Wire Swift repository implementation to generated UniFFI bindings and open the real SQLite database.
- [x] Display the basic task list from the shared Rust core instead of the preview repository.
- [x] Run and fix current Rust Phase 1 validation checks (`cargo fmt --check`, `cargo test -p taskmanager-core`, `cargo clippy -p taskmanager-core --all-targets -- -D warnings`).
- [x] Run and fix current Swift Phase 1 validation checks (`DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --package-path ios/tsk`).

## Phase 2: Offline task UI

Not started. Do not begin until Phase 1 is complete.

- [ ] Add/create/edit/delete task flows backed by core.
- [ ] Add smart views backed by core filters.
- [ ] Add user lists backed by core.
- [ ] Add due dates, tags, done/open state, and list assignment persistence through core.
- [ ] Add native search backed by core/local database behavior.

## Phase 3: iOS platform adapter

Not started. Do not begin until Phase 2 is complete.

- [ ] Add production device-only, after-first-unlock Keychain storage integration.
- [ ] Add notification scheduling/cancellation hooks.
- [ ] Add reachability monitoring.
- [ ] Add iOS app path integration.
- [ ] Add local-first onboarding/account bootstrap.

## Phase 4: Sync/auth parity

Not started. Do not begin until Phase 3 is complete.

- [ ] Add sync setup/sign-in flow.
- [ ] Add basic device enrollment/key unwrap flow for existing accounts.
- [ ] Store access/refresh tokens in Keychain.
- [ ] Implement foreground sync.
- [ ] Implement token refresh and one sync retry.
- [ ] Surface real sync status.

## Phase 5: Background sync and polish

Not started. Do not begin until Phase 4 is complete.

- [ ] Register background refresh.
- [ ] Reconcile notifications after background/foreground sync once reminder semantics exist.
- [ ] Add iPad adaptive layout polish.
- [ ] Add accessibility pass.
- [ ] Add README and development checks.
- [ ] Document Linux API reuse opportunities.

## Validation notes

- Rust checks pass when the installed toolchain is added to PATH:
  - `PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH" cargo fmt --check`
  - `PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH" cargo test -p taskmanager-core`
  - `PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH" cargo clippy -p taskmanager-core --all-targets -- -D warnings`
- Swift package tests pass with full Xcode selected via `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --package-path ios/tsk`.
- `ios/tsk/Scripts/generate-uniffi-bindings.sh` builds the Rust core for `aarch64-apple-darwin`, `aarch64-apple-ios-sim`, and `aarch64-apple-ios`, then packages those static libraries plus the UniFFI header/module map into `ios/tsk/Frameworks/TaskmanagerCore.xcframework`.
- The Swift package consumes that XCFramework through the `taskmanager_coreFFI` binary target, allowing the same generated bindings to compile for SwiftPM tests.
- Simulator/device app launches should use `ios/tsk/tsk.xcodeproj` from the repo root, or `tsk/tsk.xcodeproj` from the `ios/` directory. The project builds a real `tsk.app` bundle with `com.matthewyjiang.tsk`; running a Swift package executable directly can crash in UIKit because SwiftPM executables do not provide an iOS app bundle identifier.
