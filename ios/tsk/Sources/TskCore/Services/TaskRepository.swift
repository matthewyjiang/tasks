import Foundation

public protocol TaskRepository: Sendable {
    func loadTasks() async throws -> [TaskItem]
    func loadLists() async throws -> [TaskListItem]
    func createTask(title: String, body: String, dueAt: Date?, listID: UUID?, tags: [String]) async throws -> TaskItem
    func updateTask(_ task: TaskItem) async throws -> TaskItem
    func deleteTask(id: UUID) async throws
    func createList(name: String) async throws -> TaskListItem
    func updateList(_ list: TaskListItem) async throws -> TaskListItem
    func deleteList(id: UUID) async throws
    func syncSummary() async throws -> SyncSummary
}

public actor PreviewTaskRepository: TaskRepository {
    private var tasks: [TaskItem]
    private var lists: [TaskListItem]
    private var sync: SyncSummary

    public init(tasks: [TaskItem] = PreviewTaskRepository.sampleTasks, lists: [TaskListItem] = PreviewTaskRepository.sampleLists, sync: SyncSummary = SyncSummary(dirtyCount: 2, retryQueueDepth: 0, conflictCount: 0, isOnline: true)) {
        self.tasks = tasks
        self.lists = lists
        self.sync = sync
    }

    public func loadTasks() async throws -> [TaskItem] { tasks }
    public func loadLists() async throws -> [TaskListItem] { lists }
    public func syncSummary() async throws -> SyncSummary { sync }

    public func createTask(title: String, body: String, dueAt: Date?, listID: UUID?, tags: [String]) async throws -> TaskItem {
        let task = TaskItem(title: title, body: body, dueAt: dueAt, listID: listID, tags: tags)
        tasks.append(task)
        sync.dirtyCount += 1
        return task
    }

    public func updateTask(_ task: TaskItem) async throws -> TaskItem {
        var updated = task
        updated.updatedAt = Date()
        updated.dirty = true
        if let index = tasks.firstIndex(where: { $0.id == task.id }) {
            tasks[index] = updated
        } else {
            tasks.append(updated)
        }
        sync.dirtyCount += 1
        return updated
    }

    public func deleteTask(id: UUID) async throws {
        if let index = tasks.firstIndex(where: { $0.id == id }) {
            tasks[index].deleted = true
            tasks[index].dirty = true
            tasks[index].updatedAt = Date()
            sync.dirtyCount += 1
        }
    }

    public func createList(name: String) async throws -> TaskListItem {
        let list = TaskListItem(name: name)
        lists.append(list)
        return list
    }

    public func updateList(_ list: TaskListItem) async throws -> TaskListItem {
        var updated = list
        updated.updatedAt = Date()
        updated.dirty = true
        if let index = lists.firstIndex(where: { $0.id == list.id }) {
            lists[index] = updated
        } else {
            lists.append(updated)
        }
        return updated
    }

    public func deleteList(id: UUID) async throws {
        lists.removeAll { $0.id == id }
        for index in tasks.indices where tasks[index].listID == id {
            tasks[index].listID = nil
            tasks[index].dirty = true
            tasks[index].updatedAt = Date()
        }
    }
}

public extension PreviewTaskRepository {
    static let personalListID = UUID(uuidString: "00000000-0000-0000-0000-000000000101")!
    static let workListID = UUID(uuidString: "00000000-0000-0000-0000-000000000102")!

    static var sampleLists: [TaskListItem] {
        [
            TaskListItem(id: personalListID, name: "Personal"),
            TaskListItem(id: workListID, name: "Work")
        ]
    }

    static var sampleTasks: [TaskItem] {
        let now = Date()
        return [
            TaskItem(title: "Review iOS FFI surface", body: "Task list, settings, sync status", dueAt: now, listID: workListID, tags: ["ios", "ffi"]),
            TaskItem(title: "Buy coffee", body: "Local-only offline task", listID: personalListID, tags: ["errand"]),
            TaskItem(title: "Ship Linux parity", body: "Done example", status: .done, tags: ["release"]),
            TaskItem(title: "Plan background refresh", body: "BGAppRefreshTask", dueAt: Calendar.current.date(byAdding: .day, value: 3, to: now), tags: ["sync"])
        ]
    }
}
