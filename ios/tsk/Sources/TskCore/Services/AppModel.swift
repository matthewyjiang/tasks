import Foundation
import SwiftUI

@MainActor
public final class AppModel: ObservableObject {
    @Published public private(set) var tasks: [TaskItem] = []
    @Published public private(set) var lists: [TaskListItem] = []
    @Published public var selection: TaskSelection = .builtIn(.inbox)
    @Published public var destination: AppDestination = .tasks(.builtIn(.inbox))
    @Published public var selectedTaskID: UUID?
    @Published public var searchQuery: String = ""
    @Published public private(set) var syncSummary = SyncSummary()
    @Published public private(set) var localAccount: LocalAccountBootstrapState?
    @Published public private(set) var errorMessage: String?
    @Published public var syncServerURL: String
    @Published public private(set) var syncAuthState: FfiSyncAuthState = .localOnlyReady
    @Published public private(set) var enrollmentState: FfiEnrollmentState = .localOnlyReady
    @Published public private(set) var enrollmentDevicePublicKey: String?
    @Published public private(set) var isSyncOperationInFlight = false
    @Published public private(set) var lastSyncStatusMessage: String?

    public static let syncServerURLDefaultsKey = "tsk.sync.serverURL"

    private let repository: any TaskRepository
    private let filterEngine: TaskFilterEngine
    private let notificationScheduler: (any LocalNotificationScheduling)?
    private let syncCoordinator: SyncCoordinator?
    private let serverURLDefaults: UserDefaults

    public init(
        repository: any TaskRepository = PreviewTaskRepository(),
        filterEngine: TaskFilterEngine = TaskFilterEngine(),
        localAccount: LocalAccountBootstrapState? = nil,
        notificationScheduler: (any LocalNotificationScheduling)? = nil,
        syncCoordinator: SyncCoordinator? = nil,
        serverURLDefaults: UserDefaults = .standard
    ) {
        self.repository = repository
        self.filterEngine = filterEngine
        self.localAccount = localAccount
        self.notificationScheduler = notificationScheduler
        self.syncCoordinator = syncCoordinator
        self.serverURLDefaults = serverURLDefaults
        self.syncServerURL = serverURLDefaults.string(forKey: Self.syncServerURLDefaultsKey) ?? ""
    }

    public var selectedTask: TaskItem? {
        guard let selectedTaskID else { return nil }
        return tasks.first { $0.id == selectedTaskID }
    }

    public var visibleTasks: [TaskItem] {
        visibleTasks(for: selection)
    }

    public func visibleTasks(for selection: TaskSelection) -> [TaskItem] {
        filterEngine.tasks(tasks, for: selection, searchQuery: searchQuery)
    }

    public func listName(for id: UUID?) -> String? {
        guard let id else { return nil }
        return lists.first { $0.id == id }?.name
    }

    public func title(for selection: TaskSelection) -> String {
        switch selection {
        case .builtIn(let view): view.title
        case .list(let id): listName(for: id) ?? "List"
        }
    }

    public func clearError() {
        errorMessage = nil
    }

    public func updateReachability(_ status: ReachabilityStatus) {
        syncSummary.isOnline = status.isOnline
    }

    public var canSyncNow: Bool {
        syncAuthState == .syncReady && syncSummary.isOnline && !isSyncOperationInFlight
    }

    public func refreshSyncState() async {
        guard let syncCoordinator else { return }
        await syncCoordinator.updateServerURL(syncServerURL)
        syncAuthState = await syncCoordinator.syncAuthState()
        enrollmentState = await syncCoordinator.enrollmentState()
        enrollmentDevicePublicKey = try? await syncCoordinator.devicePublicKeyBase64()
    }

    public func updateSyncServerURL(_ serverURL: String) async {
        let trimmed = serverURL.trimmingCharacters(in: .whitespacesAndNewlines)
        syncServerURL = trimmed
        serverURLDefaults.set(trimmed, forKey: Self.syncServerURLDefaultsKey)
        await syncCoordinator?.updateServerURL(trimmed)
        await refreshSyncState()
    }

