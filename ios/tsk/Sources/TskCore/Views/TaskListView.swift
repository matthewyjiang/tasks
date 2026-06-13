import SwiftUI

public struct TaskListView: View {
    @ObservedObject var model: AppModel
    private var selectionOverride: TaskSelection?
    @State private var isNewTaskPresented = false

    public init(model: AppModel, selection: TaskSelection? = nil) {
        self.model = model
        self.selectionOverride = selection
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
            .onAppear {
                if let selectionOverride {
                    model.select(.tasks(selectionOverride))
                }
            }
    }

    private var taskList: some View {
        List {
            ForEach(visibleTasks) { task in
                NavigationLink {
                    TaskDetailView(model: model, task: task)
                } label: {
                    TaskRowView(task: task, listName: model.listName(for: task.listID))
                }
                .swipeActions(edge: .leading) {
                    Button(task.status == .done ? "Open" : "Done") {
                        Task { await model.toggleTaskStatus(task) }
                    }
                    .tint(.green)
                }
                .swipeActions(edge: .trailing) {
                    Button("Delete", role: .destructive) {
                        Task { await model.deleteTask(id: task.id) }
                    }
                }
                .accessibilityAction(named: task.status == .done ? "Mark Open" : "Mark Done") {
                    Task { await model.toggleTaskStatus(task) }
                }
            }
        }
        .overlay {
            if visibleTasks.isEmpty {
                ContentUnavailableView("No tasks", systemImage: "tray", description: Text("Create a task or adjust search."))
            }
        }
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
                .foregroundStyle(task.status == .done ? .green : .secondary)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                Text(task.title.isEmpty ? "Untitled" : task.title)
                    .strikethrough(task.status == .done)
                    .font(.body)
                HStack(spacing: 8) {
                    if let dueAt = task.dueAt {
                        Label(dueAt.formatted(date: .abbreviated, time: .omitted), systemImage: "calendar")
                    }
                    if let listName {
                        Label(listName, systemImage: "list.bullet")
                    }
                    if !task.tags.isEmpty {
                        Label(task.tags.joined(separator: ", "), systemImage: "tag")
                    }
                }
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
            }
        }
        .accessibilityElement(children: .combine)
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
