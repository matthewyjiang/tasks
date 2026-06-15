use taskmanager_core::{Task, TaskFilter, TaskSort, TaskStatus};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskFilterState {
    #[default]
    Inbox,
    Today,
    Upcoming,
    NoDueDate,
    Done,
}

impl TaskFilterState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Inbox => "Inbox",
            Self::Today => "Today",
            Self::Upcoming => "Upcoming",
            Self::NoDueDate => "Anytime",
            Self::Done => "Done",
        }
    }

    pub fn to_filter(self, now_ms: i64) -> TaskFilter {
        let mut filter = TaskFilter::default();
        match self {
            Self::Inbox => {
                filter.status = Some(TaskStatus::Open);
            }
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

pub fn end_of_today_ms(now_ms: i64) -> i64 {
    const DAY_MS: i64 = 24 * 60 * 60 * 1000;
    ((now_ms / DAY_MS) + 1) * DAY_MS - 1
}

pub fn task_matches_view(
    task: &Task,
    view: TaskFilterState,
    now_ms: i64,
    show_completed: bool,
) -> bool {
    let visible_status = show_completed || task.status == TaskStatus::Open;
    match view {
        TaskFilterState::Inbox => visible_status && task.project_id.is_none(),
        TaskFilterState::Today => {
            visible_status
                && task
                    .due_at
                    .is_some_and(|due_at| due_at <= end_of_today_ms(now_ms))
        }
        TaskFilterState::Upcoming => visible_status && task.due_at.is_some(),
        TaskFilterState::NoDueDate => visible_status && task.due_at.is_none(),
        TaskFilterState::Done => task.status == TaskStatus::Done,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn show_completed_includes_done_tasks_in_default_views() {
        let mut task = Task {
            id: uuid::Uuid::nil(),
            title: "Done task".to_owned(),
            body: String::new(),
            due_at: None,
            reminder_offset_ms: None,
            status: TaskStatus::Done,
            project_id: None,
            tags: Vec::new(),
            created_at: 0,
            updated_at: 0,
            deleted: false,
            dirty: false,
        };

        assert!(!task_matches_view(&task, TaskFilterState::Inbox, 0, false));
        assert!(task_matches_view(&task, TaskFilterState::Inbox, 0, true));

        task.due_at = Some(100);
        assert!(!task_matches_view(
            &task,
            TaskFilterState::Today,
            100,
            false
        ));
        assert!(task_matches_view(&task, TaskFilterState::Today, 100, true));
    }
}
