use taskmanager_core::{Task, TaskFilter, TaskSort, TaskStatus};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskFilterState {
    #[default]
    Today,
    Upcoming,
    NoDueDate,
    Done,
}

impl TaskFilterState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Upcoming => "Upcoming",
            Self::NoDueDate => "No Due Date",
            Self::Done => "Done",
        }
    }

    pub fn to_filter(self, now_ms: i64) -> TaskFilter {
        let mut filter = TaskFilter::default();
        match self {
            Self::Today => {
                filter.status = Some(TaskStatus::Open);
                filter.due_before = Some(end_of_today_ms(now_ms));
            }
            Self::Upcoming | Self::NoDueDate => {
                filter.status = Some(TaskStatus::Open);
            }
            Self::Done => filter.status = Some(TaskStatus::Done),
        }
        filter
    }
}

pub fn default_sort() -> TaskSort {
    TaskSort::DueAtAsc
}

pub fn status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "Open",
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

pub fn end_of_today_ms(now_ms: i64) -> i64 {
    const DAY_MS: i64 = 24 * 60 * 60 * 1000;
    ((now_ms / DAY_MS) + 1) * DAY_MS - 1
}

pub fn task_matches_view(task: &Task, view: TaskFilterState, now_ms: i64) -> bool {
    match view {
        TaskFilterState::Today => {
            task.status == TaskStatus::Open
                && task
                    .due_at
                    .is_some_and(|due_at| due_at <= end_of_today_ms(now_ms))
        }
        TaskFilterState::Upcoming => task.status == TaskStatus::Open && task.due_at.is_some(),
        TaskFilterState::NoDueDate => task.status == TaskStatus::Open && task.due_at.is_none(),
        TaskFilterState::Done => task.status == TaskStatus::Done,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn filter_state_maps_to_core_filters() {
        assert_eq!(
            TaskFilterState::Today.to_filter(0).status,
            Some(TaskStatus::Open)
        );
        assert_eq!(
            TaskFilterState::Done.to_filter(0).status,
            Some(TaskStatus::Done)
        );
        assert_eq!(
            TaskFilterState::Today.to_filter(100).due_before,
            Some(86_399_999)
        );
    }

    #[test]
    fn summary_includes_status_tags_and_dirty_marker() {
        let task = Task {
            id: Uuid::new_v4(),
            title: "Title".to_owned(),
            body: String::new(),
            due_at: None,
            status: TaskStatus::Open,
            project_id: None,
            tags: vec!["home".to_owned(), "quick".to_owned()],
            created_at: 0,
            updated_at: 0,
            deleted: false,
            dirty: true,
        };
        assert_eq!(format_task_summary(&task), "Open · #home #quick · unsynced");
    }
}
