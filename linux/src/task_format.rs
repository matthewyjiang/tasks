use gtk4 as gtk;
use taskmanager_core::{Task, TaskStatus};

use crate::task_model::{task_matches_view, TaskFilterState};

pub(crate) fn parse_tags(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn parse_due_date_entry(text: &str) -> Result<Option<i64>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if let Ok(value) = text.parse::<i64>() {
        return Ok(Some(value));
    }
    let parts = text
        .split('-')
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Use due date format YYYY-MM-DD".to_owned())?;
    if parts.len() != 3 {
        return Err("Use due date format YYYY-MM-DD".to_owned());
    }
    gtk::glib::DateTime::from_local(parts[0], parts[1], parts[2], 12, 0, 0.0)
        .map(|date_time| Some(date_time.to_unix() * 1000))
        .map_err(|error| format!("Invalid due date: {error}"))
}

pub(crate) fn markdown_to_pango_markup(markdown: &str) -> String {
    if markdown.trim().is_empty() {
        return "<span foreground=\"#888888\">No notes</span>".to_owned();
    }
    markdown
        .lines()
        .map(|line| {
            let escaped = gtk::glib::markup_escape_text(line).to_string();
            if let Some(text) = escaped.strip_prefix("# ") {
                format!("<span size=\"x-large\" weight=\"bold\">{text}</span>")
            } else if let Some(text) = escaped.strip_prefix("## ") {
                format!("<span size=\"large\" weight=\"bold\">{text}</span>")
            } else if let Some(text) = escaped.strip_prefix("### ") {
                format!("<b>{text}</b>")
            } else if let Some(text) = escaped.strip_prefix("- ") {
                format!("• {text}")
            } else if let Some(text) = escaped.strip_prefix("* ") {
                format!("• {text}")
            } else {
                escaped
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn format_task_row_summary(task: &Task) -> String {
    let now = gtk::glib::DateTime::now_local().ok();
    format_task_row_summary_at(task, now.as_ref())
}

fn format_task_row_summary_at(task: &Task, now: Option<&gtk::glib::DateTime>) -> String {
    let mut parts = Vec::new();
    if let Some(due_at) = task.due_at {
        let due_label = if task_is_overdue_at(task, now) {
            "Overdue"
        } else {
            "Due"
        };
        parts.push(format!("{due_label} {}", format_due_date_at(due_at, now)));
    }
    if !task.tags.is_empty() {
        parts.push(format!("#{}", task.tags.join(" #")));
    }
    parts.join(" · ")
}

pub(crate) fn task_is_overdue(task: &Task) -> bool {
    let Ok(now) = gtk::glib::DateTime::now_local() else {
        return false;
    };
    task_is_overdue_at(task, Some(&now))
}

fn task_is_overdue_at(task: &Task, now: Option<&gtk::glib::DateTime>) -> bool {
    if task.status != TaskStatus::Open || task.deleted {
        return false;
    }
    let (Some(due_at), Some(now)) = (task.due_at, now) else {
        return false;
    };
    due_at_is_overdue_at(due_at, now)
}

fn due_at_is_overdue_at(due_at_ms: i64, now: &gtk::glib::DateTime) -> bool {
    let Ok(due_at) = gtk::glib::DateTime::from_unix_local(due_at_ms / 1000) else {
        return false;
    };
    local_day_key(&due_at) < local_day_key(now)
}

fn local_day_key(date_time: &gtk::glib::DateTime) -> (i32, i32, i32) {
    (
        date_time.year(),
        date_time.month(),
        date_time.day_of_month(),
    )
}

pub(crate) fn format_deleted_summary(deleted_at_ms: i64) -> String {
    let Ok(date_time) = gtk::glib::DateTime::from_unix_local(deleted_at_ms / 1000) else {
        return "Deleted".to_owned();
    };
    if let Ok(now) = gtk::glib::DateTime::now_local() {
        if same_local_day(&date_time, &now) {
            return "Deleted today".to_owned();
        }
        if now
            .add_days(-1)
            .map(|yesterday| same_local_day(&date_time, &yesterday))
            .unwrap_or(false)
        {
            return "Deleted yesterday".to_owned();
        }
    }
    date_time
        .format("Deleted %b %d, %Y")
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "Deleted".to_owned())
}

fn format_due_date_at(due_at_ms: i64, now: Option<&gtk::glib::DateTime>) -> String {
    let Ok(date_time) = gtk::glib::DateTime::from_unix_local(due_at_ms / 1000) else {
        return "unknown".to_owned();
    };
    let Some(now) = now else {
        return date_time
            .format("%b %d, %Y")
            .map(|value| value.to_string())
            .unwrap_or_else(|_| "unknown".to_owned());
    };

    if same_local_day(&date_time, now) {
        "today".to_owned()
    } else if now
        .add_days(1)
        .map(|tomorrow| same_local_day(&date_time, &tomorrow))
        .unwrap_or(false)
    {
        "tomorrow".to_owned()
    } else {
        date_time
            .format("%b %d, %Y")
            .map(|value| value.to_string())
            .unwrap_or_else(|_| "unknown".to_owned())
    }
}

pub(crate) fn same_local_day(left: &gtk::glib::DateTime, right: &gtk::glib::DateTime) -> bool {
    left.year() == right.year()
        && left.month() == right.month()
        && left.day_of_month() == right.day_of_month()
}

pub(crate) fn sidebar_filter_order() -> [TaskFilterState; 5] {
    [
        TaskFilterState::Inbox,
        TaskFilterState::Today,
        TaskFilterState::Upcoming,
        TaskFilterState::NoDueDate,
        TaskFilterState::Done,
    ]
}

pub(crate) fn sidebar_filter_title(filter: TaskFilterState) -> &'static str {
    match filter {
        TaskFilterState::Inbox => "Inbox",
        TaskFilterState::Today => "Today",
        TaskFilterState::Upcoming => "Upcoming",
        TaskFilterState::NoDueDate => "Anytime",
        TaskFilterState::Done => "Done",
        TaskFilterState::RecentlyDeleted => "Recently Deleted",
    }
}

pub(crate) fn sidebar_filter_icon(filter: TaskFilterState) -> &'static str {
    match filter {
        TaskFilterState::Inbox => "\u{f01c}",
        TaskFilterState::Today => "\u{f783}",
        TaskFilterState::Upcoming => "\u{f073}",
        TaskFilterState::NoDueDate => "\u{f5fd}",
        TaskFilterState::Done => "\u{f058}",
        TaskFilterState::RecentlyDeleted => "\u{f1f8}",
    }
}

pub(crate) fn sidebar_filter_icon_class(filter: TaskFilterState) -> &'static str {
    match filter {
        TaskFilterState::Inbox => "sidebar-icon-inbox",
        TaskFilterState::Today => "sidebar-icon-today",
        TaskFilterState::Upcoming => "sidebar-icon-upcoming",
        TaskFilterState::NoDueDate => "sidebar-icon-anytime",
        TaskFilterState::Done => "sidebar-icon-done",
        TaskFilterState::RecentlyDeleted => "sidebar-icon-deleted",
    }
}

