use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use taskmanager_core::{Task, TaskManagerCore, TaskPatch, TaskStatus};
use uuid::Uuid;

use crate::paths::{resolve_paths, APP_ID, APP_NAME};
use crate::platform::LinuxPlatform;
use crate::task_model::{default_sort, format_task_summary, TaskFilterState};
use crate::ui::onboarding::needs_onboarding;
use crate::ui::search::normalize_query;
use crate::ui::settings::{read_settings, write_settings, LinuxSettings};

pub fn run() {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

struct AppState {
    core: TaskManagerCore,
    tasks: RefCell<Vec<Task>>,
    selected_task_id: RefCell<Option<Uuid>>,
    active_filter: RefCell<TaskFilterState>,
    search_query: RefCell<String>,
    list: gtk::ListBox,
    list_heading: gtk::Label,
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
        let result = if query.is_empty() {
            self.core.list_tasks(
                self.active_filter.borrow().to_filter(now_ms()),
                default_sort(),
            )
        } else {
            self.core.search_tasks(query)
        };

        match result {
            Ok(tasks) => {
                let task_count = tasks.len();
                self.tasks.replace(tasks);
                self.list_heading.set_text(&format!(
                    "{} · {task_count}",
                    self.active_filter.borrow().label()
                ));
                self.render_list();
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
                TaskStatus::InProgress => "◐",
                TaskStatus::Inbox => "○",
            }));
            status_dot.add_css_class("status-dot");
            status_dot.set_valign(gtk::Align::Start);

            let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
            text.set_hexpand(true);
            let title = gtk::Label::new(Some(&task.title));
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.add_css_class("task-title");
            let summary = gtk::Label::new(Some(&format_task_summary(task)));
            summary.set_xalign(0.0);
            summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
            summary.add_css_class("task-summary");

            text.append(&title);
            text.append(&summary);
            container.append(&status_dot);
            container.append(&text);
            row.set_child(Some(&container));
            self.list.append(&row);
        }
    }

    fn select_task(self: &Rc<Self>, task_id: Uuid) {
        self.selected_task_id.replace(Some(task_id));
        match self.core.get_task(task_id) {
            Ok(task) => self.show_task(&task),
            Err(error) => self.toast(format!("Failed to open task: {error}")),
        }
    }

    fn show_task(&self, task: &Task) {
        self.title_entry.set_text(&task.title);
        self.body_view.buffer().set_text(&task.body);
        self.status_combo.set_active(Some(match task.status {
            TaskStatus::Inbox => 0,
            TaskStatus::InProgress => 1,
            TaskStatus::Done => 2,
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

    fn save_selected(self: &Rc<Self>) {
        let Some(task_id) = *self.selected_task_id.borrow() else {
            return;
        };
        let buffer = self.body_view.buffer();
        let body = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();
        let status = match self.status_combo.active() {
            Some(1) => TaskStatus::InProgress,
            Some(2) => TaskStatus::Done,
            _ => TaskStatus::Inbox,
        };
        let tags = self
            .tags_entry
            .text()
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        let patch = TaskPatch {
            title: Some(self.title_entry.text().to_string()),
            body: Some(body),
            status: Some(status),
            tags: Some(tags),
            ..TaskPatch::default()
        };

        match self.core.update_task(task_id, patch) {
            Ok(task) => {
                self.show_task(&task);
                self.load_tasks();
            }
            Err(error) => self.toast(format!("Failed to save task: {error}")),
        }
    }

    fn delete_selected(self: &Rc<Self>) {
        let Some(task_id) = *self.selected_task_id.borrow() else {
            return;
        };
        match self.core.delete_task(task_id) {
            Ok(()) => {
                self.selected_task_id.replace(None);
                self.title_entry.set_text("");
                self.body_view.buffer().set_text("");
                self.tags_entry.set_text("");
                self.load_tasks();
            }
            Err(error) => self.toast(format!("Failed to delete task: {error}")),
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
    let new_button = gtk::Button::with_label("＋ New To-Do");
    new_button.add_css_class("suggested-action");
    let save_button = gtk::Button::with_label("Save");
    let delete_button = gtk::Button::with_label("Delete");
    delete_button.add_css_class("destructive-action");
    header.pack_start(&new_button);
    header.pack_end(&delete_button);
    header.pack_end(&save_button);

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search"));
    search.add_css_class("sidebar-search");

    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 14);
    sidebar.add_css_class("tsk-sidebar");
    sidebar.set_margin_top(18);
    sidebar.set_margin_bottom(18);
    sidebar.set_margin_start(18);
    sidebar.set_margin_end(18);

    let app_label = gtk::Label::new(Some("tsk"));
    app_label.set_xalign(0.0);
    app_label.add_css_class("app-title");
    sidebar.append(&app_label);
    sidebar.append(&search);

    let filter_list = gtk::ListBox::new();
    for filter in [
        TaskFilterState::All,
        TaskFilterState::Inbox,
        TaskFilterState::InProgress,
        TaskFilterState::Done,
        TaskFilterState::DueSoon,
    ] {
        let row = gtk::ListBoxRow::new();
        row.add_css_class("sidebar-row");
        row.set_widget_name(filter.label());
        let label = gtk::Label::new(Some(filter.label()));
        label.set_xalign(0.0);
        label.set_margin_top(8);
        label.set_margin_bottom(8);
        label.set_margin_start(10);
        label.set_margin_end(10);
        row.set_child(Some(&label));
        filter_list.append(&row);
    }
    sidebar.append(&filter_list);

    let list_heading = gtk::Label::new(Some("All · 0"));
    list_heading.set_xalign(0.0);
    list_heading.add_css_class("pane-title");

    let task_list = gtk::ListBox::new();
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
    list_stack.set_child(Some(&task_list));
    list_stack.add_overlay(&empty_state);
    let scrolled_list = gtk::ScrolledWindow::builder()
        .min_content_width(340)
        .child(&list_stack)
        .build();

    let list_pane = gtk::Box::new(gtk::Orientation::Vertical, 12);
    list_pane.add_css_class("tsk-list-pane");
    list_pane.set_margin_top(18);
    list_pane.set_margin_bottom(18);
    list_pane.set_margin_start(18);
    list_pane.set_margin_end(18);
    list_pane.append(&list_heading);
    list_pane.append(&scrolled_list);

    let title_entry = gtk::Entry::new();
    title_entry.set_placeholder_text(Some("What do you want to do?"));
    title_entry.add_css_class("editor-title");
    let body_view = gtk::TextView::new();
    body_view.set_vexpand(true);
    body_view.set_wrap_mode(gtk::WrapMode::Word);
    body_view.add_css_class("editor-notes");
    let status_combo = gtk::ComboBoxText::new();
    status_combo.append_text("Inbox");
    status_combo.append_text("In Progress");
    status_combo.append_text("Done");
    status_combo.set_active(Some(0));
    let tags_entry = gtk::Entry::new();
    tags_entry.set_placeholder_text(Some("Tags, comma separated"));

    let editor = gtk::Box::new(gtk::Orientation::Vertical, 16);
    editor.add_css_class("tsk-editor");
    editor.set_margin_top(28);
    editor.set_margin_bottom(28);
    editor.set_margin_start(28);
    editor.set_margin_end(28);

    let details = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    details.append(&status_combo);
    details.append(&tags_entry);

    let notes_frame = gtk::Frame::new(None);
    notes_frame.add_css_class("notes-card");
    notes_frame.set_vexpand(true);
    notes_frame.set_child(Some(&body_view));

    editor.append(&title_entry);
    editor.append(&details);
    editor.append(&notes_frame);

    let content = gtk::Paned::new(gtk::Orientation::Horizontal);
    let left = gtk::Paned::new(gtk::Orientation::Horizontal);
    left.set_start_child(Some(&sidebar));
    left.set_end_child(Some(&list_pane));
    left.set_resize_start_child(false);
    content.set_start_child(Some(&left));
    content.set_end_child(Some(&editor));

    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.append(&header);
    page.append(&content);

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&page));
    window.set_content(Some(&toast_overlay));

    let state = Rc::new(AppState {
        core,
        tasks: RefCell::new(Vec::new()),
        selected_task_id: RefCell::new(None),
        active_filter: RefCell::new(TaskFilterState::All),
        search_query: RefCell::new(String::new()),
        list: task_list,
        list_heading,
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
    save_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| state.save_selected()
    });
    delete_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| state.delete_selected()
    });
    search.connect_search_changed({
        let state = Rc::clone(&state);
        move |entry| {
            state.search_query.replace(normalize_query(&entry.text()));
            state.load_tasks();
        }
    });
    filter_list.connect_row_selected({
        let state = Rc::clone(&state);
        move |_, row| {
            let Some(row) = row else {
                return;
            };
            let filter = match row.index() {
                1 => TaskFilterState::Inbox,
                2 => TaskFilterState::InProgress,
                3 => TaskFilterState::Done,
                4 => TaskFilterState::DueSoon,
                _ => TaskFilterState::All,
            };
            state.active_filter.replace(filter);
            state.load_tasks();
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

    state.load_tasks();
    window.present();
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        r#"
        .tsk-sidebar {
            background: color-mix(in srgb, @window_bg_color 90%, @accent_color 10%);
        }
        .app-title {
            font-size: 26px;
            font-weight: 800;
            letter-spacing: -0.04em;
            color: @accent_color;
        }
        .sidebar-search {
            border-radius: 12px;
        }
        .sidebar-row {
            border-radius: 10px;
            margin: 2px 0;
        }
        .pane-title {
            font-size: 28px;
            font-weight: 800;
            letter-spacing: -0.04em;
        }
        .task-list {
            background: transparent;
        }
        .task-row {
            border-radius: 14px;
            margin: 4px 0;
            background: color-mix(in srgb, @card_bg_color 90%, transparent);
        }
        .task-row:hover {
            background: color-mix(in srgb, @card_bg_color 84%, @accent_color 16%);
        }
        .status-dot {
            color: @accent_color;
            font-size: 22px;
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
            font-size: 30px;
            font-weight: 800;
            letter-spacing: -0.04em;
            border-radius: 14px;
            padding: 10px 12px;
        }
        .notes-card {
            border-radius: 16px;
            background: @card_bg_color;
            padding: 12px;
        }
        .editor-notes {
            font-size: 15px;
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
