import Foundation

public protocol BackgroundRefreshScheduling: Sendable {
    @discardableResult
    func register(handler: @escaping @Sendable () async -> Bool) -> Bool
    func schedule()
}

public final class BackgroundSyncService: @unchecked Sendable {
    private let scheduler: any BackgroundRefreshScheduling
    private let sync: @Sendable () async -> Bool
    private let lock = NSLock()
    private var registrationResult: Bool?

    public init(scheduler: any BackgroundRefreshScheduling, sync: @escaping @Sendable () async -> Bool) {
        self.scheduler = scheduler
        self.sync = sync
    }

    @discardableResult
    public func register() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if let registrationResult {
            return registrationResult
        }
        let result = scheduler.register { [sync] in
            await sync()
        }
        registrationResult = result
        return result
    }

    public func scheduleNextRefresh() {
        scheduler.schedule()
    }
}

#if canImport(BackgroundTasks) && os(iOS)
import BackgroundTasks

public final class BGAppRefreshScheduler: BackgroundRefreshScheduling, @unchecked Sendable {
    public static let defaultIdentifier = "com.matthewyjiang.tsk.refresh"

    private let identifier: String
    private let earliestBeginDate: @Sendable () -> Date
    private let scheduler: BGTaskScheduler

    public init(
        identifier: String = BGAppRefreshScheduler.defaultIdentifier,
        earliestBeginDate: @escaping @Sendable () -> Date = { Date(timeIntervalSinceNow: 15 * 60) },
        scheduler: BGTaskScheduler = .shared
    ) {
        self.identifier = identifier
        self.earliestBeginDate = earliestBeginDate
        self.scheduler = scheduler
    }

    @discardableResult
    public func register(handler: @escaping @Sendable () async -> Bool) -> Bool {
        scheduler.register(forTaskWithIdentifier: identifier, using: nil) { task in
            guard let task = task as? BGAppRefreshTask else {
                task.setTaskCompleted(success: false)
                return
            }
            self.schedule()
            let completion = BGTaskCompletion(task: task)
            let work = Task {
                let success = await handler()
                completion.complete(success: success)
            }
            task.expirationHandler = {
                work.cancel()
                completion.complete(success: false)
            }
        }
    }

    public func schedule() {
        let request = BGAppRefreshTaskRequest(identifier: identifier)
        request.earliestBeginDate = earliestBeginDate()
        do {
            try scheduler.submit(request)
        } catch {
            // Best-effort by design; foreground launch/resume/manual sync remains authoritative.
        }
    }
}

private final class BGTaskCompletion: @unchecked Sendable {
    private let task: BGTask
    private let lock = NSLock()
    private var completed = false

    init(task: BGTask) {
        self.task = task
    }

    func complete(success: Bool) {
        lock.lock()
        guard !completed else {
            lock.unlock()
            return
        }
        completed = true
        lock.unlock()
        task.setTaskCompleted(success: success)
    }
}
#endif
