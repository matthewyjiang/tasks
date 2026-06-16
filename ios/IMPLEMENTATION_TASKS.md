# Issue 92 iOS implementation task list

Branch: `issue-92-ios-phase2-offline-ui`

Rule for this branch: implement and validate **one phase at a time**. Current active phase: **Phase 4**.

SwiftUI rule: keep the iOS app on intended native SwiftUI containers/modifiers first (`NavigationStack`, `NavigationSplitView`, `List`, `Form`, `.searchable`, toolbars, selection bindings). Avoid custom geometry, fixed-width panes, safe-area compensations, or gesture/state workarounds unless a native approach has been ruled out and the reason is documented.

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

Implemented on `issue-92-ios-phase2-offline-ui`.

- [x] Add/create/edit/delete task flows backed by core.
- [x] Add smart views over the local core-backed task database.
- [x] Add user lists backed by core.
- [x] Add due dates, tags, done/open state, and list assignment persistence through core.
- [x] Add native local search over core-backed task data.
- [x] Refactor regular-width iPad UI onto native SwiftUI `NavigationSplitView` and sidebar `List(selection:)`, removing custom pane/header/sidebar layout workarounds.
- [x] Review compact iPhone UI for layout/navigation workarounds and keep it on native `NavigationStack`, `List`, `NavigationLink`, and `Form` patterns.

## Phase 3: iOS platform adapter

Implemented on `issue-92-ios-phase2-offline-ui` after Phase 2 completion.

- [x] Add production device-only, after-first-unlock Keychain storage integration.
- [x] Add notification scheduling/cancellation hooks.
- [x] Add reachability monitoring.
- [x] Add iOS app path integration.
- [x] Add local-first onboarding/account bootstrap.

## Phase 4: Sync/auth parity

Implemented on `ios-phase4-sync-auth-92`.

- [x] Add sync setup/sign-in flow.
- [x] Add basic device enrollment/key unwrap flow for existing accounts.
- [x] Store access/refresh tokens in Keychain.
- [x] Implement foreground sync.
- [x] Implement token refresh and one sync retry.
- [x] Surface real sync status.

## Phase 5: Background sync and polish

Not started. Do not begin until Phase 4 is complete.

- [ ] Register background refresh.
- [ ] Reconcile notifications after background/foreground sync once reminder semantics exist.
- [ ] Add iPad adaptive layout polish.
- [ ] Evaluate a native three-column iPad split view for sidebar, task list, and task detail once Phase 4 sync flows are stable.
- [ ] Add UI smoke tests or snapshot-style checks for iPad sidebar/task-list geometry to catch regressions in split-view layout.
- [ ] Add accessibility pass.
- [ ] Add README and development checks.
- [ ] Document Linux API reuse opportunities.

## Validation notes

- Rust checks pass when the installed toolchain is added to PATH:
  - `PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH" cargo fmt --check`
  - `PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH" cargo test -p taskmanager-core`
  - `PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH" cargo clippy -p taskmanager-core --all-targets -- -D warnings`
- Swift package tests pass with full Xcode selected via `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --package-path ios/tsk` from the repository root, or with an absolute package path from `ios/`.
- `ios/tsk/Scripts/generate-uniffi-bindings.sh` builds the Rust core for `aarch64-apple-darwin`, `aarch64-apple-ios-sim`, and `aarch64-apple-ios`, then packages those static libraries plus the UniFFI header/module map into `ios/tsk/Frameworks/TaskmanagerCore.xcframework`.
- The Swift package consumes that XCFramework through the `taskmanager_coreFFI` binary target, allowing the same generated bindings to compile for SwiftPM tests.
- Simulator/device app launches should use `ios/tsk/tsk.xcodeproj` from the repo root, or `tsk/tsk.xcodeproj` from the `ios/` directory. The project builds a real `tsk.app` bundle with `com.matthewyjiang.tsk`; running a Swift package executable directly can crash in UIKit because SwiftPM executables do not provide an iOS app bundle identifier.
