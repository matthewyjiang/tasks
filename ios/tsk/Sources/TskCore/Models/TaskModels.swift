import Foundation

public enum TaskStatus: String, Codable, CaseIterable, Sendable {
    case open
    case done
}

public enum TaskSort: String, Codable, CaseIterable, Sendable {
    case updatedAtDesc
    case updatedAtAsc
    case dueAtAsc
    case dueAtDesc
    case createdAtAsc
    case createdAtDesc
}

public enum BuiltInView: String, CaseIterable, Identifiable, Sendable {
    case inbox
    case today
    case upcoming
    case anytime
    case done

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .inbox: "Inbox"
        case .today: "Today"
        case .upcoming: "Upcoming"
        case .anytime: "Anytime"
        case .done: "Done"
        }
    }

    public var systemImage: String {
        switch self {
        case .inbox: "tray"
        case .today: "sun.max"
        case .upcoming: "calendar"
        case .anytime: "archivebox"
        case .done: "checkmark.circle"
        }
    }
}

public enum TaskSelection: Hashable, Identifiable, Sendable {
    case builtIn(BuiltInView)
    case list(UUID)

    public var id: String {
        switch self {
        case .builtIn(let view): "builtin:\(view.rawValue)"
        case .list(let id): "list:\(id.uuidString)"
        }
    }
}

public enum AppDestination: Hashable, Identifiable, Sendable {
    case tasks(TaskSelection)
    case settings

    public var id: String {
        switch self {
        case .tasks(let selection): "tasks:\(selection.id)"
        case .settings: "settings"
        }
    }
}

public struct TaskItem: Identifiable, Equatable, Sendable {
    public var id: UUID
    public var title: String
    public var body: String
    public var dueAt: Date?
    public var status: TaskStatus
    public var reminderOffsetMs: Int64?
    public var listID: UUID?
    public var tags: [String]
    public var createdAt: Date
    public var updatedAt: Date
    public var deleted: Bool
    public var dirty: Bool

    public init(
        id: UUID = UUID(),
        title: String,
        body: String = "",
        dueAt: Date? = nil,
        status: TaskStatus = .open,
        reminderOffsetMs: Int64? = nil,
        listID: UUID? = nil,
        tags: [String] = [],
        createdAt: Date = Date(),
        updatedAt: Date = Date(),
        deleted: Bool = false,
        dirty: Bool = true
    ) {
        self.id = id
        self.title = title
        self.body = body
        self.dueAt = dueAt
        self.status = status
        self.reminderOffsetMs = reminderOffsetMs
        self.listID = listID
        self.tags = tags
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.deleted = deleted
        self.dirty = dirty
    }

    public func notificationFireDate(now: Date = Date()) -> Date? {
        guard status == .open, let dueAt, let reminderOffsetMs else { return nil }
        let fireDate = dueAt.addingTimeInterval(-TimeInterval(reminderOffsetMs) / 1_000)
        guard fireDate > now else { return nil }
        return fireDate
    }
}

public struct TaskListItem: Identifiable, Equatable, Sendable {
    public var id: UUID
    public var name: String
    public var createdAt: Date
    public var updatedAt: Date
    public var deleted: Bool
    public var dirty: Bool

    public init(
        id: UUID = UUID(),
        name: String,
        createdAt: Date = Date(),
        updatedAt: Date = Date(),
        deleted: Bool = false,
        dirty: Bool = true
    ) {
        self.id = id
        self.name = name
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.deleted = deleted
        self.dirty = dirty
    }
}

public struct SyncSummary: Equatable, Sendable {
    public var dirtyCount: UInt64
    public var retryQueueDepth: UInt64
    public var conflictCount: UInt64
    public var isOnline: Bool

    public init(dirtyCount: UInt64 = 0, retryQueueDepth: UInt64 = 0, conflictCount: UInt64 = 0, isOnline: Bool = true) {
        self.dirtyCount = dirtyCount
        self.retryQueueDepth = retryQueueDepth
        self.conflictCount = conflictCount
        self.isOnline = isOnline
    }
}
