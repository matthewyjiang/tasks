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
