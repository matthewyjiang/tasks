import SwiftUI

public struct TaskListView: View {
    @ObservedObject var model: AppModel
    private var selectionOverride: TaskSelection?
    private var usesSplitSelection: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isNewTaskPresented = false
    @State private var expandedTaskID: UUID?
    @State private var activeRowPresentation: TaskRowPresentation?
    @State private var taskPendingDeletion: TaskItem?
    @State private var isDeleteConfirmationPresented = false
    @State private var detailTaskID: UUID?
    @State private var flushInlineEditsToken = 0
    @State private var taskSaveChains: [UUID: Task<Void, Never>] = [:]

    public init(model: AppModel, selection: TaskSelection? = nil, usesSplitSelection: Bool = false) {
        self.model = model
        self.selectionOverride = selection
        self.usesSplitSelection = usesSplitSelection
    }

    private var activeSelection: TaskSelection {
        selectionOverride ?? model.selection
    }

    private var visibleTasks: [TaskItem] {
        model.visibleTasks(for: activeSelection)
    }

    public var body: some View {
        taskList
            .navigationTitle(model.title(for: activeSelection))
            .searchable(text: $model.searchQuery, prompt: "Search tasks")
            .toolbar { newTaskToolbarItem }
            .sheet(isPresented: $isNewTaskPresented) {
                NewTaskView(model: model, defaults: TaskDefaults(selection: activeSelection))
            }
            .sheet(item: $activeRowPresentation) { presentation in
                taskRowPresentationView(for: presentation)
            }
            .navigationDestination(item: $detailTaskID) { taskID in
                if let latestTask = latestTask(id: taskID) {
                    TaskDetailView(model: model, task: latestTask)
                } else {
                    ContentUnavailableView("Task unavailable", systemImage: "exclamationmark.triangle")
                }
            }
            .confirmationDialog(
                "Delete Task?",
                isPresented: $isDeleteConfirmationPresented,
                titleVisibility: .visible
            ) {
                Button("Delete Task", role: .destructive) {
                    guard let task = taskPendingDeletion else { return }
                    Task {
                        if await model.deleteTask(id: task.id), expandedTaskID == task.id {
                            expandedTaskID = nil
                        }
                        taskPendingDeletion = nil
                    }
                }
                Button("Cancel", role: .cancel) { taskPendingDeletion = nil }
            } message: {
                if let task = taskPendingDeletion {
                    Text("Delete \"\(task.title.isEmpty ? "Untitled" : task.title)\"?")
                }
            }
            .onAppear {
                if let selectionOverride {
                    model.select(.tasks(selectionOverride))
                }
            }
    }

    private var taskList: some View {
        List(selection: usesSplitSelection ? $model.selectedTaskID : .constant(nil)) {
            ForEach(visibleTasks) { task in
                taskListRow(for: task)
            }
        }
        .platformTaskListStyle()
        .animation(reduceMotion ? nil : .snappy(duration: 0.22), value: visibleTasks.map(\.id))
        .overlay {
            if visibleTasks.isEmpty {
                ContentUnavailableView("No tasks", systemImage: "tray", description: Text("Create a task or adjust search."))
            }
        }
        .onChange(of: visibleTasks.map(\.id)) { _, ids in
            if let expandedTaskID, !ids.contains(expandedTaskID) {
                self.expandedTaskID = nil
            }
        }
    }

