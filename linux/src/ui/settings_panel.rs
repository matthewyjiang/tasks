use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use taskmanager_core::{Keybindings, TaskManagerCore};

use crate::platform::LinuxPlatform;
use crate::ui::floating_panel::{hide_floating_panel, show_floating_panel};
use crate::ui::settings::{read_settings, write_settings, LinuxSettings, SyncStatus, ThemeChoice};
use crate::ui::sync_setup::{logout_sync_auth, show_sync_setup_window, sync_auth_configured};
use crate::ui::widgets::settings_entry;

pub(crate) fn apply_theme_choice(theme: ThemeChoice) {
    let color_scheme = match theme {
        ThemeChoice::System => adw::ColorScheme::Default,
        ThemeChoice::Light => adw::ColorScheme::ForceLight,
        ThemeChoice::Dark => adw::ColorScheme::ForceDark,
    };
    adw::StyleManager::default().set_color_scheme(color_scheme);
}

pub(crate) fn show_settings_panel(
    panel: &gtk::Box,
    settings_path: PathBuf,
    core: Rc<TaskManagerCore>,
    on_auth_changed: Option<Rc<dyn Fn()>>,
) {
    let settings = read_settings(&settings_path).unwrap_or_default();
    let vault_settings = core.vault_settings().unwrap_or_default();
    while let Some(child) = panel.first_child() {
        panel.remove(&child);
    }

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    content.set_margin_top(0);
    content.set_margin_bottom(0);
    content.set_margin_start(0);
    content.set_margin_end(0);
    content.set_vexpand(true);
    content.set_hexpand(true);

    let settings_nav = gtk::ListBox::new();
    settings_nav.add_css_class("settings-nav");
    settings_nav.set_selection_mode(gtk::SelectionMode::Single);
    settings_nav.set_width_request(170);
    settings_nav.set_vexpand(true);
    for name in ["Sync", "Appearance", "Keybindings"] {
        let row = gtk::ListBoxRow::new();
        row.add_css_class("sidebar-row");
        let label = gtk::Label::new(Some(name));
        label.set_xalign(0.0);
        label.set_margin_top(8);
        label.set_margin_bottom(8);
        label.set_margin_start(10);
        label.set_margin_end(10);
        row.set_child(Some(&label));
        settings_nav.append(&row);
    }

    let settings_stack = gtk::Stack::new();
    settings_stack.add_css_class("settings-content");
    settings_stack.set_hexpand(true);
    settings_stack.set_vexpand(true);

    let sync_page = gtk::Box::new(gtk::Orientation::Vertical, 14);
    let appearance_page = gtk::Box::new(gtk::Orientation::Vertical, 14);
    let keybindings_page = gtk::Box::new(gtk::Orientation::Vertical, 14);

    let title = gtk::Label::new(Some("Sync"));
    title.set_xalign(0.0);
    title.add_css_class("pane-title");
    sync_page.append(&title);

    let server_label = gtk::Label::new(Some("Server URL"));
    server_label.set_xalign(0.0);
    let server_entry = gtk::Entry::new();
    server_entry.set_placeholder_text(Some("Optional sync server URL"));
    server_entry.set_text(&settings.server_url);
    sync_page.append(&server_label);
    sync_page.append(&server_entry);

    let platform = LinuxPlatform::new();
    let signed_in = sync_auth_configured(&platform, &settings);
    let sync_setup_button = gtk::Button::with_label("Sync login / setup…");
    sync_setup_button.set_halign(gtk::Align::Start);
    sync_setup_button.set_visible(!signed_in);
    sync_page.append(&sync_setup_button);

    let sync_account_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    sync_account_row.set_visible(signed_in);
    let sync_account = gtk::Label::new(Some(&format!(
        "Signed in as {}",
        if settings.sync_email.is_empty() {
            "unknown account"
        } else {
            &settings.sync_email
        }
    )));
    sync_account.set_xalign(0.0);
    sync_account.set_hexpand(true);
    let sync_logout_button = gtk::Button::with_label("Log out");
    sync_logout_button.add_css_class("destructive-action");
    sync_account_row.append(&sync_account);
    sync_account_row.append(&sync_logout_button);
    sync_page.append(&sync_account_row);

    let sync_status_title = gtk::Label::new(Some("Status"));
    sync_status_title.set_xalign(0.0);
    sync_status_title.add_css_class("task-menu-heading");
    sync_page.append(&sync_status_title);
    let sync_status_label =
        gtk::Label::new(Some(&format_sync_settings_status(&settings.sync_status)));
    sync_status_label.set_xalign(0.0);
    sync_status_label.set_wrap(true);
    sync_status_label.add_css_class("dim-label");
    sync_page.append(&sync_status_label);

    let appearance_title = gtk::Label::new(Some("Appearance"));
    appearance_title.set_xalign(0.0);
    appearance_title.add_css_class("pane-title");
    appearance_page.append(&appearance_title);

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
    appearance_page.append(&theme_label);
    appearance_page.append(&theme_combo);

    let show_completed = gtk::CheckButton::with_label("Show completed tasks");
    show_completed.set_active(vault_settings.show_completed);
    appearance_page.append(&show_completed);

    let keybind_label = gtk::Label::new(Some("Keybindings (encrypted + synced)"));
    keybind_label.set_xalign(0.0);
    keybind_label.add_css_class("task-menu-heading");
    let keybindings_title = gtk::Label::new(Some("Keybindings"));
    keybindings_title.set_xalign(0.0);
    keybindings_title.add_css_class("pane-title");
    keybindings_page.append(&keybindings_title);
    keybindings_page.append(&keybind_label);
    let add_task_key = settings_entry(
        "Add task",
        &vault_settings.keybindings.add_task,
        &keybindings_page,
    );
    let search_key = settings_entry(
        "Search",
        &vault_settings.keybindings.search,
        &keybindings_page,
    );
    let close_overlay_key = settings_entry(
        "Close overlay",
        &vault_settings.keybindings.close_overlay,
        &keybindings_page,
    );
    let confirm_rename_key = settings_entry(
        "Confirm rename",
        &vault_settings.keybindings.confirm_rename,
        &keybindings_page,
    );
    let delete_task_key = settings_entry(
        "Delete task",
        &vault_settings.keybindings.delete_task,
        &keybindings_page,
    );
    let toggle_done_key = settings_entry(
        "Toggle done",
        &vault_settings.keybindings.toggle_done,
        &keybindings_page,
    );

    let save_button = gtk::Button::with_label("Save");
    save_button.add_css_class("suggested-action");
    save_button.set_halign(gtk::Align::End);
    save_button.set_margin_end(22);
    save_button.set_margin_bottom(22);

    settings_stack.add_named(&sync_page, Some("sync"));
    settings_stack.add_named(&appearance_page, Some("appearance"));
    settings_stack.add_named(&keybindings_page, Some("keybindings"));
    settings_stack.set_visible_child_name("sync");
    settings_nav.connect_row_selected({
        let settings_stack = settings_stack.clone();
        move |_, row| {
            let Some(row) = row else {
                return;
            };
            settings_stack.set_visible_child_name(match row.index() {
                0 => "sync",
                1 => "appearance",
                2 => "keybindings",
                _ => "sync",
            });
        }
    });
    if let Some(row) = settings_nav.row_at_index(0) {
        settings_nav.select_row(Some(&row));
    }
    let settings_body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    settings_body.set_hexpand(true);
    settings_body.set_vexpand(true);
    settings_body.append(&settings_stack);
    settings_body.append(&save_button);
    content.append(&settings_nav);
    content.append(&settings_body);

    sync_setup_button.connect_clicked({
        let panel = panel.clone();
        let settings_path = settings_path.clone();
        let sync_setup_button = sync_setup_button.clone();
        let sync_account_row = sync_account_row.clone();
        let sync_account = sync_account.clone();
        let sync_status_label = sync_status_label.clone();
        let on_auth_changed = on_auth_changed.clone();
        move |_| {
            let refresh_settings_sync_state: Rc<dyn Fn()> = Rc::new({
                let settings_path = settings_path.clone();
                let sync_setup_button = sync_setup_button.clone();
                let sync_account_row = sync_account_row.clone();
                let sync_account = sync_account.clone();
                let sync_status_label = sync_status_label.clone();
                let on_auth_changed = on_auth_changed.clone();
                move || {
                    let settings = read_settings(&settings_path).unwrap_or_default();
                    let signed_in = sync_auth_configured(&LinuxPlatform::new(), &settings);
                    sync_setup_button.set_visible(!signed_in);
                    sync_account_row.set_visible(signed_in);
                    sync_status_label.set_text(&format_sync_settings_status(&settings.sync_status));
                    if signed_in {
                        sync_account.set_text(&format!(
                            "Signed in as {}",
                            if settings.sync_email.is_empty() {
                                "unknown account"
                            } else {
                                &settings.sync_email
                            }
                        ));
                    }
                    if let Some(on_auth_changed) = &on_auth_changed {
                        on_auth_changed();
                    }
                }
            });
            let Some(root) = panel
                .root()
                .and_then(|root| root.downcast::<gtk::Window>().ok())
            else {
                return;
            };
            show_sync_setup_window(
                &root,
                settings_path.clone(),
                false,
                Some(refresh_settings_sync_state),
            );
        }
    });
    sync_logout_button.connect_clicked({
        let panel = panel.clone();
        let settings_path = settings_path.clone();
        let on_auth_changed = on_auth_changed.clone();
        move |_| {
            if let Err(error) = logout_sync_auth(&LinuxPlatform::new(), &settings_path) {
                eprintln!("Failed to log out: {error}");
            }
            if let Some(on_auth_changed) = &on_auth_changed {
                on_auth_changed();
            }
            hide_floating_panel(&panel);
        }
    });

    save_button.connect_clicked({
        let panel = panel.clone();
        move |_| {
            let theme = match theme_combo.active_id().as_deref() {
                Some("light") => ThemeChoice::Light,
                Some("dark") => ThemeChoice::Dark,
                _ => ThemeChoice::System,
            };
            let current_settings = read_settings(&settings_path).unwrap_or_default();
            let settings = LinuxSettings {
                server_url: server_entry.text().to_string(),
                sync_email: current_settings.sync_email,
                theme,
                show_completed: false,
                sync_status: current_settings.sync_status,
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
                if let Some(on_auth_changed) = &on_auth_changed {
                    on_auth_changed();
                }
                hide_floating_panel(&panel);
            }
        }
    });

    panel.append(&content);
    show_floating_panel(panel);
}

