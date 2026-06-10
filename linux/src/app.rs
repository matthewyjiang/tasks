use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use taskmanager_core::{
    Keybindings, Task, TaskFilter, TaskList, TaskManagerCore, TaskPatch, TaskStatus,
};
use uuid::Uuid;

use crate::paths::{resolve_paths, APP_ID, APP_NAME};
use crate::platform::LinuxPlatform;
use crate::task_model::{default_sort, format_task_summary, task_matches_view, TaskFilterState};
use crate::ui::onboarding::needs_onboarding;
use crate::ui::search::normalize_query;
use crate::ui::settings::{read_settings, write_settings, LinuxSettings, ThemeChoice};

pub fn run() {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

struct AppState {
    core: Rc<TaskManagerCore>,
    tasks: RefCell<Vec<Task>>,
    pending_focus_task_id: RefCell<Option<Uuid>>,
    active_filter: RefCell<TaskFilterState>,
    selected_list_id: RefCell<Option<Uuid>>,
    search_query: RefCell<String>,
    list: gtk::ListBox,
    list_heading: gtk::Label,
    list_name_entry: gtk::Entry,
    list_rename_button: gtk::Button,
    list_actions_button: gtk::MenuButton,
    filter_count_labels: Vec<gtk::Label>,
    user_lists: RefCell<Vec<TaskList>>,
    user_list_box: gtk::ListBox,
    empty_state: gtk::Box,
    title_entry: gtk::Entry,
    body_view: gtk::TextView,
    status_combo: gtk::ComboBoxText,
    tags_entry: gtk::Entry,
    toast_overlay: adw::ToastOverlay,
}

impl AppState {
    fn load_tasks(self: &Rc<Self>) {
        let query = self.search_query.borrow().clone();
        let selected_list_id = *self.selected_list_id.borrow();
        let result = if query.is_empty() {
            let mut filter = self.active_filter.borrow().to_filter(now_ms());
            filter.project_id = selected_list_id;
            self.core.list_tasks(filter, default_sort())
        } else {
            self.core.search_tasks(query)
        };

        match result {
            Ok(tasks) => {
                let view = *self.active_filter.borrow();
                let now = now_ms();
                let tasks = tasks
                    .into_iter()
                    .filter(|task| task_matches_view(task, view, now))
                    .collect::<Vec<_>>();
                self.tasks.replace(tasks);
                if let Some(list_id) = selected_list_id {
                    if let Some(list) = self
                        .user_lists
                        .borrow()
                        .iter()
                        .find(|list| list.id == list_id)
                    {
                        self.list_heading.set_visible(false);
                        self.list_name_entry.set_visible(true);
                        self.list_rename_button.set_visible(false);
                        self.list_actions_button.set_visible(true);
                        self.list_name_entry.set_text(&list.name);
                    }
                } else {
                    self.list_heading.set_visible(true);
                    self.list_name_entry.set_visible(false);
                    self.list_rename_button.set_visible(false);
                    self.list_actions_button.set_visible(false);
                    self.list_heading
                        .set_text(self.active_filter.borrow().label());
                }
                self.render_list();
                self.refresh_sidebar_metadata();
            }
            Err(error) => self.toast(format!("Failed to load tasks: {error}")),
        }
    }

    fn render_list(self: &Rc<Self>) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }

        self.empty_state.set_visible(self.tasks.borrow().is_empty());

        for task in self.tasks.borrow().iter() {
            let row = gtk::ListBoxRow::new();
            row.add_css_class("task-row");
            row.set_selectable(true);
            row.set_activatable(true);
            row.set_widget_name(&task.id.to_string());

            let container = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            container.set_margin_top(12);
            container.set_margin_bottom(12);
            container.set_margin_start(14);
            container.set_margin_end(14);

            let status_dot = gtk::Label::new(Some(match task.status {
                TaskStatus::Done => "✓",
                TaskStatus::Open => "○",
            }));
            status_dot.add_css_class("status-dot");
            status_dot.set_valign(gtk::Align::Start);

            let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
            text.set_hexpand(true);
            let title = gtk::Entry::new();
            title.set_text(&task.title);
            title.add_css_class("task-title");
            title.add_css_class("rename-entry");
            title.add_css_class("flat");
            title.set_hexpand(false);
            update_entry_width(&title);
            title.connect_changed(update_entry_width);
            let task_confirm = gtk::Button::with_label("✓");
            task_confirm.add_css_class("confirm-button");
            task_confirm.add_css_class("task-confirm");
            task_confirm.set_visible(false);

            title.connect_activate({
                let state = Rc::clone(self);
                let task_id = task.id;
                let task_confirm = task_confirm.clone();
                move |entry| {
                    entry.remove_css_class("renaming");
                    update_task_title(&state, task_id, &entry.text());
                    task_confirm.set_visible(false);
                    state.list.grab_focus();
                }
            });
            let task_title_focus = gtk::EventControllerFocus::new();
            task_title_focus.connect_enter({
                let task_confirm = task_confirm.clone();
                let title = title.clone();
                move |_| {
                    title.add_css_class("renaming");
                    task_confirm.set_visible(true);
                }
            });
            task_title_focus.connect_leave({
                let task_confirm = task_confirm.clone();
                let title = title.clone();
                move |_| {
                    title.remove_css_class("renaming");
                    task_confirm.set_visible(false);
                }
            });
            title.add_controller(task_title_focus);
            task_confirm.connect_clicked({
                let state = Rc::clone(self);
                let task_id = task.id;
                let title = title.clone();
                let task_confirm = task_confirm.clone();
                move |_| {
                    title.remove_css_class("renaming");
                    update_task_title(&state, task_id, &title.text());
                    task_confirm.set_visible(false);
                    state.list.grab_focus();
                }
            });
            if self.pending_focus_task_id.borrow().as_ref() == Some(&task.id) {
                let title_to_focus = title.clone();
                gtk::glib::idle_add_local_once(move || {
                    title_to_focus.grab_focus();
                    title_to_focus.select_region(0, -1);
                });
                self.pending_focus_task_id.replace(None);
            }
            let summary = gtk::Label::new(Some(&format_task_row_summary(task)));
            summary.set_xalign(0.0);
            summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
            summary.add_css_class("task-summary");

            text.append(&title);
            text.append(&summary);

            let actions = gtk::MenuButton::new();
            actions.set_label("⋯");
            actions.add_css_class("flat");
            actions.add_css_class("task-actions");
            let popover = gtk::Popover::new();
            let action_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
            let toggle_done = gtk::Button::with_label(if task.status == TaskStatus::Done {
                "Mark Open"
            } else {
                "Mark Done"
            });
            toggle_done.add_css_class("flat");
            toggle_done.connect_clicked({
                let state = Rc::clone(self);
                let task_id = task.id;
                let next_status = if task.status == TaskStatus::Done {
                    TaskStatus::Open
                } else {
                    TaskStatus::Done
                };
                move |_| {
                    let patch = TaskPatch {
                        status: Some(next_status),
                        ..TaskPatch::default()
                    };
                    if let Err(error) = state.core.update_task(task_id, patch) {
                        state.toast(format!("Failed to update task: {error}"));
                    }
                    state.load_tasks();
                }
            });
            let due_label = gtk::Label::new(Some("Due date"));
            due_label.set_xalign(0.0);
            due_label.add_css_class("task-menu-heading");
            let calendar = gtk::Calendar::new();
            calendar.add_css_class("task-calendar");
            calendar.connect_day_selected({
                let state = Rc::clone(self);
                let task_id = task.id;
                move |calendar| {
                    let date = calendar.date();
                    // Store the selected calendar day at local noon. Noon avoids
                    // day-shift surprises around timezone/DST boundaries while
                    // preserving the user-selected date.
                    let due_at = gtk::glib::DateTime::from_local(
                        date.year(),
                        date.month(),
                        date.day_of_month(),
                        12,
                        0,
                        0.0,
                    )
                    .map(|date_time| date_time.to_unix() * 1000);
                    match due_at {
                        Ok(due_at) => {
                            let patch = TaskPatch {
                                due_at: Some(Some(due_at)),
                                ..TaskPatch::default()
                            };
                            if let Err(error) = state.core.update_task(task_id, patch) {
                                state.toast(format!("Failed to set due date: {error}"));
                            }
                            state.load_tasks();
                        }
                        Err(error) => state.toast(format!("Failed to read due date: {error}")),
                    }
                }
            });
            let clear_due = gtk::Button::with_label("Clear Due Date");
            clear_due.add_css_class("flat");
            clear_due.connect_clicked({
                let state = Rc::clone(self);
                let task_id = task.id;
                move |_| {
                    let patch = TaskPatch {
                        due_at: Some(None),
                        ..TaskPatch::default()
                    };
                    if let Err(error) = state.core.update_task(task_id, patch) {
                        state.toast(format!("Failed to clear due date: {error}"));
                    }
                    state.load_tasks();
                }
            });

            let list_label = gtk::Label::new(Some("List"));
            list_label.set_xalign(0.0);
            list_label.add_css_class("task-menu-heading");
            let inbox_button = gtk::Button::with_label("Inbox");
            inbox_button.add_css_class("flat");
            inbox_button.connect_clicked({
                let state = Rc::clone(self);
                let task_id = task.id;
                move |_| {
                    let patch = TaskPatch {
                        project_id: Some(None),
                        ..TaskPatch::default()
                    };
                    if let Err(error) = state.core.update_task(task_id, patch) {
                        state.toast(format!("Failed to move task: {error}"));
                    }
                    state.load_tasks();
                }
            });

            action_box.append(&due_label);
            action_box.append(&calendar);
            action_box.append(&clear_due);
            action_box.append(&list_label);
            action_box.append(&inbox_button);
            for list in self.user_lists.borrow().iter() {
                let list_button = gtk::Button::with_label(&list.name);
                list_button.add_css_class("flat");
                list_button.connect_clicked({
                    let state = Rc::clone(self);
                    let task_id = task.id;
                    let list_id = list.id;
                    move |_| {
                        let patch = TaskPatch {
                            project_id: Some(Some(list_id)),
                            ..TaskPatch::default()
                        };
                        if let Err(error) = state.core.update_task(task_id, patch) {
                            state.toast(format!("Failed to move task: {error}"));
                        }
                        state.load_tasks();
                    }
                });
                action_box.append(&list_button);
            }

            let delete = gtk::Button::with_label("Delete");
            delete.add_css_class("flat");
            delete.connect_clicked({
                let state = Rc::clone(self);
                let task_id = task.id;
                move |_| {
                    if let Err(error) = state.core.delete_task(task_id) {
                        state.toast(format!("Failed to delete task: {error}"));
                    }
                    state.load_tasks();
                }
            });
            action_box.append(&toggle_done);
            action_box.append(&delete);
            popover.set_child(Some(&action_box));
            actions.set_popover(Some(&popover));

            container.append(&status_dot);
            container.append(&text);
            container.append(&task_confirm);
            container.append(&actions);
            row.set_child(Some(&container));
            self.list.append(&row);
        }
    }

    fn select_task(self: &Rc<Self>, task_id: Uuid) {
        match self.core.get_task(task_id) {
            Ok(task) => self.show_task(&task),
            Err(error) => self.toast(format!("Failed to open task: {error}")),
        }
    }

    fn show_task(&self, task: &Task) {
        self.title_entry.set_text(&task.title);
        self.body_view.buffer().set_text(&task.body);
        self.status_combo.set_active(Some(match task.status {
            TaskStatus::Open => 0,
            TaskStatus::Done => 1,
        }));
        self.tags_entry.set_text(&task.tags.join(", "));
    }

    fn create_task(self: &Rc<Self>) {
        match self
            .core
            .create_task("New task".to_owned(), String::new(), None)
        {
            Ok(task) => {
                self.selected_list_id.replace(None);
                self.active_filter.replace(TaskFilterState::Inbox);
                self.pending_focus_task_id.replace(Some(task.id));
                self.load_tasks();
                self.select_task(task.id);
            }
            Err(error) => self.toast(format!("Failed to create task: {error}")),
        }
    }

    fn refresh_sidebar_metadata(self: &Rc<Self>) {
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
        self.render_user_lists();
    }

    fn render_user_lists(self: &Rc<Self>) {
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

        for list in lists {
            let row = gtk::ListBoxRow::new();
            row.add_css_class("sidebar-row");
            row.set_widget_name(&list.id.to_string());
            let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row_box.set_margin_top(8);
            row_box.set_margin_bottom(8);
            row_box.set_margin_start(10);
            row_box.set_margin_end(10);
            let icon = font_awesome_label("\u{f03a}");
            icon.add_css_class("sidebar-icon");
            icon.add_css_class("sidebar-icon-list");
            let name = gtk::Label::new(Some(&list.name));
            name.set_xalign(0.0);
            name.set_hexpand(true);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row_box.append(&icon);
            row_box.append(&name);
            row.set_child(Some(&row_box));
            self.user_list_box.append(&row);
        }
    }

    fn rename_selected_list(self: &Rc<Self>) {
        let Some(list_id) = *self.selected_list_id.borrow() else {
            return;
        };
        if let Err(error) = self
            .core
            .update_list(list_id, self.list_name_entry.text().to_string())
        {
            self.toast(format!("Failed to rename list: {error}"));
        }
        self.render_user_lists();
        self.load_tasks();
        self.list.grab_focus();
    }

    fn create_list(self: &Rc<Self>) {
        match self.core.create_list("New List".to_owned()) {
            Ok(list) => {
                self.selected_list_id.replace(Some(list.id));
                self.active_filter.replace(TaskFilterState::Upcoming);
                self.render_user_lists();
                self.load_tasks();
                self.list_name_entry.grab_focus();
                self.list_name_entry.select_region(0, -1);
            }
            Err(error) => self.toast(format!("Failed to create list: {error}")),
        }
    }

    fn toast(&self, message: String) {
        self.toast_overlay.add_toast(adw::Toast::new(&message));
    }
}