    private func taskListRow(for task: TaskItem) -> some View {
        InlineTaskRowView(
            task: task,
            listName: model.listName(for: task.listID),
            isExpanded: expandedTaskID == task.id,
            reduceMotion: reduceMotion,
            flushEditsToken: flushInlineEditsToken,
            toggleExpansion: { toggleExpansion(for: task) },
            toggleStatus: { toggleStatus(taskID: task.id) },
            saveInline: { title, body in saveInline(taskID: task.id, title: title, body: body) },
            presentDue: { presentRowInterface(.due(task.id)) },
            presentListTags: { presentRowInterface(.listTags(task.id)) },
            presentMore: { presentRowInterface(.more(task.id)) }
        )
            .listRowSeparator(.hidden)
            .listRowBackground(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(Self.rowBackgroundColor)
                    .padding(.vertical, 3)
            )
            .swipeActions(edge: .leading) {
                statusSwipeButton(for: task)
            }
            .swipeActions(edge: .trailing) {
                Button(role: .destructive) {
                    taskPendingDeletion = task
                    isDeleteConfirmationPresented = true
                } label: {
                    Label("Delete Task", systemImage: "trash")
                }
                .accessibilityLabel("Delete Task")
                .accessibilityHint("Deletes this task.")
            }
            .accessibilityAction(named: statusActionTitle(for: task)) {
                toggleStatus(taskID: task.id)
            }
    }

    private func statusActionTitle(for task: TaskItem) -> String {
        task.status == .done ? "Mark Open" : "Mark Done"
    }

    private func statusActionImage(for task: TaskItem) -> String {
        task.status == .done ? "arrow.uturn.backward.circle" : "checkmark.circle"
    }

    private func statusActionHint(for task: TaskItem) -> String {
        task.status == .done ? "Reopens this task." : "Completes this task."
    }

    private func statusSwipeButton(for task: TaskItem) -> some View {
        Button(statusActionTitle(for: task), systemImage: statusActionImage(for: task)) {
            toggleStatus(taskID: task.id)
        }
        .tint(.green)
        .accessibilityLabel(statusActionTitle(for: task))
        .accessibilityHint(statusActionHint(for: task))
    }

    private func toggleExpansion(for task: TaskItem) {
        flushInlineEdits()
        if expandedTaskID == task.id {
            expandedTaskID = nil
        } else {
            expandedTaskID = task.id
            if usesSplitSelection {
                model.selectedTaskID = task.id
            }
        }
    }

    private func presentRowInterface(_ presentation: TaskRowPresentation) {
        flushInlineEdits()
        activeRowPresentation = presentation
    }

    private func flushInlineEdits() {
        flushInlineEditsToken += 1
    }

    private func latestTask(id: UUID) -> TaskItem? {
        model.tasks.first { $0.id == id }
    }

    private func mutateLatestTask(id: UUID, _ mutate: @escaping (inout TaskItem) -> Void) {
        let previousSave = taskSaveChains[id]
        let save = Task { @MainActor in
            await previousSave?.value
            guard var latest = latestTask(id: id) else { return }
            mutate(&latest)
            await model.save(task: latest)
        }
        taskSaveChains[id] = save
    }

    private func toggleStatus(taskID: UUID) {
        mutateLatestTask(id: taskID) { task in
            task.status = task.status == .done ? .open : .done
        }
    }

    private func saveInline(taskID: UUID, title: String, body: String) {
        mutateLatestTask(id: taskID) { task in
            task.title = title
            task.body = body
        }
    }

    private func saveDueReminder(taskID: UUID, dueAt: Date?, reminderOffsetMs: Int64?) {
        mutateLatestTask(id: taskID) { task in
            task.dueAt = dueAt
            task.reminderOffsetMs = reminderOffsetMs
        }
    }

    private func saveListTags(taskID: UUID, listID: UUID?, tags: [String]) {
        mutateLatestTask(id: taskID) { task in
            task.listID = listID
            task.tags = tags
        }
    }

