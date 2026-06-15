import Foundation
import UserNotifications

public struct LocalNotificationRequest: Equatable, Sendable {
    public var identifier: String
    public var title: String
    public var body: String
    public var fireDate: Date

    public init(identifier: String, title: String, body: String, fireDate: Date) {
        self.identifier = identifier
        self.title = title
        self.body = body
        self.fireDate = fireDate
    }

    public init(taskID: UUID, title: String, body: String, fireDate: Date) {
        self.init(identifier: Self.identifier(forTaskID: taskID), title: title, body: body, fireDate: fireDate)
    }

    public static func identifier(forTaskID taskID: UUID) -> String {
        "task.\(taskID.uuidString)"
    }
}

public protocol LocalNotificationScheduling: Sendable {
    func requestAuthorizationIfNeeded() async throws -> Bool
    func schedule(_ request: LocalNotificationRequest) async throws
    func cancel(identifier: String)
    func cancelTaskNotification(taskID: UUID)
}

public actor UserNotificationScheduler: LocalNotificationScheduling {
    private let center: UNUserNotificationCenter

    public init(center: UNUserNotificationCenter = .current()) {
        self.center = center
    }

    public func requestAuthorizationIfNeeded() async throws -> Bool {
        let settings = await notificationSettings()
        switch settings.authorizationStatus {
        case .authorized, .provisional, .ephemeral:
            return true
        case .denied:
            return false
        case .notDetermined:
            return try await requestAuthorization()
        @unknown default:
            return false
        }
    }

    public func schedule(_ request: LocalNotificationRequest) async throws {
        let content = UNMutableNotificationContent()
        content.title = request.title
        content.body = request.body
        content.sound = .default

        let components = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute, .second], from: request.fireDate)
        let trigger = UNCalendarNotificationTrigger(dateMatching: components, repeats: false)
        let notification = UNNotificationRequest(identifier: request.identifier, content: content, trigger: trigger)

        try await add(notification)
    }

    public nonisolated func cancel(identifier: String) {
        UNUserNotificationCenter.current().removePendingNotificationRequests(withIdentifiers: [identifier])
    }

    public nonisolated func cancelTaskNotification(taskID: UUID) {
        cancel(identifier: LocalNotificationRequest.identifier(forTaskID: taskID))
    }

    private func notificationSettings() async -> UNNotificationSettings {
        await withCheckedContinuation { continuation in
            center.getNotificationSettings { settings in
                continuation.resume(returning: settings)
            }
        }
    }

    private func requestAuthorization() async throws -> Bool {
        try await withCheckedThrowingContinuation { continuation in
            center.requestAuthorization(options: [.alert, .badge, .sound]) { granted, error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume(returning: granted)
                }
            }
        }
    }

    private func add(_ request: UNNotificationRequest) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, any Error>) in
            center.add(request) { error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume(returning: ())
                }
            }
        }
    }
}
