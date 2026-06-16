import SwiftUI

#if SWIFT_PACKAGE
import TskCore
#endif

@main
struct TskApp: App {
    @Environment(\.scenePhase) private var scenePhase
    @StateObject private var model = RootView.makeDefaultModel()

    #if canImport(BackgroundTasks) && os(iOS)
    private let backgroundSync = BackgroundSyncService(
        scheduler: BGAppRefreshScheduler(),
        sync: { @MainActor in
            let model = RootView.makeDefaultModel()
            await model.load()
            return await model.backgroundSyncNow()
        }
    )
    #endif

    init() {
        #if canImport(BackgroundTasks) && os(iOS)
        _ = backgroundSync.register()
        #endif
    }

    var body: some Scene {
        WindowGroup {
            RootView(model: model)
                .task {
                    #if canImport(BackgroundTasks) && os(iOS)
                    await scheduleBackgroundRefreshIfReady()
                    #endif
                }
                .onChange(of: scenePhase) { _, phase in
                    switch phase {
                    case .active:
                        Task { await model.load() }
                    case .background:
                        #if canImport(BackgroundTasks) && os(iOS)
                        Task { await scheduleBackgroundRefreshIfReady() }
                        #endif
                    default:
                        break
                    }
                }
        }
    }

    #if canImport(BackgroundTasks) && os(iOS)
    @MainActor
    private func scheduleBackgroundRefreshIfReady() async {
        await model.refreshSyncState()
        guard model.canScheduleBackgroundSync else { return }
        backgroundSync.scheduleNextRefresh()
    }
    #endif
}
