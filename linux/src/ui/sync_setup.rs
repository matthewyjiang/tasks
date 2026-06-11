use std::path::{Path, PathBuf};
use std::rc::Rc;

use base64::Engine;
use gtk::prelude::*;
use gtk4 as gtk;
use serde::{Deserialize, Serialize};
use taskmanager_core::{
    init_account, init_device_keypair, public_key_from_private_key, Platform, ACCOUNT_DATA_KEY_ID,
    DEVICE_PRIVATE_KEY_ID,
};

use crate::platform::LinuxPlatform;
use crate::sync::{AUTH_ACCESS_TOKEN_ID, AUTH_REFRESH_TOKEN_ID};
use crate::ui::settings::{read_settings, write_settings, LinuxSettings};

pub(crate) struct SyncSetupPanelWidgets {
    pub(crate) panel: gtk::Box,
    pub(crate) server_entry: gtk::Entry,
    pub(crate) email_entry: gtk::Entry,
    pub(crate) password_entry: gtk::PasswordEntry,
    pub(crate) status_label: gtk::Label,
    pub(crate) local_button: gtk::Button,
    pub(crate) login_button: gtk::Button,
}

pub(crate) fn build_sync_setup_panel(configured: bool, server_url: &str) -> SyncSetupPanelWidgets {
    let setup_panel = gtk::Box::new(gtk::Orientation::Vertical, 14);
    setup_panel.add_css_class("setup-panel");
    setup_panel.set_halign(gtk::Align::Fill);
    setup_panel.set_valign(gtk::Align::Fill);
    setup_panel.set_visible(!configured);

    let setup_card = gtk::Box::new(gtk::Orientation::Vertical, 12);
    setup_card.set_halign(gtk::Align::Center);
    setup_card.set_valign(gtk::Align::Center);
    setup_card.set_width_request(460);
    let setup_title = gtk::Label::new(Some("Set up sync"));
    setup_title.set_xalign(0.0);
    setup_title.add_css_class("pane-title");
    let setup_subtitle = gtk::Label::new(Some(
        "Sign in to sync tasks across devices, or keep working locally.",
    ));
    setup_subtitle.set_xalign(0.0);
    setup_subtitle.set_wrap(true);
    setup_subtitle.add_css_class("dim-label");
    let server_entry = gtk::Entry::new();
    server_entry.set_placeholder_text(Some("Server URL, e.g. http://127.0.0.1:18080"));
    server_entry.set_text(server_url);
    let email_entry = gtk::Entry::new();
    email_entry.set_placeholder_text(Some("Email"));
    let password_entry = gtk::PasswordEntry::new();
    password_entry.set_placeholder_text(Some("Password"));
    let status_label = gtk::Label::new(None);
    status_label.set_xalign(0.0);
    status_label.add_css_class("dim-label");
    let setup_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    setup_actions.set_halign(gtk::Align::End);
    let local_button = gtk::Button::with_label("Work local");
    let login_button = gtk::Button::with_label("Login / Register");
    login_button.add_css_class("suggested-action");
    setup_actions.append(&local_button);
    setup_actions.append(&login_button);

    setup_card.append(&setup_title);
    setup_card.append(&setup_subtitle);
    setup_card.append(&server_entry);
    setup_card.append(&email_entry);
    setup_card.append(&password_entry);
    setup_card.append(&status_label);
    setup_card.append(&setup_actions);
    let setup_top_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    setup_top_spacer.set_vexpand(true);
    let setup_bottom_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    setup_bottom_spacer.set_vexpand(true);
    setup_panel.append(&setup_top_spacer);
    setup_panel.append(&setup_card);
    setup_panel.append(&setup_bottom_spacer);

    SyncSetupPanelWidgets {
        panel: setup_panel,
        server_entry,
        email_entry,
        password_entry,
        status_label,
        local_button,
        login_button,
    }
}

pub(crate) fn sync_auth_configured(platform: &LinuxPlatform, settings: &LinuxSettings) -> bool {
    !settings.server_url.trim().is_empty()
        && platform.load_key(AUTH_ACCESS_TOKEN_ID).is_ok()
        && platform.load_key(AUTH_REFRESH_TOKEN_ID).is_ok()
        && platform.load_key(ACCOUNT_DATA_KEY_ID).is_ok()
        && platform.load_key(DEVICE_PRIVATE_KEY_ID).is_ok()
}

#[derive(Serialize)]
struct AuthRequest {
    email: String,
    password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub_key: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    jwt: String,
    refresh_token: String,
}

