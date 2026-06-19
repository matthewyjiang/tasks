use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use taskmanager_core::{
    approve_pending_enrollment_request, load_access_token, normalize_sync_server_url,
    public_key_fingerprint, sync_auth_state as core_sync_auth_state, DefaultSort, DisplayDensity,
    EnrollmentClient, Keybindings, LocalDataEnrollmentStrategy, PendingEnrollmentRequest,
    SyncAuthState, TaskManagerCore, VaultSettings,
};

use crate::platform::LinuxPlatform;
use crate::sync::LinuxHttpSyncClient;
use crate::ui::floating_panel::{hide_floating_panel, show_floating_panel};
use crate::ui::settings::{read_settings, write_settings, LinuxSettings, SyncStatus, ThemeChoice};
use crate::ui::sync_setup::{
    complete_linux_enrollment, complete_linux_enrollment_for_local_data,
    confirm_replace_local_data, logout_sync_auth, show_sync_setup_window, sync_auth_configured,
};
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
    let (vault_settings, vault_settings_error) = match core.vault_settings() {
        Ok(settings) => (settings, None),
        Err(error) => {
            eprintln!("Failed to read encrypted settings: {error}");
            (VaultSettings::default(), Some(error.to_string()))
        }
    };
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
    for name in ["Sync", "Tasks", "Appearance", "Keybindings"] {
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
    let tasks_page = gtk::Box::new(gtk::Orientation::Vertical, 14);
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
    let auth_state = core_sync_auth_state(&platform, &settings.server_url);
    let signed_in = auth_state == SyncAuthState::SyncReady;
    let enrollment_pending = auth_state == SyncAuthState::AuthenticatedEnrollmentPending;
    let enrollment_available = enrollment_pending || signed_in;
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

    let enrollment_title = gtk::Label::new(Some("Device enrollment"));
    enrollment_title.set_xalign(0.0);
    enrollment_title.add_css_class("task-menu-heading");
    sync_page.append(&enrollment_title);
    let enrollment_help = gtk::Label::new(Some(if enrollment_pending {
        "This device is waiting for approval. Choose Merge local data to keep local tasks or Replace local data to delete local tasks on this device and download server data. Private keys and plaintext account data keys never leave devices."
    } else if signed_in {
        "Approve only devices you recognize. Private keys and plaintext account data keys never leave devices; approval wraps your account data key for the requested public key."
    } else {
        "Sign in to manage device enrollment."
    }));
    enrollment_help.set_xalign(0.0);
    enrollment_help.set_wrap(true);
    enrollment_help.add_css_class("dim-label");
    sync_page.append(&enrollment_help);
    let enrollment_status = gtk::Label::new(None);
    enrollment_status.set_xalign(0.0);
    enrollment_status.set_wrap(true);
    enrollment_status.add_css_class("dim-label");
    sync_page.append(&enrollment_status);
    let enrollment_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    enrollment_actions.set_halign(gtk::Align::Start);
    let check_enrollment_button = gtk::Button::with_label("Check approval / complete");
    check_enrollment_button.set_visible(false);
    let merge_enrollment_button = gtk::Button::with_label("Merge local data");
    merge_enrollment_button.set_visible(enrollment_pending);
    let replace_enrollment_button = gtk::Button::with_label("Replace local data");
    replace_enrollment_button.add_css_class("destructive-action");
    replace_enrollment_button.set_visible(enrollment_pending);
    let refresh_enrollment_button = gtk::Button::with_label("Refresh pending requests");
    enrollment_actions.append(&check_enrollment_button);
    enrollment_actions.append(&merge_enrollment_button);
    enrollment_actions.append(&replace_enrollment_button);
    enrollment_actions.append(&refresh_enrollment_button);
    enrollment_actions.set_visible(enrollment_available);
    sync_page.append(&enrollment_actions);
    let pending_list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    pending_list.set_visible(enrollment_available);
    sync_page.append(&pending_list);

    if enrollment_available {
        enrollment_status
            .set_text("Use Refresh pending requests to check for devices waiting for approval.");
    } else {
        enrollment_status.set_text("Sign in before managing device enrollment.");
    }

    let tasks_title = gtk::Label::new(Some("Tasks"));
    tasks_title.set_xalign(0.0);
    tasks_title.add_css_class("pane-title");
    tasks_page.append(&tasks_title);

    let default_sort_label = gtk::Label::new(Some("Default sort"));
    default_sort_label.set_xalign(0.0);
    let default_sort_combo = gtk::ComboBoxText::new();
    default_sort_combo.append(Some("due_at_asc"), "Due date, soonest first");
    default_sort_combo.append(Some("updated_at_desc"), "Recently updated first");
    default_sort_combo.set_active_id(Some(match vault_settings.default_sort {
        DefaultSort::DueAtAsc => "due_at_asc",
        DefaultSort::UpdatedAtDesc => "updated_at_desc",
    }));
    tasks_page.append(&default_sort_label);
    tasks_page.append(&default_sort_combo);

    let default_reminder_label = gtk::Label::new(Some("Default reminder"));
    default_reminder_label.set_xalign(0.0);
    let default_reminder_combo = gtk::ComboBoxText::new();
    default_reminder_combo.append(Some("0"), "No default reminder");
    default_reminder_combo.append(Some("5"), "5 minutes before");
    default_reminder_combo.append(Some("15"), "15 minutes before");
    default_reminder_combo.append(Some("30"), "30 minutes before");
    default_reminder_combo.append(Some("60"), "1 hour before");
    default_reminder_combo.append(Some("1440"), "1 day before");
    default_reminder_combo
        .set_active_id(Some(&vault_settings.default_reminder_minutes.to_string()));
    tasks_page.append(&default_reminder_label);
    tasks_page.append(&default_reminder_combo);

    let first_day_label = gtk::Label::new(Some("First day of week"));
    first_day_label.set_xalign(0.0);
    let first_day_combo = gtk::ComboBoxText::new();
    first_day_combo.append(Some("0"), "Sunday");
    first_day_combo.append(Some("1"), "Monday");
    first_day_combo.set_active_id(Some(&vault_settings.first_day_of_week.to_string()));
    tasks_page.append(&first_day_label);
    tasks_page.append(&first_day_combo);

    let notification_sound_label = gtk::Label::new(Some("Notification sound"));
    notification_sound_label.set_xalign(0.0);
    let notification_sound_combo = gtk::ComboBoxText::new();
    notification_sound_combo.append(Some("default"), "Default");
    notification_sound_combo.append(Some("silent"), "Silent");
    match vault_settings.notification_sound.as_str() {
        "default" | "silent" => {
            notification_sound_combo.set_active_id(Some(&vault_settings.notification_sound));
        }
        _ => {}
    }
    tasks_page.append(&notification_sound_label);
    tasks_page.append(&notification_sound_combo);

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
    show_completed.set_active(vault_settings.show_completed || settings.show_completed);
    appearance_page.append(&show_completed);

    let density_label = gtk::Label::new(Some("Display density"));
    density_label.set_xalign(0.0);
    let density_combo = gtk::ComboBoxText::new();
    density_combo.append(Some("compact"), "Compact");
    density_combo.append(Some("comfortable"), "Comfortable");
    density_combo.append(Some("spacious"), "Spacious");
    density_combo.set_active_id(Some(match vault_settings.display_density {
        DisplayDensity::Compact => "compact",
        DisplayDensity::Comfortable => "comfortable",
        DisplayDensity::Spacious => "spacious",
    }));
    appearance_page.append(&density_label);
    appearance_page.append(&density_combo);

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
    if let Some(error) = &vault_settings_error {
        let error_label = gtk::Label::new(Some(&format!(
            "Encrypted settings could not be loaded, so settings cannot be saved until this is resolved: {error}"
        )));
        error_label.set_xalign(0.0);
        error_label.set_wrap(true);
        error_label.add_css_class("dim-label");
        error_label.add_css_class("error");
        tasks_page.append(&error_label);
        save_button.set_sensitive(false);
    }

    settings_stack.add_named(&sync_page, Some("sync"));
    settings_stack.add_named(&tasks_page, Some("tasks"));
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
                1 => "tasks",
                2 => "appearance",
                3 => "keybindings",
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
        let core = Rc::clone(&core);
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
                Some(Rc::clone(&core)),
                Some(refresh_settings_sync_state),
            );
        }
    });
    check_enrollment_button.connect_clicked({
        let settings_path = settings_path.clone();
        let enrollment_status = enrollment_status.clone();
        let on_auth_changed = on_auth_changed.clone();
        move |_| match complete_linux_enrollment(&LinuxPlatform::new(), &settings_path) {
            Ok(taskmanager_core::EnrollmentState::SyncReady) => {
                enrollment_status.set_text("Enrollment complete. Sync ready.");
                if let Some(on_auth_changed) = &on_auth_changed {
                    on_auth_changed();
                }
            }
            Ok(_) => enrollment_status.set_text("No approved key yet. Approve this device from an enrolled device, then check again."),
            Err(error) => enrollment_status.set_text(&enrollment_error_message(
                "Could not complete enrollment",
                &error,
                "Check your network and server URL, then try again. If your session expired, log out and sign in again. If no approval is available yet, approve this device from a sync-ready device first.",
            )),
        }
    });
    merge_enrollment_button.connect_clicked({
        let settings_path = settings_path.clone();
        let enrollment_status = enrollment_status.clone();
        let on_auth_changed = on_auth_changed.clone();
        let core = Rc::clone(&core);
        move |_| {
            match complete_linux_enrollment_for_local_data(
                &core,
                &LinuxPlatform::new(),
                &settings_path,
                LocalDataEnrollmentStrategy::MergeLocalData,
            ) {
                Ok((taskmanager_core::EnrollmentState::SyncReady, count)) => {
                    enrollment_status.set_text(&format!(
                        "Enrollment complete. Kept local tasks and queued {count} local item(s) to merge on next sync."
                    ));
                    if let Some(on_auth_changed) = &on_auth_changed {
                        on_auth_changed();
                    }
                },
                Ok(_) => enrollment_status.set_text("No approved key yet. Approve this device from an enrolled device, then try again."),
                Err(error) => enrollment_status.set_text(&enrollment_error_message(
                    "Could not merge local data into this account",
                    &error,
                    "First approve this device from a sync-ready device. This option keeps local tasks and uploads them with the approved account key on next sync.",
                )),
            }
        }
    });
    replace_enrollment_button.connect_clicked({
        let panel = panel.clone();
        let settings_path = settings_path.clone();
        let enrollment_status = enrollment_status.clone();
        let on_auth_changed = on_auth_changed.clone();
        let core = Rc::clone(&core);
        move |_| {
            let Some(root) = panel.root().and_then(|root| root.downcast::<gtk::Window>().ok()) else {
                enrollment_status.set_text("Could not show confirmation dialog.");
                return;
            };
            confirm_replace_local_data(&root, {
                let settings_path = settings_path.clone();
                let enrollment_status = enrollment_status.clone();
                let on_auth_changed = on_auth_changed.clone();
                let core = Rc::clone(&core);
                move || {
                    match complete_linux_enrollment_for_local_data(
                        &core,
                        &LinuxPlatform::new(),
                        &settings_path,
                        LocalDataEnrollmentStrategy::ReplaceLocalData,
                    ) {
                        Ok((taskmanager_core::EnrollmentState::SyncReady, count)) => {
                            enrollment_status.set_text(&format!(
                                "Enrollment complete. Removed {count} local item(s); server data will download on next sync."
                            ));
                            if let Some(on_auth_changed) = &on_auth_changed {
                                on_auth_changed();
                            }
                        },
                        Ok(_) => enrollment_status.set_text("No approved key yet. Approve this device from an enrolled device, then try again."),
                        Err(error) => enrollment_status.set_text(&enrollment_error_message(
                            "Could not replace local data with this account",
                            &error,
                            "Approve this device from a sync-ready device first, then try Replace local data again.",
                        )),
                    }
                }
            });
        }
    });
    refresh_enrollment_button.connect_clicked({
        let settings_path = settings_path.clone();
        let pending_list = pending_list.clone();
        let enrollment_status = enrollment_status.clone();
        move |_| {
            refresh_pending_enrollment_requests(&settings_path, &pending_list, &enrollment_status)
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

    let saving_settings = Rc::new(Cell::new(false));
    save_button.connect_clicked({
        let panel = panel.clone();
        let saving_settings = Rc::clone(&saving_settings);
        move |button| {
            if saving_settings.replace(true) {
                return;
            }
            button.set_sensitive(false);
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
            let mut vault_settings = match core.vault_settings() {
                Ok(settings) => settings,
                Err(error) => {
                    eprintln!("Failed to read encrypted settings: {error}");
                    saving_settings.set(false);
                    button.set_sensitive(true);
                    return;
                }
            };
            vault_settings.show_completed = show_completed.is_active();
            vault_settings.default_sort = match default_sort_combo.active_id().as_deref() {
                Some("updated_at_desc") => DefaultSort::UpdatedAtDesc,
                _ => DefaultSort::DueAtAsc,
            };
            if let Some(default_reminder_minutes) = default_reminder_combo
                .active_id()
                .and_then(|id| id.parse::<i32>().ok())
            {
                vault_settings.default_reminder_minutes = default_reminder_minutes;
            }
            if let Some(first_day_of_week) = first_day_combo
                .active_id()
                .and_then(|id| id.parse::<i32>().ok())
            {
                vault_settings.first_day_of_week = first_day_of_week;
            }
            if let Some(notification_sound) = notification_sound_combo.active_id() {
                vault_settings.notification_sound = notification_sound.to_string();
            }
            vault_settings.display_density = match density_combo.active_id().as_deref() {
                Some("compact") => DisplayDensity::Compact,
                Some("spacious") => DisplayDensity::Spacious,
                _ => DisplayDensity::Comfortable,
            };
            vault_settings.keybindings = Keybindings {
                add_task: add_task_key.text().to_string(),
                search: search_key.text().to_string(),
                close_overlay: close_overlay_key.text().to_string(),
                confirm_rename: confirm_rename_key.text().to_string(),
                delete_task: delete_task_key.text().to_string(),
                toggle_done: toggle_done_key.text().to_string(),
            };
            if let Err(error) = core.update_vault_settings(vault_settings) {
                eprintln!("Failed to save encrypted settings: {error}");
                saving_settings.set(false);
                button.set_sensitive(true);
            } else if let Err(error) = write_settings(&settings_path, &settings) {
                eprintln!("Failed to save local settings: {error}");
                saving_settings.set(false);
                button.set_sensitive(true);
            } else {
                apply_theme_choice(theme);
                hide_floating_panel(&panel);
                if let Some(on_auth_changed) = &on_auth_changed {
                    let on_auth_changed = Rc::clone(on_auth_changed);
                    gtk::glib::idle_add_local_once(move || on_auth_changed());
                }
            }
        }
    });

    panel.append(&content);
    show_floating_panel(panel);
}

fn refresh_pending_enrollment_requests(
    settings_path: &PathBuf,
    pending_list: &gtk::Box,
    enrollment_status: &gtk::Label,
) {
    while let Some(child) = pending_list.first_child() {
        pending_list.remove(&child);
    }
    match enrollment_client(settings_path).and_then(|client| {
        client
            .list_pending_requests()
            .map_err(|error| error.to_string())
    }) {
        Ok(requests) if requests.is_empty() => {
            enrollment_status.set_text("No pending device requests. If this device is waiting for approval, use Check approval / complete.");
        }
        Ok(requests) => {
            enrollment_status.set_text(&format!("{} pending device request(s).", requests.len()));
            for request in requests {
                pending_list.append(&pending_enrollment_row(
                    settings_path.clone(),
                    request,
                    enrollment_status.clone(),
                    pending_list.clone(),
                ));
            }
        }
        Err(error) => enrollment_status.set_text(&enrollment_error_message(
            "Could not load pending requests",
            &error,
            "Check your network and server URL. If your session expired, log out and sign in again.",
        )),
    }
}

fn pending_enrollment_row(
    settings_path: PathBuf,
    request: PendingEnrollmentRequest,
    enrollment_status: gtk::Label,
    pending_list: gtk::Box,
) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
    row.add_css_class("setup-panel");
    let title = gtk::Label::new(Some(&format!(
        "{} on {}",
        display_device_name(&request),
        display_platform(&request)
    )));
    title.set_xalign(0.0);
    let details = gtk::Label::new(Some(&format!(
        "Requested {} · public key fingerprint {}",
        request.created_at,
        public_key_fingerprint(&request.recipient_public_key)
    )));
    details.set_xalign(0.0);
    details.set_wrap(true);
    details.add_css_class("dim-label");
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::Start);
    let approve_button = gtk::Button::with_label("Approve");
    approve_button.add_css_class("suggested-action");
    let reject_button = gtk::Button::with_label("Reject");
    reject_button.add_css_class("destructive-action");
    actions.append(&approve_button);
    actions.append(&reject_button);
    row.append(&title);
    row.append(&details);
    row.append(&actions);

    approve_button.connect_clicked({
        let settings_path = settings_path.clone();
        let request = request.clone();
        let enrollment_status = enrollment_status.clone();
        let pending_list = pending_list.clone();
        move |_| match enrollment_client(&settings_path).and_then(|client| {
            approve_pending_enrollment_request(&LinuxPlatform::new(), &client, &request)
                .map_err(|error| error.to_string())
        }) {
            Ok(()) => {
                enrollment_status.set_text(
                    "Device approved. The account data key was wrapped locally for that device.",
                );
                refresh_pending_enrollment_requests(&settings_path, &pending_list, &enrollment_status);
            }
            Err(error) => enrollment_status.set_text(&enrollment_error_message(
                "Approve failed",
                &error,
                "Make sure this device has a local account data key and is sync-ready. If your session expired, log out and sign in again, then refresh pending requests.",
            )),
        }
    });
    reject_button.connect_clicked({
        let settings_path = settings_path.clone();
        let request_id = request.request_id.clone();
        let enrollment_status = enrollment_status.clone();
        let pending_list = pending_list.clone();
        move |_| match enrollment_client(&settings_path).and_then(|client| {
            client
                .reject_request(&request_id)
                .map_err(|error| error.to_string())
        }) {
            Ok(()) => {
                enrollment_status.set_text("Device request rejected.");
                refresh_pending_enrollment_requests(&settings_path, &pending_list, &enrollment_status);
            }
            Err(error) => enrollment_status.set_text(&enrollment_error_message(
                "Reject failed",
                &error,
                "Check your network and server URL. If your session expired, log out and sign in again, then refresh pending requests.",
            )),
        }
    });

    row
}

