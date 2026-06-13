import SwiftUI

@MainActor
public struct RootView: View {
    @StateObject private var model: AppModel

    public init() {
        _model = StateObject(wrappedValue: Self.makeDefaultModel())
    }

    public init(model: AppModel) {
        _model = StateObject(wrappedValue: model)
    }

    private static func makeDefaultModel() -> AppModel {
        do {
            let paths = try AppPaths()
            try paths.createDirectories()
            let repository = try CoreTaskRepository(databaseURL: paths.databaseURL)
            return AppModel(repository: repository)
        } catch {
            return AppModel(repository: StartupFailedRepository(error: error))
        }
    }

    public var body: some View {
        NavigationSplitView {
            SidebarView(model: model)
        } content: {
            TaskListView(model: model)
        } detail: {
            if let task = model.selectedTask {
                TaskDetailView(model: model, task: task)
            } else {
                ContentUnavailableView("Select a task", systemImage: "checklist", description: Text("Choose a task or create a new one."))
            }
        }
        .task { await model.load() }
        .alert("tsk", isPresented: Binding(get: { model.errorMessage != nil }, set: { if !$0 { model.clearError() } })) {
            Button("OK", role: .cancel) { model.clearError() }
        } message: {
            Text(model.errorMessage ?? "")
        }
    }
}