    @ViewBuilder
    private func taskRowPresentationView(for presentation: TaskRowPresentation) -> some View {
        if let task = model.tasks.first(where: { $0.id == presentation.taskID }) {
            switch presentation {
            case .due:
                TaskDueReminderEditor(task: task) { dueAt, reminderOffsetMs in
                    saveDueReminder(taskID: task.id, dueAt: dueAt, reminderOffsetMs: reminderOffsetMs)
                }
                .presentationTaskSheet()
            case .listTags:
                TaskListTagsEditor(task: task, lists: model.lists) { listID, tags in
                    saveListTags(taskID: task.id, listID: listID, tags: tags)
                }
                .presentationTaskSheet()
            case .more:
                TaskMoreActionsView(
                    task: task,
                    openDetails: {
                        openFullDetails(for: task)
                    },
                    requestDelete: {
                        Task {
                            if await model.deleteTask(id: task.id), expandedTaskID == task.id {
                                expandedTaskID = nil
                            }
                            activeRowPresentation = nil
                        }
                    }
                )
                .presentationTaskSheet()
            }
        } else {
            ContentUnavailableView("Task unavailable", systemImage: "exclamationmark.triangle")
        }
    }

    private func openFullDetails(for task: TaskItem) {
        flushInlineEdits()
        activeRowPresentation = nil
        model.selectedTaskID = task.id
        if !usesSplitSelection {
            detailTaskID = task.id
        }
    }

    private static var rowBackgroundColor: Color {
        #if os(iOS)
        Color(.secondarySystemGroupedBackground)
        #else
        Color(nsColor: .controlBackgroundColor)
        #endif
    }

    @ToolbarContentBuilder
    private var newTaskToolbarItem: some ToolbarContent {
        ToolbarItem(placement: .primaryAction) {
            Button {
                isNewTaskPresented = true
            } label: {
                Label("New Task", systemImage: "plus")
            }
            .accessibilityLabel("New Task")
            .accessibilityHint("Opens a sheet to create a task.")
            .help("New Task")
        }
    }
}

private enum TaskRowPresentation: Identifiable, Equatable {
    case due(UUID)
    case listTags(UUID)
    case more(UUID)

    var id: String {
        switch self {
        case .due(let taskID): "due-\(taskID.uuidString)"
        case .listTags(let taskID): "list-tags-\(taskID.uuidString)"
        case .more(let taskID): "more-\(taskID.uuidString)"
        }
    }

    var taskID: UUID {
        switch self {
        case .due(let taskID), .listTags(let taskID), .more(let taskID): taskID
        }
    }
}

private extension View {
    @ViewBuilder
    func platformTaskListStyle() -> some View {
        #if os(iOS)
        self.listStyle(.insetGrouped)
        #else
        self.listStyle(.inset)
        #endif
    }

    @ViewBuilder
    func presentationTaskSheet() -> some View {
        #if os(iOS)
        self.presentationDetents([.medium, .large])
        #else
        self
        #endif
    }
}

private struct InlineTaskRowView: View {
    var task: TaskItem
    var listName: String?
    var isExpanded: Bool
    var reduceMotion: Bool
    var flushEditsToken: Int
    var toggleExpansion: () -> Void
    var toggleStatus: () -> Void
    var saveInline: (String, String) -> Void
    var presentDue: () -> Void
    var presentListTags: () -> Void
    var presentMore: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top, spacing: 12) {
                Button(action: toggleStatus) {
                    Image(systemName: task.status == .done ? "checkmark.circle.fill" : "circle")
                        .font(.title3)
                        .foregroundStyle(task.status == .done ? .green : .secondary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(task.status == .done ? "Mark Open" : "Mark Done")
                .accessibilityHint(task.status == .done ? "Reopens this task." : "Completes this task.")

                Button(action: toggleExpansion) {
                    TaskRowHeader(task: task, listName: listName, isExpanded: isExpanded)
                }
                .buttonStyle(.plain)
                .accessibilityHint(isExpanded ? "Collapses inline editing." : "Expands this task for inline editing.")
                .accessibilityValue(isExpanded ? "Expanded" : "Collapsed")
            }

            if isExpanded {
                ExpandedTaskEditor(
                    task: task,
                    flushEditsToken: flushEditsToken,
                    saveInline: saveInline,
                    presentDue: presentDue,
                    presentListTags: presentListTags,
                    presentMore: presentMore
                )
                .transition(reduceMotion ? .opacity : .move(edge: .top).combined(with: .opacity))
            }
        }
        .padding(.vertical, 6)
        .animation(reduceMotion ? nil : .snappy(duration: 0.22), value: isExpanded)
        .tag(task.id)
    }
}