fn build_ui(app: &adw::Application) {
    let paths = match resolve_paths() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("Failed to resolve app paths: {error}");
            return;
        }
    };
    let settings = read_settings(&paths.settings_path).unwrap_or_else(|error| {
        eprintln!("Failed to read settings, using defaults: {error}");
        LinuxSettings::default()
    });
    if let Err(error) = write_settings(&paths.settings_path, &settings) {
        eprintln!("Failed to persist settings: {error}");
    }
    apply_theme_choice(settings.theme);

    let platform = LinuxPlatform::new();
    if needs_onboarding(&platform) {
        if let Err(error) = taskmanager_core::init_account(&platform) {
            eprintln!("Failed to initialize local account: {error}");
        }
    }

    let core = match TaskManagerCore::open(&paths.database_path) {
        Ok(core) => core,
        Err(error) => {
            eprintln!("Failed to open database: {error}");
            return;
        }
    };

    install_css();

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(APP_NAME)
        .default_width(1000)
        .default_height(700)
        .build();

    let new_button = gtk::Button::with_label("＋ Task");
    new_button.add_css_class("flat");

    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 14);
    sidebar.add_css_class("tsk-sidebar");
    sidebar.set_width_request(260);
    sidebar.set_hexpand(false);
    sidebar.set_halign(gtk::Align::Start);

    let filter_list = gtk::ListBox::new();
    filter_list.add_css_class("sidebar-list");
    filter_list.set_selection_mode(gtk::SelectionMode::Single);
    let mut filter_count_labels = Vec::new();
    for filter in sidebar_filter_order() {
        let row = gtk::ListBoxRow::new();
        row.add_css_class("sidebar-row");
        row.set_widget_name(filter.label());

        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);
        row_box.set_margin_start(10);
        row_box.set_margin_end(10);

        let icon = font_awesome_label(sidebar_filter_icon(filter));
        icon.add_css_class("sidebar-icon");
        icon.add_css_class(sidebar_filter_icon_class(filter));

        let label = gtk::Label::new(Some(sidebar_filter_title(filter)));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let count_label = gtk::Label::new(Some("0"));
        count_label.add_css_class("sidebar-count");

        row_box.append(&icon);
        row_box.append(&label);
        row_box.append(&count_label);
        row.set_child(Some(&row_box));
        filter_count_labels.push(count_label);
        filter_list.append(&row);
    }
    sidebar.append(&filter_list);

    let user_list_box = gtk::ListBox::new();
    user_list_box.add_css_class("sidebar-list");
    user_list_box.set_selection_mode(gtk::SelectionMode::Single);
    sidebar.append(&user_list_box);

    let sidebar_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar_spacer.set_vexpand(true);
    sidebar.append(&sidebar_spacer);

    let sidebar_bottom_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    sidebar_bottom_bar.add_css_class("sidebar-bottom-bar");
    let add_list_button = gtk::Button::new();
    add_list_button.set_child(Some(&icon_text_label("\u{f067}", "List")));
    add_list_button.add_css_class("flat");
    add_list_button.set_halign(gtk::Align::Start);
    let sidebar_bottom_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    sidebar_bottom_spacer.set_hexpand(true);
    let settings_button = icon_button("\u{f013}");
    settings_button.add_css_class("flat");
    settings_button.set_tooltip_text(Some("Settings"));
    sidebar_bottom_bar.append(&add_list_button);
    sidebar_bottom_bar.append(&sidebar_bottom_spacer);
    sidebar_bottom_bar.append(&settings_button);
    sidebar.append(&sidebar_bottom_bar);

    let list_heading = gtk::Label::new(Some("Inbox"));
    list_heading.set_xalign(0.0);
    list_heading.set_hexpand(true);
    list_heading.add_css_class("pane-title");

    let list_name_entry = gtk::Entry::new();
    list_name_entry.set_hexpand(false);
    update_entry_width(&list_name_entry);
    list_name_entry.connect_changed(update_entry_width);
    list_name_entry.add_css_class("pane-title");
    list_name_entry.add_css_class("rename-entry");
    list_name_entry.add_css_class("flat");
    list_name_entry.set_visible(false);

    let list_rename_button = gtk::Button::with_label("✓");
    list_rename_button.add_css_class("confirm-button");
    list_rename_button.set_visible(false);

    let list_actions_button = gtk::MenuButton::new();
    list_actions_button.set_label("⋯");
    list_actions_button.set_halign(gtk::Align::End);
    list_actions_button.add_css_class("flat");
    list_actions_button.set_visible(false);
    let list_actions_popover = gtk::Popover::new();
    let list_actions_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let list_delete_button = gtk::Button::with_label("Delete List");
    list_delete_button.add_css_class("flat");
    list_actions_box.append(&list_delete_button);
    list_actions_popover.set_child(Some(&list_actions_box));
    list_actions_button.set_popover(Some(&list_actions_popover));

    let page_title_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    page_title_spacer.set_hexpand(true);

    let page_title = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    page_title.set_hexpand(true);
    page_title.append(&list_heading);
    page_title.append(&list_name_entry);
    page_title.append(&list_rename_button);
    page_title.append(&page_title_spacer);
    page_title.append(&list_actions_button);

    let task_list = gtk::ListBox::new();
    task_list.set_vexpand(true);
    task_list.set_hexpand(true);
    task_list.add_css_class("task-list");
    task_list.set_selection_mode(gtk::SelectionMode::Single);

    let empty_state = gtk::Box::new(gtk::Orientation::Vertical, 6);
    empty_state.set_valign(gtk::Align::Center);
    empty_state.set_halign(gtk::Align::Center);
    let empty_title = gtk::Label::new(Some("Nothing here"));
    empty_title.add_css_class("empty-title");
    let empty_subtitle = gtk::Label::new(Some("Create a to-do or choose another list."));
    empty_subtitle.add_css_class("dim-label");
    empty_state.append(&empty_title);
    empty_state.append(&empty_subtitle);

    let list_stack = gtk::Overlay::new();
    list_stack.set_vexpand(true);
    list_stack.set_hexpand(true);
    list_stack.set_child(Some(&task_list));
    list_stack.add_overlay(&empty_state);
    let scrolled_list = gtk::ScrolledWindow::builder()
        .min_content_width(340)
        .vexpand(true)
        .hexpand(true)
        .child(&list_stack)
        .build();

    let list_pane = gtk::Box::new(gtk::Orientation::Vertical, 12);
    list_pane.set_hexpand(true);
    list_pane.set_vexpand(true);
    list_pane.add_css_class("tsk-list-pane");
    list_pane.set_margin_top(18);
    list_pane.set_margin_bottom(18);
    list_pane.set_margin_start(18);
    list_pane.set_margin_end(18);
    list_pane.append(&page_title);
    list_pane.append(&scrolled_list);

    let title_entry = gtk::Entry::new();
    title_entry.set_placeholder_text(Some("What do you want to do?"));
    title_entry.add_css_class("editor-title");
    let body_view = gtk::TextView::new();
    body_view.set_vexpand(true);
    body_view.set_wrap_mode(gtk::WrapMode::Word);
    body_view.add_css_class("editor-notes");
    let status_combo = gtk::ComboBoxText::new();
    status_combo.append_text("Open");
    status_combo.append_text("Done");
    status_combo.set_active(Some(0));
    let tags_entry = gtk::Entry::new();
    tags_entry.set_placeholder_text(Some("Tags, comma separated"));

    let content_bottom_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content_bottom_bar.set_homogeneous(true);
    content_bottom_bar.add_css_class("content-bottom-bar");
    let search_button = icon_button("\u{f002}");
    search_button.add_css_class("flat");
    search_button.set_hexpand(true);
    search_button.set_tooltip_text(Some("Search"));
    let bottom_new_button = icon_button("\u{f067}");
    bottom_new_button.add_css_class("flat");
    bottom_new_button.set_hexpand(true);
    bottom_new_button.set_tooltip_text(Some("Add Task"));
    content_bottom_bar.append(&search_button);
    content_bottom_bar.append(&bottom_new_button);

    let main_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    main_area.set_hexpand(true);
    main_area.set_vexpand(true);
    main_area.append(&list_pane);
    main_area.append(&content_bottom_bar);

    let page = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    page.set_vexpand(true);
    page.set_hexpand(true);
    page.append(&sidebar);
    page.append(&main_area);

    let search_panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
    search_panel.add_css_class("search-panel");
    search_panel.set_width_request(560);
    search_panel.set_halign(gtk::Align::Center);
    search_panel.set_valign(gtk::Align::Center);
    search_panel.set_margin_bottom(140);
    search_panel.set_visible(false);
    let overlay_search = gtk::SearchEntry::new();
    overlay_search.set_placeholder_text(Some("Search tasks"));
    overlay_search.add_css_class("search-panel-entry");
    let search_results = gtk::ListBox::new();
    search_results.add_css_class("search-results");
    search_results.set_selection_mode(gtk::SelectionMode::None);
    search_panel.append(&overlay_search);
    search_panel.append(&search_results);

    let root_overlay = gtk::Overlay::new();
    root_overlay.set_child(Some(&page));
    root_overlay.add_overlay(&search_panel);

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&root_overlay));
    window.set_content(Some(&toast_overlay));

    let keybindings = core.vault_settings().unwrap_or_default().keybindings;

    let state = Rc::new(AppState {
        core: Rc::new(core),
        tasks: RefCell::new(Vec::new()),
        pending_focus_task_id: RefCell::new(None),
        active_filter: RefCell::new(TaskFilterState::Inbox),
        selected_list_id: RefCell::new(None),
        search_query: RefCell::new(String::new()),
        list: task_list,
        list_heading,
        list_name_entry,
        list_rename_button,
        list_actions_button,
        filter_count_labels,
        user_lists: RefCell::new(Vec::new()),
        user_list_box,
        empty_state,
        title_entry,
        body_view,
        status_combo,
        tags_entry,
        toast_overlay,
    });

    let create_task_action: Rc<dyn Fn()> = Rc::new({
        let state = Rc::clone(&state);
        let filter_list = filter_list.clone();
        move || {
            if let Some(row) = filter_list.row_at_index(0) {
                filter_list.select_row(Some(&row));
            }
            state.create_task();
        }
    });
    new_button.connect_clicked({
        let create_task_action = Rc::clone(&create_task_action);
        move |_| create_task_action()
    });
    bottom_new_button.connect_clicked({
        let create_task_action = Rc::clone(&create_task_action);
        move |_| create_task_action()
    });
    add_list_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| state.create_list()
    });
    settings_button.connect_clicked({
        let window = window.clone();
        let settings_path = paths.settings_path.clone();
        let core = Rc::clone(&state.core);
        move |_| show_settings_window(&window, settings_path.clone(), Rc::clone(&core))
    });
    let list_name_focus = gtk::EventControllerFocus::new();
    list_name_focus.connect_enter({
        let state = Rc::clone(&state);
        move |_| {
            state.list_name_entry.add_css_class("renaming");
            state.list_rename_button.set_visible(true);
        }
    });
    list_name_focus.connect_leave({
        let state = Rc::clone(&state);
        move |_| {
            state.list_name_entry.remove_css_class("renaming");
            state.list_rename_button.set_visible(false);
        }
    });
    state.list_name_entry.add_controller(list_name_focus);
    state.list_name_entry.connect_activate({
        let state = Rc::clone(&state);
        move |_| {
            state.list_name_entry.remove_css_class("renaming");
            state.rename_selected_list();
            state.list_rename_button.set_visible(false);
        }
    });
    state.list_rename_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| {
            state.list_name_entry.remove_css_class("renaming");
            state.rename_selected_list();
            state.list_rename_button.set_visible(false);
        }
    });
    list_delete_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| {
            let Some(list_id) = *state.selected_list_id.borrow() else {
                return;
            };
            if let Err(error) = state.core.delete_list(list_id) {
                state.toast(format!("Failed to delete list: {error}"));
            }
            state.selected_list_id.replace(None);
            state.active_filter.replace(TaskFilterState::Inbox);
            state.render_user_lists();
            state.load_tasks();
        }
    });
    let open_search_action: Rc<dyn Fn()> = Rc::new({
        let search_panel = search_panel.clone();
        let overlay_search = overlay_search.clone();
        move || {
            search_panel.set_visible(true);
            overlay_search.grab_focus();
        }
    });
    search_button.connect_clicked({
        let open_search_action = Rc::clone(&open_search_action);
        move |_| open_search_action()
    });
    let search_key_controller = gtk::EventControllerKey::new();
    search_key_controller.connect_key_pressed({
        let search_panel = search_panel.clone();
        move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                search_panel.set_visible(false);
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        }
    });
    overlay_search.add_controller(search_key_controller);
    overlay_search.connect_search_changed({
        let state = Rc::clone(&state);
        let search_results = search_results.clone();
        move |entry| render_search_results(&state, &search_results, &normalize_query(&entry.text()))
    });
    search_results.connect_row_activated({
        let state = Rc::clone(&state);
        let search_panel = search_panel.clone();
        move |_, row| {
            if let Ok(task_id) = Uuid::parse_str(&row.widget_name()) {
                match state.core.get_task(task_id) {
                    Ok(task) => {
                        open_task_from_search(&state, &task);
                        search_panel.set_visible(false);
                    }
                    Err(error) => state.toast(format!("Failed to open task: {error}")),
                }
            }
        }
    });
    filter_list.connect_row_activated({
        let state = Rc::clone(&state);
        let user_list_box = state.user_list_box.clone();
        move |_, row| {
            let filter = match row.index() {
                0 => TaskFilterState::Inbox,
                1 => TaskFilterState::Today,
                2 => TaskFilterState::Upcoming,
                3 => TaskFilterState::NoDueDate,
                4 => TaskFilterState::Done,
                _ => TaskFilterState::Inbox,
            };
            user_list_box.unselect_all();
            state.selected_list_id.replace(None);
            state.active_filter.replace(filter);
            state.load_tasks();
        }
    });
    state.user_list_box.connect_row_activated({
        let state = Rc::clone(&state);
        let filter_list_for_user_rows = filter_list.clone();
        move |_, row| {
            if let Ok(list_id) = Uuid::parse_str(&row.widget_name()) {
                filter_list_for_user_rows.unselect_all();
                state.selected_list_id.replace(Some(list_id));
                state.active_filter.replace(TaskFilterState::Upcoming);
                state.load_tasks();
            }
        }
    });
    state.list.connect_row_selected({
        let state = Rc::clone(&state);
        move |_, row| {
            let Some(row) = row else {
                return;
            };
            if let Ok(task_id) = Uuid::parse_str(&row.widget_name()) {
                state.select_task(task_id);
            }
        }
    });

    install_keybindings(
        &window,
        &keybindings,
        Rc::clone(&create_task_action),
        Rc::clone(&open_search_action),
        search_panel.clone(),
        Rc::clone(&state),
    );

    if let Some(row) = filter_list.row_at_index(0) {
        filter_list.select_row(Some(&row));
    }
    state.load_tasks();
    window.present();
}