pub(crate) fn show_sync_setup_window(
    parent: &impl IsA<gtk::Window>,
    settings_path: PathBuf,
    first_run: bool,
    on_auth_changed: Option<Rc<dyn Fn()>>,
) -> gtk::Window {
    let settings = read_settings(&settings_path).unwrap_or_default();
    let dialog = gtk::Window::builder()
        .title("Sync setup")
        .transient_for(parent)
        .modal(true)
        .default_width(440)
        .default_height(320)
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);

    let title = gtk::Label::new(Some("Set up sync"));
    title.set_xalign(0.0);
    title.add_css_class("pane-title");
    content.append(&title);

    let configured = sync_auth_configured(&LinuxPlatform::new(), &settings);
    let subtitle_text = if configured {
        format!(
            "Signed in as {}.",
            if settings.sync_email.is_empty() {
                "unknown account"
            } else {
                &settings.sync_email
            }
        )
    } else {
        "Sign in to sync tasks across devices, or keep working locally.".to_owned()
    };
    let subtitle = gtk::Label::new(Some(&subtitle_text));
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");
    content.append(&subtitle);

    let server_entry = gtk::Entry::new();
    server_entry.set_placeholder_text(Some("Server URL, e.g. http://127.0.0.1:18080"));
    server_entry.set_text(&settings.server_url);
    server_entry.set_visible(!configured);
    content.append(&server_entry);

    let email_entry = gtk::Entry::new();
    email_entry.set_placeholder_text(Some("Email"));
    email_entry.set_visible(!configured);
    content.append(&email_entry);

    let password_entry = gtk::PasswordEntry::new();
    password_entry.set_placeholder_text(Some("Password"));
    password_entry.set_visible(!configured);
    content.append(&password_entry);

    let status = gtk::Label::new(None);
    status.set_xalign(0.0);
    status.add_css_class("dim-label");
    content.append(&status);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let local_button = gtk::Button::with_label(if configured { "Close" } else { "Work local" });
    let login_button = gtk::Button::with_label("Login / Register");
    login_button.add_css_class("suggested-action");
    let logout_button = gtk::Button::with_label("Log out");
    logout_button.add_css_class("destructive-action");
    actions.append(&local_button);
    if configured {
        actions.append(&logout_button);
    } else {
        actions.append(&login_button);
    }
    content.append(&actions);

    local_button.connect_clicked({
        let dialog = dialog.clone();
        move |_| dialog.close()
    });

    logout_button.connect_clicked({
        let dialog = dialog.clone();
        let settings_path = settings_path.clone();
        let status = status.clone();
        let on_auth_changed = on_auth_changed.clone();
        move |_| match logout_sync_auth(&LinuxPlatform::new(), &settings_path) {
            Ok(()) => {
                if let Some(on_auth_changed) = &on_auth_changed {
                    on_auth_changed();
                }
                dialog.close()
            }
            Err(error) => status.set_text(&format!("Logout failed: {error}")),
        }
    });

    login_button.connect_clicked({
        let dialog = dialog.clone();
        let status = status.clone();
        let on_auth_changed = on_auth_changed.clone();
        move |_| {
            status.set_text("Signing in…");
            let platform = LinuxPlatform::new();
            match configure_sync_auth(
                &platform,
                &settings_path,
                &server_entry.text(),
                &email_entry.text(),
                &password_entry.text(),
            ) {
                Ok(()) => {
                    status.set_text("Sync configured.");
                    if let Some(on_auth_changed) = &on_auth_changed {
                        on_auth_changed();
                    }
                    dialog.close();
                }
                Err(error) => status.set_text(&format!("Sync setup failed: {error}")),
            }
        }
    });

    if first_run {
        dialog.connect_close_request(|_| gtk::glib::Propagation::Proceed);
    }
    dialog.set_child(Some(&content));
    dialog.present();
    dialog
}

pub(crate) fn configure_sync_auth(
    platform: &LinuxPlatform,
    settings_path: &Path,
    server_url: &str,
    email: &str,
    password: &str,
) -> Result<(), String> {
    let server_url = server_url.trim().trim_end_matches('/').to_owned();
    let email = email.trim().to_owned();
    let password = password.to_string();
    if server_url.is_empty() || email.is_empty() || password.is_empty() {
        return Err("server, email, and password are required".to_owned());
    }

    let public_key = match platform.load_key(DEVICE_PRIVATE_KEY_ID) {
        Ok(private_key) => {
            public_key_from_private_key(&private_key).map_err(|error| error.to_string())?
        }
        Err(_) => init_device_keypair(platform).map_err(|error| error.to_string())?,
    };
    if platform.load_key(ACCOUNT_DATA_KEY_ID).is_err() {
        init_account(platform).map_err(|error| error.to_string())?;
    }

    let client = reqwest::blocking::Client::new();
    let register = AuthRequest {
        email: email.clone(),
        password: password.clone(),
        pub_key: Some(base64::engine::general_purpose::STANDARD.encode(public_key)),
    };
    let login = AuthRequest {
        email: email.clone(),
        password,
        pub_key: None,
    };
    let tokens = client
        .post(format!("{server_url}/auth/register"))
        .json(&register)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<TokenResponse>())
        .or_else(|_| {
            client
                .post(format!("{server_url}/auth/login"))
                .json(&login)
                .send()
                .and_then(|response| response.error_for_status())
                .and_then(|response| response.json::<TokenResponse>())
        })
        .map_err(|error| error.to_string())?;

    platform
        .store_key(AUTH_ACCESS_TOKEN_ID, tokens.jwt.as_bytes())
        .map_err(|error| error.to_string())?;
    platform
        .store_key(AUTH_REFRESH_TOKEN_ID, tokens.refresh_token.as_bytes())
        .map_err(|error| error.to_string())?;

    let mut settings = read_settings(settings_path).unwrap_or_default();
    settings.server_url = server_url;
    settings.sync_email = email;
    write_settings(settings_path, &settings).map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn logout_sync_auth(
    platform: &LinuxPlatform,
    settings_path: &Path,
) -> Result<(), String> {
    platform
        .delete_key(AUTH_ACCESS_TOKEN_ID)
        .map_err(|error| error.to_string())?;
    platform
        .delete_key(AUTH_REFRESH_TOKEN_ID)
        .map_err(|error| error.to_string())?;
    let mut settings = read_settings(settings_path).unwrap_or_default();
    settings.sync_email.clear();
    write_settings(settings_path, &settings).map_err(|error| error.to_string())?;
    Ok(())
}
