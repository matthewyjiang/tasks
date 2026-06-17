use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;
use taskmanager_core::{Task, TaskStatus};
use uuid::Uuid;

use crate::task_format::{format_deleted_summary, format_task_row_summary, task_is_overdue};
use crate::ui::layout::TASK_ACTION_POPOVER_WIDTH;
use crate::ui::widgets::font_awesome_label;

pub(crate) struct TaskRowActions {
    pub(crate) save_task: Rc<dyn Fn(Uuid, String, String)>,
    pub(crate) update_due_date: Rc<dyn Fn(Uuid, Option<i64>)>,
    pub(crate) update_reminder_offset: Rc<dyn Fn(Uuid, Option<i64>)>,
    pub(crate) toggle_status: Rc<dyn Fn(Uuid, TaskStatus)>,
    pub(crate) move_task: Rc<dyn Fn(Uuid, gtk::Button)>,
    pub(crate) shared_state_summary: Rc<dyn Fn(Uuid) -> Option<String>>,
    pub(crate) manage_sharing: Rc<dyn Fn(Uuid, gtk::Button)>,
    pub(crate) delete_task: Rc<dyn Fn(Uuid, gtk::Button)>,
    pub(crate) restore_task: Rc<dyn Fn(Uuid)>,
    pub(crate) finish_expand: Rc<dyn Fn(Uuid)>,
    pub(crate) finish_collapse: Rc<dyn Fn(Uuid)>,
    pub(crate) finish_delete_editor: Rc<dyn Fn(Uuid)>,
    pub(crate) finish_delete: Rc<dyn Fn(Uuid)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskRowExpansion {
    Collapsed,
    Expanding,
    Expanded,
    Collapsing,
    DeletingEditor,
    DeletingRow,
}

impl TaskRowExpansion {
    fn has_editor(self) -> bool {
        matches!(
            self,
            Self::Expanding | Self::Expanded | Self::Collapsing | Self::DeletingEditor
        )
    }

    fn is_editing(self) -> bool {
        matches!(self, Self::Expanding | Self::Expanded)
    }
}

pub(crate) fn task_row(
    task: &Task,
    expansion: TaskRowExpansion,
    actions: &TaskRowActions,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("task-row");
    row.set_widget_name(&task.id.to_string());

    if task.deleted {
        row.add_css_class("task-row-deleted");
        row.set_selectable(false);
        row.set_activatable(false);
        row.set_child(Some(&build_deleted_row(task, actions)));
        return row;
    }

    if task_is_overdue(task) {
        row.add_css_class("task-row-overdue");
    }
    if expansion.is_editing() {
        row.add_css_class("task-row-expanded");
    }
    row.set_selectable(true);
    row.set_activatable(true);

    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    container.set_margin_top(7);
    container.set_margin_bottom(7);
    container.set_margin_start(14);
    container.set_margin_end(14);

    let title_entry = gtk::Entry::new();
    let notes = gtk::TextView::new();

    container.append(&build_row_header(
        task,
        expansion.is_editing(),
        &title_entry,
        actions,
    ));
    if expansion.has_editor() {
        container.append(&build_inline_editor(
            task,
            expansion,
            &title_entry,
            &notes,
            actions,
        ));
    }

    if expansion == TaskRowExpansion::DeletingRow {
        let revealer = gtk::Revealer::new();
        revealer.set_transition_type(gtk::RevealerTransitionType::SlideUp);
        revealer.set_transition_duration(160);
        revealer.set_child(Some(&container));
        revealer.set_reveal_child(true);
        revealer.connect_child_revealed_notify({
            let finish_delete = Rc::clone(&actions.finish_delete);
            let task_id = task.id;
            move |revealer| {
                if !revealer.is_child_revealed() {
                    finish_delete(task_id);
                }
            }
        });
        let revealer_clone = revealer.clone();
        gtk::glib::idle_add_local_once(move || revealer_clone.set_reveal_child(false));
        row.set_child(Some(&revealer));
    } else {
        row.set_child(Some(&container));
    }
    row
}

fn build_row_header(
    task: &Task,
    editing: bool,
    title_entry: &gtk::Entry,
    actions: &TaskRowActions,
) -> gtk::Box {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header.append(&status_button(task, actions));
    header.append(&title_stack(task, editing, title_entry));

    if let Some(summary) = (actions.shared_state_summary)(task.id) {
        let shared_status = font_awesome_label("\u{f1e0}");
        shared_status.add_css_class("sync-status");
        shared_status.set_tooltip_text(Some(&summary));
        header.append(&shared_status);
    }

    let sync_status = font_awesome_label("\u{f071}");
    sync_status.add_css_class("sync-status");
    sync_status.set_tooltip_text(Some("Out of date"));
    sync_status.set_visible(task.dirty);
    header.append(&sync_status);
    header
}

fn status_button(task: &Task, actions: &TaskRowActions) -> gtk::Button {
    let next_status = if task.status == TaskStatus::Done {
        TaskStatus::Open
    } else {
        TaskStatus::Done
    };
    let status_dot = gtk::Button::with_label(match task.status {
        TaskStatus::Done => "✓",
        TaskStatus::Open => "○",
    });
    status_dot.add_css_class("flat");
    status_dot.add_css_class("status-dot");
    status_dot.set_valign(gtk::Align::Center);
    status_dot.set_tooltip_text(Some(if task.status == TaskStatus::Done {
        "Mark open"
    } else {
        "Mark done"
    }));
    status_dot.connect_clicked({
        let toggle_status = Rc::clone(&actions.toggle_status);
        let task_id = task.id;
        move |_| toggle_status(task_id, next_status)
    });
    status_dot
}

fn title_stack(task: &Task, editing: bool, title_entry: &gtk::Entry) -> gtk::Box {
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);