fn apply_theme_choice(theme: ThemeChoice) {
    let color_scheme = match theme {
        ThemeChoice::System => adw::ColorScheme::Default,
        ThemeChoice::Light => adw::ColorScheme::ForceLight,
        ThemeChoice::Dark => adw::ColorScheme::ForceDark,
    };
    adw::StyleManager::default().set_color_scheme(color_scheme);
}

fn show_settings_window(
    parent: &adw::ApplicationWindow,
    settings_path: PathBuf,
    core: Rc<TaskManagerCore>,
) {
    let settings = read_settings(&settings_path).unwrap_or_default();
    let vault_settings = core.vault_settings().unwrap_or_default();
    let dialog = gtk::Window::builder()
        .title("Settings")
        .transient_for(parent)
        .modal(true)
        .default_width(420)
        .default_height(260)
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let title = gtk::Label::new(Some("Settings"));
    title.set_xalign(0.0);
    title.add_css_class("pane-title");
    content.append(&title);

    let server_label = gtk::Label::new(Some("Server URL"));
    server_label.set_xalign(0.0);
    let server_entry = gtk::Entry::new();
    server_entry.set_placeholder_text(Some("Optional sync server URL"));
    server_entry.set_text(&settings.server_url);
    content.append(&server_label);
    content.append(&server_entry);

    let theme_label = gtk::Label::new(Some("Theme"));
    theme_label.set_xalign(0.0);
    let theme_combo = gtk::ComboBoxText::new();
    theme_combo.append(Some("system"), "System");
    theme_combo.append(Some("light"), "Light");
    theme_combo.append(Some("dark"), "Dark");
    theme_combo.set_active_id(Some(match settings.theme {
        ThemeChoice::System => "system",
        ThemeChoice::Light => "light",
        ThemeChoice::Dark => "dark",
    }));
    content.append(&theme_label);
    content.append(&theme_combo);

    let show_completed = gtk::CheckButton::with_label("Show completed tasks");
    show_completed.set_active(vault_settings.show_completed);
    content.append(&show_completed);

    let keybind_label = gtk::Label::new(Some("Keybindings (encrypted + synced)"));
    keybind_label.set_xalign(0.0);
    keybind_label.add_css_class("task-menu-heading");
    content.append(&keybind_label);
    let add_task_key = settings_entry("Add task", &vault_settings.keybindings.add_task, &content);
    let search_key = settings_entry("Search", &vault_settings.keybindings.search, &content);
    let close_overlay_key = settings_entry(
        "Close overlay",
        &vault_settings.keybindings.close_overlay,
        &content,
    );
    let confirm_rename_key = settings_entry(
        "Confirm rename",
        &vault_settings.keybindings.confirm_rename,
        &content,
    );
    let delete_task_key = settings_entry(
        "Delete task",
        &vault_settings.keybindings.delete_task,
        &content,
    );
    let toggle_done_key = settings_entry(
        "Toggle done",
        &vault_settings.keybindings.toggle_done,
        &content,
    );

    let save_button = gtk::Button::with_label("Save");
    save_button.add_css_class("suggested-action");
    save_button.set_halign(gtk::Align::End);
    content.append(&save_button);

    save_button.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            let theme = match theme_combo.active_id().as_deref() {
                Some("light") => ThemeChoice::Light,
                Some("dark") => ThemeChoice::Dark,
                _ => ThemeChoice::System,
            };
            let settings = LinuxSettings {
                server_url: server_entry.text().to_string(),
                theme,
                show_completed: false,
            };
            let mut vault_settings = core.vault_settings().unwrap_or_default();
            vault_settings.show_completed = show_completed.is_active();
            vault_settings.keybindings = Keybindings {
                add_task: add_task_key.text().to_string(),
                search: search_key.text().to_string(),
                close_overlay: close_overlay_key.text().to_string(),
                confirm_rename: confirm_rename_key.text().to_string(),
                delete_task: delete_task_key.text().to_string(),
                toggle_done: toggle_done_key.text().to_string(),
            };
            if let Err(error) = write_settings(&settings_path, &settings) {
                eprintln!("Failed to save local settings: {error}");
            } else if let Err(error) = core.update_vault_settings(vault_settings) {
                eprintln!("Failed to save encrypted settings: {error}");
            } else {
                apply_theme_choice(theme);
                dialog.close();
            }
        }
    });

    dialog.set_child(Some(&content));
    dialog.present();
}

