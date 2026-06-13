import SwiftUI

public struct SettingsView: View {
    @ObservedObject var model: AppModel

    public init(model: AppModel) {
        self.model = model
    }

    public var body: some View {
        Form {
            Section("Sync Status") {
                SyncStatusView(summary: model.syncSummary)
            }

            Section("Local Storage") {
                Text("Database: Application Support/tsk/tasks.sqlite3")
                Text("Secrets: iOS Keychain")
            }

            Section("Phase 1") {
                Text("This shell is ready to be wired to generated UniFFI bindings and the shared Rust core.")
                    .foregroundStyle(.secondary)
            }
        }
        .navigationTitle("Settings")
    }
}
