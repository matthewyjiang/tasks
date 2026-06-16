use gtk4 as gtk;
use taskmanager_core::Task;

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
    let mut parts = Vec::new();
    if let Some(due_at) = task.due_at {
        parts.push(format!("Due {}", format_due_date(due_at)));
    }
    if !task.tags.is_empty() {
        parts.push(format!("#{}", task.tags.join(" #")));
    }
    parts.join(" · ")
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

pub(crate) fn format_due_date(due_at_ms: i64) -> String {
    let Ok(date_time) = gtk::glib::DateTime::from_unix_local(due_at_ms / 1000) else {
        return "unknown".to_owned();
    };
    let Ok(now) = gtk::glib::DateTime::now_local() else {
        return date_time
            .format("%b %d, %Y")
            .map(|value| value.to_string())
            .unwrap_or_else(|_| "unknown".to_owned());
    };

    if same_local_day(&date_time, &now) {
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
