import Foundation
import SwiftUI

@MainActor
public final class AppModel: ObservableObject {
    @Published public private(set) var tasks: [TaskItem] = []
    @Published public private(set) var lists: [TaskListItem] = []
    @Published public var selection: TaskSelection = .builtIn(.inbox)
    @Published public var selectedTaskID: UUID?
    @Published public var searchQuery: String = ""
    @Published public private(set) var syncSummary = SyncSummary()
    @Published public private(set) var errorMessage: String?

    private let repository: any TaskRepository
    private let filterEngine: TaskFilterEngine

    public init(repository: any TaskRepository = PreviewTaskRepository(), filterEngine: TaskFilterEngine = TaskFilterEngine()) {
        self.repository = repository
        self.filterEngine = filterEngine
    }

    public var selectedTask: TaskItem? {
        guard let selectedTaskID else { return nil }
        return tasks.first { $0.id == selectedTaskID }
    }

    public var visibleTasks: [TaskItem] {
        filterEngine.tasks(tasks, for: selection, searchQuery: searchQuery)
    }

    public func listName(for id: UUID?) -> String? {
        guard let id else { return nil }
        return lists.first { $0.id == id }?.name
    }

    public func title(for selection: TaskSelection) -> String {
        switch selection {
        case .builtIn(let view): view.title
        case .list(let id): listName(for: id) ?? "List"
        }
    }

    public func clearError() {
        errorMessage = nil
    }

    public func load() async {
        do {
            async let loadedTasks = repository.loadTasks()
            async let loadedLists = repository.loadLists()
            async let loadedSync = repository.syncSummary()
            tasks = try await loadedTasks
            lists = try await loadedLists
            syncSummary = try await loadedSync
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func createQuickTask() async {
        let listID: UUID? = {
            if case .list(let id) = selection { return id }
            return nil
        }()
        do {
            let task = try await repository.createTask(title: "New Task", body: "", dueAt: nil, listID: listID, tags: [])
            tasks.append(task)
            selectedTaskID = task.id
            syncSummary = try await repository.syncSummary()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func save(task: TaskItem) async {
        do {
            let saved = try await repository.updateTask(task)
            if let index = tasks.firstIndex(where: { $0.id == saved.id }) {
                tasks[index] = saved
            } else {
                tasks.append(saved)
            }
            syncSummary = try await repository.syncSummary()
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func deleteTask(id: UUID) async {
        do {
            try await repository.deleteTask(id: id)
            tasks.removeAll { $0.id == id }
            if selectedTaskID == id { selectedTaskID = nil }
            syncSummary = try await repository.syncSummary()
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
