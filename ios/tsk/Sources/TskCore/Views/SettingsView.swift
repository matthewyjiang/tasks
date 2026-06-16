import SwiftUI

public struct SettingsView: View {
    @ObservedObject var model: AppModel
    @State private var serverURL: String
    @State private var email = ""
    @State private var password = ""
    @State private var enrollmentPayload = ""

    public init(model: AppModel) {
        self.model = model
        _serverURL = State(initialValue: model.syncServerURL)
    }

    public var body: some View {
        Form {
            Section("Sync Setup") {
                TextField("Server URL", text: $serverURL)
                    .autocorrectionDisabled()
                    .onSubmit { Task { await model.updateSyncServerURL(serverURL) } }

                TextField("Email", text: $email)
                    .autocorrectionDisabled()
                SecureField("Password", text: $password)

                Button("Sign In or Register") {
                    Task {
                        await model.updateSyncServerURL(serverURL)
                        await model.configureSync(email: email, password: password)
                        password = ""
                    }
                }
                .disabled(model.isSyncOperationInFlight || serverURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || email.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || password.isEmpty)

                Button("Sign Out", role: .destructive) {
                    Task { await model.signOutSync() }
                }
                .disabled(model.isSyncOperationInFlight || model.syncAuthState == .localOnlyReady)
            }

            Section("Sync Status") {
                LabeledContent("Auth", value: model.syncAuthState.displayName)
                LabeledContent("Enrollment", value: model.enrollmentState.displayName)
                LabeledContent("Network", value: model.syncSummary.isOnline ? "Online" : "Offline")
                SyncStatusView(summary: model.syncSummary)
                LabeledContent("Cursor", value: String(model.syncSummary.cursor))
                LabeledContent("Failed/conflicts", value: String(model.syncSummary.conflictCount))
                if let message = model.lastSyncStatusMessage {
                    Text(message).foregroundStyle(.secondary)
                }
                Button("Sync Now") {
                    Task { await model.syncNow() }
                }
                .disabled(!model.canSyncNow)
            }

            if model.syncAuthState == .authenticatedEnrollmentPending || model.enrollmentState == .existingAccountPending {
                Section("Existing Account Enrollment") {
                    Text("You are signed in, but this device needs approval from another enrolled device before it can decrypt and sync existing tasks.")
                        .foregroundStyle(.secondary)
                    if let devicePublicKey = model.enrollmentDevicePublicKey {
                        LabeledContent("This device public key", value: devicePublicKey)
                            .textSelection(.enabled)
                        Text("On an enrolled device, wrap the account data key for this public key, then paste the wrapped JSON payload below. The raw account key and private keys should never be copied.")
                            .foregroundStyle(.secondary)
                    }
                    TextEditor(text: $enrollmentPayload)
                        .frame(minHeight: 96)
                        .font(.system(.body, design: .monospaced))
                    Button("Accept Wrapped Account Data Key") {
                        Task {
                            await model.acceptWrappedAccountDataKeyPayload(json: enrollmentPayload)
                            enrollmentPayload = ""
                        }
                    }
                    .disabled(model.isSyncOperationInFlight || enrollmentPayload.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
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
                Text("Server URL: local plaintext UserDefaults metadata")
                    .foregroundStyle(.secondary)
            }

            Section("Platform") {
                Text("Notification scheduling/cancellation hooks use UNUserNotificationCenter and shared core reminder semantics.")
                    .foregroundStyle(.secondary)
                Text("Background refresh is configured with BGAppRefreshTask and performs best-effort sync when iOS grants runtime.")
                    .foregroundStyle(.secondary)
            }
        }
        .navigationTitle("Settings")
        .task {
            serverURL = model.syncServerURL
            await model.refreshSyncState()
        }
    }
}

private extension FfiSyncAuthState {
    var displayName: String {
        switch self {
        case .localOnlyReady: "Local Only"
        case .authenticatedEnrollmentPending: "Signed In, Approval Needed"
        case .syncReady: "Sync Ready"
        case .authRequired: "Auth Required"
        case .misconfiguredOrigin: "Misconfigured Origin"
        }
    }
}

private extension FfiEnrollmentState {
    var displayName: String {
        switch self {
        case .localOnlyReady: "Local Only"
        case .existingAccountPending: "Existing Account Pending"
        case .syncReady: "Sync Ready"
        }
    }
}
