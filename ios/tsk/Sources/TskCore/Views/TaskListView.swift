import SwiftUI

public struct TaskListView: View {
    @ObservedObject var model: AppModel

    public init(model: AppModel) {
        self.model = model
    }

    public var body: some View {
        List {
            ForEach(model.visibleTasks) { task in
                NavigationLink {
                    TaskDetailView(model: model, task: task)
                } label: {
                    TaskRowView(task: task, listName: model.listName(for: task.listID))
                }
                .simultaneousGesture(TapGesture().onEnded { model.selectedTaskID = task.id })
                .swipeActions(edge: .leading) {
                    Button(task.status == .done ? "Open" : "Done") {
                        var updated = task
                        updated.status = task.status == .done ? .open : .done
                        Task { await model.save(task: updated) }
                    }
                    .tint(.green)
                }
                .swipeActions(edge: .trailing) {
                    Button("Delete", role: .destructive) {
                        Task { await model.deleteTask(id: task.id) }
                    }
                }
                .accessibilityAction(named: task.status == .done ? "Mark Open" : "Mark Done") {
                    var updated = task
                    updated.status = task.status == .done ? .open : .done
                    Task { await model.save(task: updated) }
                }
            }
        }
        .overlay {
            if model.visibleTasks.isEmpty {
                ContentUnavailableView("No tasks", systemImage: "tray", description: Text("Create a task or adjust search."))
            }
        }
        .navigationTitle(model.title(for: model.selection))
        .searchable(text: $model.searchQuery, prompt: "Search tasks")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    Task { await model.createQuickTask() }
                } label: {
                    Label("New Task", systemImage: "plus")
                }
                .accessibilityLabel("New Task")
            }
        }
    }
}

public struct TaskRowView: View {
    public var task: TaskItem
    public var listName: String?

    public init(task: TaskItem, listName: String?) {
        self.task = task
        self.listName = listName
    }

    public var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: task.status == .done ? "checkmark.circle.fill" : "circle")
                .foregroundStyle(task.status == .done ? .green : .secondary)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                Text(task.title.isEmpty ? "Untitled" : task.title)
                    .strikethrough(task.status == .done)
                    .font(.body)
                HStack(spacing: 8) {
                    if let dueAt = task.dueAt {
                        Label(dueAt.formatted(date: .abbreviated, time: .omitted), systemImage: "calendar")
                    }
                    if let listName {
                        Label(listName, systemImage: "list.bullet")
                    }
                    if !task.tags.isEmpty {
                        Label(task.tags.joined(separator: ", "), systemImage: "tag")
                    }
                }
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
            }
        }
        .accessibilityElement(children: .combine)
    }
}
