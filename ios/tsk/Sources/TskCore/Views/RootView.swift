import SwiftUI

@MainActor
public struct RootView: View {
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @StateObject private var model: AppModel
    @StateObject private var reachability = ReachabilityMonitor()

    public init() {
        _model = StateObject(wrappedValue: Self.makeDefaultModel())
    }

    public init(model: AppModel) {
        _model = StateObject(wrappedValue: model)
    }

    public static func makeDefaultModel() -> AppModel {
        do {
            let paths = try AppPaths()
            try paths.createDirectories()
            let secretStore = KeychainSecretStore(service: paths.bundleIdentifier)
            let account = try LocalFirstBootstrapService(
                secretStore: secretStore
            ).ensureBootstrapped()
            let platform = CorePlatformAdapter(secretStore: secretStore)
            let serverURL = UserDefaults.standard.string(forKey: "tsk.sync.serverURL") ?? ""
            let syncCoordinator = SyncCoordinator(serverURL: serverURL, platform: platform, authClient: SyncHTTPAuthClient())
            let repository = try CoreTaskRepository(databaseURL: paths.databaseURL, syncCoordinator: syncCoordinator)
            return AppModel(repository: repository, localAccount: account, notificationScheduler: UserNotificationScheduler(), syncCoordinator: syncCoordinator)
        } catch {
            return AppModel(repository: StartupFailedRepository(error: error))
        }
    }

    public var body: some View {
        rootContent
            .task {
                reachability.start()
                await model.load()
            }
            .onReceive(reachability.$status) { status in
                model.updateReachability(status)
            }
            .alert("tsk", isPresented: Binding(get: { model.errorMessage != nil }, set: { if !$0 { model.clearError() } })) {
                Button("OK", role: .cancel) { model.clearError() }
            } message: {
                Text(model.errorMessage ?? "")
            }
    }

    @ViewBuilder
    private var rootContent: some View {
        if horizontalSizeClass == .compact {
            NavigationStack {
                CompactSidebarView(model: model)
            }
        } else {
            NavigationSplitView {
                SidebarView(model: model)
            } content: {
                switch model.destination {
                case .tasks(let selection):
                    NavigationStack {
                        TaskListView(model: model, selection: selection, usesSplitSelection: true)
                    }
                case .settings:
                    ContentUnavailableView("Settings", systemImage: "gear", description: Text("Select settings details."))
                }
            } detail: {
                NavigationStack {
                    switch model.destination {
                    case .tasks:
                        if let selectedTask = model.selectedTask {
                            TaskDetailView(model: model, task: selectedTask)
                        } else {
                            ContentUnavailableView("Select a Task", systemImage: "checklist", description: Text("Choose a task from the list to view or edit details."))
                        }
                    case .settings:
                        SettingsView(model: model)
                    }
                }
            }
            .navigationSplitViewStyle(.balanced)
        }
    }
}