private struct TaskRowHeader: View {
    var task: TaskItem
    var listName: String?
    var isExpanded: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 6) {
                    Text(task.title.isEmpty ? "Untitled" : task.title)
                        .strikethrough(task.status == .done)
                        .font(.body.weight(.medium))
                        .contentTransition(.opacity)
                    Image(systemName: "chevron.right")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.tertiary)
                        .rotationEffect(.degrees(isExpanded ? 90 : 0))
                        .accessibilityHidden(true)
                }
            TaskRowMetadataSummary(task: task, listName: listName)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct ExpandedTaskEditor: View {
    enum Field: Hashable { case title, body }

    var task: TaskItem
    var flushEditsToken: Int
    var saveInline: (String, String) -> Void
    var presentDue: () -> Void
    var presentListTags: () -> Void
    var presentMore: () -> Void

    @State private var draftTitle: String
    @State private var draftBody: String
    @State private var bodySaveTask: Task<Void, Never>?
    @FocusState private var focusedField: Field?

    init(
        task: TaskItem,
        flushEditsToken: Int,
        saveInline: @escaping (String, String) -> Void,
        presentDue: @escaping () -> Void,
        presentListTags: @escaping () -> Void,
        presentMore: @escaping () -> Void
    ) {
        self.task = task
        self.flushEditsToken = flushEditsToken
        self.saveInline = saveInline
        self.presentDue = presentDue
        self.presentListTags = presentListTags
        self.presentMore = presentMore
        _draftTitle = State(initialValue: task.title)
        _draftBody = State(initialValue: task.body)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField("Title", text: $draftTitle)
                .textFieldStyle(.plain)
                .font(.body.weight(.medium))
                .submitLabel(.done)
                .focused($focusedField, equals: .title)
                .accessibilityLabel("Task Title")
                .onSubmit { saveNow() }

            ZStack(alignment: .topLeading) {
                if draftBody.isEmpty {
                    Text("Notes")
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 5)
                        .padding(.vertical, 8)
                        .allowsHitTesting(false)
                }
                TextEditor(text: $draftBody)
                    .frame(minHeight: 56)
                    .scrollContentBackground(.hidden)
                    .focused($focusedField, equals: .body)
                    .accessibilityLabel("Notes")
            }
            .padding(6)
            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))

            HStack(spacing: 10) {
                Spacer()
                TaskRowIconButton(systemImage: "calendar.badge.clock", title: "Due Date and Reminder", action: presentDue)
                TaskRowIconButton(systemImage: "list.bullet", title: "List and Tags", action: presentListTags)
                TaskRowIconButton(systemImage: "ellipsis.circle", title: "More Task Actions", action: presentMore)
            }
        }
        .padding(.leading, 34)
        .padding(.top, 2)
        .onChange(of: draftBody) { _, _ in scheduleBodySave() }
        .onChange(of: focusedField) { oldValue, newValue in
            if oldValue != nil, newValue == nil { saveNow() }
        }
        .onChange(of: task) { _, newTask in
            if focusedField == nil {
                draftTitle = newTask.title
                draftBody = newTask.body
            }
        }
        .onChange(of: flushEditsToken) { _, _ in saveNow() }
        .onDisappear { saveNow() }
    }

    private func scheduleBodySave() {
        bodySaveTask?.cancel()
        bodySaveTask = Task { @MainActor in
            try? await Task.sleep(nanoseconds: 700_000_000)
            guard !Task.isCancelled else { return }
            saveNow()
        }
    }

    private func saveNow() {
        bodySaveTask?.cancel()
        let title = draftTitle.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty else { return }
        guard title != task.title || draftBody != task.body else { return }
        saveInline(title, draftBody)
    }
}