fn font_awesome_label(icon: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_markup(&format!(
        "<span font_desc=\"Font Awesome 7 Free Solid 12\" fallback=\"false\">{icon}</span>"
    ));
    label
}

fn icon_text_label(icon: &str, text: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&font_awesome_label(icon));
    row.append(&gtk::Label::new(Some(text)));
    row
}

fn icon_button(icon: &str) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_child(Some(&font_awesome_label(icon)));
    button
}

fn install_keybindings(
    window: &adw::ApplicationWindow,
    keybindings: &Keybindings,
    create_task_action: Rc<dyn Fn()>,
    open_search_action: Rc<dyn Fn()>,
    search_panel: gtk::Box,
    state: Rc<AppState>,
) {
    let controller = gtk::ShortcutController::new();
    controller.set_scope(gtk::ShortcutScope::Global);
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);

    add_shortcut(&controller, &keybindings.add_task, {
        let create_task_action = Rc::clone(&create_task_action);
        move || create_task_action()
    });
    add_shortcut(&controller, &keybindings.search, {
        let open_search_action = Rc::clone(&open_search_action);
        move || open_search_action()
    });
    if keybindings.search != "<Primary>f" {
        add_shortcut(&controller, "<Primary>f", {
            let open_search_action = Rc::clone(&open_search_action);
            move || open_search_action()
        });
    }
    add_shortcut(&controller, &keybindings.close_overlay, {
        let search_panel = search_panel.clone();
        move || search_panel.set_visible(false)
    });
    add_shortcut(&controller, &keybindings.delete_task, {
        let state = Rc::clone(&state);
        move || delete_selected_task(&state)
    });
    add_shortcut(&controller, &keybindings.toggle_done, {
        let state = Rc::clone(&state);
        move || toggle_selected_task_done(&state)
    });

    window.add_controller(controller);

    let key_controller = gtk::EventControllerKey::new();
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let add_task = parse_accel(&keybindings.add_task);
    let search = parse_accel(&keybindings.search);
    let search_fallback = parse_accel("<Primary>f");
    let close_overlay = parse_accel(&keybindings.close_overlay);
    let delete_task = parse_accel(&keybindings.delete_task);
    let toggle_done = parse_accel(&keybindings.toggle_done);
    key_controller.connect_key_pressed(move |_, key, _, modifiers| {
        if accel_matches(add_task, key, modifiers) {
            create_task_action();
            gtk::glib::Propagation::Stop
        } else if accel_matches(search, key, modifiers)
            || accel_matches(search_fallback, key, modifiers)
        {
            open_search_action();
            gtk::glib::Propagation::Stop
        } else if accel_matches(close_overlay, key, modifiers) {
            search_panel.set_visible(false);
            gtk::glib::Propagation::Stop
        } else if accel_matches(delete_task, key, modifiers) {
            delete_selected_task(&state);
            gtk::glib::Propagation::Stop
        } else if accel_matches(toggle_done, key, modifiers) {
            toggle_selected_task_done(&state);
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    window.add_controller(key_controller);
}

fn add_shortcut(
    controller: &gtk::ShortcutController,
    accelerator: &str,
    action: impl Fn() + 'static,
) {
    let Some(trigger) = gtk::ShortcutTrigger::parse_string(accelerator) else {
        eprintln!("Ignoring invalid keybinding: {accelerator}");
        return;
    };
    let shortcut = gtk::Shortcut::new(
        Some(trigger),
        Some(gtk::CallbackAction::new(move |_, _| {
            action();
            gtk::glib::Propagation::Stop
        })),
    );
    controller.add_shortcut(shortcut);
}

fn parse_accel(accelerator: &str) -> Option<(gtk::gdk::Key, gtk::gdk::ModifierType)> {
    gtk::accelerator_parse(accelerator)
}

fn accel_matches(
    parsed: Option<(gtk::gdk::Key, gtk::gdk::ModifierType)>,
    key: gtk::gdk::Key,
    modifiers: gtk::gdk::ModifierType,
) -> bool {
    let Some((expected_key, expected_modifiers)) = parsed else {
        return false;
    };
    key == expected_key && modifiers.contains(expected_modifiers)
}

fn delete_selected_task(state: &Rc<AppState>) {
    if let Some(task_id) = selected_task_id(state) {
        if let Err(error) = state.core.delete_task(task_id) {
            state.toast(format!("Failed to delete task: {error}"));
        }
        state.load_tasks();
    }
}

fn toggle_selected_task_done(state: &Rc<AppState>) {
    if let Some(task_id) = selected_task_id(state) {
        match state.core.get_task(task_id) {
            Ok(task) => {
                let patch = TaskPatch {
                    status: Some(if task.status == TaskStatus::Done {
                        TaskStatus::Open
                    } else {
                        TaskStatus::Done
                    }),
                    ..TaskPatch::default()
                };
                if let Err(error) = state.core.update_task(task_id, patch) {
                    state.toast(format!("Failed to update task: {error}"));
                }
                state.load_tasks();
            }
            Err(error) => state.toast(format!("Failed to load task: {error}")),
        }
    }
}

fn selected_task_id(state: &AppState) -> Option<Uuid> {
    state
        .list
        .selected_row()
        .and_then(|row| Uuid::parse_str(&row.widget_name()).ok())
}

fn settings_entry(label: &str, value: &str, content: &gtk::Box) -> gtk::Entry {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = gtk::Label::new(Some(label));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    let entry = gtk::Entry::new();
    entry.set_text(value);
    entry.set_width_chars(18);
    row.append(&name);
    row.append(&entry);
    content.append(&row);
    entry
}

fn render_search_results(state: &Rc<AppState>, results: &gtk::ListBox, query: &str) {
    while let Some(row) = results.first_child() {
        results.remove(&row);
    }
    if query.is_empty() {
        return;
    }

    match state.core.search_tasks(query.to_owned()) {
        Ok(tasks) => {
            for task in tasks.into_iter().take(10) {
                let row = gtk::ListBoxRow::new();
                row.set_widget_name(&task.id.to_string());
                row.add_css_class("search-result-row");
                let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
                content.set_margin_top(8);
                content.set_margin_bottom(8);
                content.set_margin_start(10);
                content.set_margin_end(10);
                let title = gtk::Label::new(Some(&task.title));
                title.set_xalign(0.0);
                title.set_ellipsize(gtk::pango::EllipsizeMode::End);
                let summary = gtk::Label::new(Some(&format_task_row_summary(&task)));
                summary.set_xalign(0.0);
                summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
                summary.add_css_class("task-summary");
                content.append(&title);
                content.append(&summary);
                row.set_child(Some(&content));
                results.append(&row);
            }
        }
        Err(error) => state.toast(format!("Search failed: {error}")),
    }
}

fn open_task_from_search(state: &Rc<AppState>, task: &Task) {
    state.selected_list_id.replace(task.project_id);
    state
        .active_filter
        .replace(if task.status == TaskStatus::Done {
            TaskFilterState::Done
        } else if task.project_id.is_some() {
            TaskFilterState::Upcoming
        } else {
            TaskFilterState::Inbox
        });
    state.load_tasks();
    state.select_task(task.id);
}

fn update_entry_width(entry: &gtk::Entry) {
    let width = entry.text().chars().count().clamp(1, 48) as i32;
    entry.set_width_chars(width);
    entry.set_max_width_chars(width);
}

fn update_task_title(state: &Rc<AppState>, task_id: Uuid, title: &str) {
    let patch = TaskPatch {
        title: Some(title.to_owned()),
        ..TaskPatch::default()
    };
    if let Err(error) = state.core.update_task(task_id, patch) {
        state.toast(format!("Failed to update task: {error}"));
    }
    state.load_tasks();
}

fn format_task_row_summary(task: &Task) -> String {
    let mut parts = Vec::new();
    if let Some(due_at) = task.due_at {
        parts.push(format!("Due {}", format_due_date(due_at)));
    }
    let base = format_task_summary(task);
    if !base.is_empty() {
        parts.push(base);
    }
    parts.join(" · ")
}

fn format_due_date(due_at_ms: i64) -> String {
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

fn same_local_day(left: &gtk::glib::DateTime, right: &gtk::glib::DateTime) -> bool {
    left.year() == right.year()
        && left.month() == right.month()
        && left.day_of_month() == right.day_of_month()
}

fn sidebar_filter_order() -> [TaskFilterState; 5] {
    [
        TaskFilterState::Inbox,
        TaskFilterState::Today,
        TaskFilterState::Upcoming,
        TaskFilterState::NoDueDate,
        TaskFilterState::Done,
    ]
}

fn sidebar_filter_title(filter: TaskFilterState) -> &'static str {
    match filter {
        TaskFilterState::Inbox => "Inbox",
        TaskFilterState::Today => "Today",
        TaskFilterState::Upcoming => "Upcoming",
        TaskFilterState::NoDueDate => "Anytime",
        TaskFilterState::Done => "Done",
    }
}

fn sidebar_filter_icon(filter: TaskFilterState) -> &'static str {
    match filter {
        TaskFilterState::Inbox => "\u{f01c}",
        TaskFilterState::Today => "\u{f783}",
        TaskFilterState::Upcoming => "\u{f073}",
        TaskFilterState::NoDueDate => "\u{f5fd}",
        TaskFilterState::Done => "\u{f058}",
    }
}

fn sidebar_filter_icon_class(filter: TaskFilterState) -> &'static str {
    match filter {
        TaskFilterState::Inbox => "sidebar-icon-inbox",
        TaskFilterState::Today => "sidebar-icon-today",
        TaskFilterState::Upcoming => "sidebar-icon-upcoming",
        TaskFilterState::NoDueDate => "sidebar-icon-anytime",
        TaskFilterState::Done => "sidebar-icon-done",
    }
}

