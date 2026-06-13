import SwiftUI

public struct TaskDetailView: View {
    @ObservedObject var model: AppModel
    private let task: TaskItem
    @State private var draft: TaskItem
    @State private var hasDueDate: Bool
    @State private var dueAt: Date
    @State private var tagsText: String

    public init(model: AppModel, task: TaskItem) {
        self.model = model
        self.task = task
        _draft = State(initialValue: task)
        _hasDueDate = State(initialValue: task.dueAt != nil)
        _dueAt = State(initialValue: task.dueAt ?? Date())
        _tagsText = State(initialValue: task.tags.joined(separator: ", "))
    }

    public var body: some View {
        Form {
            Section("Task") {
                TextField("Title", text: $draft.title)
                TextEditor(text: $draft.body)
                    .frame(minHeight: 120)
            }

            Section("Status") {
                Picker("Status", selection: $draft.status) {
                    Text("Open").tag(TaskStatus.open)
                    Text("Done").tag(TaskStatus.done)
                }
                .pickerStyle(.segmented)
            }

            TaskMetadataEditor(
                lists: model.lists,
                hasDueDate: $hasDueDate,
                dueAt: $dueAt,
                listID: $draft.listID,
                tagsText: $tagsText
            )

            Section {
                Button("Delete Task", role: .destructive) {
                    Task { await model.deleteTask(id: draft.id) }
                }
            }
        }
        .navigationTitle(draft.title.isEmpty ? "Task" : draft.title)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") {
                    Task { await saveDraft() }
                }
            }
        }
        .onChange(of: task) { _, newTask in
            resetDraft(to: newTask)
        }
    }

    private func saveDraft() async {
        var savedDraft = draft
        savedDraft.dueAt = hasDueDate ? dueAt : nil
        savedDraft.tags = tags(from: tagsText)
        await model.save(task: savedDraft)
    }

    private func resetDraft(to task: TaskItem) {
        draft = task
        hasDueDate = task.dueAt != nil
        dueAt = task.dueAt ?? Date()
        tagsText = task.tags.joined(separator: ", ")
    }
}