    public func configureSync(email: String, password: String) async {
        guard let syncCoordinator else {
            errorMessage = TaskRepositoryError.syncAdapterUnavailable.localizedDescription
            return
        }
        let trimmedEmail = email.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !syncServerURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !trimmedEmail.isEmpty,
              !password.isEmpty else {
            errorMessage = "Server URL, email, and password are required."
            return
        }
        isSyncOperationInFlight = true
        defer { isSyncOperationInFlight = false }
        do {
            await syncCoordinator.updateServerURL(syncServerURL)
            let result = try await syncCoordinator.configureSyncAuth(email: trimmedEmail, password: password)
            syncAuthState = result.state
            await refreshSyncState()
            lastSyncStatusMessage = "Signed in."
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
            await refreshSyncState()
        }
    }

    public func signOutSync() async {
        guard let syncCoordinator else { return }
        isSyncOperationInFlight = true
        defer { isSyncOperationInFlight = false }
        do {
            try await syncCoordinator.logout()
            lastSyncStatusMessage = "Signed out."
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
        await refreshSyncState()
    }

    public func acceptWrappedAccountDataKeyPayload(json: String) async {
        guard let syncCoordinator else { return }
        isSyncOperationInFlight = true
        defer { isSyncOperationInFlight = false }
        do {
            let payload = try Self.decodeWrappedAccountDataKeyPayload(json: json)
            enrollmentState = try await syncCoordinator.acceptWrappedAccountDataKeyPayload(payload)
            await refreshSyncState()
            lastSyncStatusMessage = "Account data key accepted."
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
            await refreshSyncState()
        }
    }

    public func syncNow() async {
        isSyncOperationInFlight = true
        defer { isSyncOperationInFlight = false }
        do {
            await refreshSyncState()
            syncSummary = try await repository.syncNow(isOnline: syncSummary.isOnline)
            async let loadedTasks = repository.loadTasks(includeDeleted: true)
            async let loadedLists = repository.loadLists()
            let allTasks = try await loadedTasks
            let allLists = try await loadedLists
            tasks = allTasks.filter { !$0.deleted }
            lists = allLists.filter { !$0.deleted }
            lastSyncStatusMessage = "Sync completed."
            errorMessage = nil
            await refreshSyncState()
        } catch {
            lastSyncStatusMessage = "Sync failed."
            errorMessage = error.localizedDescription
            if var summary = try? await repository.syncSummary() {
                summary.isOnline = syncSummary.isOnline
                syncSummary = summary
            }
            await refreshSyncState()
        }
    }

    public func select(_ destination: AppDestination) {
        self.destination = destination
        switch destination {
        case .tasks(let selection):
            self.selection = selection
            selectedTaskID = nil
        case .settings:
            selectedTaskID = nil
        }
    }

    public func load() async {
        do {
            async let loadedTasks = repository.loadTasks(includeDeleted: true)
            async let loadedLists = repository.loadLists()
            async let loadedSync = repository.syncSummary()
            let allTasks = try await loadedTasks
            let allLists = try await loadedLists
            let wasOnline = syncSummary.isOnline
            tasks = allTasks.filter { !$0.deleted }
            lists = allLists.filter { !$0.deleted }
            let notificationError = await reconcileNotifications(for: allTasks)
            syncSummary = try await loadedSync
            syncSummary.isOnline = wasOnline
            await refreshSyncState()
            errorMessage = notificationError
        } catch {
            errorMessage = error.localizedDescription
            await refreshSyncState()
        }
    }

    @discardableResult
    public func createTask(
        title: String,
        body: String,
        dueAt: Date?,
        listID: UUID?,
        tags: [String],
        status: TaskStatus = .open
    ) async -> TaskItem? {
        let trimmedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedTitle.isEmpty else { return nil }
        do {
            var task = try await repository.createTask(
                title: trimmedTitle,
                body: body,
                dueAt: dueAt,
                listID: listID,
                tags: Self.normalizedTags(tags)
            )
            if task.status != status {
                task.status = status
                task = try await repository.updateTask(task)
            }
            tasks.append(task)
            selectedTaskID = task.id
            let notificationError = await reconcileNotification(for: task)
            syncSummary = try await repository.syncSummary()
            errorMessage = notificationError
            return task
        } catch {
            errorMessage = error.localizedDescription
            return nil
        }
    }

    public func createQuickTask() async {
        let listID: UUID? = {
            if case .list(let id) = selection { return id }
            return nil
        }()
        await createTask(title: "New Task", body: "", dueAt: nil, listID: listID, tags: [])
    }

    public func toggleTaskStatus(_ task: TaskItem) async {
        var updated = task
        updated.status = task.status == .done ? .open : .done
        await save(task: updated)
    }

    public func save(task: TaskItem) async {
        do {
            var task = task
            task.title = task.title.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !task.title.isEmpty else { return }
            task.tags = Self.normalizedTags(task.tags)
            let saved = try await repository.updateTask(task)
            if let index = tasks.firstIndex(where: { $0.id == saved.id }) {
                tasks[index] = saved
            } else {
                tasks.append(saved)
            }
            let notificationError = await reconcileNotification(for: saved)
            syncSummary = try await repository.syncSummary()
            errorMessage = notificationError
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    @discardableResult
    public func deleteTask(id: UUID) async -> Bool {
        do {
            try await repository.deleteTask(id: id)
        } catch {
            errorMessage = error.localizedDescription
            return false
        }

        tasks.removeAll { $0.id == id }
        if selectedTaskID == id { selectedTaskID = nil }
        notificationScheduler?.cancelTaskNotification(taskID: id)
        do {
            syncSummary = try await repository.syncSummary()
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
        return true
    }

    @discardableResult
    public func createList(name: String) async -> TaskListItem? {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        do {
            let list = try await repository.createList(name: trimmed)
            lists.append(list)
            select(.tasks(.list(list.id)))
            syncSummary = try await repository.syncSummary()
            errorMessage = nil
            return list
        } catch {
            errorMessage = error.localizedDescription
            return nil
        }
    }

    public func renameList(id: UUID, name: String) async {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let current = lists.first(where: { $0.id == id }) else { return }
        do {
            var list = current
            list.name = trimmed
            let saved = try await repository.updateList(list)
            if let index = lists.firstIndex(where: { $0.id == id }) {
                lists[index] = saved
            }
            syncSummary = try await repository.syncSummary()
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func deleteList(id: UUID) async {
        do {
            try await repository.deleteList(id: id)
            lists.removeAll { $0.id == id }
            for index in tasks.indices where tasks[index].listID == id {
                tasks[index].listID = nil
            }
            if destination == .tasks(.list(id)) || selection == .list(id) {
                select(.tasks(.builtIn(.inbox)))
            }
            syncSummary = try await repository.syncSummary()
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private static func decodeWrappedAccountDataKeyPayload(json: String) throws -> FfiWrappedAccountDataKeyPayload {
        struct Payload: Decodable {
            let senderPublicKey: String
            let recipientPublicKey: String
            let ciphertext: String
            let nonce: String

            enum CodingKeys: String, CodingKey {
                case senderPublicKey = "sender_public_key"
                case recipientPublicKey = "recipient_public_key"
                case ciphertext
                case nonce
            }
        }

        let data = Data(json.utf8)
        let payload = try JSONDecoder().decode(Payload.self, from: data)
        guard let sender = Data(base64Encoded: payload.senderPublicKey),
              let recipient = Data(base64Encoded: payload.recipientPublicKey),
              let ciphertext = Data(base64Encoded: payload.ciphertext),
              let nonce = Data(base64Encoded: payload.nonce) else {
            throw TaskRepositoryError.invalidEnrollmentPayload
        }
        return FfiWrappedAccountDataKeyPayload(
            senderPublicKey: Array(sender),
            recipientPublicKey: Array(recipient),
            wrappedAccountDataKey: FfiBlob(ciphertext: Array(ciphertext), nonce: Array(nonce))
        )
    }

    private func reconcileNotifications(for tasks: [TaskItem]) async -> String? {
        var notificationError: String?
        for task in tasks {
            notificationError = await reconcileNotification(for: task) ?? notificationError
        }
        return notificationError
    }

    private func reconcileNotification(for task: TaskItem) async -> String? {
        guard let notificationScheduler else { return nil }
        guard !task.deleted, let fireDate = task.notificationFireDate() else {
            notificationScheduler.cancelTaskNotification(taskID: task.id)
            return nil
        }
        do {
            guard try await notificationScheduler.requestAuthorizationIfNeeded() else { return nil }
            try await notificationScheduler.schedule(
                LocalNotificationRequest(
                    taskID: task.id,
                    title: task.title,
                    body: task.body,
                    fireDate: fireDate
                )
            )
            return nil
        } catch {
            return error.localizedDescription
        }
    }

    private static func normalizedTags(_ tags: [String]) -> [String] {
        var seen = Set<String>()
        return tags.compactMap { tag in
            let trimmed = tag.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return nil }
            let key = trimmed.lowercased()
            guard seen.insert(key).inserted else { return nil }
            return trimmed
        }
    }
}
