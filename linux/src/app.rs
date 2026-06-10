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
                self.tasks.replace(tasks);
                self.render_list();
            }
            Err(error) => self.toast(format!("Failed to load tasks: {error}")),
        }
    }

    fn render_list(self: &Rc<Self>) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }

        for task in self.tasks.borrow().iter() {
            let row = gtk::ListBoxRow::new();
            row.set_selectable(true);
            row.set_activatable(true);
            row.set_widget_name(&task.id.to_string());

            let container = gtk::Box::new(gtk::Orientation::Vertical, 4);
            container.set_margin_top(8);
            container.set_margin_bottom(8);
            container.set_margin_start(12);
            container.set_margin_end(12);

            let title = gtk::Label::new(Some(&task.title));
            title.set_xalign(0.0);
            title.add_css_class("heading");
            let summary = gtk::Label::new(Some(&format_task_summary(task)));
            summary.set_xalign(0.0);
            summary.add_css_class("dim-label");

            container.append(&title);
            container.append(&summary);
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

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(APP_NAME)
        .default_width(1000)
        .default_height(700)
        .build();

    let header = adw::HeaderBar::new();
    let new_button = gtk::Button::with_label("New");
    let save_button = gtk::Button::with_label("Save");
    let delete_button = gtk::Button::with_label("Delete");
    header.pack_start(&new_button);
    header.pack_end(&delete_button);
    header.pack_end(&save_button);

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search tasks"));

    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 8);
    sidebar.set_margin_top(12);
    sidebar.set_margin_bottom(12);
    sidebar.set_margin_start(12);
    sidebar.set_margin_end(12);
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
        row.set_widget_name(filter.label());
        row.set_child(Some(&gtk::Label::new(Some(filter.label()))));
        filter_list.append(&row);
    }
    sidebar.append(&filter_list);

    let task_list = gtk::ListBox::new();
    task_list.set_selection_mode(gtk::SelectionMode::Single);
    let scrolled_list = gtk::ScrolledWindow::builder()
        .min_content_width(300)
        .child(&task_list)
        .build();

    let title_entry = gtk::Entry::new();
    title_entry.set_placeholder_text(Some("Title"));
    let body_view = gtk::TextView::new();
    body_view.set_vexpand(true);
    body_view.set_wrap_mode(gtk::WrapMode::Word);
    let status_combo = gtk::ComboBoxText::new();
    status_combo.append_text("Inbox");
    status_combo.append_text("In Progress");
    status_combo.append_text("Done");
    status_combo.set_active(Some(0));
    let tags_entry = gtk::Entry::new();
    tags_entry.set_placeholder_text(Some("Tags, comma separated"));

    let editor = gtk::Box::new(gtk::Orientation::Vertical, 12);
    editor.set_margin_top(12);
    editor.set_margin_bottom(12);
    editor.set_margin_start(12);
    editor.set_margin_end(12);
    editor.append(&title_entry);
    editor.append(&status_combo);
    editor.append(&tags_entry);
    editor.append(&body_view);

    let content = gtk::Paned::new(gtk::Orientation::Horizontal);
    let left = gtk::Paned::new(gtk::Orientation::Horizontal);
    left.set_start_child(Some(&sidebar));
    left.set_end_child(Some(&scrolled_list));
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

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}
