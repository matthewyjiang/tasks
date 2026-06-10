use taskmanager_core::{Task, TaskFilter, TaskSort, TaskStatus};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskFilterState {
    #[default]
    All,
    Inbox,
    InProgress,
    Done,
    DueSoon,
}

impl TaskFilterState {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Inbox => "Inbox",
            Self::InProgress => "In Progress",
            Self::Done => "Done",
            Self::DueSoon => "Due soon",
        }
    }

    pub fn to_filter(self, now_ms: i64) -> TaskFilter {
        let mut filter = TaskFilter::default();
        match self {
            Self::All => {}
            Self::Inbox => filter.status = Some(TaskStatus::Inbox),
            Self::InProgress => filter.status = Some(TaskStatus::InProgress),
            Self::Done => filter.status = Some(TaskStatus::Done),
            Self::DueSoon => {
                filter.due_before = Some(now_ms + 7 * 24 * 60 * 60 * 1000);
            }
        }
        filter
    }
}

pub fn default_sort() -> TaskSort {
    TaskSort::UpdatedAtDesc
}

pub fn status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Inbox => "Inbox",
        TaskStatus::InProgress => "In Progress",
        TaskStatus::Done => "Done",
    }
}

pub fn format_task_summary(task: &Task) -> String {
    let tags = if task.tags.is_empty() {
        String::new()
    } else {
        format!(" · #{}", task.tags.join(" #"))
    };
    let dirty = if task.dirty { " · unsynced" } else { "" };
    format!("{}{}{}", status_label(task.status), tags, dirty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn filter_state_maps_to_core_filters() {
        assert_eq!(
            TaskFilterState::Inbox.to_filter(0).status,
            Some(TaskStatus::Inbox)
        );
        assert_eq!(
            TaskFilterState::InProgress.to_filter(0).status,
            Some(TaskStatus::InProgress)
        );
        assert_eq!(
            TaskFilterState::Done.to_filter(0).status,
            Some(TaskStatus::Done)
        );
        assert_eq!(
            TaskFilterState::DueSoon.to_filter(100).due_before,
            Some(604_800_100)
        );
    }

    #[test]
    fn summary_includes_status_tags_and_dirty_marker() {
        let task = Task {
            id: Uuid::new_v4(),
            title: "Title".to_owned(),
            body: String::new(),
            due_at: None,
            status: TaskStatus::InProgress,
            project_id: None,
            tags: vec!["home".to_owned(), "quick".to_owned()],
            created_at: 0,
            updated_at: 0,
            deleted: false,
            dirty: true,
        };
        assert_eq!(
            format_task_summary(&task),
            "In Progress · #home #quick · unsynced"
        );
    }
}
