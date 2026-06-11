use gtk::prelude::*;
use gtk4 as gtk;

use crate::ui::layout::{TASK_EDITOR_BODY_HEIGHT, TASK_EDITOR_MIN_HEIGHT, TASK_EDITOR_MIN_WIDTH};
use crate::ui::widgets::{field_label, font_awesome_label};

pub(crate) struct TaskEditorWidgets {
    pub(crate) panel: gtk::Box,
    pub(crate) body_stack: gtk::Stack,
    pub(crate) due_calendar: gtk::Calendar,
    pub(crate) due_popover: gtk::Popover,
    pub(crate) today_due_button: gtk::Button,
    pub(crate) clear_due_button: gtk::Button,
}

pub(crate) fn build_task_editor_panel(
    title_entry: &gtk::Entry,
    body_view: &gtk::TextView,
    markdown_preview: &gtk::Label,
    status_combo: &gtk::ComboBoxText,
    list_combo: &gtk::ComboBoxText,
    due_entry: &gtk::Entry,
    tags_entry: &gtk::Entry,
) -> TaskEditorWidgets {
    let editor_panel = gtk::Box::new(gtk::Orientation::Vertical, 16);
    editor_panel.add_css_class("task-editor-panel");
    editor_panel.set_size_request(TASK_EDITOR_MIN_WIDTH, TASK_EDITOR_MIN_HEIGHT);
    editor_panel.set_focusable(true);
    editor_panel.set_halign(gtk::Align::Center);
    editor_panel.set_valign(gtk::Align::Center);
    editor_panel.set_opacity(0.0);
    editor_panel.set_visible(false);

    title_entry.add_css_class("task-editor-title");
    body_view.add_css_class("task-editor-body");
    status_combo.add_css_class("task-editor-field");
    list_combo.add_css_class("task-editor-field");
    due_entry.add_css_class("task-editor-field");
    tags_entry.add_css_class("task-editor-field");
    status_combo.set_hexpand(true);
    list_combo.set_hexpand(true);
    due_entry.set_hexpand(true);
    tags_entry.set_hexpand(true);

    let body_stack = gtk::Stack::new();
    body_stack.set_vexpand(true);
    body_stack.add_named(body_view, Some("write"));
    let preview_scroll = gtk::ScrolledWindow::builder()
        .min_content_height(TASK_EDITOR_BODY_HEIGHT)
        .vexpand(true)
        .child(markdown_preview)
        .build();
    body_stack.add_named(&preview_scroll, Some("preview"));
    body_stack.set_visible_child_name("preview");

    let due_calendar = gtk::Calendar::new();
    due_calendar.add_css_class("task-calendar");
    let today_due_button = gtk::Button::new();
    let today_due_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    today_due_content.set_halign(gtk::Align::Center);
    let today_due_icon = font_awesome_label("\u{f185}");
    today_due_icon.add_css_class("task-date-quick-icon");
    today_due_content.append(&today_due_icon);
    today_due_content.append(&gtk::Label::new(Some("Today")));
    today_due_button.set_child(Some(&today_due_content));
    today_due_button.add_css_class("task-date-quick-button");
    today_due_button.set_tooltip_text(Some("Set due date to today"));
    let due_popover_content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    due_popover_content.add_css_class("task-date-popover");
    due_popover_content.append(&today_due_button);
    due_popover_content.append(&due_calendar);
    let due_popover = gtk::Popover::new();
    due_popover.set_child(Some(&due_popover_content));
    let due_icon = font_awesome_label("\u{f073}");
    let due_button = gtk::MenuButton::builder()
        .child(&due_icon)
        .tooltip_text("Calendar")
        .build();
    due_button.add_css_class("task-editor-button");
    due_button.set_popover(Some(&due_popover));
    let clear_due_button = gtk::Button::with_label("Clear");
    clear_due_button.add_css_class("task-editor-button");
    let due_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    due_row.set_hexpand(true);
    due_row.append(due_entry);
    due_row.append(&due_button);
    due_row.append(&clear_due_button);

    let metadata_grid = gtk::Grid::new();
    metadata_grid.add_css_class("task-editor-meta");
    metadata_grid.set_column_spacing(14);
    metadata_grid.set_row_spacing(12);
    metadata_grid.attach(&field_label("Status"), 0, 0, 1, 1);
    metadata_grid.attach(status_combo, 1, 0, 1, 1);
    metadata_grid.attach(&field_label("List"), 0, 1, 1, 1);
    metadata_grid.attach(list_combo, 1, 1, 1, 1);
    metadata_grid.attach(&field_label("Due"), 0, 2, 1, 1);
    metadata_grid.attach(&due_row, 1, 2, 1, 1);
    metadata_grid.attach(&field_label("Tags"), 0, 3, 1, 1);
    metadata_grid.attach(tags_entry, 1, 3, 1, 1);

    editor_panel.append(title_entry);
    editor_panel.append(&body_stack);
    editor_panel.append(&metadata_grid);

    TaskEditorWidgets {
        panel: editor_panel,
        body_stack,
        due_calendar,
        due_popover,
        today_due_button,
        clear_due_button,
    }
}