pub(crate) fn count_for_filter(tasks: &[Task], filter: TaskFilterState, now_ms: i64) -> usize {
    tasks
        .iter()
        .filter(|task| task_matches_view(task, filter, now_ms, false))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn local_ms(year: i32, month: i32, day: i32) -> i64 {
        gtk::glib::DateTime::from_local(year, month, day, 12, 0, 0.0)
            .expect("valid local test date")
            .to_unix()
            * 1000
    }

    fn open_task(due_at: Option<i64>) -> Task {
        Task {
            id: Uuid::nil(),
            title: "Task".to_owned(),
            body: String::new(),
            due_at,
            reminder_offset_ms: None,
            status: TaskStatus::Open,
            project_id: None,
            tags: vec!["urgent".to_owned()],
            created_at: 0,
            updated_at: 0,
            deleted: false,
            dirty: false,
        }
    }

    #[test]
    fn due_at_is_overdue_only_before_current_local_day() {
        let now =
            gtk::glib::DateTime::from_local(2026, 6, 17, 9, 0, 0.0).expect("valid local test date");

        assert!(due_at_is_overdue_at(local_ms(2026, 6, 16), &now));
        assert!(!due_at_is_overdue_at(local_ms(2026, 6, 17), &now));
        assert!(!due_at_is_overdue_at(local_ms(2026, 6, 18), &now));
    }

    fn assert_summary_prefix_and_tags(summary: &str, prefix: &str) {
        assert!(
            summary.starts_with(prefix),
            "expected summary {summary:?} to start with {prefix:?}"
        );
        assert!(
            summary.ends_with(" · #urgent"),
            "expected summary {summary:?} to preserve tags"
        );
    }

    #[test]
    fn row_summary_marks_only_open_live_past_due_tasks_overdue() {
        let mut task = open_task(Some(local_ms(2026, 6, 16)));
        let now =
            gtk::glib::DateTime::from_local(2026, 6, 17, 9, 0, 0.0).expect("valid local test date");

        let summary = format_task_row_summary_at(&task, Some(&now));
        assert_summary_prefix_and_tags(&summary, "Overdue ");

        task.due_at = Some(local_ms(2026, 6, 17));
        assert_eq!(
            format_task_row_summary_at(&task, Some(&now)),
            "Due today · #urgent"
        );

        task.due_at = Some(local_ms(2026, 6, 18));
        assert_eq!(
            format_task_row_summary_at(&task, Some(&now)),
            "Due tomorrow · #urgent"
        );

        task.due_at = None;
        assert_eq!(format_task_row_summary_at(&task, Some(&now)), "#urgent");

        task.due_at = Some(local_ms(2026, 6, 16));
        task.status = TaskStatus::Done;
        let summary = format_task_row_summary_at(&task, Some(&now));
        assert_summary_prefix_and_tags(&summary, "Due ");
        assert!(!summary.starts_with("Overdue "));

        task.status = TaskStatus::Open;
        task.deleted = true;
        let summary = format_task_row_summary_at(&task, Some(&now));
        assert_summary_prefix_and_tags(&summary, "Due ");
        assert!(!summary.starts_with("Overdue "));
    }
}
