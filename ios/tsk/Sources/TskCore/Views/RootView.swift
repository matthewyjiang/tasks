import SwiftUI

@MainActor
public struct RootView: View {
    @StateObject private var model: AppModel

    public init() {
        _model = StateObject(wrappedValue: AppModel())
    }

    public init(model: AppModel) {
        _model = StateObject(wrappedValue: model)
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