fn enrollment_error_message(context: &str, error: &str, recovery: &str) -> String {
    format!("{context}: {error}. {recovery}")
}

fn display_device_name(request: &PendingEnrollmentRequest) -> &str {
    if request.device_name.is_empty() {
        "Unknown device"
    } else {
        &request.device_name
    }
}

fn display_platform(request: &PendingEnrollmentRequest) -> &str {
    if request.platform.is_empty() {
        "unknown platform"
    } else {
        &request.platform
    }
}

fn enrollment_client(settings_path: &PathBuf) -> Result<LinuxHttpSyncClient, String> {
    let settings = read_settings(settings_path).unwrap_or_default();
    let server_url =
        normalize_sync_server_url(&settings.server_url).map_err(|error| error.to_string())?;
    let token = load_access_token(&LinuxPlatform::new()).map_err(|error| error.to_string())?;
    LinuxHttpSyncClient::new(&server_url, token).map_err(|error| error.to_string())
}

fn format_sync_settings_status(status: &SyncStatus) -> String {
    match (&status.last_attempt_at, &status.last_success_at) {
        (None, _) => "No sync has run yet.".to_owned(),
        (_, Some(success_at)) if status.last_error.is_empty() => {
            let mut details = format!(
                "Last synced {}. {} pushed · {} pulled",
                relative_time(*success_at),
                status.last_pushed,
                status.last_pulled
            );
            if status.last_failed > 0 {
                details.push_str(&format!(" · {} failed", status.last_failed));
            }
            if status.dirty_count > 0 {
                details.push_str(&format!(" · {} unsynced local change", status.dirty_count));
            }
            if status.pending_retries > 0 {
                details.push_str(&format!(" · {} pending retry", status.pending_retries));
            }
            if status.cursor > 0 {
                details.push_str(&format!(" · cursor {}", status.cursor));
            }
            if status.conflicts > 0 {
                details.push_str(&format!(
                    " · {} conflict resolved automatically (last write wins)",
                    status.conflicts
                ));
            }
            details
        }
        (Some(attempt_at), last_success) => {
            let previous_success = last_success
                .map(|success_at| format!(" Last successful sync {}.", relative_time(success_at)))
                .unwrap_or_default();
            let availability = if !status.network_available {
                " Network unavailable; sync will retry when connectivity returns."
            } else if !status.backend_available {
                " Sync server unreachable; check your server URL or try again later."
            } else {
                ""
            };
            format!(
                "Last sync failed {}. {}{}{}",
                relative_time(*attempt_at),
                status.last_error,
                previous_success,
                availability
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
