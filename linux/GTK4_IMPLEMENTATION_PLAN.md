# tsk Linux GTK4 Implementation Plan

## App identity

Use the new user-facing app name **tsk** for the Linux GUI now. Broader repository/package renaming can happen later.

- Display name: `tsk`
- Binary name: `tsk-gui`
- Cargo package: `tsk-linux`
- App ID: `io.github.matthewyjiang.tsk`
- Desktop file: `io.github.matthewyjiang.tsk.desktop`
- Metainfo file: `io.github.matthewyjiang.tsk.metainfo.xml`
- Config dir: `~/.config/tsk/`
- Data dir: `~/.local/share/tsk/`
- Database: `~/.local/share/tsk/tasks.sqlite3`
- Secret service name: `tsk`

Keep the GUI binary as `tsk-gui` for now because the existing CLI already uses `tsk`.

## Progress tracker

- [x] App identity settled on `io.github.matthewyjiang.tsk` / display name `tsk` / binary `tsk-gui`.
- [x] Linux crate added as `tsk-linux`.
- [x] Basic GTK4/libadwaita app boots and opens the core database.
- [x] Linux path handling added with `TSK_LINUX_DB` and `TSK_LINUX_CONFIG` overrides.
- [x] Initial task list, filters, search, editor, create/save/delete flow implemented.
- [x] Things-inspired three-pane visual polish added with styled sidebar, list cards, empty state, and cleaner editor.
- [x] Removed copied placeholder Things content; sidebar now uses core-accurate lists, live counts, and tags derived from actual tasks.
- [x] Removed hard-coded light colors from CSS so the app follows the system GTK/libadwaita theme.
- [x] Simplified the layout to one sidebar plus one main task list window.
- [x] Removed divider lines between sidebar and main content; sidebar distinction now comes from theme background color only.
- [x] Sidebar now owns the full window height instead of starting below the header.
- [x] Removed sidebar title/section labels for a more minimal layout.
- [x] Sidebar category rows now share the same base color, with only a subtle rounded hover state.
- [x] Disabled persistent sidebar row selection so categories return to the sidebar background after click.
- [x] Simplified core task status to two states: `Open` and `Done`.
- [x] Updated Linux sidebar views to `Today`, `Upcoming`, `Anytime`, and `Done`.
- [x] Minimalized counts: only the sidebar `Today` row shows a number; section titles and tags do not.
- [x] Added small colored icons to sidebar view rows.
- [x] Restored Inbox as the first built-in sidebar view.
- [x] Added core-backed user-defined task lists and display them below built-in sidebar views.
- [x] User-defined lists can be renamed inline and deleted from a row action menu.
- [x] New tasks are created in Inbox, task titles can be edited inline, and row actions include mark open/done and delete.
- [x] Task list layout expands to fill the available vertical space.
- [x] Linux platform adapter skeleton added with libsecret keyring and notifications.
- [x] Initial settings JSON helpers and onboarding detection helper added.
- [x] Component placeholder modules added for later extraction.
- [x] Onboarding detection and local account initialization wired into startup.
- [ ] Add due date editor.
- [ ] Add full settings dialog.
- [ ] Add sync facade/client/UI.
- [ ] Add Flatpak packaging.
- [x] Validation completed for the initial GTK4 offline CRUD implementation.
- [ ] Create PR to `main`.

## Recommended stack

Use:

```text
Relm4 + GTK4 + libadwaita
```

Reasons:

- Native Linux/GNOME look and behavior.
- Rust-first integration with the existing `core/` crate.
- Relm4 provides a clean message/update model for task lists, editors, sync state, settings, and async commands.
- `libadwaita` provides modern Linux widgets, adaptive layouts, preferences UI, toasts, header bars, and theme integration.

## System dependencies

Ubuntu/Debian development packages:

```sh
sudo apt install libgtk-4-dev libadwaita-1-dev libsecret-1-dev pkg-config
```

## Phase 1: Add Linux crate

Update root `Cargo.toml`:

```toml
[workspace]
members = ["core", "cli", "linux"]
resolver = "2"
```

Create `linux/Cargo.toml`:

```toml
[package]
name = "tsk-linux"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "tsk-gui"
path = "src/main.rs"

[dependencies]
taskmanager-core = { path = "../core" }
relm4 = { version = "0.9", features = ["libadwaita"] }
gtk4 = "0.9"
libadwaita = "0.7"
directories = "5"
keyring = { version = "3.6", default-features = false, features = ["linux-native-sync-persistent", "crypto-rust", "vendored"] }
notify-rust = "4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = "1"
thiserror = "1"
chrono = "0.4"
```

Use exact dependency versions after checking current Relm4/gtk-rs compatibility.

