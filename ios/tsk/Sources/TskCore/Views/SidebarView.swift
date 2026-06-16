import SwiftUI

public struct SidebarView: View {
    @ObservedObject var model: AppModel
    @State private var isNewListPresented = false
    @State private var listBeingRenamed: TaskListItem?
    @State private var listPendingDeletion: TaskListItem?

    public init(model: AppModel) {
        self.model = model
    }

    public var body: some View {
        List(selection: destinationBinding) {
            Section("Views") {
                ForEach(BuiltInView.allCases) { view in
                    Label(view.title, systemImage: view.systemImage)
                        .tag(AppDestination.tasks(.builtIn(view)))
                        .accessibilityLabel(view.title)
                }
            }

            Section {
                ForEach(model.lists) { list in
                    Label(list.name, systemImage: "list.bullet")
                        .tag(AppDestination.tasks(.list(list.id)))
                        .accessibilityLabel("List, \(list.name)")
                        .contextMenu {
                            Button("Rename") { listBeingRenamed = list }
                            Button("Delete", role: .destructive) { listPendingDeletion = list }
                        }
                }

                Button {
                    isNewListPresented = true
                } label: {
                    Label("New List", systemImage: "plus")
                }
            } header: {
                Text("Lists")
            }

            Section {
                Label("Settings", systemImage: "gear")
                    .tag(AppDestination.settings)
            }

            Section("Sync") {
                SyncStatusView(summary: model.syncSummary)
                LabeledContent("Auth", value: model.syncAuthState.sidebarDisplayName)
                LabeledContent("Failed", value: String(model.syncSummary.conflictCount))
            }
        }
        .listStyle(.sidebar)
        .navigationTitle("tsk")
        .sheet(isPresented: $isNewListPresented) {
            ListNameEditorView(title: "New List", actionTitle: "Create") { name in
                Task {
                    let created = await model.createList(name: name)
                    if created != nil { isNewListPresented = false }
                }
            }
        }
        .sheet(item: $listBeingRenamed) { list in
            ListNameEditorView(title: "Rename List", initialName: list.name, actionTitle: "Save") { name in
                Task {
                    await model.renameList(id: list.id, name: name)
                    listBeingRenamed = nil
                }
            }
        }
        .confirmationDialog(
            "Delete List?",
            isPresented: Binding(
                get: { listPendingDeletion != nil },
                set: { if !$0 { listPendingDeletion = nil } }
            ),
            presenting: listPendingDeletion
        ) { list in
            Button("Delete \"\(list.name)\"", role: .destructive) {
                Task { await model.deleteList(id: list.id) }
                listPendingDeletion = nil
            }
        } message: { _ in
            Text("Tasks in this list move back to Inbox. The list deletion is saved locally and will sync later.")
        }
    }

    private var destinationBinding: Binding<AppDestination?> {
        Binding(
            get: { model.destination },
            set: { destination in
                guard let destination else { return }
                model.select(destination)
            }
        )
    }
}

private struct ListNameEditorView: View {
    @Environment(\.dismiss) private var dismiss
    var title: String
    var actionTitle: String
    var onSave: (String) -> Void
    @State private var name: String

    init(title: String, initialName: String = "", actionTitle: String, onSave: @escaping (String) -> Void) {
        self.title = title
        self.actionTitle = actionTitle
        self.onSave = onSave
        _name = State(initialValue: initialName)
    }

    var body: some View {
        NavigationStack {
            Form {
                TextField("Name", text: $name)
            }
            .navigationTitle(title)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(actionTitle) {
                        onSave(name)
                    }
                    .disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
        }
    }
}

public struct CompactSidebarView: View {
    @ObservedObject var model: AppModel
    @State private var isNewListPresented = false
    @State private var listBeingRenamed: TaskListItem?
    @State private var listPendingDeletion: TaskListItem?

    public init(model: AppModel) {
        self.model = model
    }

    public var body: some View {
        List {
            Section("Views") {
                ForEach(BuiltInView.allCases) { view in
                    NavigationLink {
                        TaskListView(model: model, selection: .builtIn(view))
                    } label: {
                        Label(view.title, systemImage: view.systemImage)
                    }
                    .accessibilityLabel(view.title)
                }
            }

            Section {
                ForEach(model.lists) { list in
                    NavigationLink {
                        TaskListView(model: model, selection: .list(list.id))
                    } label: {
                        Label(list.name, systemImage: "list.bullet")
                    }
                    .accessibilityLabel("List, \(list.name)")
                    .swipeActions(edge: .trailing) {
                        Button("Delete", role: .destructive) { listPendingDeletion = list }
                        Button("Rename") { listBeingRenamed = list }
                            .tint(.blue)
                    }
                    .contextMenu {
                        Button("Rename") { listBeingRenamed = list }
                        Button("Delete", role: .destructive) { listPendingDeletion = list }
                    }
                }

                Button {
                    isNewListPresented = true
                } label: {
                    Label("New List", systemImage: "plus")
                }
            } header: {
                Text("Lists")
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
                LabeledContent("Auth", value: model.syncAuthState.sidebarDisplayName)
                LabeledContent("Failed", value: String(model.syncSummary.conflictCount))
            }
        }
        .navigationTitle("tsk")
        .sheet(isPresented: $isNewListPresented) {
            ListNameEditorView(title: "New List", actionTitle: "Create") { name in
                Task {
                    let created = await model.createList(name: name)
                    if created != nil { isNewListPresented = false }
                }
            }
        }
        .sheet(item: $listBeingRenamed) { list in
            ListNameEditorView(title: "Rename List", initialName: list.name, actionTitle: "Save") { name in
                Task {
                    await model.renameList(id: list.id, name: name)
                    listBeingRenamed = nil
                }
            }
        }
        .confirmationDialog(
            "Delete List?",
            isPresented: Binding(
                get: { listPendingDeletion != nil },
                set: { if !$0 { listPendingDeletion = nil } }
            ),
            presenting: listPendingDeletion
        ) { list in
            Button("Delete \"\(list.name)\"", role: .destructive) {
                Task { await model.deleteList(id: list.id) }
                listPendingDeletion = nil
            }
        } message: { _ in
            Text("Tasks in this list move back to Inbox. The list deletion is saved locally and will sync later.")
        }
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

private extension FfiSyncAuthState {
    var sidebarDisplayName: String {
        switch self {
        case .localOnlyReady: "Local"
        case .authenticatedEnrollmentPending: "Enroll"
        case .syncReady: "Ready"
        case .authRequired: "Auth"
        case .misconfiguredOrigin: "Config"
        }
    }
}