    let title = gtk::Label::new(Some(&task.title));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.set_valign(gtk::Align::Center);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("task-title");
    title.set_visible(!editing);

    title_entry.set_text(&task.title);
    title_entry.add_css_class("task-inline-title");
    title_entry.add_css_class("flat");
    title_entry.set_visible(editing);

    let summary_text = format_task_row_summary(task);
    let summary = gtk::Label::new(Some(&summary_text));
    summary.set_xalign(0.0);
    summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
    summary.add_css_class("task-summary");
    if task_is_overdue(task) {
        summary.add_css_class("task-summary-overdue");
    }
    summary.set_visible(!summary_text.is_empty());

    text.append(&title);
    text.append(title_entry);
    text.append(&summary);
    text
}

fn build_inline_editor(
    task: &Task,
    expansion: TaskRowExpansion,
    title_entry: &gtk::Entry,
    notes: &gtk::TextView,
    actions: &TaskRowActions,
) -> gtk::Revealer {
    let expanded = gtk::Box::new(gtk::Orientation::Vertical, 8);
    expanded.add_css_class("task-row-editor");
    expanded.set_margin_start(40);
    expanded.set_margin_top(8);

    expanded.append(&notes_editor(task, notes));

    let inline_actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    inline_actions.set_halign(gtk::Align::End);
    inline_actions.add_css_class("task-inline-actions");
    inline_actions.append(&due_date_button(task, actions));
    inline_actions.append(&list_button(task, actions));
    inline_actions.append(&share_button(task, actions));
    inline_actions.append(&delete_button(task, actions));
    expanded.append(&inline_actions);

    connect_inline_autosave(task, title_entry, notes, actions);

    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    revealer.set_transition_duration(240);
    revealer.set_child(Some(&expanded));

    match expansion {
        TaskRowExpansion::Expanding => {
            revealer.set_reveal_child(false);
            revealer.connect_child_revealed_notify({
                let finish_expand = Rc::clone(&actions.finish_expand);
                let task_id = task.id;
                move |revealer| {
                    if revealer.is_child_revealed() {
                        finish_expand(task_id);
                    }
                }
            });
            let revealer = revealer.clone();
            gtk::glib::idle_add_local_once(move || revealer.set_reveal_child(true));
        }
        TaskRowExpansion::Expanded => {
            revealer.set_reveal_child(true);
        }
        TaskRowExpansion::Collapsing | TaskRowExpansion::DeletingEditor => {
            revealer.set_reveal_child(true);
            revealer.connect_child_revealed_notify({
                let finish = if expansion == TaskRowExpansion::DeletingEditor {
                    Rc::clone(&actions.finish_delete_editor)
                } else {
                    Rc::clone(&actions.finish_collapse)
                };
                let task_id = task.id;
                move |revealer| {
                    if !revealer.is_child_revealed() {
                        finish(task_id);
                    }
                }
            });
            let revealer = revealer.clone();
            gtk::glib::idle_add_local_once(move || revealer.set_reveal_child(false));
        }
        TaskRowExpansion::Collapsed | TaskRowExpansion::DeletingRow => {}
    }

    revealer
}

