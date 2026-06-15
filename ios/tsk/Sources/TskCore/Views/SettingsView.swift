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
                LabeledContent("Cursor", value: String(model.syncSummary.cursor))
                LabeledContent("Conflicts this sync", value: String(model.syncSummary.conflictCount))
                LabeledContent("Foreground sync", value: "HTTP adapter pending")
            }

            Section("Local Account") {
                if let account = model.localAccount {
                    LabeledContent("Device key", value: account.publicKeyFingerprint)
                    Text(account.createdAnySecret ? "Local-first account keys were initialized on this device." : "Local-first account keys are stored on this device.")
                        .foregroundStyle(.secondary)
                } else {
                    Text("Local account bootstrap is unavailable in this preview.")
                        .foregroundStyle(.secondary)
                }
            }

            Section("Local Storage") {
                Text("Database: Application Support app container/tasks.sqlite3")
                Text("Secrets: device-only iOS Keychain, available after first unlock")
            }

            Section("Platform") {
                LabeledContent("Network", value: model.syncSummary.isOnline ? "Online" : "Offline")
                Text("Notification scheduling/cancellation hooks use UNUserNotificationCenter. Reminder semantics are deferred until the shared core defines a reminder model.")
                    .foregroundStyle(.secondary)
            }

            Section("Offline UI") {
                Text("Tasks, lists, due dates, tags, done/open state, and local search are backed by the shared Rust core database and work offline.")
                    .foregroundStyle(.secondary)
            }
        }
        .navigationTitle("Settings")
    }
}
