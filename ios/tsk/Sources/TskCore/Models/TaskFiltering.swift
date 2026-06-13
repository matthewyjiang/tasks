import Foundation

public struct TaskFilterEngine: Sendable {
    public var calendar: Calendar

    public init(calendar: Calendar = .current) {
        self.calendar = calendar
    }

    public func tasks(
        _ tasks: [TaskItem],
        for selection: TaskSelection,
        searchQuery: String = "",
        now: Date = Date()
    ) -> [TaskItem] {
        let visible = tasks.filter { !$0.deleted }
        let scoped = visible.filter { task in
            switch selection {
            case .builtIn(let view): matches(task, builtInView: view, now: now)
            case .list(let listID): task.status == .open && task.listID == listID
            }
        }
        let searched = applySearch(searchQuery, to: scoped)
        return searched.sorted(by: taskSort)
    }

    public func matches(_ task: TaskItem, builtInView: BuiltInView, now: Date = Date()) -> Bool {
        switch builtInView {
        case .inbox:
            return task.status == .open && task.listID == nil
        case .today:
            guard task.status == .open, let dueAt = task.dueAt else { return false }
            return dueAt <= endOfDay(containing: now)
        case .upcoming:
            guard task.status == .open, let dueAt = task.dueAt else { return false }
            return dueAt > endOfDay(containing: now)
        case .anytime:
            return task.status == .open && task.dueAt == nil
        case .done:
            return task.status == .done
        }
    }

    public func applySearch(_ query: String, to tasks: [TaskItem]) -> [TaskItem] {
        let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return tasks }
        return tasks.filter { task in
            task.title.localizedCaseInsensitiveContains(normalized)
                || task.body.localizedCaseInsensitiveContains(normalized)
                || task.tags.contains { $0.localizedCaseInsensitiveContains(normalized) }
        }
    }

    private func endOfDay(containing date: Date) -> Date {
        let start = calendar.startOfDay(for: date)
        return calendar.date(byAdding: DateComponents(day: 1, second: -1), to: start) ?? date
    }

    private func taskSort(_ lhs: TaskItem, _ rhs: TaskItem) -> Bool {
        switch (lhs.dueAt, rhs.dueAt) {
        case (.some(let left), .some(let right)) where left != right:
            return left < right
        case (.some, .none):
            return true
        case (.none, .some):
            return false
        default:
            if lhs.updatedAt != rhs.updatedAt {
                return lhs.updatedAt > rhs.updatedAt
            }
            return lhs.id.uuidString < rhs.id.uuidString
        }
    }
}
