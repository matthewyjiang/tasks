import Foundation
import SwiftUI

@MainActor
public final class AppModel: ObservableObject {
    @Published public private(set) var tasks: [TaskItem] = []
    @Published public private(set) var lists: [TaskListItem] = []
    @Published public var selection: TaskSelection = .builtIn(.inbox)
    @Published public var destination: AppDestination = .tasks(.builtIn(.inbox))
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
        visibleTasks(for: selection)
    }

    public func visibleTasks(for selection: TaskSelection) -> [TaskItem] {
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

    public func select(_ destination: AppDestination) {
        self.destination = destination
        switch destination {
        case .tasks(let selection):
            self.selection = selection
            selectedTaskID = nil
        case .settings:
            selectedTaskID = nil
        }
    }

    public func load() async {
        do {
            async let loadedTasks = repository.loadTasks()
            async let loadedLists = repository.loadLists()
            async let loadedSync = repository.syncSummary()
            let allTasks = try await loadedTasks
            let allLists = try await loadedLists
            tasks = allTasks.filter { !$0.deleted }
            lists = allLists.filter { !$0.deleted }
            syncSummary = try await loadedSync
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    @discardableResult
    public func createTask(
        title: String,
        body: String,
        dueAt: Date?,
        listID: UUID?,
        tags: [String],
        status: TaskStatus = .open
    ) async -> TaskItem? {
        let trimmedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedTitle.isEmpty else { return nil }
        do {
            var task = try await repository.createTask(
                title: trimmedTitle,
                body: body,
                dueAt: dueAt,
                listID: listID,
                tags: Self.normalizedTags(tags)
            )
            if task.status != status {
                task.status = status
                task = try await repository.updateTask(task)
            }
            tasks.append(task)
            selectedTaskID = task.id
            syncSummary = try await repository.syncSummary()
            errorMessage = nil
            return task
        } catch {
            errorMessage = error.localizedDescription
            return nil
        }
    }

    public func createQuickTask() async {
        let listID: UUID? = {
            if case .list(let id) = selection { return id }
            return nil
        }()
        await createTask(title: "New Task", body: "", dueAt: nil, listID: listID, tags: [])
    }

    public func toggleTaskStatus(_ task: TaskItem) async {
        var updated = task
        updated.status = task.status == .done ? .open : .done
        await save(task: updated)
    }

    public func save(task: TaskItem) async {
        do {
            var task = task
            task.title = task.title.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !task.title.isEmpty else { return }
            task.tags = Self.normalizedTags(task.tags)
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
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    @discardableResult
    public func createList(name: String) async -> TaskListItem? {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        do {
            let list = try await repository.createList(name: trimmed)
            lists.append(list)
            select(.tasks(.list(list.id)))
            syncSummary = try await repository.syncSummary()
            errorMessage = nil
            return list
        } catch {
            errorMessage = error.localizedDescription
            return nil
        }
    }

    public func renameList(id: UUID, name: String) async {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let current = lists.first(where: { $0.id == id }) else { return }
        do {
            var list = current
            list.name = trimmed
            let saved = try await repository.updateList(list)
            if let index = lists.firstIndex(where: { $0.id == id }) {
                lists[index] = saved
            }
            syncSummary = try await repository.syncSummary()
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func deleteList(id: UUID) async {
        do {
            try await repository.deleteList(id: id)
            lists.removeAll { $0.id == id }
            for index in tasks.indices where tasks[index].listID == id {
                tasks[index].listID = nil
            }
            if destination == .tasks(.list(id)) || selection == .list(id) {
                select(.tasks(.builtIn(.inbox)))
            }
            syncSummary = try await repository.syncSummary()
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private static func normalizedTags(_ tags: [String]) -> [String] {
        var seen = Set<String>()
        return tags.compactMap { tag in
            let trimmed = tag.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return nil }
            let key = trimmed.lowercased()
            guard seen.insert(key).inserted else { return nil }
            return trimmed
        }
    }
}