## Proposed file layout

```text
linux/
  Cargo.toml
  README.md
  GTK4_IMPLEMENTATION_PLAN.md
  src/
    main.rs
    app.rs
    paths.rs
    platform.rs
    error.rs
    task_model.rs
    sync_client.rs
    ui/
      mod.rs
      shell.rs
      task_list.rs
      task_editor.rs
      search.rs
      settings.rs
      onboarding.rs
  resources/
    io.github.matthewyjiang.tsk.desktop
    io.github.matthewyjiang.tsk.metainfo.xml
```

## Phase 2: Paths and app startup

Implement `linux/src/paths.rs`.

Responsibilities:

- Resolve Linux data/config paths.
- Ensure parent directories exist.
- Use:
  - DB: `~/.local/share/tsk/tasks.sqlite3`
  - Config: `~/.config/tsk/settings.json`
- Support environment overrides for tests/development:
  - `TSK_LINUX_DB`
  - `TSK_LINUX_CONFIG`

Implement `linux/src/main.rs`.

Responsibilities:

- Initialize GTK/libadwaita.
- Create a `RelmApp` using app ID `io.github.matthewyjiang.tsk`.
- Start the root app component.
- Open `TaskManagerCore` with the database path.

## Phase 3: App model

Implement `linux/src/app.rs`.

The app should depend directly on the Rust core crate, not UniFFI.

Use core types directly where possible:

```rust
use taskmanager_core::{
    TaskManagerCore,
    Task,
    TaskPatch,
    TaskFilter,
    TaskSort,
    TaskStatus,
};
```

Suggested app state:

```rust
struct AppModel {
    core: TaskManagerCore,
    tasks: Vec<Task>,
    selected_task_id: Option<uuid::Uuid>,
    search_query: String,
    active_filter: TaskFilterState,
    sync_state: SyncUiState,
    error: Option<String>,
}
```

Suggested messages:

```rust
enum AppMsg {
    LoadTasks,
    TasksLoaded(Vec<Task>),
    CreateTask,
    SelectTask(uuid::Uuid),
    UpdateTitle(String),
    UpdateBody(String),
    SetStatus(TaskStatus),
    SetDueAt(Option<i64>),
    SetTags(Vec<String>),
    DeleteSelected,
    SearchChanged(String),
    SyncNow,
    ShowSettings,
    Error(String),
}
```

## Phase 4: Main UI shell

Implement `linux/src/ui/shell.rs`.

Use libadwaita widgets:

- `AdwApplicationWindow`
- `AdwHeaderBar`
- `AdwToastOverlay`
- `AdwNavigationSplitView` or `AdwOverlaySplitView`

Main layout:

```text
Window
  Header bar
    New task
    Search
    Sync
    Settings
  Split view
    Sidebar filters
    Task list
    Task editor/detail pane
```

Sidebar filters:

- Inbox
- In Progress
- Done
- Due soon
- All
- Settings

## Phase 5: Task list

Implement `linux/src/ui/task_list.rs`.

Show each task with:

- Title
- Status badge
- Due date
- Tags
- Dirty/sync indicator later

Back task listing with:

```rust
core.list_tasks(filter, sort)
```

Back search with:

```rust
core.search_tasks(query)
```

Initial sort can be updated-date descending or due-date ascending.

## Phase 6: Task editor

Implement `linux/src/ui/task_editor.rs`.

Fields:

- Title
- Body/notes
- Status: Inbox / In Progress / Done
- Due date/time
- Tags
- Delete button

Use core methods:

```rust
core.update_task(task_id, TaskPatch { ... })
core.delete_task(task_id)
```

Avoid writing to SQLite on every keystroke. Prefer one of:

- save on focus loss
- explicit save/apply
- short debounce for text fields

## Phase 7: Search

Implement `linux/src/ui/search.rs`.

Behavior:

- Empty search uses active sidebar filter.
- Non-empty search calls `TaskManagerCore::search_tasks`.
- Search should remain offline/local-first.

## Phase 8: Linux platform adapter

Implement `linux/src/platform.rs` for the core `Platform` trait.

Use:

- `keyring`/libsecret for key storage
- `notify-rust` for desktop notifications

Responsibilities:

- `store_key`
- `load_key`
- `delete_key`
- `schedule_notification`
- `cancel_notification`
- `network_available`

Initial behavior:

- Real libsecret-backed key storage.
- Immediate notifications via `notify-rust`.
- Scheduled notifications can be deferred or implemented as a no-op initially.
- Network availability can initially return `true` unless offline mode is added.

## Phase 9: Onboarding

Implement `linux/src/ui/onboarding.rs`.

First-launch flow:

