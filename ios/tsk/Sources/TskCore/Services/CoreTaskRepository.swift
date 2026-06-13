import Foundation

public actor CoreTaskRepository: TaskRepository {
    private let core: FfiTaskManagerCore

    public init(databaseURL: URL) throws {
        self.core = try FfiTaskManagerCore(databasePath: databaseURL.path)
    }

    public init(databasePath: String) throws {
        self.core = try FfiTaskManagerCore(databasePath: databasePath)
    }

    public func loadTasks() async throws -> [TaskItem] {
        let filter = FfiTaskFilter(status: nil, projectId: nil, tags: [], dueAfter: nil, dueBefore: nil, includeDeleted: false)
        return try core.listTasks(filter: filter, sort: .updatedAtDesc).compactMap(TaskItem.init(ffi:))
    }

    public func loadLists() async throws -> [TaskListItem] {
        try core.listTaskLists().compactMap(TaskListItem.init(ffi:))
    }

    public func createTask(title: String, body: String, dueAt: Date?, listID: UUID?, tags: [String]) async throws -> TaskItem {
        let created = try core.createTaskWithOptions(
            title: title,
            body: body,
            dueAt: dueAt.map(Self.millisecondsSinceEpoch),
            projectId: listID?.uuidString,
            tags: tags
        )
        guard let task = TaskItem(ffi: created) else { throw CoreRepositoryError.invalidTaskID(created.id) }
        return task
    }

    public func updateTask(_ task: TaskItem) async throws -> TaskItem {
        let updated = try core.updateTask(
            taskId: task.id.uuidString,
            patch: FfiTaskPatch(
                title: task.title,
                body: task.body,
                dueAt: task.dueAt.map(Self.millisecondsSinceEpoch),
                clearDueAt: task.dueAt == nil,
                status: task.status.ffi,
                projectId: task.listID?.uuidString,
                clearProjectId: task.listID == nil,
                tags: task.tags
            )
        )
        guard let mapped = TaskItem(ffi: updated) else { throw CoreRepositoryError.invalidTaskID(updated.id) }
        return mapped
    }

    public func deleteTask(id: UUID) async throws {
        try core.deleteTask(taskId: id.uuidString)
    }

    public func createList(name: String) async throws -> TaskListItem {
        let created = try core.createList(name: name)
        guard let list = TaskListItem(ffi: created) else { throw CoreRepositoryError.invalidListID(created.id) }
        return list
    }

    public func updateList(_ list: TaskListItem) async throws -> TaskListItem {
        let updated = try core.updateList(listId: list.id.uuidString, name: list.name)
        guard let mapped = TaskListItem(ffi: updated) else { throw CoreRepositoryError.invalidListID(updated.id) }
        return mapped
    }

    public func deleteList(id: UUID) async throws {
        try core.deleteList(listId: id.uuidString)
    }

    public func syncSummary() async throws -> SyncSummary {
        let status = try core.syncStatus()
        return SyncSummary(dirtyCount: status.dirtyCount, retryQueueDepth: status.retryQueueDepth, conflictCount: 0, isOnline: true)
    }

    fileprivate static func date(millisecondsSinceEpoch milliseconds: Int64) -> Date {
        Date(timeIntervalSince1970: TimeInterval(milliseconds) / 1_000)
    }

    private static func millisecondsSinceEpoch(_ date: Date) -> Int64 {
        Int64((date.timeIntervalSince1970 * 1_000).rounded())
    }
}

public enum CoreRepositoryError: Error, Equatable, Sendable {
    case invalidTaskID(String)
    case invalidListID(String)
}

private extension TaskItem {
    init?(ffi task: FfiTask) {
        guard let id = UUID(uuidString: task.id) else { return nil }
        self.init(
            id: id,
            title: task.title,
            body: task.body,
            dueAt: task.dueAt.map(CoreTaskRepository.date(millisecondsSinceEpoch:)),
            status: TaskStatus(ffi: task.status),
            listID: task.projectId.flatMap(UUID.init(uuidString:)),
            tags: task.tags,
            createdAt: CoreTaskRepository.date(millisecondsSinceEpoch: task.createdAt),
            updatedAt: CoreTaskRepository.date(millisecondsSinceEpoch: task.updatedAt),
            deleted: task.deleted,
            dirty: task.dirty
        )
    }
}

private extension TaskListItem {
    init?(ffi list: FfiTaskList) {
        guard let id = UUID(uuidString: list.id) else { return nil }
        self.init(
            id: id,
            name: list.name,
            createdAt: CoreTaskRepository.date(millisecondsSinceEpoch: list.createdAt),
            updatedAt: CoreTaskRepository.date(millisecondsSinceEpoch: list.updatedAt),
            deleted: list.deleted,
            dirty: list.dirty
        )
    }
}

private extension TaskStatus {
    init(ffi status: FfiTaskStatus) {
        switch status {
        case .open: self = .open
        case .done: self = .done
        }
    }

    var ffi: FfiTaskStatus {
        switch self {
        case .open: .open
        case .done: .done
        }
    }
}