fn notes_editor(task: &Task, notes: &gtk::TextView) -> gtk::Stack {
    notes.buffer().set_text(&task.body);
    notes.set_wrap_mode(gtk::WrapMode::Word);
    notes.set_top_margin(0);
    notes.set_bottom_margin(0);
    notes.set_size_request(-1, 42);
    notes.add_css_class("task-inline-notes");
    notes.set_tooltip_text(Some("Plain text notes"));

    let placeholder = gtk::Label::new(Some("Notes"));
    placeholder.add_css_class("task-inline-notes-placeholder");
    placeholder.set_xalign(0.0);
    placeholder.set_yalign(0.0);
    placeholder.set_halign(gtk::Align::Start);
    placeholder.set_valign(gtk::Align::Start);
    placeholder.set_margin_top(0);
    placeholder.set_size_request(-1, 42);

    let stack = gtk::Stack::new();
    stack.add_named(&placeholder, Some("placeholder"));
    stack.add_named(notes, Some("notes"));
    stack.set_visible_child_name(if task.body.is_empty() {
        "placeholder"
    } else {
        "notes"
    });

    let placeholder_click = gtk::GestureClick::new();
    placeholder_click.connect_pressed({
        let stack = stack.clone();
        let notes = notes.clone();
        move |_, _, _, _| {
            stack.set_visible_child_name("notes");
            notes.grab_focus();
        }
    });
    placeholder.add_controller(placeholder_click);

    let focus = gtk::EventControllerFocus::new();
    focus.connect_leave({
        let stack = stack.clone();
        let notes = notes.clone();
        move |_| {
            let buffer = notes.buffer();
            if buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), false)
                .is_empty()
            {
                stack.set_visible_child_name("placeholder");
            }
        }
    });
    notes.add_controller(focus);

    stack
}

fn due_date_button(task: &Task, actions: &TaskRowActions) -> gtk::MenuButton {
    let calendar = gtk::Calendar::new();
    calendar.add_css_class("task-calendar");
    if let Some(due_at) = task.due_at {
        match gtk::glib::DateTime::from_unix_local(due_at / 1000) {
            Ok(due_date) => calendar.select_day(&due_date),
            Err(error) => eprintln!("Failed to select due date: {error}"),
        }
    } else {
        calendar.add_css_class("task-calendar-no-selection");
    }

    let today = gtk::Button::new();
    let today_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    today_content.set_halign(gtk::Align::Center);
    let today_icon = font_awesome_label("\u{f185}");
    today_icon.add_css_class("task-date-quick-icon");
    today_content.append(&today_icon);
    today_content.append(&gtk::Label::new(Some("Today")));
    today.set_child(Some(&today_content));
    today.add_css_class("task-date-quick-button");
    today.set_tooltip_text(Some("Set due date to today"));

    let clear = gtk::Button::new();
    let clear_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    clear_content.set_halign(gtk::Align::Center);
    let clear_icon = font_awesome_label("\u{f00d}");
    clear_icon.add_css_class("task-date-quick-icon");
    clear_content.append(&clear_icon);
    clear_content.append(&gtk::Label::new(Some("Clear")));
    clear.set_child(Some(&clear_content));
    clear.add_css_class("task-date-quick-button");
    clear.set_tooltip_text(Some("Clear due date"));

    let popover_content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    popover_content.add_css_class("task-action-popover");
    popover_content.set_size_request(TASK_ACTION_POPOVER_WIDTH, -1);
    popover_content.append(&today);
    popover_content.append(&calendar);

    let reminder_label = gtk::Label::new(Some("Reminder"));
    reminder_label.set_xalign(0.0);
    reminder_label.add_css_class("dim-label");
    let reminder_combo = gtk::ComboBoxText::new();
    for (id, label) in reminder_presets() {
        reminder_combo.append(Some(id), label);
    }
    let active_reminder_id = reminder_offset_id(task.reminder_offset_ms);
    if let Some(offset) = task.reminder_offset_ms {
        let custom_id = reminder_custom_id(offset);
        if active_reminder_id.as_deref() == Some(custom_id.as_str()) {
            reminder_combo.append(Some(&custom_id), &format_reminder_offset(offset));
        }
    }
    reminder_combo.set_active_id(active_reminder_id.as_deref());
    popover_content.append(&reminder_label);
    popover_content.append(&reminder_combo);
    popover_content.append(&clear);

    let popover = gtk::Popover::new();
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    popover.set_offset(0, 8);
    popover.set_child(Some(&popover_content));

    let button = gtk::MenuButton::new();
    let icon = gtk::Label::new(Some(""));
    button.set_child(Some(&icon));
    button.set_always_show_arrow(false);
    button.add_css_class("flat");
    button.set_tooltip_text(Some("Due date"));
    button.set_popover(Some(&popover));

    today.connect_clicked({
        let update_due_date = Rc::clone(&actions.update_due_date);
        let calendar = calendar.clone();
        let popover = popover.clone();
        let task_id = task.id;
        move |_| match gtk::glib::DateTime::now_local() {
            Ok(today) => {
                calendar.select_day(&today);
                update_due_date(task_id, Some(today.to_unix() * 1000));
                popover.popdown();
            }
            Err(error) => eprintln!("Failed to read local date: {error}"),
        }
    });

    clear.connect_clicked({
        let update_due_date = Rc::clone(&actions.update_due_date);
        let popover = popover.clone();
        let task_id = task.id;
        move |_| {
            update_due_date(task_id, None);
            popover.popdown();
        }
    });

    reminder_combo.connect_changed({
        let update_reminder_offset = Rc::clone(&actions.update_reminder_offset);
        let task_id = task.id;
        move |combo| {
            if let Some(offset) = combo
                .active_id()
                .and_then(|id| reminder_offset_from_id(&id))
            {
                update_reminder_offset(task_id, offset);
            }
        }
    });

    calendar.connect_day_selected({
        let update_due_date = Rc::clone(&actions.update_due_date);
        let popover = popover.clone();
        let task_id = task.id;
        move |calendar| {
            let date = calendar.date();
            match gtk::glib::DateTime::from_local(
                date.year(),
                date.month(),
                date.day_of_month(),
                12,
                0,
                0.0,
            ) {
                Ok(date_time) => {
                    update_due_date(task_id, Some(date_time.to_unix() * 1000));
                    popover.popdown();
                }
                Err(error) => eprintln!("Failed to set due date: {error}"),
            }
        }
    });

    button
}