private struct TaskRowIconButton: View {
    var systemImage: String
    var title: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.body.weight(.semibold))
                .frame(width: 34, height: 34)
                .background(.regularMaterial, in: Circle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
        .help(title)
    }
}

private struct TaskRowMetadataSummary: View {
    var task: TaskItem
    var listName: String?

    var body: some View {
        HStack(spacing: 8) {
            if let dueAt = task.dueAt {
                Label(dueAt.formatted(date: .abbreviated, time: .omitted), systemImage: "calendar")
            }
            if let listName {
                Label(listName, systemImage: "list.bullet")
            }
            if !task.tags.isEmpty {
                Label("\(task.tags.count)", systemImage: "tag")
                    .accessibilityLabel("\(task.tags.count) tags")
            }
        }
        .font(.caption)
        .foregroundStyle(.secondary)
        .lineLimit(1)
    }
}

private struct TaskDueReminderEditor: View {
    @Environment(\.dismiss) private var dismiss
    @State private var hasDueDate: Bool
    @State private var dueAt: Date
    @State private var reminderSelection: ReminderPreset
    var save: (Date?, Int64?) -> Void

    init(task: TaskItem, save: @escaping (Date?, Int64?) -> Void) {
        _hasDueDate = State(initialValue: task.dueAt != nil)
        _dueAt = State(initialValue: task.dueAt ?? Date())
        _reminderSelection = State(initialValue: ReminderPreset(offsetMs: task.reminderOffsetMs))
        self.save = save
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Due Date") {
                    Toggle("Has Due Date", isOn: $hasDueDate)
                    if hasDueDate {
                        DatePicker("Due", selection: $dueAt, displayedComponents: [.date, .hourAndMinute])
                        HStack {
                            Button("Today") { setDueDate(daysFromToday: 0) }
                            Button("Tomorrow") { setDueDate(daysFromToday: 1) }
                            Button("Clear", role: .destructive) { hasDueDate = false }
                        }
                    }
                }

                Section("Reminder") {
                    Picker("Reminder", selection: $reminderSelection) {
                        ForEach(ReminderPreset.allCases) { preset in
                            Text(preset.title).tag(preset)
                        }
                    }
                }
            }
            .navigationTitle("Due & Reminder")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
            .onChange(of: hasDueDate) { _, _ in saveDraft() }
            .onChange(of: dueAt) { _, _ in saveDraft() }
            .onChange(of: reminderSelection) { _, _ in saveDraft() }
        }
    }

    private func setDueDate(daysFromToday days: Int) {
        hasDueDate = true
        dueAt = Calendar.current.date(byAdding: .day, value: days, to: Date()) ?? Date()
    }

    private func saveDraft() {
        save(hasDueDate ? dueAt : nil, reminderSelection.offsetMs)
    }
}

private enum ReminderPreset: String, CaseIterable, Identifiable {
    case none
    case atDueTime
    case fiveMinutes
    case fifteenMinutes
    case oneHour
    case oneDay

    var id: String { rawValue }

    init(offsetMs: Int64?) {
        switch offsetMs {
        case nil: self = .none
        case 0: self = .atDueTime
        case 5 * 60 * 1_000: self = .fiveMinutes
        case 15 * 60 * 1_000: self = .fifteenMinutes
        case 60 * 60 * 1_000: self = .oneHour
        case 24 * 60 * 60 * 1_000: self = .oneDay
        default: self = .none
        }
    }

    var title: String {
        switch self {
        case .none: "None"
        case .atDueTime: "At due time"
        case .fiveMinutes: "5 minutes before"
        case .fifteenMinutes: "15 minutes before"
        case .oneHour: "1 hour before"
        case .oneDay: "1 day before"
        }
    }

