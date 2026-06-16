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
            } detail: {
                NavigationStack {
                    switch model.destination {
                    case .tasks(let selection):
                        TaskListView(model: model, selection: selection)
                    case .settings:
                        SettingsView(model: model)
                    }
                }
            }
            .navigationSplitViewStyle(.balanced)
        }
    }
}
