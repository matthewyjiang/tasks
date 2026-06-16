import Foundation
import Testing
@testable import TskCore

private enum BackgroundSyncTestError: LocalizedError {
    case deleteFailed

    var errorDescription: String? { "Delete failed" }
}

private final class SuccessfulSyncRepository: TaskRepository, @unchecked Sendable {
    var failDeletes = false

    func loadTasks(includeDeleted: Bool) async throws -> [TaskItem] { [] }
    func loadLists() async throws -> [TaskListItem] { [] }
    func createTask(title: String, body: String, dueAt: Date?, listID: UUID?, tags: [String]) async throws -> TaskItem {
        TaskItem(title: title, body: body, dueAt: dueAt, listID: listID, tags: tags)
    }
    func updateTask(_ task: TaskItem) async throws -> TaskItem { task }
    func deleteTask(id: UUID) async throws {
        if failDeletes { throw BackgroundSyncTestError.deleteFailed }
    }
    func createList(name: String) async throws -> TaskListItem { TaskListItem(name: name) }
    func updateList(_ list: TaskListItem) async throws -> TaskListItem { list }
    func deleteList(id: UUID) async throws {}
    func syncSummary() async throws -> SyncSummary { SyncSummary(isOnline: true) }
    func syncNow(isOnline: Bool) async throws -> SyncSummary { SyncSummary(isOnline: isOnline) }
}

private final class RecordingBackgroundScheduler: BackgroundRefreshScheduling, @unchecked Sendable {
    private let lock = NSLock()
    private var handler: (@Sendable () async -> Bool)?
    private(set) var scheduleCount = 0
    private(set) var registerCount = 0
    var registerResult = true

    @discardableResult
    func register(handler: @escaping @Sendable () async -> Bool) -> Bool {
        lock.lock()
        self.handler = handler
        registerCount += 1
        let result = registerResult
        lock.unlock()
        return result
    }

    func schedule() {
        lock.lock()
        scheduleCount += 1
        lock.unlock()
    }

    func runRegisteredTask() async -> Bool {
        lock.lock()
        let handler = self.handler
        lock.unlock()
        return await handler?() ?? false
    }
}

@Test func backgroundSyncServiceRegistersOnceAndSchedulesBestEffortRefresh() async {
    let scheduler = RecordingBackgroundScheduler()
    let service = BackgroundSyncService(scheduler: scheduler) { true }

    #expect(service.register())
    #expect(service.register())
    service.scheduleNextRefresh()

    #expect(scheduler.registerCount == 1)
    #expect(scheduler.scheduleCount == 1)
    #expect(await scheduler.runRegisteredTask())
}

@Test func backgroundSyncServiceExposesRegistrationFailureWithoutRetryingFromLifecycle() async {
    let scheduler = RecordingBackgroundScheduler()
    scheduler.registerResult = false
    let service = BackgroundSyncService(scheduler: scheduler) { true }

    #expect(!service.register())
    #expect(!service.register())

    #expect(scheduler.registerCount == 1)
}

@Test @MainActor func appModelShowsFfiSyncErrorsWithoutDebugEnumWrapper() async {
    let model = AppModel(repository: StartupFailedRepository(error: FfiCoreError.SyncError(errorMessage: "server error 400: invalid registration")))

    await model.load()

    #expect(model.errorMessage == "server error 400: invalid registration")
}

@Test @MainActor func backgroundSyncDoesNotReplaceVisibleErrorOnFailure() async {
    let repository = PreviewTaskRepository()
    let model = AppModel(repository: repository)
    model.updateReachability(.offline)
    model.clearError()

    let success = await model.backgroundSyncNow()

    #expect(!success)
    #expect(model.errorMessage == nil)
    #expect(model.lastSyncStatusMessage == "Sync failed.")
}

@Test @MainActor func backgroundSyncDoesNotClearVisibleErrorOnSuccess() async {
    let repository = SuccessfulSyncRepository()
    repository.failDeletes = true
    let model = AppModel(repository: repository)
    model.updateReachability(.online)
    _ = await model.deleteTask(id: UUID())
    #expect(model.errorMessage == "Delete failed")
    repository.failDeletes = false

    let success = await model.backgroundSyncNow()

    #expect(success)
    #expect(model.errorMessage == "Delete failed")
    #expect(model.lastSyncStatusMessage == "Sync completed.")
}
