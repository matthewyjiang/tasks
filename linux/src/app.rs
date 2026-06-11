use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use taskmanager_core::{
    init_account, Task, TaskFilter, TaskList, TaskManagerCore, TaskPatch, TaskStatus,
};
use uuid::Uuid;

use crate::keybindings::{install_keybindings, KeybindingActions};
use crate::paths::{resolve_paths, APP_ID, APP_NAME};
use crate::platform::LinuxPlatform;
use crate::style::install_css;
use crate::sync::{linux_sync_configured, run_linux_sync, LinuxSyncSummary};
use crate::task_format::{
    count_for_filter, format_due_date_value, markdown_to_pango_markup, parse_due_date_entry,
    parse_tags, sidebar_filter_icon, sidebar_filter_icon_class, sidebar_filter_order,
    sidebar_filter_title,
};
use crate::task_model::{default_sort, task_matches_view, TaskFilterState};
use crate::time::now_ms;
use crate::ui::floating_panel::{
    hide_floating_panel, resize_settings_panel, resize_task_editor_panel, show_floating_panel,
};
use crate::ui::layout::{
    FLOATING_PANEL_FADE_MS, SETTINGS_PANEL_MIN_HEIGHT, SETTINGS_PANEL_MIN_WIDTH,
    TASK_EDITOR_BODY_HEIGHT, TASK_EDITOR_INNER_PADDING,
};
use crate::ui::onboarding::needs_onboarding;
use crate::ui::search::{
    build_move_list_panel, move_list_result_row, normalize_query, regex_from_query,
    regex_matches_task, task_search_result_row,
};
use crate::ui::settings::{read_settings, write_settings, LinuxSettings};
use crate::ui::settings_panel::{apply_theme_choice, show_settings_panel};
use crate::ui::sidebar::user_list_row;
use crate::ui::sync_setup::{build_sync_setup_panel, configure_sync_auth, sync_auth_configured};
use crate::ui::task_editor::build_task_editor_panel;
use crate::ui::task_row::{task_row, TaskRowActions};
use crate::ui::widgets::{
    font_awesome_label, icon_button, icon_text_label, text_buffer_string, update_entry_width,
};

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
    due_entry: gtk::Entry,
    list_combo: gtk::ComboBoxText,
    markdown_preview: gtk::Label,
    body_stack: gtk::Stack,
    editor_panel: gtk::Box,
    current_editing_task_id: RefCell<Option<Uuid>>,
    move_list_panel: gtk::Box,
    move_list_search: gtk::SearchEntry,
    move_list_results: gtk::ListBox,
    moving_task_id: RefCell<Option<Uuid>>,
    toast_overlay: adw::ToastOverlay,
    db_path: PathBuf,
    settings_path: PathBuf,
    sync_button: gtk::Button,
    sync_stack: gtk::Stack,
    sync_icon: gtk::Label,
    sync_activity: gtk::DrawingArea,
    sync_angle: Rc<Cell<f64>>,
    sync_in_progress: RefCell<bool>,
    sync_pending: RefCell<bool>,
}

impl AppState {
    fn request_sync(self: &Rc<Self>) {
        self.request_sync_with_feedback(false);
    }

    fn request_manual_sync(self: &Rc<Self>) {
        self.request_sync_with_feedback(true);
    }

