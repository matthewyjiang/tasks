use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use taskmanager_core::{Task, TaskFilter, TaskList, TaskManagerCore, TaskPatch, TaskStatus};
use uuid::Uuid;

use crate::paths::{resolve_paths, APP_ID, APP_NAME};
use crate::platform::LinuxPlatform;
use crate::task_model::{default_sort, format_task_summary, task_matches_view, TaskFilterState};
use crate::ui::onboarding::needs_onboarding;
use crate::ui::search::normalize_query;
use crate::ui::settings::{read_settings, write_settings, LinuxSettings};

pub fn run() {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

struct AppState {
    core: Rc<TaskManagerCore>,
    tasks: RefCell<Vec<Task>>,
    active_filter: RefCell<TaskFilterState>,
    selected_list_id: RefCell<Option<Uuid>>,
    search_query: RefCell<String>,
    list: gtk::ListBox,
    list_heading: gtk::Label,
    list_name_entry: gtk::Entry,
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
                        self.list_actions_button.set_visible(true);
                        self.list_name_entry.set_text(&list.name);
                    }
                } else {
                    self.list_heading.set_visible(true);
                    self.list_name_entry.set_visible(false);
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
            title.add_css_class("flat");
            title.set_hexpand(true);
            title.connect_activate({
                let state = Rc::clone(self);
                let task_id = task.id;
                move |entry| {
                    let patch = TaskPatch {
                        title: Some(entry.text().to_string()),
                        ..TaskPatch::default()
                    };
                    if let Err(error) = state.core.update_task(task_id, patch) {
                        state.toast(format!("Failed to update task: {error}"));
                    }
                    state.load_tasks();
                }
            });
            let summary = gtk::Label::new(Some(&format_task_summary(task)));
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
            let icon = gtk::Label::new(Some("●"));
            icon.add_css_class("sidebar-icon");
            icon.add_css_class("sidebar-icon-list");
            let name = gtk::Label::new(Some(&list.name));
            name.set_xalign(0.0);
            name.set_hexpand(true);
            row_box.append(&icon);
            row_box.append(&name);
            row.set_child(Some(&row_box));
            self.user_list_box.append(&row);
        }
    }

    fn create_list(self: &Rc<Self>) {
        match self.core.create_list("New List".to_owned()) {
            Ok(list) => {
                self.selected_list_id.replace(Some(list.id));
                self.active_filter.replace(TaskFilterState::Upcoming);
                self.render_user_lists();
                self.load_tasks();
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

    let header = adw::HeaderBar::new();
    header.add_css_class("flat");
    header.set_show_start_title_buttons(false);
    let new_button = gtk::Button::with_label("＋");
    header.pack_start(&new_button);

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search"));
    search.add_css_class("sidebar-search");

    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 14);
    sidebar.add_css_class("tsk-sidebar");
    sidebar.set_size_request(260, -1);

    sidebar.append(&search);

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

        let icon = gtk::Label::new(Some(sidebar_filter_icon(filter)));
        icon.add_css_class("sidebar-icon");
        icon.add_css_class(sidebar_filter_icon_class(filter));

        let label = gtk::Label::new(Some(sidebar_filter_title(filter)));
        label.set_xalign(0.0);
        label.set_hexpand(true);
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

    let add_list_button = gtk::Button::with_label("＋ List");
    add_list_button.add_css_class("flat");
    sidebar.append(&add_list_button);

    let list_heading = gtk::Label::new(Some("Inbox"));
    list_heading.set_xalign(0.0);
    list_heading.add_css_class("pane-title");

    let list_name_entry = gtk::Entry::new();
    list_name_entry.add_css_class("pane-title");
    list_name_entry.add_css_class("flat");
    list_name_entry.set_visible(false);

    let list_actions_button = gtk::MenuButton::new();
    list_actions_button.set_label("⋯");
    list_actions_button.add_css_class("flat");
    list_actions_button.set_visible(false);
    let list_actions_popover = gtk::Popover::new();
    let list_actions_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let list_delete_button = gtk::Button::with_label("Delete List");
    list_delete_button.add_css_class("flat");
    list_actions_box.append(&list_delete_button);
    list_actions_popover.set_child(Some(&list_actions_box));
    list_actions_button.set_popover(Some(&list_actions_popover));

    let page_title = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    page_title.append(&list_heading);
    page_title.append(&list_name_entry);
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

    let main_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    main_area.set_hexpand(true);
    main_area.set_vexpand(true);
    main_area.append(&header);
    main_area.append(&list_pane);

    let page = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    page.set_vexpand(true);
    page.set_hexpand(true);
    page.append(&sidebar);
    page.append(&main_area);

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&page));
    window.set_content(Some(&toast_overlay));

    let state = Rc::new(AppState {
        core: Rc::new(core),
        tasks: RefCell::new(Vec::new()),
        active_filter: RefCell::new(TaskFilterState::Inbox),
        selected_list_id: RefCell::new(None),
        search_query: RefCell::new(String::new()),
        list: task_list,
        list_heading,
        list_name_entry,
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

    new_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| state.create_task()
    });
    add_list_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| state.create_list()
    });
    state.list_name_entry.connect_activate({
        let state = Rc::clone(&state);
        move |entry| {
            let Some(list_id) = *state.selected_list_id.borrow() else {
                return;
            };
            if let Err(error) = state.core.update_list(list_id, entry.text().to_string()) {
                state.toast(format!("Failed to rename list: {error}"));
            }
            state.render_user_lists();
            state.load_tasks();
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
    search.connect_search_changed({
        let state = Rc::clone(&state);
        move |entry| {
            state.search_query.replace(normalize_query(&entry.text()));
            state.load_tasks();
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

    if let Some(row) = filter_list.row_at_index(0) {
        filter_list.select_row(Some(&row));
    }
    state.load_tasks();
    window.present();
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
        TaskFilterState::Inbox => "▣",
        TaskFilterState::Today => "●",
        TaskFilterState::Upcoming => "◆",
        TaskFilterState::NoDueDate => "■",
        TaskFilterState::Done => "✓",
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
        .tsk-sidebar {
            background: @sidebar_bg_color;
            padding: 10px 10px 10px 12px;
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
        .task-summary {
            color: @dim_label_color;
            font-size: 13px;
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
