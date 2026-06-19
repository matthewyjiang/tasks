use taskmanager_core::{DefaultSort, Task, TaskFilter, TaskSort, TaskStatus};

/// How long a soft-deleted task remains visible in the Recently Deleted view.
///
/// This is a display-only window: the local SQLite tombstone row is retained
/// regardless, and restoring re-pushes the task so it syncs to other devices.
/// 30 days mirrors the server's default tombstone retention horizon.
pub const RECENTLY_DELETED_WINDOW_MS: i64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskFilterState {
    #[default]
    Inbox,
    Today,
    Upcoming,
    NoDueDate,
    Done,
    RecentlyDeleted,
}

impl TaskFilterState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Inbox => "Inbox",
            Self::Today => "Today",
            Self::Upcoming => "Upcoming",
            Self::NoDueDate => "Anytime",
            Self::Done => "Done",
            Self::RecentlyDeleted => "Recently Deleted",
        }
    }

    /// Whether this view shows soft-deleted tasks instead of live tasks.
    pub fn is_deleted_view(self) -> bool {
        matches!(self, Self::RecentlyDeleted)
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
            Self::RecentlyDeleted => {
                filter.include_deleted = true;
            }
        }
        filter
    }
}

pub fn default_sort(sort: DefaultSort) -> TaskSort {
    match sort {
        DefaultSort::DueAtAsc => TaskSort::DueAtAsc,
        DefaultSort::UpdatedAtDesc => TaskSort::UpdatedAtDesc,
    }
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
    if view.is_deleted_view() {
        return task.deleted && task.updated_at >= now_ms - RECENTLY_DELETED_WINDOW_MS;
    }
    if task.deleted {
        return false;
    }
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
        TaskFilterState::RecentlyDeleted => unreachable!("handled by is_deleted_view guard"),
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

    #[test]
    fn recently_deleted_view_shows_only_recent_tombstones() {
        let now = RECENTLY_DELETED_WINDOW_MS + 1_000_000;
        let mut task = Task {
            id: uuid::Uuid::nil(),
            title: "Deleted task".to_owned(),
            body: String::new(),
            due_at: None,
            reminder_offset_ms: None,
            status: TaskStatus::Open,
            project_id: None,
            tags: Vec::new(),
            created_at: 0,
            updated_at: now,
            deleted: true,
            dirty: true,
        };

        // Recently deleted within the window: shown.
        assert!(task_matches_view(
            &task,
            TaskFilterState::RecentlyDeleted,
            now,
            false
        ));

        // Deleted longer ago than the window: hidden.
        task.updated_at = now - RECENTLY_DELETED_WINDOW_MS - 1;
        assert!(!task_matches_view(
            &task,
            TaskFilterState::RecentlyDeleted,
            now,
            false
        ));

        // Live tasks never appear in the deleted view.
        task.deleted = false;
        task.updated_at = now;
        assert!(!task_matches_view(
            &task,
            TaskFilterState::RecentlyDeleted,
            now,
            false
        ));
    }

    #[test]
    fn deleted_tasks_excluded_from_normal_views() {
        let mut task = Task {
            id: uuid::Uuid::nil(),
            title: "Deleted".to_owned(),
            body: String::new(),
            due_at: None,
            reminder_offset_ms: None,
            status: TaskStatus::Open,
            project_id: None,
            tags: Vec::new(),
            created_at: 0,
            updated_at: 0,
            deleted: true,
            dirty: true,
        };

        assert!(!task_matches_view(&task, TaskFilterState::Inbox, 0, false));
        assert!(!task_matches_view(
            &task,
            TaskFilterState::NoDueDate,
            0,
            true
        ));

        task.deleted = false;
        assert!(task_matches_view(&task, TaskFilterState::Inbox, 0, false));
    }
}