fn format_sync_settings_status(status: &SyncStatus) -> String {
    match (&status.last_attempt_at, &status.last_success_at) {
        (None, _) => "No sync has run yet.".to_owned(),
        (_, Some(success_at)) if status.last_error.is_empty() => format!(
            "Last synced {}. {} pushed · {} pulled{}",
            relative_time(*success_at),
            status.last_pushed,
            status.last_pulled,
            if status.last_failed == 0 {
                String::new()
            } else {
                format!(" · {} failed", status.last_failed)
            }
        ),
        (Some(attempt_at), last_success) => {
            let previous_success = last_success
                .map(|success_at| format!(" Last successful sync {}.", relative_time(success_at)))
                .unwrap_or_default();
            format!(
                "Last sync failed {}. {}{}",
                relative_time(*attempt_at),
                status.last_error,
                previous_success
            )
        }
    }
}

fn relative_time(timestamp_ms: i64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default();
    let elapsed = ((now_ms - timestamp_ms).max(0)) / 1000;
    match elapsed {
        0..=4 => "just now".to_owned(),
        5..=59 => format!("{elapsed}s ago"),
        60..=3599 => format!("{}m ago", elapsed / 60),
        3600..=86_399 => format!("{}h ago", elapsed / 3600),
        _ => format!("{}d ago", elapsed / 86_400),
    }
}
