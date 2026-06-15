use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::crypto::{decrypt_blob, encrypt_blob};
use crate::error::{CoreResult, SettingsError};
use crate::types::{Blob, Task, TaskStatus};
use serde::{Deserialize, Serialize};

pub const PLAINTEXT_SETTINGS_SCHEMA_VERSION: i32 = 1;
pub const VAULT_SETTINGS_SCHEMA_VERSION: i32 = 1;
pub const VAULT_SETTINGS_ID: &str = "vault_settings";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaintextSettings {
    pub schema_version: i32,
    pub server_url: String,
    pub auth_method: AuthMethod,
    pub language: String,
    pub last_sync_cursor: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    Biometric,
    Pin,
    Password,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaintextSettingsSyncPayload {
    pub schema_version: i32,
    pub server_url: String,
    pub auth_method: AuthMethod,
    pub language: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultSettings {
    pub schema_version: i32,
    pub theme: Theme,
    pub default_sort: DefaultSort,
    pub show_completed: bool,
    pub default_reminder_minutes: i32,
    pub tag_colors: BTreeMap<String, String>,
    pub display_density: DisplayDensity,
    pub first_day_of_week: i32,
    pub notification_sound: String,
    #[serde(default)]
    pub keybindings: Keybindings,
    #[serde(default = "default_show_share_revocation_warning")]
    pub show_share_revocation_warning: bool,
}

fn default_show_share_revocation_warning() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keybindings {
    pub add_task: String,
    pub search: String,
    pub close_overlay: String,
    pub confirm_rename: String,
    pub delete_task: String,
    pub toggle_done: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultSettingsBlob {
    pub id: String,
    pub blob: Blob,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultSort {
    DueAtAsc,
    UpdatedAtDesc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayDensity {
    Compact,
    Comfortable,
    Spacious,
}

impl Default for PlaintextSettings {
    fn default() -> Self {
        Self {
            schema_version: PLAINTEXT_SETTINGS_SCHEMA_VERSION,
            server_url: String::new(),
            auth_method: AuthMethod::Password,
            language: "en".to_owned(),
            last_sync_cursor: 0,
        }
    }
}

impl PlaintextSettings {
    pub fn read_from_file(path: impl AsRef<Path>) -> CoreResult<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }

        let bytes = fs::read(path)?;
        let settings = serde_json::from_slice(&bytes)?;
        Ok(settings)
    }

    pub fn write_to_file(&self, path: impl AsRef<Path>) -> CoreResult<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    pub fn sync_payload(&self) -> PlaintextSettingsSyncPayload {
        PlaintextSettingsSyncPayload {
            schema_version: self.schema_version,
            server_url: self.server_url.clone(),
            auth_method: self.auth_method,
            language: self.language.clone(),
        }
    }
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            add_task: "<Primary>n".to_owned(),
            search: "<Primary>f".to_owned(),
            close_overlay: "Escape".to_owned(),
            confirm_rename: "Return".to_owned(),
            delete_task: "Delete".to_owned(),
            toggle_done: "space".to_owned(),
        }
    }
}

impl Default for VaultSettings {
    fn default() -> Self {
        Self {
            schema_version: VAULT_SETTINGS_SCHEMA_VERSION,
            theme: Theme::System,
            default_sort: DefaultSort::DueAtAsc,
            show_completed: false,
            default_reminder_minutes: 30,
            tag_colors: BTreeMap::new(),
            display_density: DisplayDensity::Comfortable,
            first_day_of_week: 1,
            notification_sound: "default".to_owned(),
            keybindings: Keybindings::default(),
            show_share_revocation_warning: true,
        }
    }
}

impl VaultSettings {
    pub fn encrypt(&self, key: &[u8]) -> CoreResult<VaultSettingsBlob> {
        let task = self.to_reserved_task(0)?;
        Ok(VaultSettingsBlob {
            id: VAULT_SETTINGS_ID.to_owned(),
            blob: encrypt_blob(&task, key)?,
        })
    }

    pub fn decrypt(encrypted: &VaultSettingsBlob, key: &[u8]) -> CoreResult<Self> {
        if encrypted.id != VAULT_SETTINGS_ID {
            return Err(SettingsError::UnexpectedVaultSettingsId(encrypted.id.clone()).into());
        }

        let task = decrypt_blob(&encrypted.blob, key)?;
        let settings: Self = serde_json::from_str(&task.body)?;
        settings.validate_schema_version()?;
        Ok(settings)
    }

    pub fn resolve_conflict(local: &Task, remote: &Task) -> Task {
        if remote.updated_at > local.updated_at {
            remote.clone()
        } else {
            local.clone()
        }
    }

    pub(crate) fn to_reserved_task(&self, updated_at: i64) -> CoreResult<Task> {
        self.validate_schema_version()?;

        Ok(Task {
            id: uuid::Uuid::nil(),
            title: VAULT_SETTINGS_ID.to_owned(),
            body: serde_json::to_string(self)?,
            due_at: None,
            reminder_offset_ms: None,
            status: TaskStatus::Open,
            project_id: None,
            tags: Vec::new(),
            created_at: 0,
            updated_at,
            deleted: false,
            dirty: true,
        })
    }

    pub(crate) fn from_reserved_task(task: &Task) -> CoreResult<Self> {
        let settings: Self = serde_json::from_str(&task.body)?;
        settings.validate_schema_version()?;
        Ok(settings)
    }

    fn validate_schema_version(&self) -> CoreResult<()> {
        if self.schema_version != VAULT_SETTINGS_SCHEMA_VERSION {
            return Err(SettingsError::UnsupportedVaultSchemaVersion(self.schema_version).into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::generate_data_key;

    #[test]
    fn plaintext_settings_serialize_to_documented_json_shape() {
        let settings = PlaintextSettings {
            schema_version: 1,
            server_url: "https://api.example.com".to_owned(),
            auth_method: AuthMethod::Biometric,
            language: "en".to_owned(),
            last_sync_cursor: 1_717_603_200_000,
        };

        let json = serde_json::to_value(&settings).unwrap();

        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["server_url"], "https://api.example.com");
        assert_eq!(json["auth_method"], "biometric");
        assert_eq!(json["language"], "en");
        assert_eq!(json["last_sync_cursor"], 1_717_603_200_000_i64);
    }

    #[test]
    fn plaintext_settings_deserialize_from_documented_json_shape() {
        let json = r#"{
            "schema_version": 1,
            "server_url": "https://api.example.com",
            "auth_method": "pin",
            "language": "en",
            "last_sync_cursor": 1717603200000
        }"#;

        let settings: PlaintextSettings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.schema_version, 1);
        assert_eq!(settings.server_url, "https://api.example.com");
        assert_eq!(settings.auth_method, AuthMethod::Pin);
        assert_eq!(settings.language, "en");
        assert_eq!(settings.last_sync_cursor, 1_717_603_200_000);
    }

    #[test]
    fn missing_plaintext_settings_file_returns_defaults() {
        let path = temporary_settings_path("missing_plaintext_settings_file_returns_defaults");
        let _ = fs::remove_file(&path);

        let settings = PlaintextSettings::read_from_file(&path).unwrap();

        assert_eq!(settings, PlaintextSettings::default());
    }

    #[test]
    fn plaintext_settings_write_and_read_round_trip() {
        let path = temporary_settings_path("plaintext_settings_write_and_read_round_trip");
        let settings = PlaintextSettings {
            server_url: "https://api.example.com".to_owned(),
            auth_method: AuthMethod::Biometric,
            last_sync_cursor: 42,
            ..PlaintextSettings::default()
        };

        settings.write_to_file(&path).unwrap();
        let loaded = PlaintextSettings::read_from_file(&path).unwrap();

        assert_eq!(loaded, settings);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn plaintext_sync_payload_excludes_last_sync_cursor() {
        let settings = PlaintextSettings {
            last_sync_cursor: 99,
            ..PlaintextSettings::default()
        };

        let value = serde_json::to_value(settings.sync_payload()).unwrap();

        assert!(value.get("last_sync_cursor").is_none());
        assert_eq!(value["schema_version"], 1);
    }

    #[test]
    fn vault_settings_serialize_to_documented_json_shape() {
        let mut settings = VaultSettings::default();
        settings
            .tag_colors
            .insert("work".to_owned(), "#4A90D9".to_owned());

        let json = serde_json::to_value(&settings).unwrap();

        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["theme"], "system");
        assert_eq!(json["default_sort"], "due_at_asc");
        assert_eq!(json["show_completed"], false);
        assert_eq!(json["default_reminder_minutes"], 30);
        assert_eq!(json["tag_colors"]["work"], "#4A90D9");
        assert_eq!(json["display_density"], "comfortable");
        assert_eq!(json["first_day_of_week"], 1);
        assert_eq!(json["notification_sound"], "default");
        assert_eq!(json["keybindings"]["add_task"], "<Primary>n");
        assert_eq!(json["keybindings"]["search"], "<Primary>f");
    }

    #[test]
    fn vault_settings_deserialize_from_documented_json_shape() {
        let json = r##"{
            "schema_version": 1,
            "theme": "dark",
            "default_sort": "due_at_asc",
            "show_completed": false,
            "default_reminder_minutes": 30,
            "tag_colors": { "work": "#4A90D9" },
            "display_density": "comfortable",
            "first_day_of_week": 1,
            "notification_sound": "default",
            "keybindings": {
                "add_task": "<Primary><Shift>n",
                "search": "<Primary>f",
                "close_overlay": "Escape",
                "confirm_rename": "Return",
                "delete_task": "Delete",
                "toggle_done": "space"
            }
        }"##;

        let settings: VaultSettings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.theme, Theme::Dark);
        assert_eq!(settings.tag_colors["work"], "#4A90D9");
        assert_eq!(settings.keybindings.add_task, "<Primary><Shift>n");
        assert_eq!(settings.keybindings.search, "<Primary>f");
    }

    #[test]
    fn vault_settings_missing_keybindings_deserializes_defaults() {
        let json = r##"{
            "schema_version": 1,
            "theme": "dark",
            "default_sort": "due_at_asc",
            "show_completed": false,
            "default_reminder_minutes": 30,
            "tag_colors": {},
            "display_density": "comfortable",
            "first_day_of_week": 1,
            "notification_sound": "default"
        }"##;

        let settings: VaultSettings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.keybindings, Keybindings::default());
    }

    #[test]
    fn vault_settings_encrypt_and_decrypt_with_account_data_key() {
        let key = generate_data_key();
        let settings = VaultSettings::default();

        let encrypted = settings.encrypt(&key).unwrap();
        let decrypted = VaultSettings::decrypt(&encrypted, &key).unwrap();

        assert_eq!(decrypted, settings);
    }

    #[test]
    fn vault_settings_use_literal_reserved_blob_id() {
        let encrypted = VaultSettings::default()
            .encrypt(&generate_data_key())
            .unwrap();

        assert_eq!(encrypted.id, VAULT_SETTINGS_ID);
    }

    #[test]
    fn vault_settings_reject_unexpected_blob_id() {
        let key = generate_data_key();
        let mut encrypted = VaultSettings::default().encrypt(&key).unwrap();
        encrypted.id = "not_vault_settings".to_owned();

        let error = VaultSettings::decrypt(&encrypted, &key).unwrap_err();

        assert!(matches!(
            error,
            crate::error::CoreError::Settings(SettingsError::UnexpectedVaultSettingsId(id))
                if id == "not_vault_settings"
        ));
    }

    #[test]
    fn vault_settings_conflict_resolution_uses_last_write_wins() {
        let mut local = VaultSettings::default().to_reserved_task(0).unwrap();
        let mut remote = local.clone();
        local.updated_at = 10;
        remote.updated_at = 20;
        remote.body = "remote".to_owned();

        assert_eq!(VaultSettings::resolve_conflict(&local, &remote), remote);
        assert_eq!(VaultSettings::resolve_conflict(&remote, &local), remote);
    }

    #[test]
    fn unsupported_vault_schema_version_returns_error() {
        let settings = VaultSettings {
            schema_version: 2,
            ..VaultSettings::default()
        };

        let error = settings.to_reserved_task(0).unwrap_err();

        assert!(matches!(
            error,
            crate::error::CoreError::Settings(SettingsError::UnsupportedVaultSchemaVersion(2))
        ));
    }

    fn temporary_settings_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "taskmanager-core-{name}-{}-{}.json",
            std::process::id(),
            uuid::Uuid::new_v4()
        ))
    }
}
