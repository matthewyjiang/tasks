import Foundation
import Testing
@testable import TskCore

@Test func builtInViewsMatchLocalFirstCoreConcepts() {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(secondsFromGMT: 0)!
    let engine = TaskFilterEngine(calendar: calendar)
    let now = Date(timeIntervalSince1970: 1_700_000_000)
    let tomorrow = calendar.date(byAdding: .day, value: 1, to: now)!
    let listID = UUID()

    let inbox = TaskItem(title: "Inbox", updatedAt: now)
    let today = TaskItem(title: "Today", dueAt: now, updatedAt: now)
    let upcoming = TaskItem(title: "Upcoming", dueAt: tomorrow, updatedAt: now)
    let anytimeInList = TaskItem(title: "List task", listID: listID, updatedAt: now)
    let done = TaskItem(title: "Done", status: .done, updatedAt: now)
    let deleted = TaskItem(title: "Deleted", updatedAt: now, deleted: true)
    let tasks = [inbox, today, upcoming, anytimeInList, done, deleted]

    #expect(Set(engine.tasks(tasks, for: .builtIn(.inbox), now: now).map(\.title)) == Set(["Inbox", "Today", "Upcoming"]))
    #expect(engine.tasks(tasks, for: .builtIn(.today), now: now).map(\.title) == ["Today"])
    #expect(engine.tasks(tasks, for: .builtIn(.upcoming), now: now).map(\.title) == ["Upcoming"])
    #expect(Set(engine.tasks(tasks, for: .builtIn(.anytime), now: now).map(\.title)) == Set(["Inbox", "List task"]))
    #expect(engine.tasks(tasks, for: .builtIn(.done), now: now).map(\.title) == ["Done"])
    #expect(engine.tasks(tasks, for: .list(listID), now: now).map(\.title) == ["List task"])
}

@Test func searchChecksTitleBodyAndTagsLocally() {
    let engine = TaskFilterEngine()
    let tasks = [
        TaskItem(title: "Write FFI", body: "Swift bindings", tags: []),
        TaskItem(title: "Groceries", body: "Buy milk", tags: ["errand"]),
        TaskItem(title: "Sync", body: "Retry queue", tags: ["ios"])
    ]

    #expect(engine.applySearch("swift", to: tasks).map(\.title) == ["Write FFI"])
    #expect(engine.applySearch("ERRAND", to: tasks).map(\.title) == ["Groceries"])
    #expect(engine.applySearch("retry", to: tasks).map(\.title) == ["Sync"])
}