1. Detect missing account/device keys.
2. Show a simple welcome screen.
3. Offer **Create local account**.
4. Call core account initialization.
5. Store keys through the Linux platform adapter.
6. Open the main task UI.

Do not display or log secret key material.

Sync login/register can come later.

## Phase 10: Settings

Implement `linux/src/ui/settings.rs`.

Initial settings:

- Server URL
- Theme: system/light/dark
- Show completed tasks
- Default sort
- Display density
- Last sync status/cursor display

Store plaintext app preferences in:

```text
~/.config/tsk/settings.json
```

Use libadwaita style manager for theme switching.

## Phase 11: Sync integration

Initial GUI can include a disabled or placeholder sync button.

For real sync, add a small core facade so GUI code does not need to access private database internals:

```rust
impl TaskManagerCore {
    pub fn sync_push(
        &self,
        platform: &dyn Platform,
        client: &dyn SyncClient,
        data_key: &[u8],
    ) -> CoreResult<SyncResult>;

    pub fn sync_pull(
        &self,
        client: &dyn SyncClient,
        data_key: &[u8],
    ) -> CoreResult<SyncResult>;
}
```

Then implement `linux/src/sync_client.rs` using `reqwest` and the server wire format.

Sync UI states:

- Idle
- Syncing
- Last synced timestamp
- Offline
- Error

## Phase 12: Packaging

Add resources:

```text
linux/resources/io.github.matthewyjiang.tsk.desktop
linux/resources/io.github.matthewyjiang.tsk.metainfo.xml
```

Later add:

- app icon
- Flatpak manifest
- release packaging workflow

## Milestones

### Milestone 1: App boots

- Add Linux crate.
- Add workspace member.
- Launch GTK/libadwaita window.
- Resolve paths.
- Open core database.

Validation:

```sh
cargo run -p tsk-linux
```

### Milestone 2: Offline CRUD

- Create task.
- Edit title/body/status/due date/tags.
- Delete task.
- Refresh task list from SQLite.

### Milestone 3: Search/filter/sort

- Sidebar filters.
- Search box.
- Sort options.
- Show/hide completed setting.

### Milestone 4: Linux services

- libsecret key storage.
- First-launch onboarding.
- Basic notifications.

### Milestone 5: Settings

- Settings page/dialog.
- Persist local preferences.
- Theme switching.

### Milestone 6: Sync

- Add core sync facade methods.
- Implement HTTP sync client.
- Add sync button and status display.

### Milestone 7: Packaging

- `.desktop` file.
- metainfo XML.
- icon.
- Flatpak manifest.

## Validation commands

From repository root:

```sh
cargo fmt --check
cargo check -p tsk-linux
cargo clippy -p tsk-linux --all-targets
cargo test -p tsk-linux
cargo run -p tsk-linux
```

Existing packages should remain healthy:

```sh
cargo test -p taskmanager-core
cargo test -p taskmanager-cli
cargo build --workspace
```

GTK/system dependency checks:

```sh
pkg-config --modversion gtk4
pkg-config --modversion libadwaita-1
pkg-config --modversion libsecret-1
```

## First implementation target

Build a GTK4/libadwaita app that opens the core database and supports offline task CRUD before implementing sync.

Status: complete for the initial native Linux MVP. The implementation currently uses raw gtk-rs/libadwaita instead of Relm4 to keep the first version small and compilable. The layout has been polished toward a minimal Things-style desktop app by adapting its information architecture rather than copying sample content: a calm sidebar for built-in views (`Inbox`, `Today`, `Upcoming`, `Anytime`, `Done`), user-defined lists, live Today count, a focused inline-editable task list, hover actions, and empty state. Styling uses GTK/libadwaita theme colors instead of hard-coded light colors. Future iterations can extract the monolithic `app.rs` into the reserved component modules or introduce Relm4 if the state model grows.

## Latest validation

Completed from repository root:

```sh
cargo fmt --check
cargo check -p tsk-linux
cargo clippy -p tsk-linux --all-targets -- -D warnings
cargo test -p tsk-linux
cargo test -p taskmanager-core
cargo test -p taskmanager-cli
cargo build --workspace
pkg-config --modversion gtk4 libadwaita-1 libsecret-1
```

Results:

- `tsk-linux`: builds, clippy-clean, and all tests pass.
- `taskmanager-core`: all tests pass.
- `taskmanager-cli`: all tests pass.
- Workspace build passes.
- GTK4/libadwaita/libsecret development packages are available.

Latest UI polish validation also passed:

```sh
cargo fmt --check
cargo check -p tsk-linux
cargo clippy -p tsk-linux --all-targets -- -D warnings
cargo test -p tsk-linux
cargo test -p taskmanager-core
cargo test -p taskmanager-cli
cargo build --workspace
```