    fn request_sync_with_feedback(self: &Rc<Self>, notify: bool) {
        if *self.sync_in_progress.borrow() {
            if !notify {
                self.sync_pending.replace(true);
            }
            return;
        }
        if !linux_sync_configured(&self.settings_path) {
            if notify {
                self.toast("Sync is not configured yet.".to_owned());
            }
            return;
        }
        self.set_sync_running(true);
        self.sync_in_progress.replace(true);
        let db_path = self.db_path.clone();
        let settings_path = self.settings_path.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = run_linux_sync(&db_path, &settings_path);
            let _ = sender.send(result);
        });
        let state = Rc::clone(self);
        gtk::glib::timeout_add_local(
            std::time::Duration::from_millis(120),
            move || match receiver.try_recv() {
                Ok(result) => {
                    state.sync_in_progress.replace(false);
                    state.set_sync_running(false);
                    match result {
                        Ok(summary) => {
                            if summary.changed() {
                                state.load_tasks();
                            }
                            if notify {
                                state.toast(format_sync_status(&summary));
                            }
                        }
                        Err(error) => state.toast(format!("Sync failed: {error}")),
                    }
                    if state.sync_pending.replace(false) {
                        state.request_sync();
                    }
                    gtk::glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    state.sync_in_progress.replace(false);
                    state.set_sync_running(false);
                    state.toast("Sync failed: worker disconnected".to_owned());
                    gtk::glib::ControlFlow::Break
                }
            },
        );
    }

    fn load_tasks(self: &Rc<Self>) {
        let query = self.search_query.borrow().clone();
        let selected_list_id = *self.selected_list_id.borrow();
        let show_completed = self
            .core
            .vault_settings()
            .map(|settings| settings.show_completed)
            .unwrap_or(false);
        let result = if query.is_empty() {
            let mut filter = if selected_list_id.is_some() {
                TaskFilter::default()
            } else {
                self.active_filter.borrow().to_filter(now_ms())
            };
            if show_completed
                && selected_list_id.is_none()
                && filter.status == Some(TaskStatus::Open)
            {
                filter.status = None;
            }
            filter.project_id = selected_list_id;
            self.core.list_tasks(filter, default_sort())
        } else {
            match regex_from_query(&query) {
                Ok(regex) => self
                    .core
                    .list_tasks(TaskFilter::default(), default_sort())
                    .map(|tasks| {
                        tasks
                            .into_iter()
                            .filter(|task| regex_matches_task(&regex, task))
                            .collect()
                    }),
                Err(error) => {
                    self.toast(format!("Invalid regex: {error}"));
                    Ok(Vec::new())
                }
            }
        };

        match result {
            Ok(tasks) => {
                let view = *self.active_filter.borrow();
                let now = now_ms();
                let tasks = tasks
                    .into_iter()
                    .filter(|task| {
                        selected_list_id.is_some()
                            || task_matches_view(task, view, now, show_completed)
                    })
                    .collect::<Vec<_>>();
                self.tasks.replace(tasks);
                if let Some(list_id) = selected_list_id {
                    if let Some(list) = self
                        .user_lists
                        .borrow()
                        .iter()
                        .find(|list| list.id == list_id)
                    {
                        self.list_heading.set_visible(true);
                        self.list_heading.set_text(&list.name);
                        self.list_name_entry.set_visible(false);
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

        let actions = TaskRowActions {
            toggle_status: Rc::new({
                let state = Rc::clone(self);
                move |task_id, status| {
                    let patch = TaskPatch {
                        status: Some(status),
                        ..TaskPatch::default()
                    };
                    if let Err(error) = state.core.update_task(task_id, patch) {
                        state.toast(format!("Failed to update task: {error}"));
                    } else {
                        state.request_sync();
                    }
                    state.load_tasks();
                }
            }),
            move_task: Rc::new({
                let state = Rc::clone(self);
                move |task_id| state.show_move_list_panel(task_id)
            }),
            delete_task: Rc::new({
                let state = Rc::clone(self);
                move |task_id| {
                    if let Err(error) = state.core.delete_task(task_id) {
                        state.toast(format!("Failed to delete task: {error}"));
                    } else {
                        state.request_sync();
                    }
                    state.load_tasks();
                }
            }),
        };

        for task in self.tasks.borrow().iter() {
            self.list.append(&task_row(task, &actions));
        }
    }

    fn select_task(self: &Rc<Self>, task_id: Uuid) {
        self.select_task_with_focus(task_id, false);
    }

    fn select_task_with_focus(self: &Rc<Self>, task_id: Uuid, focus_title: bool) {
        match self.core.get_task(task_id) {
            Ok(task) => self.show_task(&task, focus_title),
            Err(error) => self.toast(format!("Failed to open task: {error}")),
        }
    }

    fn show_task(&self, task: &Task, focus_title: bool) {
        self.current_editing_task_id.replace(Some(task.id));
        self.title_entry.set_text(&task.title);
        self.body_view.buffer().set_text(&task.body);
        self.markdown_preview
            .set_markup(&markdown_to_pango_markup(&task.body));
        self.body_stack.set_visible_child_name("preview");
        self.status_combo.set_active(Some(match task.status {
            TaskStatus::Open => 0,
            TaskStatus::Done => 1,
        }));
        self.tags_entry.set_text(&task.tags.join(", "));
        self.due_entry
            .set_text(&task.due_at.map(format_due_date_value).unwrap_or_default());
        self.list_combo.set_active_id(Some(
            task.project_id
                .map(|id| id.to_string())
                .as_deref()
                .unwrap_or("inbox"),
        ));
        show_floating_panel(&self.editor_panel);
        if focus_title {
            self.title_entry.grab_focus();
        }
    }

    fn hide_task_editor(&self) {
        self.current_editing_task_id.replace(None);
        hide_floating_panel(&self.editor_panel);
    }

    fn show_move_list_panel(self: &Rc<Self>, task_id: Uuid) {
        self.moving_task_id.replace(Some(task_id));
        self.move_list_search.set_text("");
        render_move_list_results(self, "");
        show_floating_panel(&self.move_list_panel);
        self.move_list_search.grab_focus();
    }

    fn hide_move_list_panel(&self) {
        self.moving_task_id.replace(None);
        hide_floating_panel(&self.move_list_panel);
    }

    fn save_task_editor(self: &Rc<Self>) {
        let Some(task_id) = *self.current_editing_task_id.borrow() else {
            return;
        };
        let body = text_buffer_string(&self.body_view.buffer());
        let due_at = match parse_due_date_entry(&self.due_entry.text()) {
            Ok(due_at) => due_at,
            Err(error) => {
                self.toast(error);
                return;
            }
        };
        let project_id = match self.list_combo.active_id().as_deref() {
            Some("inbox") | None => Some(None),
            Some(id) => match Uuid::parse_str(id) {
                Ok(id) => Some(Some(id)),
                Err(error) => {
                    self.toast(format!("Invalid list id: {error}"));
                    return;
                }
            },
        };
        let patch = TaskPatch {
            title: Some(self.title_entry.text().trim().to_owned()),
            body: Some(body),
            status: Some(if self.status_combo.active() == Some(1) {
                TaskStatus::Done
            } else {
                TaskStatus::Open
            }),
            due_at: Some(due_at),
            project_id,
            tags: Some(parse_tags(&self.tags_entry.text())),
        };
        if let Err(error) = self.core.update_task(task_id, patch) {
            self.toast(format!("Failed to save task: {error}"));
            return;
        }
        self.hide_task_editor();
        self.load_tasks();
        self.request_sync();
    }

    fn create_task(self: &Rc<Self>) {
        match self
            .core
            .create_task("New task".to_owned(), String::new(), None)
        {
            Ok(task) => {
                self.selected_list_id.replace(None);
                self.active_filter.replace(TaskFilterState::Inbox);
                self.load_tasks();
                self.select_task_with_focus(task.id, true);
                self.request_sync();
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
        self.refresh_editor_list_choices(&lists);

        for list in lists {
            self.user_list_box.append(&user_list_row(&list));
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

    fn show_list_rename_editor(&self) {
        self.list_heading.set_visible(false);
        self.list_name_entry.set_visible(true);
        self.list_rename_button.set_visible(true);
        update_entry_width(&self.list_name_entry);
        self.list_name_entry.grab_focus();
        self.list_name_entry.select_region(0, -1);
    }

    fn rename_selected_list(self: &Rc<Self>) -> bool {
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
                self.render_user_lists();
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

    fn create_list(self: &Rc<Self>) {
        match self.core.create_list("New List".to_owned()) {
            Ok(list) => {
                self.selected_list_id.replace(Some(list.id));
                self.active_filter.replace(TaskFilterState::Upcoming);
                self.render_user_lists();
                self.load_tasks();
                self.show_list_rename_editor();
                self.request_sync();
            }
            Err(error) => self.toast(format!("Failed to create list: {error}")),
        }
    }

    fn toast(&self, message: String) {
        self.toast_overlay.add_toast(adw::Toast::new(&message));
    }

    fn set_sync_running(&self, running: bool) {
        self.sync_button.set_sensitive(!running);
        self.sync_icon.set_visible(!running);
        self.sync_activity.set_visible(running);
        if running {
            self.sync_stack.set_visible_child_name("activity");
            self.sync_button.set_tooltip_text(Some("Syncing…"));
        } else {
            self.sync_stack.set_visible_child_name("icon");
            self.sync_button.set_tooltip_text(Some("Sync now"));
        }
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
        if let Err(error) = init_account(&platform) {
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

    install_css(FLOATING_PANEL_FADE_MS, TASK_EDITOR_INNER_PADDING);

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
    list_heading.set_valign(gtk::Align::Center);
    list_heading.add_css_class("pane-title");

    let list_name_entry = gtk::Entry::new();
    list_name_entry.set_hexpand(false);
    list_name_entry.set_valign(gtk::Align::Center);
    update_entry_width(&list_name_entry);
    list_name_entry.connect_changed(update_entry_width);
    list_name_entry.add_css_class("pane-title");
    list_name_entry.add_css_class("rename-entry");
    list_name_entry.add_css_class("flat");
    list_name_entry.set_visible(false);

    let list_rename_button = gtk::Button::with_label("✓");
    list_rename_button.set_valign(gtk::Align::Center);
    list_rename_button.add_css_class("confirm-button");
    list_rename_button.set_visible(false);

    let list_actions_button = gtk::MenuButton::new();
    list_actions_button.set_label("⋯");
    list_actions_button.set_halign(gtk::Align::End);
    list_actions_button.set_valign(gtk::Align::Center);
    list_actions_button.add_css_class("flat");
    list_actions_button.set_visible(false);
    let list_actions_popover = gtk::Popover::new();
    let list_actions_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let list_edit_button = gtk::Button::with_label("Rename List");
    list_edit_button.add_css_class("flat");
    let list_delete_button = gtk::Button::with_label("Delete List");
    list_delete_button.add_css_class("flat");
    list_actions_box.append(&list_edit_button);
    list_actions_box.append(&list_delete_button);
    list_actions_popover.set_child(Some(&list_actions_box));
    list_actions_button.set_popover(Some(&list_actions_popover));

    let page_title_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    page_title_spacer.set_hexpand(true);

    let page_title = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    page_title.set_hexpand(true);
    page_title.set_height_request(40);
    page_title.set_valign(gtk::Align::Center);
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
    body_view.set_size_request(-1, TASK_EDITOR_BODY_HEIGHT);
    body_view.add_css_class("editor-notes");
    let markdown_preview = gtk::Label::new(None);
    markdown_preview.set_xalign(0.0);
    markdown_preview.set_yalign(0.0);
    markdown_preview.set_wrap(true);
    markdown_preview.set_selectable(false);
    markdown_preview.set_tooltip_text(Some("Click to edit notes"));
    markdown_preview.add_css_class("markdown-preview");
    let status_combo = gtk::ComboBoxText::new();
    status_combo.append_text("Open");
    status_combo.append_text("Done");
    status_combo.set_active(Some(0));
    let tags_entry = gtk::Entry::new();
    tags_entry.set_placeholder_text(Some("Tags, comma separated"));
    let due_entry = gtk::Entry::new();
    due_entry.set_placeholder_text(Some("Due date"));
    let list_combo = gtk::ComboBoxText::new();
    list_combo.append(Some("inbox"), "Inbox");
    if let Ok(lists) = core.list_task_lists() {
        for list in lists {
            list_combo.append(Some(&list.id.to_string()), &list.name);
        }
    }

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
    let sync_button = gtk::Button::new();
    sync_button.add_css_class("flat");
    sync_button.set_hexpand(true);
    sync_button.set_tooltip_text(Some("Sync now"));
    let sync_stack = gtk::Stack::new();
    let sync_icon = font_awesome_label("\u{f021}");
    let sync_angle = Rc::new(Cell::new(0.0));
    let sync_activity = build_sync_activity_icon(Rc::clone(&sync_angle));
    sync_activity.set_visible(false);
    sync_stack.add_named(&sync_icon, Some("icon"));
    sync_stack.add_named(&sync_activity, Some("activity"));
    sync_button.set_child(Some(&sync_stack));
    content_bottom_bar.append(&search_button);
    content_bottom_bar.append(&bottom_new_button);
    content_bottom_bar.append(&sync_button);

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
    search_panel.set_valign(gtk::Align::Start);
    search_panel.set_margin_top(120);
    search_panel.set_opacity(0.0);
    search_panel.set_visible(false);
    let overlay_search = gtk::SearchEntry::new();
    overlay_search.set_placeholder_text(Some("Search tasks"));
    overlay_search.add_css_class("search-panel-entry");
    let search_results = gtk::ListBox::new();
    search_results.add_css_class("search-results");
    search_results.set_selection_mode(gtk::SelectionMode::None);
    search_results.set_visible(false);
    search_panel.append(&overlay_search);
    search_panel.append(&search_results);

    let editor_widgets = build_task_editor_panel(
        &title_entry,
        &body_view,
        &markdown_preview,
        &status_combo,
        &list_combo,
        &due_entry,
        &tags_entry,
    );
    let editor_panel = editor_widgets.panel;
    let body_stack = editor_widgets.body_stack;
    let due_calendar = editor_widgets.due_calendar;
    let due_popover = editor_widgets.due_popover;
    let today_due_button = editor_widgets.today_due_button;
    let clear_due_button = editor_widgets.clear_due_button;

    let move_list_widgets = build_move_list_panel();
    let move_list_panel = move_list_widgets.panel;
    let move_list_search = move_list_widgets.search_entry;
    let move_list_results = move_list_widgets.results;

    let setup_widgets = build_sync_setup_panel(
        sync_auth_configured(&platform, &settings),
        &settings.server_url,
    );
    let setup_panel = setup_widgets.panel;
    let setup_server = setup_widgets.server_entry;
    let setup_email = setup_widgets.email_entry;
    let setup_password = setup_widgets.password_entry;
    let setup_status = setup_widgets.status_label;
    let setup_local = setup_widgets.local_button;
    let setup_login = setup_widgets.login_button;

    let settings_panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
    settings_panel.add_css_class("settings-panel");
    settings_panel.set_size_request(SETTINGS_PANEL_MIN_WIDTH, SETTINGS_PANEL_MIN_HEIGHT);
    settings_panel.set_halign(gtk::Align::Center);
    settings_panel.set_valign(gtk::Align::Center);
    settings_panel.set_opacity(0.0);
    settings_panel.set_visible(false);

    let page_click = gtk::GestureClick::new();
    page.add_controller(page_click.clone());

    let root_overlay = gtk::Overlay::new();
    root_overlay.set_child(Some(&page));
    root_overlay.add_overlay(&editor_panel);
    root_overlay.add_overlay(&move_list_panel);
    root_overlay.add_overlay(&search_panel);
    root_overlay.add_overlay(&settings_panel);
    root_overlay.add_overlay(&setup_panel);

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&root_overlay));
    window.set_content(Some(&toast_overlay));

    let keybindings = core.vault_settings().unwrap_or_default().keybindings;

    setup_local.connect_clicked({
        let setup_panel = setup_panel.clone();
        move |_| setup_panel.set_visible(false)
    });
    setup_login.connect_clicked({
        let settings_path = paths.settings_path.clone();
        let setup_panel = setup_panel.clone();
        move |_| {
            setup_status.set_text("Signing in…");
            let platform = LinuxPlatform::new();
            match configure_sync_auth(
                &platform,
                &settings_path,
                &setup_server.text(),
                &setup_email.text(),
                &setup_password.text(),
            ) {
                Ok(()) => {
                    setup_status.set_text("Sync configured.");
                    setup_panel.set_visible(false);
                }
                Err(error) => setup_status.set_text(&format!("Sync setup failed: {error}")),
            }
        }
    });

    let state = Rc::new(AppState {
        core: Rc::new(core),
        tasks: RefCell::new(Vec::new()),
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
        due_entry,
        list_combo,
        markdown_preview,
        body_stack,
        editor_panel,
        current_editing_task_id: RefCell::new(None),
        move_list_panel,
        move_list_search,
        move_list_results,
        moving_task_id: RefCell::new(None),
        toast_overlay,
        db_path: paths.database_path.clone(),
        settings_path: paths.settings_path.clone(),
        sync_button,
        sync_stack,
        sync_icon,
        sync_activity,
        sync_angle,
        sync_in_progress: RefCell::new(false),
        sync_pending: RefCell::new(false),
    });

    setup_login.connect_clicked({
        let state = Rc::clone(&state);
        move |_| state.request_sync()
    });

    due_calendar.connect_day_selected({
        let due_entry = state.due_entry.clone();
        let due_popover = due_popover.clone();
        move |calendar| {
            let date = calendar.date();
            due_entry.set_text(&format!(
                "{:04}-{:02}-{:02}",
                date.year(),
                date.month(),
                date.day_of_month()
            ));
            due_popover.popdown();
        }
    });
    today_due_button.connect_clicked({
        let due_entry = state.due_entry.clone();
        let due_calendar = due_calendar.clone();
        let due_popover = due_popover.clone();
        move |_| match gtk::glib::DateTime::now_local() {
            Ok(today) => {
                due_calendar.select_day(&today);
                due_entry.set_text(&format!(
                    "{:04}-{:02}-{:02}",
                    today.year(),
                    today.month(),
                    today.day_of_month()
                ));
                due_popover.popdown();
            }
            Err(error) => eprintln!("Failed to read local date: {error}"),
        }
    });
    clear_due_button.connect_clicked({
        let due_entry = state.due_entry.clone();
        move |_| due_entry.set_text("")
    });
    state.move_list_search.connect_search_changed({
        let state = Rc::clone(&state);
        move |entry| render_move_list_results(&state, &normalize_query(&entry.text()))
    });
    state.move_list_results.connect_row_activated({
        let state = Rc::clone(&state);
        move |_, row| {
            let Some(task_id) = *state.moving_task_id.borrow() else {
                return;
            };
            let project_id = if row.widget_name() == "inbox" {
                None
            } else {
                match Uuid::parse_str(&row.widget_name()) {
                    Ok(id) => Some(id),
                    Err(error) => {
                        state.toast(format!("Invalid list id: {error}"));
                        return;
                    }
                }
            };
            let patch = TaskPatch {
                project_id: Some(project_id),
                ..TaskPatch::default()
            };
            if let Err(error) = state.core.update_task(task_id, patch) {
                state.toast(format!("Failed to move task: {error}"));
            } else {
                state.request_sync();
            }
            state.hide_move_list_panel();
            state.load_tasks();
        }
    });

    state.body_view.buffer().connect_changed({
        let markdown_preview = state.markdown_preview.clone();
        move |buffer| {
            markdown_preview.set_markup(&markdown_to_pango_markup(&text_buffer_string(buffer)))
        }
    });
    let preview_click = gtk::GestureClick::new();
    preview_click.connect_released({
        let body_stack = state.body_stack.clone();
        let body_view = state.body_view.clone();
        move |_, _, _, _| {
            body_stack.set_visible_child_name("write");
            body_view.grab_focus();
        }
    });
    state.markdown_preview.add_controller(preview_click);
    let editor_click = gtk::GestureClick::new();
    editor_click.set_propagation_phase(gtk::PropagationPhase::Capture);
    editor_click.connect_pressed({
        let editor_panel = state.editor_panel.clone();
        let title_entry = state.title_entry.clone();
        let body_view = state.body_view.clone();
        move |_, _, x, y| {
            let Some(widget) = editor_panel.pick(x, y, gtk::PickFlags::DEFAULT) else {
                return;
            };
            if widget.is::<gtk::Entry>()
                || widget.is::<gtk::TextView>()
                || widget.ancestor(gtk::Entry::static_type()).is_some()
                || widget.ancestor(gtk::TextView::static_type()).is_some()
            {
                return;
            }
            title_entry.set_position(-1);
            body_view
                .buffer()
                .place_cursor(&body_view.buffer().end_iter());
            editor_panel.grab_focus();
        }
    });
    state.editor_panel.add_controller(editor_click);
    let body_focus = gtk::EventControllerFocus::new();
    body_focus.connect_leave({
        let body_stack = state.body_stack.clone();
        move |_| body_stack.set_visible_child_name("preview")
    });
    state.body_view.add_controller(body_focus);
    page_click.connect_pressed({
        let state = Rc::clone(&state);
        move |_, _, _, _| {
            if state.editor_panel.is_visible() {
                state.save_task_editor();
            }
            if state.move_list_panel.is_visible() {
                state.hide_move_list_panel();
            }
        }
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
    state.sync_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| state.request_manual_sync()
    });
    add_list_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| state.create_list()
    });
    settings_button.connect_clicked({
        let settings_panel = settings_panel.clone();
        let settings_path = paths.settings_path.clone();
        let core = Rc::clone(&state.core);
        let state = Rc::clone(&state);
        move |_| {
            let request_sync: Rc<dyn Fn()> = Rc::new({
                let state = Rc::clone(&state);
                move || {
                    state.request_sync();
                    state.load_tasks();
                }
            });
            show_settings_panel(
                &settings_panel,
                settings_path.clone(),
                Rc::clone(&core),
                Some(request_sync),
            )
        }
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
        }
    });
    state.list_name_entry.add_controller(list_name_focus);
    state.list_name_entry.connect_activate({
        let state = Rc::clone(&state);
        move |_| {
            if state.rename_selected_list() {
                state.list_name_entry.remove_css_class("renaming");
                state.list_heading.set_visible(true);
                state.list_name_entry.set_visible(false);
                state.list_rename_button.set_visible(false);
            }
        }
    });
    state.list_rename_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| {
            if state.rename_selected_list() {
                state.list_name_entry.remove_css_class("renaming");
                state.list_heading.set_visible(true);
                state.list_name_entry.set_visible(false);
                state.list_rename_button.set_visible(false);
            }
        }
    });
    list_edit_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| state.show_list_rename_editor()
    });
    list_delete_button.connect_clicked({
        let state = Rc::clone(&state);
        move |_| {
            let Some(list_id) = *state.selected_list_id.borrow() else {
                return;
            };
            if let Err(error) = state.core.delete_list(list_id) {
                state.toast(format!("Failed to delete list: {error}"));
            } else {
                state.request_sync();
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
            show_floating_panel(&search_panel);
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
                hide_floating_panel(&search_panel);
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
                        hide_floating_panel(&search_panel);
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
            state.request_sync();
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
                state.request_sync();
            }
        }
    });
    state.list.connect_row_activated({
        let state = Rc::clone(&state);
        move |_, row| {
            if let Ok(task_id) = Uuid::parse_str(&row.widget_name()) {
                state.select_task(task_id);
            }
        }
    });

    resize_task_editor_panel(&window, &state.editor_panel);
    window.add_tick_callback({
        let window = window.clone();
        let editor_panel = state.editor_panel.clone();
        let settings_panel = settings_panel.clone();
        let state = Rc::clone(&state);
        move |_, _| {
            resize_task_editor_panel(&window, &editor_panel);
            resize_settings_panel(&window, &settings_panel);
            if *state.sync_in_progress.borrow() {
                state
                    .sync_angle
                    .set((state.sync_angle.get() + 0.10) % std::f64::consts::TAU);
                state.sync_activity.queue_draw();
            }
            gtk::glib::ControlFlow::Continue
        }
    });

    install_keybindings(
        &window,
        &keybindings,
        KeybindingActions {
            create_task: Rc::clone(&create_task_action),
            open_search: Rc::clone(&open_search_action),
            close_overlay: Rc::new({
                let search_panel = search_panel.clone();
                let state = Rc::clone(&state);
                move || {
                    hide_floating_panel(&search_panel);
                    if state.editor_panel.is_visible() {
                        state.save_task_editor();
                    }
                    if state.move_list_panel.is_visible() {
                        state.hide_move_list_panel();
                    }
                }
            }),
            delete_task: Rc::new({
                let state = Rc::clone(&state);
                move || delete_selected_task(&state)
            }),
            toggle_done: Rc::new({
                let state = Rc::clone(&state);
                move || toggle_selected_task_done(&state)
            }),
        },
    );

    if let Some(row) = filter_list.row_at_index(0) {
        filter_list.select_row(Some(&row));
    }
    state.load_tasks();
    state.request_sync();
    window.present();
}

fn build_sync_activity_icon(angle: Rc<Cell<f64>>) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(18);
    area.set_content_height(18);
    area.set_draw_func(move |_, cr, width, height| {
        let size = f64::from(width.min(height));
        let center = size / 2.0;
        let radius = size * 0.31;
        cr.translate(f64::from(width) / 2.0, f64::from(height) / 2.0);
        cr.rotate(angle.get());
        cr.set_source_rgba(0.50, 0.58, 0.68, 1.0);
        cr.set_line_width(1.8);
        cr.set_line_cap(gtk::cairo::LineCap::Round);
        cr.arc(0.0, 0.0, radius, -0.35, std::f64::consts::TAU - 0.95);
        let _ = cr.stroke();

        let tip_angle: f64 = -0.35;
        let tip_x = radius * tip_angle.cos();
        let tip_y = radius * tip_angle.sin();
        cr.move_to(tip_x, tip_y);
        cr.line_to(tip_x - center * 0.18, tip_y - center * 0.02);
        cr.move_to(tip_x, tip_y);
        cr.line_to(tip_x - center * 0.06, tip_y + center * 0.17);
        let _ = cr.stroke();
    });
    area
}