fn reminder_presets() -> &'static [(&'static str, &'static str)] {
    &[
        ("none", "None"),
        ("0", "At due time"),
        ("300000", "5 minutes before"),
        ("900000", "15 minutes before"),
        ("3600000", "1 hour before"),
        ("86400000", "1 day before"),
    ]
}

fn reminder_offset_id(offset: Option<i64>) -> Option<String> {
    Some(match offset {
        None => "none".to_owned(),
        Some(0) => "0".to_owned(),
        Some(300_000) => "300000".to_owned(),
        Some(900_000) => "900000".to_owned(),
        Some(3_600_000) => "3600000".to_owned(),
        Some(86_400_000) => "86400000".to_owned(),
        Some(offset) if offset >= 0 => reminder_custom_id(offset),
        Some(_) => return None,
    })
}

fn reminder_custom_id(offset: i64) -> String {
    format!("custom:{offset}")
}

fn format_reminder_offset(offset: i64) -> String {
    if offset == 0 {
        return "At due time".to_owned();
    }
    let minutes = offset / 60_000;
    if minutes > 0 && offset % 60_000 == 0 {
        if minutes % (24 * 60) == 0 {
            let days = minutes / (24 * 60);
            return format!("{days} day{} before", if days == 1 { "" } else { "s" });
        }
        if minutes % 60 == 0 {
            let hours = minutes / 60;
            return format!("{hours} hour{} before", if hours == 1 { "" } else { "s" });
        }
        return format!(
            "{minutes} minute{} before",
            if minutes == 1 { "" } else { "s" }
        );
    }
    format!("{} ms before", offset)
}

fn reminder_offset_from_id(id: &str) -> Option<Option<i64>> {
    match id {
        "none" => Some(None),
        "0" => Some(Some(0)),
        "300000" => Some(Some(300_000)),
        "900000" => Some(Some(900_000)),
        "3600000" => Some(Some(3_600_000)),
        "86400000" => Some(Some(86_400_000)),
        custom if custom.starts_with("custom:") => custom["custom:".len()..]
            .parse::<i64>()
            .ok()
            .filter(|offset| *offset >= 0)
            .map(Some),
        _ => None,
    }
}

