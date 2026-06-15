import Foundation
import Testing
@testable import TskCore

private final class RecordingNotificationScheduler: LocalNotificationScheduling, @unchecked Sendable {
    var authorizationRequested = 0
    var scheduled: [LocalNotificationRequest] = []
    var canceled: [UUID] = []

    func requestAuthorizationIfNeeded() async throws -> Bool {
        authorizationRequested += 1
        return true
    }

    func schedule(_ request: LocalNotificationRequest) async throws {
        scheduled.append(request)
    }

    func cancel(identifier: String) {}

    func cancelTaskNotification(taskID: UUID) {
        canceled.append(taskID)
    }
}

@Test func taskNotificationFireDateUsesSharedReminderOffsetSemantics() {
    let dueAt = Date(timeIntervalSince1970: 1_800_000_000)
    let task = TaskItem(title: "Remind me", dueAt: dueAt, reminderOffsetMs: 30 * 60 * 1_000)

    #expect(task.notificationFireDate(now: dueAt.addingTimeInterval(-3_600)) == dueAt.addingTimeInterval(-1_800))

    var done = task
    done.status = .done
    #expect(done.notificationFireDate(now: dueAt.addingTimeInterval(-3_600)) == nil)

    var overdue = task
    overdue.reminderOffsetMs = 60 * 60 * 1_000
    #expect(overdue.notificationFireDate(now: dueAt) == nil)
}

@Test @MainActor func appModelReconcilesNotificationsWhenLoadingAndMutatingTasks() async throws {
    let dueAt = Date().addingTimeInterval(3_600)
    let future = TaskItem(title: "Future", body: "Body", dueAt: dueAt, reminderOffsetMs: 15 * 60 * 1_000)
    let noReminder = TaskItem(title: "No reminder", dueAt: dueAt)
    let scheduler = RecordingNotificationScheduler()
    let repository = PreviewTaskRepository(tasks: [future, noReminder], lists: [], sync: SyncSummary())
    let model = AppModel(repository: repository, notificationScheduler: scheduler)

    await model.load()

    #expect(scheduler.authorizationRequested == 1)
    #expect(scheduler.scheduled.map(\.identifier) == [LocalNotificationRequest.identifier(forTaskID: future.id)])
    #expect(scheduler.canceled == [noReminder.id])

    await model.deleteTask(id: future.id)
    #expect(scheduler.canceled.contains(future.id))
}
