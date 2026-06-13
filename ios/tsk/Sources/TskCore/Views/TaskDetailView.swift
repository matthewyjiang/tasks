import SwiftUI

public struct TaskDetailView: View {
    @ObservedObject var model: AppModel
    private let task: TaskItem
    @State private var draft: TaskItem

    public init(model: AppModel, task: TaskItem) {
        self.model = model
        self.task = task
        _draft = State(initialValue: task)
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

            Section("Metadata") {
                if let dueAt = draft.dueAt {
                    LabeledContent("Due", value: dueAt.formatted(date: .abbreviated, time: .shortened))
                } else {
                    LabeledContent("Due", value: "None")
                }

                LabeledContent("List", value: model.listName(for: draft.listID) ?? "Inbox")
                LabeledContent("Tags", value: draft.tags.isEmpty ? "None" : draft.tags.joined(separator: ", "))
            }
        }
        .navigationTitle(draft.title.isEmpty ? "Task" : draft.title)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") {
                    Task { await model.save(task: draft) }
                }
            }
        }
        .onChange(of: task.id) { _, _ in draft = task }
    }
}