    var offsetMs: Int64? {
        switch self {
        case .none: nil
        case .atDueTime: 0
        case .fiveMinutes: 5 * 60 * 1_000
        case .fifteenMinutes: 15 * 60 * 1_000
        case .oneHour: 60 * 60 * 1_000
        case .oneDay: 24 * 60 * 60 * 1_000
        }
    }
}

private struct TaskListTagsEditor: View {
    @Environment(\.dismiss) private var dismiss
    @State private var listID: UUID?
    @State private var tagsText: String
    var lists: [TaskListItem]
    var save: (UUID?, [String]) -> Void

    init(task: TaskItem, lists: [TaskListItem], save: @escaping (UUID?, [String]) -> Void) {
        _listID = State(initialValue: task.listID)
        _tagsText = State(initialValue: task.tags.joined(separator: ", "))
        self.lists = lists
        self.save = save
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("List") {
                    Picker("List", selection: $listID) {
                        Text("Inbox").tag(UUID?.none)
                        ForEach(lists) { list in
                            Text(list.name).tag(Optional(list.id))
                        }
                    }
                }

                Section("Tags") {
                    TextField("Tags", text: $tagsText, prompt: Text("ios, errands"))
                }
            }
            .navigationTitle("List & Tags")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") {
                        saveDraft()
                        dismiss()
                    }
                }
            }
            .onChange(of: listID) { _, _ in saveDraft() }
            .onChange(of: tagsText) { _, _ in saveDraft() }
        }
    }

    private func saveDraft() {
        save(listID, tags(from: tagsText))
    }
}

private struct TaskMoreActionsView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var isDeleteConfirmationPresented = false
    var task: TaskItem
    var openDetails: () -> Void
    var requestDelete: () -> Void

    var body: some View {
        NavigationStack {
            List {
                Section("Sharing") {
                    Label("Sharing UI needs AppModel wiring", systemImage: "person.2")
                        .foregroundStyle(.secondary)
                    Text("The shared core exposes sharing primitives, but this iOS screen does not yet have a high-level sharing flow.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Section("More") {
                    Button("Open Full Details", systemImage: "doc.text") {
                        openDetails()
                    }
                    Button("Delete Task", systemImage: "trash", role: .destructive) {
                        isDeleteConfirmationPresented = true
                    }
                }
            }
            .navigationTitle(task.title.isEmpty ? "More" : task.title)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
            .confirmationDialog(
                "Delete Task?",
                isPresented: $isDeleteConfirmationPresented,
                titleVisibility: .visible
            ) {
                Button("Delete Task", role: .destructive) {
                    requestDelete()
                    dismiss()
                }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("Delete \"\(task.title.isEmpty ? "Untitled" : task.title)\"?")
            }
        }
    }
}

private struct TaskDefaults {
    var listID: UUID?
    var dueAt: Date?
    var status: TaskStatus

    init(selection: TaskSelection, now: Date = Date(), calendar: Calendar = .current) {
        switch selection {
        case .builtIn(.today):
            listID = nil
            dueAt = now
            status = .open
        case .builtIn(.upcoming):
            listID = nil
            dueAt = calendar.date(byAdding: .day, value: 1, to: now)
            status = .open
        case .builtIn(.done):
            listID = nil
            dueAt = nil
            status = .done
        case .list(let id):
            listID = id
            dueAt = nil
            status = .open
        default:
            listID = nil
            dueAt = nil
            status = .open
        }
    }
}

private struct NewTaskView: View {
    @Environment(\.dismiss) private var dismiss
    @ObservedObject var model: AppModel

    @State private var title = ""
    @State private var bodyText = ""
    @State private var status: TaskStatus
    @State private var hasDueDate: Bool
    @State private var dueAt: Date
    @State private var listID: UUID?
    @State private var tagsText = ""

