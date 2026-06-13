# Issue 92 iOS implementation task list

Branch: `issue-92-ios-app`

Rule for this branch: implement and validate **one phase at a time**. Current active phase: **Phase 1**.

## Phase 1: iOS shell and core binding

- [x] Create implementation branch.
- [x] Add this task list to track implementation phases and completed work.
- [x] Extend Rust UniFFI surface for task lists, plaintext settings, encrypted vault settings, sync status, retry queue, and account/device key helpers needed by iOS.
- [x] Add Rust FFI unit tests covering the new iOS-facing surface.
- [x] Add Swift Package based iOS app shell under `ios/tsk`.
- [x] Add UniFFI binding generation script and README instructions.
- [x] Add basic native SwiftUI shell with sidebar, task list, task detail placeholder, and settings placeholder.
- [x] Add iOS sandbox path helper for the local SQLite database path.
- [x] Add first Swift unit tests for shell/path/filtering helpers.
- [ ] Generate and consume UniFFI Swift bindings for iOS once `cargo` and `uniffi-bindgen` are available in the environment.
- [ ] Wire Swift repository implementation to generated UniFFI bindings and open the real SQLite database.
- [ ] Display the basic task list from the shared Rust core instead of the preview repository.
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
