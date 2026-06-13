import Foundation
import Testing
@testable import TskCore

@Test func coreTaskRepositoryPersistsTasksListsAndSyncStatus() async throws {
    let databaseURL = FileManager.default.temporaryDirectory
        .appending(path: "tsk-core-repository-\(UUID().uuidString).sqlite3")
    defer { try? FileManager.default.removeItem(at: databaseURL) }

    let repository = try CoreTaskRepository(databaseURL: databaseURL)
    let list = try await repository.createList(name: "Work")
    let dueAt = Date(timeIntervalSince1970: 1_700_000_000)
    let task = try await repository.createTask(
        title: "Write bindings",
        body: "Use shared Rust core",
        dueAt: dueAt,
        listID: list.id,
        tags: ["ios", "ffi"]
    )

    #expect(task.title == "Write bindings")
    #expect(task.listID == list.id)
    #expect(task.tags == ["ios", "ffi"])

    var updated = task
    updated.status = .done
    updated.title = "Use generated bindings"
    let saved = try await repository.updateTask(updated)
    #expect(saved.status == .done)
    #expect(saved.title == "Use generated bindings")

    #expect(try await repository.loadLists() == [list])
    #expect(try await repository.loadTasks().map(\.id) == [task.id])

    let sync = try await repository.syncSummary()
    #expect(sync.dirtyCount >= 1)
}

@Test func coreTaskRepositoryPersistsPhase2OfflineCrudFlows() async throws {
    let databaseURL = FileManager.default.temporaryDirectory
        .appending(path: "tsk-core-phase2-\(UUID().uuidString).sqlite3")
    defer { try? FileManager.default.removeItem(at: databaseURL) }

    let repository = try CoreTaskRepository(databaseURL: databaseURL)
    let list = try await repository.createList(name: "Errands")
    let dueAt = Date(timeIntervalSince1970: 1_900_000_000)
    let task = try await repository.createTask(
        title: "Buy milk",
        body: "Whole milk",
        dueAt: dueAt,
        listID: list.id,
        tags: ["home", "grocery"]
    )

    var edited = task
    edited.title = "Buy oat milk"
    edited.body = "Unsweetened"
    edited.status = .done
    edited.dueAt = nil
    edited.tags = ["grocery", "vegan"]
    let saved = try await repository.updateTask(edited)
    #expect(saved.title == "Buy oat milk")
    #expect(saved.body == "Unsweetened")
    #expect(saved.status == .done)
    #expect(saved.dueAt == nil)
    #expect(saved.listID == list.id)
    #expect(saved.tags == ["grocery", "vegan"])

    try await repository.deleteList(id: list.id)
    #expect(try await repository.loadLists().isEmpty)
    let unassigned = try #require(try await repository.loadTasks().first { $0.id == task.id })
    #expect(unassigned.listID == nil)

    try await repository.deleteTask(id: task.id)
    #expect(try await repository.loadTasks().isEmpty)

    let sync = try await repository.syncSummary()
    #expect(sync.dirtyCount >= 1)
}

@Test @MainActor func appModelWithCoreRepositorySupportsPhase2OfflineViewsAndSearch() async throws {
    let databaseURL = FileManager.default.temporaryDirectory
        .appending(path: "tsk-appmodel-core-phase2-\(UUID().uuidString).sqlite3")
    defer { try? FileManager.default.removeItem(at: databaseURL) }

    let repository = try CoreTaskRepository(databaseURL: databaseURL)
    let model = AppModel(repository: repository)
    await model.load()

    let list = try #require(await model.createList(name: "Work"))
    let today = Date(timeIntervalSince1970: 1_800_000_000)
    let tomorrow = Calendar(identifier: .gregorian).date(byAdding: .day, value: 1, to: today)!
    let inbox = try #require(await model.createTask(title: "Inbox task", body: "Find locally", dueAt: nil, listID: nil, tags: ["alpha"], status: .open))
    let dueToday = try #require(await model.createTask(title: "Today task", body: "", dueAt: today, listID: nil, tags: [], status: .open))
    let upcoming = try #require(await model.createTask(title: "Upcoming task", body: "", dueAt: tomorrow, listID: nil, tags: [], status: .open))
    let listed = try #require(await model.createTask(title: "Listed task", body: "", dueAt: nil, listID: list.id, tags: ["beta"], status: .open))
    let done = try #require(await model.createTask(title: "Done task", body: "", dueAt: nil, listID: nil, tags: [], status: .done))

    let filterEngine = TaskFilterEngine(calendar: Calendar(identifier: .gregorian))
    let inboxIDs = filterEngine.tasks(model.tasks, for: .builtIn(.inbox), now: today).map(\.id)
    #expect(inboxIDs.contains(inbox.id))
    #expect(inboxIDs.contains(dueToday.id))
    #expect(inboxIDs.contains(upcoming.id))
    #expect(filterEngine.tasks(model.tasks, for: .builtIn(.today), now: today).map(\.id) == [dueToday.id])
    #expect(filterEngine.tasks(model.tasks, for: .builtIn(.upcoming), now: today).map(\.id) == [upcoming.id])
    #expect(filterEngine.tasks(model.tasks, for: .list(list.id), now: today).map(\.id) == [listed.id])
    #expect(filterEngine.tasks(model.tasks, for: .builtIn(.done), now: today).map(\.id) == [done.id])

    model.searchQuery = "alpha"
    #expect(model.visibleTasks(for: .builtIn(.inbox)).map(\.id) == [inbox.id])

    await model.deleteTask(id: inbox.id)
    await model.load()
    #expect(!model.tasks.contains { $0.id == inbox.id })
}

@Test @MainActor func appModelSupportsOfflineTaskAndListFlows() async throws {
    let repository = PreviewTaskRepository(tasks: [], lists: [], sync: SyncSummary())
    let model = AppModel(repository: repository)

    await model.load()
    guard let list = await model.createList(name: " Personal ") else {
        #expect(Bool(false))
        return
    }
    #expect(list.name == "Personal")
    #expect(model.selection == .list(list.id))

    let dueAt = Date(timeIntervalSince1970: 1_800_000_000)
    guard let task = await model.createTask(
        title: " Buy milk ",
        body: "Use the offline database",
        dueAt: dueAt,
        listID: list.id,
        tags: ["errand", "Errand", " ios "],
        status: .open
    ) else {
        #expect(Bool(false))
        return
    }
    #expect(task.title == "Buy milk")
    #expect(task.tags == ["errand", "ios"])
    #expect(model.visibleTasks.map(\.id) == [task.id])

    var saved = task
    saved.status = .done
    saved.listID = nil
    saved.dueAt = nil
    saved.tags = ["home"]
    await model.save(task: saved)
    #expect(model.tasks.first?.status == .done)
    #expect(model.tasks.first?.listID == nil)

    await model.renameList(id: list.id, name: "Home")
    #expect(model.lists.first?.name == "Home")
    await model.deleteList(id: list.id)
    #expect(model.lists.isEmpty)
    #expect(model.selection == .builtIn(.inbox))

    await model.deleteTask(id: saved.id)
    #expect(model.tasks.isEmpty)
}