    init(model: AppModel, defaults: TaskDefaults) {
        self.model = model
        _status = State(initialValue: defaults.status)
        _hasDueDate = State(initialValue: defaults.dueAt != nil)
        _dueAt = State(initialValue: defaults.dueAt ?? Date())
        _listID = State(initialValue: defaults.listID)
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Task") {
                    TextField("Title", text: $title)
                    TextEditor(text: $bodyText)
                        .frame(minHeight: 120)
                }

                Section("Status") {
                    Picker("Status", selection: $status) {
                        Text("Open").tag(TaskStatus.open)
                        Text("Done").tag(TaskStatus.done)
                    }
                    .pickerStyle(.segmented)
                }

                TaskMetadataEditor(
                    lists: model.lists,
                    hasDueDate: $hasDueDate,
                    dueAt: $dueAt,
                    listID: $listID,
                    tagsText: $tagsText
                )
            }
            .navigationTitle("New Task")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        Task {
                            let created = await model.createTask(
                                title: title,
                                body: bodyText,
                                dueAt: hasDueDate ? dueAt : nil,
                                listID: listID,
                                tags: tags(from: tagsText),
                                status: status
                            )
                            if created != nil { dismiss() }
                        }
                    }
                    .disabled(title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
        }
    }
}

public struct TaskRowView: View {
    public var task: TaskItem
    public var listName: String?

    public init(task: TaskItem, listName: String?) {
        self.task = task
        self.listName = listName
    }

    public var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: task.status == .done ? "checkmark.circle.fill" : "circle")
                .font(.title3)
                .foregroundStyle(task.status == .done ? .green : .secondary)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 6) {
                Text(task.title.isEmpty ? "Untitled" : task.title)
                    .strikethrough(task.status == .done)
                    .font(.body.weight(.medium))
                    .contentTransition(.opacity)
                metadataSummary
            }
        }
        .padding(.vertical, 6)
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private var metadataSummary: some View {
        HStack(spacing: 8) {
            if let dueAt = task.dueAt {
                Label(dueAt.formatted(date: .abbreviated, time: .omitted), systemImage: "calendar")
            }
            if let listName {
                Label(listName, systemImage: "list.bullet")
            }
            if !task.tags.isEmpty {
                Label("\(task.tags.count)", systemImage: "tag")
                    .accessibilityLabel("\(task.tags.count) tags")
            }
        }
        .font(.caption)
        .foregroundStyle(.secondary)
        .lineLimit(1)
    }
}

struct TaskMetadataEditor: View {
    var lists: [TaskListItem]
    @Binding var hasDueDate: Bool
    @Binding var dueAt: Date
    @Binding var listID: UUID?
    @Binding var tagsText: String

    var body: some View {
        Section("Metadata") {
            TaskMetadataFields(
                lists: lists,
                hasDueDate: $hasDueDate,
                dueAt: $dueAt,
                listID: $listID,
                tagsText: $tagsText
            )
        }
    }
}

struct TaskMetadataFields: View {
    var lists: [TaskListItem]
    @Binding var hasDueDate: Bool
    @Binding var dueAt: Date
    @Binding var listID: UUID?
    @Binding var tagsText: String

    var body: some View {
        Toggle("Due Date", isOn: $hasDueDate)
        if hasDueDate {
            DatePicker("Due", selection: $dueAt, displayedComponents: [.date, .hourAndMinute])
        }

        Picker("List", selection: $listID) {
            Text("Inbox").tag(UUID?.none)
            ForEach(lists) { list in
                Text(list.name).tag(Optional(list.id))
            }
        }

        TextField("Tags", text: $tagsText, prompt: Text("ios, errands"))
    }
}

func tags(from text: String) -> [String] {
    var seen = Set<String>()
    return text
        .split(separator: ",")
        .compactMap { part in
            let tag = part.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !tag.isEmpty else { return nil }
            let key = tag.lowercased()
            guard seen.insert(key).inserted else { return nil }
            return tag
        }
}