fn format_sync_status(summary: &LinuxSyncSummary) -> String {
    if summary.pushed == 0 && summary.pulled == 0 && summary.failed == 0 {
        "Synced. Everything is up to date.".to_owned()
    } else if summary.failed == 0 {
        format!(
            "Synced. {} pushed, {} pulled.",
            summary.pushed, summary.pulled
        )
    } else {
        format!(
            "Synced with issues. {} pushed, {} pulled, {} failed.",
            summary.pushed, summary.pulled, summary.failed
        )
    }
}

fn delete_selected_task(state: &Rc<AppState>) {
    let editing_task_id = *state.current_editing_task_id.borrow();
    let task_id = editing_task_id.or_else(|| selected_task_id(state));
    if let Some(task_id) = task_id {
        if let Err(error) = state.core.delete_task(task_id) {
            state.toast(format!("Failed to delete task: {error}"));
        }
        state.hide_task_editor();
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

fn render_search_results(state: &Rc<AppState>, results: &gtk::ListBox, query: &str) {
    while let Some(row) = results.first_child() {
        results.remove(&row);
    }
    results.set_visible(false);
    if query.is_empty() {
        return;
    }

    let regex = match regex_from_query(query) {
        Ok(regex) => regex,
        Err(error) => {
            state.toast(format!("Invalid regex: {error}"));
            return;
        }
    };
    match state.core.list_tasks(TaskFilter::default(), default_sort()) {
        Ok(tasks) => {
            let mut has_results = false;
            for task in tasks
                .into_iter()
                .filter(|task| regex_matches_task(&regex, task))
                .take(10)
            {
                has_results = true;
                results.append(&task_search_result_row(&task));
            }
            results.set_visible(has_results);
        }
        Err(error) => state.toast(format!("Search failed: {error}")),
    }
}

fn render_move_list_results(state: &Rc<AppState>, results_query: &str) {
    while let Some(row) = state.move_list_results.first_child() {
        state.move_list_results.remove(&row);
    }
    let regex = match regex_from_query(results_query) {
        Ok(regex) => regex,
        Err(error) => {
            state.toast(format!("Invalid regex: {error}"));
            return;
        }
    };
    append_move_list_row(state, "inbox", "Inbox");
    for list in state
        .user_lists
        .borrow()
        .iter()
        .filter(|list| regex.is_match(&list.name))
    {
        append_move_list_row(state, &list.id.to_string(), &list.name);
    }
}

fn append_move_list_row(state: &Rc<AppState>, id: &str, name: &str) {
    state
        .move_list_results
        .append(&move_list_result_row(id, name));
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
