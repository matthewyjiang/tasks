use std::rc::Rc;

use gtk4::prelude::*;

use taskmanager_core::{Task, TaskFilter, TaskList};

use super::AppState;
use crate::task_format::{count_for_filter, sidebar_filter_order};
use crate::task_model::{default_sort, TaskFilterState};
use crate::time::now_ms;
use crate::ui::sidebar::{list_progress, user_list_row};
use crate::ui::widgets::update_entry_width;

impl AppState {
    pub(super) fn refresh_sidebar_metadata(self: &Rc<Self>) {
        let Ok(tasks) = self.core.list_tasks(TaskFilter::default(), default_sort()) else {
            return;
        };
        let now = now_ms();
        for (label, filter) in self
            .filter_count_labels
            .iter()
            .zip(sidebar_filter_order().iter().copied())
        {
            if filter == TaskFilterState::Today {
                label.set_text(&count_for_filter(&tasks, filter, now).to_string());
                label.set_visible(true);
            } else {
                label.set_text("");
                label.set_visible(false);
            }
        }

        self.render_user_lists(&tasks);
    }

    fn render_user_lists(self: &Rc<Self>, tasks: &[Task]) {
        while let Some(row) = self.user_list_box.first_child() {
            self.user_list_box.remove(&row);
        }

        let lists = match self.core.list_task_lists() {
            Ok(lists) => lists,
            Err(error) => {
                self.toast(format!("Failed to load lists: {error}"));
                return;
            }
        };
        self.user_lists.replace(lists.clone());
        self.refresh_editor_list_choices(&lists);

        for list in lists {
            let progress = list_progress(&list, tasks);
            self.user_list_box.append(&user_list_row(&list, progress));
        }
    }

    fn refresh_editor_list_choices(&self, lists: &[TaskList]) {
        let active_id = self.list_combo.active_id().map(|id| id.to_string());
        self.list_combo.remove_all();
        self.list_combo.append(Some("inbox"), "Inbox");
        for list in lists {
            self.list_combo
                .append(Some(&list.id.to_string()), &list.name);
        }
        if let Some(active_id) = active_id {
            self.list_combo.set_active_id(Some(&active_id));
        }
    }

    pub(super) fn show_list_rename_editor(&self) {
        self.list_heading.set_visible(false);
        self.list_name_entry.set_visible(true);
        self.list_rename_button.set_visible(true);
        update_entry_width(&self.list_name_entry);
        self.list_name_entry.grab_focus();
        self.list_name_entry.select_region(0, -1);
    }

    pub(super) fn rename_selected_list(self: &Rc<Self>) -> bool {
        let Some(list_id) = *self.selected_list_id.borrow() else {
            return false;
        };
        let name = self.list_name_entry.text().trim().to_owned();
        if name.is_empty() {
            self.toast("List name cannot be empty".to_owned());
            self.show_list_rename_editor();
            return false;
        }
        match self.core.update_list(list_id, name) {
            Ok(list) => {
                self.list_heading.set_text(&list.name);
                self.list_name_entry.set_text(&list.name);
                self.load_tasks();
                self.request_sync();
                self.list.grab_focus();
                true
            }
            Err(error) => {
                self.toast(format!("Failed to rename list: {error}"));
                self.show_list_rename_editor();
                false
            }
        }
    }

    pub(super) fn create_list(self: &Rc<Self>) {
        match self.core.create_list("New List".to_owned()) {
            Ok(list) => {
                self.selected_list_id.replace(Some(list.id));
                self.active_filter.replace(TaskFilterState::Upcoming);
                self.list_heading.set_text(&list.name);
                self.list_name_entry.set_text(&list.name);
                self.load_tasks();
                self.show_list_rename_editor();
                self.request_sync();
            }
            Err(error) => self.toast(format!("Failed to create list: {error}")),
        }
    }
}