fn list_button(task: &Task, actions: &TaskRowActions) -> gtk::Button {
    let list = gtk::Button::with_label("");
    list.add_css_class("flat");
    list.set_tooltip_text(Some("Move to list"));
    list.connect_clicked({
        let move_task = Rc::clone(&actions.move_task);
        let task_id = task.id;
        move |button| move_task(task_id, button.clone())
    });
    list
}

fn share_button(task: &Task, actions: &TaskRowActions) -> gtk::Button {
    let share = gtk::Button::with_label("");
    share.add_css_class("flat");
    share.set_tooltip_text(Some("Manage sharing"));
    share.connect_clicked({
        let manage_sharing = Rc::clone(&actions.manage_sharing);
        let task_id = task.id;
        move |button| manage_sharing(task_id, button.clone())
    });
    share
}

fn delete_button(task: &Task, actions: &TaskRowActions) -> gtk::Button {
    let delete = gtk::Button::with_label("");
    delete.add_css_class("flat");
    delete.add_css_class("destructive-action");
    delete.set_tooltip_text(Some("Delete task"));
    delete.connect_clicked({
        let delete_task = Rc::clone(&actions.delete_task);
        let task_id = task.id;
        move |button| delete_task(task_id, button.clone())
    });
    delete
}

fn build_deleted_row(task: &Task, actions: &TaskRowActions) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    container.set_margin_top(7);
    container.set_margin_bottom(7);
    container.set_margin_start(14);
    container.set_margin_end(14);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);

    let title_text = if task.title.trim().is_empty() {
        "(untitled task)"
    } else {
        task.title.as_str()
    };
    let title = gtk::Label::new(Some(title_text));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("task-title");

    let summary = gtk::Label::new(Some(&format_deleted_summary(task.updated_at)));
    summary.set_xalign(0.0);
    summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
    summary.add_css_class("task-summary");

    text.append(&title);
    text.append(&summary);
    container.append(&text);
    container.append(&restore_button(task, actions));
    container
}

fn restore_button(task: &Task, actions: &TaskRowActions) -> gtk::Button {
    let restore = gtk::Button::new();
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.set_halign(gtk::Align::Center);
    let icon = font_awesome_label("\u{f0e2}");
    content.append(&icon);
    content.append(&gtk::Label::new(Some("Restore")));
    restore.set_child(Some(&content));
    restore.add_css_class("flat");
    restore.add_css_class("suggested-action");
    restore.set_valign(gtk::Align::Center);
    restore.set_tooltip_text(Some("Restore task"));
    restore.connect_clicked({
        let restore_task = Rc::clone(&actions.restore_task);
        let task_id = task.id;
        move |_| restore_task(task_id)
    });
    restore
}

fn connect_inline_autosave(
    task: &Task,
    title_entry: &gtk::Entry,
    notes: &gtk::TextView,
    actions: &TaskRowActions,
) {
    let save_inline: Rc<dyn Fn()> = Rc::new({
        let save_task = Rc::clone(&actions.save_task);
        let title_entry = title_entry.clone();
        let buffer = notes.buffer();
        let task_id = task.id;
        move || {
            let body = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), false)
                .to_string();
            save_task(task_id, title_entry.text().trim().to_owned(), body);
        }
    });

    title_entry.connect_activate({
        let save_inline = Rc::clone(&save_inline);
        move |_| save_inline()
    });

    let title_focus = gtk::EventControllerFocus::new();
    title_focus.connect_leave({
        let save_inline = Rc::clone(&save_inline);
        move |_| save_inline()
    });
    title_entry.add_controller(title_focus);

    let notes_focus = gtk::EventControllerFocus::new();
    notes_focus.connect_leave({
        let save_inline = Rc::clone(&save_inline);
        move |_| save_inline()
    });
    notes.add_controller(notes_focus);

    let notes_keys = gtk::EventControllerKey::new();
    notes_keys.connect_key_pressed({
        let save_inline = Rc::clone(&save_inline);
        let notes = notes.clone();
        move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                save_inline();
                if let Some(row) = notes
                    .ancestor(gtk::ListBoxRow::static_type())
                    .and_then(|widget| widget.downcast::<gtk::ListBoxRow>().ok())
                {
                    row.grab_focus();
                }
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        }
    });
    notes.add_controller(notes_keys);
}
