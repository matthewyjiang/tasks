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

            Section("Offline UI") {
                Text("Tasks, lists, due dates, tags, done/open state, and local search are backed by the shared Rust core database and work offline.")
                    .foregroundStyle(.secondary)
            }
        }
        .navigationTitle("Settings")
    }
}
