import SwiftUI

public struct SidebarView: View {
    @ObservedObject var model: AppModel

    public init(model: AppModel) {
        self.model = model
    }

    public var body: some View {
        List(selection: Binding<TaskSelection?>(get: { model.selection }, set: { model.selection = $0 ?? .builtIn(.inbox) })) {
            Section("Views") {
                ForEach(BuiltInView.allCases) { view in
                    Label(view.title, systemImage: view.systemImage)
                        .tag(TaskSelection.builtIn(view))
                        .accessibilityLabel(view.title)
                }
            }

            Section("Lists") {
                ForEach(model.lists) { list in
                    Label(list.name, systemImage: "list.bullet")
                        .tag(TaskSelection.list(list.id))
                        .accessibilityLabel("List, \(list.name)")
                }
            }

            Section {
                NavigationLink {
                    SettingsView(model: model)
                } label: {
                    Label("Settings", systemImage: "gear")
                }
            }

            Section("Sync") {
                SyncStatusView(summary: model.syncSummary)
            }
        }
        .navigationTitle("Home")
    }
}

public struct SyncStatusView: View {
    public var summary: SyncSummary

    public init(summary: SyncSummary) {
        self.summary = summary
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Label(summary.isOnline ? "Online" : "Offline", systemImage: summary.isOnline ? "wifi" : "wifi.slash")
            Text("\(summary.dirtyCount) pending • \(summary.retryQueueDepth) retries • \(summary.conflictCount) conflicts")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .accessibilityElement(children: .combine)
    }
}
