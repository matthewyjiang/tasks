use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;
use taskmanager_core::{Task, TaskStatus};
use uuid::Uuid;

use crate::task_format::format_task_row_summary;
use crate::ui::widgets::font_awesome_label;

pub(crate) struct TaskRowActions {
    pub(crate) toggle_status: Rc<dyn Fn(Uuid, TaskStatus)>,
    pub(crate) move_task: Rc<dyn Fn(Uuid)>,
    pub(crate) delete_task: Rc<dyn Fn(Uuid)>,
}

pub(crate) fn task_row(task: &Task, actions: &TaskRowActions) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("task-row");
    row.set_selectable(true);
    row.set_activatable(true);
    row.set_widget_name(&task.id.to_string());

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    container.set_margin_top(7);
    container.set_margin_bottom(7);
    container.set_margin_start(14);
    container.set_margin_end(14);

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

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    let title = gtk::Label::new(Some(&task.title));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.set_valign(gtk::Align::Center);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("task-title");
    let summary_text = format_task_row_summary(task);
    let summary = gtk::Label::new(Some(&summary_text));
    summary.set_xalign(0.0);
    summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
    summary.add_css_class("task-summary");
    summary.set_visible(!summary_text.is_empty());
    text.append(&title);
    text.append(&summary);

    let sync_status = font_awesome_label("\u{f071}");
    sync_status.add_css_class("sync-status");
    sync_status.set_tooltip_text(Some("Out of date"));
    sync_status.set_visible(task.dirty);

    let actions_button = gtk::MenuButton::new();
    actions_button.set_label("⋯");
    actions_button.add_css_class("flat");
    actions_button.add_css_class("task-actions");
    let popover = gtk::Popover::new();
    let action_box = gtk::Box::new(gtk::Orientation::Vertical, 4);

    let move_task = gtk::Button::with_label("Move");
    move_task.add_css_class("flat");
    move_task.connect_clicked({
        let move_task = Rc::clone(&actions.move_task);
        let popover = popover.clone();
        let task_id = task.id;
        move |_| {
            popover.popdown();
            move_task(task_id);
        }
    });

    let delete = gtk::Button::with_label("Delete");
    delete.add_css_class("flat");
    delete.add_css_class("destructive-action");
    delete.connect_clicked({
        let delete_task = Rc::clone(&actions.delete_task);
        let task_id = task.id;
        move |_| delete_task(task_id)
    });
    action_box.append(&move_task);
    action_box.append(&delete);
    popover.set_child(Some(&action_box));
    actions_button.set_popover(Some(&popover));

    container.append(&status_dot);
    container.append(&text);
    container.append(&sync_status);
    container.append(&actions_button);
    row.set_child(Some(&container));
    row
}
