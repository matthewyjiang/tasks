import Foundation

public struct StartupFailedRepository: TaskRepository {
    private let error: any Error

    public init(error: any Error) {
        self.error = error
    }

    public func loadTasks() async throws -> [TaskItem] { throw error }
    public func loadLists() async throws -> [TaskListItem] { throw error }
    public func createTask(title: String, body: String, dueAt: Date?, listID: UUID?, tags: [String]) async throws -> TaskItem { throw error }
    public func updateTask(_ task: TaskItem) async throws -> TaskItem { throw error }
    public func deleteTask(id: UUID) async throws { throw error }
    public func createList(name: String) async throws -> TaskListItem { throw error }
    public func updateList(_ list: TaskListItem) async throws -> TaskListItem { throw error }
    public func deleteList(id: UUID) async throws { throw error }
    public func syncSummary() async throws -> SyncSummary { throw error }
}
