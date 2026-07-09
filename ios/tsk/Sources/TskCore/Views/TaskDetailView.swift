import SwiftUI

public struct TaskDetailView: View {
    @ObservedObject var model: AppModel
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let task: TaskItem
    @State private var draft: TaskItem
    @State private var hasDueDate: Bool
    @State private var dueAt: Date
    @State private var tagsText: String
    @State private var isMetadataExpanded = true

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
            Section {
                VStack(alignment: .leading, spacing: 12) {
                    TextField("Title", text: $draft.title)
                        .font(.title2.weight(.semibold))
                    TextEditor(text: $draft.body)
                        .frame(minHeight: 120)
                }
                .padding(.vertical, 4)
            } header: {
                Text("Task")
            } footer: {
                Text("Edits stay local-first and sync through the shared core.")
            }

            Section("Status") {
                Picker("Status", selection: $draft.status) {
                    Text("Open").tag(TaskStatus.open)
                    Text("Done").tag(TaskStatus.done)
                }
                .pickerStyle(.segmented)
            }

            Section {
                DisclosureGroup(isExpanded: $isMetadataExpanded) {
                    TaskMetadataFields(
                        lists: model.lists,
                        hasDueDate: $hasDueDate,
                        dueAt: $dueAt,
                        listID: $draft.listID,
                        tagsText: $tagsText
                    )
                } label: {
                    Label("Details", systemImage: "slider.horizontal.3")
                }
                .accessibilityHint("Shows due date, list, and tag fields.")
            }

            Section {
                Button("Delete Task", role: .destructive) {
                    Task {
                        if await model.deleteTask(id: draft.id) {
                            dismiss()
                        }
                    }
                }
            }
        }
        .animation(reduceMotion ? nil : .smooth(duration: 0.2), value: isMetadataExpanded)
        .navigationTitle(draft.title.isEmpty ? "Task" : draft.title)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") {
                    Task { await saveDraft() }
                }
                .accessibilityLabel("Save Task")
                .accessibilityHint("Saves changes to this task.")
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
