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

## iOS design language checklist

The iOS client follows the shared `tsk` product language from the Linux and macOS direction while staying native to SwiftUI:

- Keep the iPhone flow compact: `NavigationStack`, `List`, `.searchable`, toolbars, and sheets are preferred over desktop-style panes or custom gestures.
- Use iPad width to preserve the conceptual sidebar → task list → detail/editor flow with native `NavigationSplitView` behavior where practical.
- Let tasks start as calm compact rows. Reveal editing fields, metadata, and secondary actions progressively in the detail view instead of overloading the row.
- Motion should be spatially coherent and quiet. Use native SwiftUI list/navigation/form transitions and short spring/ease animations, and disable nonessential animation when Reduce Motion is enabled.
- Use icon-first controls for clear common actions such as new task, complete/reopen, delete, sync, search, settings, and close/dismiss, backed by SF Symbols.
- Every icon-only control must have an accessibility label; add hints/help where the result is not obvious from the label.
- Keep visible text for ambiguous, destructive, setup, sign-in/register, save/submit, and enrollment actions.
- Do not duplicate business logic in iOS views. Task, sync, auth, crypto, reminder, and conflict semantics continue to come through the shared Rust core and UniFFI bindings.

## Why use it

Use the iOS app when you want your tasks on a mobile device without giving up the local-first and encrypted-sync model. Foreground launch, resume, and manual sync remain the dependable sync paths while background refresh support continues to mature.