fn count_for_filter(tasks: &[Task], filter: TaskFilterState, now_ms: i64) -> usize {
    tasks
        .iter()
        .filter(|task| task_matches_view(task, filter, now_ms))
        .count()
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        r#"
        @font-face {
            font-family: "Font Awesome 7 Free Solid";
            src: url("file:///usr/share/fonts/WOFF2/fa-solid-900.woff2");
        }
        .fa-icon,
        .fa-icon label,
        button.fa-icon label {
            font-family: "Font Awesome 7 Free Solid", "Font Awesome 7 Free", "Font Awesome 6 Free", sans-serif;
            font-weight: 900;
        }
        .tsk-sidebar {
            background: @sidebar_bg_color;
            padding: 10px 10px 0 12px;
        }
        .sidebar-search {
            border-radius: 8px;
            min-height: 30px;
        }
        .sidebar-list {
            background: transparent;
        }
        .sidebar-list row,
        .sidebar-row {
            border-radius: 10px;
            margin: 2px 0;
            color: @window_fg_color;
            background: transparent;
        }
        .sidebar-list row:hover,
        .sidebar-row:hover {
            background: color-mix(in srgb, @window_fg_color 6%, transparent);
        }
        .sidebar-list row:selected,
        .sidebar-row:selected {
            background: color-mix(in srgb, @window_fg_color 8%, transparent);
            color: @window_fg_color;
        }
        .sidebar-list row:selected:hover,
        .sidebar-row:selected:hover {
            background: color-mix(in srgb, @window_fg_color 10%, transparent);
        }
        .sidebar-list row:selected label {
            color: @window_fg_color;
        }
        .sidebar-static-row {
            color: @window_fg_color;
            padding: 4px 10px;
            border-radius: 6px;
        }
        .sidebar-icon {
            min-width: 16px;
            font-size: 12px;
            font-weight: 800;
        }
        .sidebar-icon-inbox { color: #64d2ff; }
        .sidebar-icon-today { color: #ff9f0a; }
        .sidebar-icon-upcoming { color: #0a84ff; }
        .sidebar-icon-anytime { color: #8e8e93; }
        .sidebar-icon-done { color: #30d158; }
        .sidebar-count {
            color: @dim_label_color;
            font-size: 12px;
            font-weight: 700;
        }
        .sidebar-bottom-bar,
        .content-bottom-bar {
            border-top: 1px solid @borders;
            padding-top: 8px;
        }
        .sidebar-bottom-bar {
            margin-left: -12px;
            margin-right: -10px;
            padding-left: 12px;
            padding-right: 10px;
            padding-bottom: 10px;
        }
        .content-bottom-bar {
            padding: 8px 18px 10px 18px;
        }
        .search-panel {
            padding: 10px;
            border-radius: 16px;
            background: @popover_bg_color;
            color: @popover_fg_color;
            box-shadow: 0 12px 36px color-mix(in srgb, black 24%, transparent);
        }
        .search-panel-entry {
            min-height: 40px;
            border-radius: 12px;
        }
        .search-results {
            background: transparent;
        }
        .search-result-row {
            border-radius: 10px;
        }
        .pane-title {
            font-size: 22px;
            font-weight: 750;
            letter-spacing: -0.03em;
        }
        .task-list {
            background: transparent;
        }
        .task-row {
            border-radius: 6px;
            margin: 1px 0;
            background: transparent;
            border-bottom: 1px solid @borders;
        }
        .task-row:hover {
            background: color-mix(in srgb, @window_fg_color 5%, transparent);
        }
        .confirm-button {
            background: @accent_bg_color;
            color: @accent_fg_color;
            border-radius: 999px;
            font-weight: 800;
        }
        .confirm-button:hover {
            background: color-mix(in srgb, @accent_bg_color 88%, @window_fg_color 12%);
            color: @accent_fg_color;
        }
        .task-actions {
            opacity: 0;
        }
        .task-row:hover .task-actions {
            opacity: 1;
        }
        .status-dot {
            color: @accent_color;
            font-size: 18px;
            font-weight: 700;
        }
        .task-title {
            font-size: 16px;
            font-weight: 650;
        }
        entry.rename-entry,
        entry.rename-entry:focus,
        entry.rename-entry:focus-within {
            background: transparent;
            border: none;
            border-bottom: 2px solid transparent;
            border-radius: 0;
            box-shadow: none;
            outline: none;
            padding-left: 0;
            padding-right: 0;
        }
        entry.rename-entry.renaming,
        entry.rename-entry.renaming:focus,
        entry.rename-entry.renaming:focus-within {
            border-bottom-color: @accent_color;
        }
        .task-summary {
            color: @dim_label_color;
            font-size: 13px;
        }
        .task-menu-heading {
            color: @dim_label_color;
            font-size: 12px;
            font-weight: 700;
            margin-top: 6px;
        }
        .task-calendar {
            border-radius: 12px;
            padding: 6px;
            background: @card_bg_color;
            color: @window_fg_color;
        }
        .task-calendar button {
            border-radius: 8px;
            color: @window_fg_color;
        }
        .task-calendar button:hover,
        .task-calendar label:hover,
        .task-calendar grid label:hover {
            background: color-mix(in srgb, @accent_color 14%, transparent);
            border-radius: 8px;
        }
        .task-calendar:selected,
        .task-calendar label:selected,
        .task-calendar grid label:selected {
            background: @accent_bg_color;
            color: @accent_fg_color;
            border-radius: 8px;
        }
        .editor-title {
            font-size: 26px;
            font-weight: 800;
            letter-spacing: -0.04em;
            border: none;
            box-shadow: none;
            background: transparent;
            padding: 4px 0;
        }
        .notes-card {
            border-radius: 0;
            background: transparent;
            border-top: 1px solid @borders;
            padding: 10px 0;
        }
        .editor-notes {
            font-size: 14px;
            line-height: 1.45;
            background: transparent;
        }
        .empty-title {
            font-size: 22px;
            font-weight: 700;
        }
        "#,
    );

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}
