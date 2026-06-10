use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use taskmanager_core::{Task, TaskFilter, TaskManagerCore, TaskStatus};
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
    core: TaskManagerCore,
    tasks: RefCell<Vec<Task>>,
    active_filter: RefCell<TaskFilterState>,
    search_query: RefCell<String>,
    list: gtk::ListBox,
    list_heading: gtk::Label,
    filter_count_labels: Vec<gtk::Label>,
    tag_box: gtk::Box,
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
                let view = *self.active_filter.borrow();
                let now = now_ms();
                let tasks = tasks
                    .into_iter()
                    .filter(|task| task_matches_view(task, view, now))
                    .collect::<Vec<_>>();
                let task_count = tasks.len();
                self.tasks.replace(tasks);
                self.list_heading.set_text(&format!(
                    "{} · {task_count}",
                    self.active_filter.borrow().label()
                ));
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

    fn refresh_sidebar_metadata(&self) {
        let Ok(tasks) = self.core.list_tasks(TaskFilter::default(), default_sort()) else {
            return;
        };
        let now = now_ms();
        for (label, filter) in self
            .filter_count_labels
            .iter()
            .zip(sidebar_filter_order().iter().copied())
        {
            label.set_text(&count_for_filter(&tasks, filter, now).to_string());
        }
        self.render_tag_rows(&tasks);
    }

    fn render_tag_rows(&self, tasks: &[Task]) {
        while let Some(row) = self.tag_box.first_child() {
            self.tag_box.remove(&row);
        }

        let mut counts = BTreeMap::<String, usize>::new();
        for task in tasks {
            for tag in &task.tags {
                *counts.entry(tag.to_owned()).or_default() += 1;
            }
        }

        for (tag, count) in counts {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("sidebar-static-row");
            let name = gtk::Label::new(Some(&format!("# {tag}")));
            name.set_xalign(0.0);
            name.set_hexpand(true);
            let count_label = gtk::Label::new(Some(&count.to_string()));
            count_label.add_css_class("sidebar-count");
            row.append(&name);
            row.append(&count_label);
            self.tag_box.append(&row);
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
    filter_list.set_selection_mode(gtk::SelectionMode::None);
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

        let label = gtk::Label::new(Some(sidebar_filter_title(filter)));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        let count_label = gtk::Label::new(Some("0"));
        count_label.add_css_class("sidebar-count");

        row_box.append(&label);
        row_box.append(&count_label);
        row.set_child(Some(&row_box));
        filter_count_labels.push(count_label);
        filter_list.append(&row);
    }
    sidebar.append(&filter_list);

    let tag_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    sidebar.append(&tag_box);

    let list_heading = gtk::Label::new(Some("Today · 0"));
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
    list_pane.set_hexpand(true);
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
    status_combo.append_text("Open");
    status_combo.append_text("Done");
    status_combo.set_active(Some(0));
    let tags_entry = gtk::Entry::new();
    tags_entry.set_placeholder_text(Some("Tags, comma separated"));

    let main_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    main_area.set_hexpand(true);
    main_area.append(&header);
    main_area.append(&list_pane);

    let page = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    page.append(&sidebar);
    page.append(&main_area);

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&page));
    window.set_content(Some(&toast_overlay));

    let state = Rc::new(AppState {
        core,
        tasks: RefCell::new(Vec::new()),
        active_filter: RefCell::new(TaskFilterState::Today),
        search_query: RefCell::new(String::new()),
        list: task_list,
        list_heading,
        filter_count_labels,
        tag_box,
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
    search.connect_search_changed({
        let state = Rc::clone(&state);
        move |entry| {
            state.search_query.replace(normalize_query(&entry.text()));
            state.load_tasks();
        }
    });
    filter_list.connect_row_activated({
        let state = Rc::clone(&state);
        move |_, row| {
            let filter = match row.index() {
                0 => TaskFilterState::Today,
                1 => TaskFilterState::Upcoming,
                2 => TaskFilterState::NoDueDate,
                3 => TaskFilterState::Done,
                _ => TaskFilterState::Today,
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

fn sidebar_filter_order() -> [TaskFilterState; 4] {
    [
        TaskFilterState::Today,
        TaskFilterState::Upcoming,
        TaskFilterState::NoDueDate,
        TaskFilterState::Done,
    ]
}

fn sidebar_filter_title(filter: TaskFilterState) -> &'static str {
    match filter {
        TaskFilterState::Today => "Today",
        TaskFilterState::Upcoming => "Upcoming",
        TaskFilterState::NoDueDate => "Anytime",
        TaskFilterState::Done => "Done",
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
        .sidebar-list row:selected:hover,
        .sidebar-list row:selected label,
        .sidebar-row:selected,
        .sidebar-row:selected:hover {
            background: transparent;
            color: @window_fg_color;
        }
        .sidebar-static-row {
            color: @window_fg_color;
            padding: 4px 10px;
            border-radius: 6px;
        }
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
