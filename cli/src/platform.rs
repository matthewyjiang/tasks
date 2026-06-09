use std::{env, fs, path::PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use serde::{Deserialize, Serialize};
use taskmanager_core::{CoreResult, Platform, PlatformError};
use uuid::Uuid;

const INSECURE_KEY_DIR_ENV: &str = "TASKMANAGER_INSECURE_KEY_DIR";
const REMINDER_DIR_ENV: &str = "TASKMANAGER_REMINDER_DIR";

#[derive(Debug, Clone)]
pub struct CliPlatform {
    offline: bool,
    key_store: KeyStore,
    reminder_store: ReminderStore,
}

impl CliPlatform {
    pub fn new(offline: bool) -> Self {
        Self {
            offline,
            key_store: KeyStore::from_env(),
            reminder_store: ReminderStore::from_env(),
        }
    }

    pub fn with_insecure_stores(offline: bool, key_dir: PathBuf, reminder_dir: PathBuf) -> Self {
        Self {
            offline,
            key_store: KeyStore::File { dir: key_dir },
            reminder_store: ReminderStore::File { dir: reminder_dir },
        }
    }
}

impl Platform for CliPlatform {
    fn store_key(&self, id: &str, bytes: &[u8]) -> CoreResult<()> {
        self.key_store.store_key(id, bytes)
    }

    fn load_key(&self, id: &str) -> CoreResult<Vec<u8>> {
        self.key_store.load_key(id)
    }

    fn delete_key(&self, id: &str) -> CoreResult<()> {
        self.key_store.delete_key(id)
    }

    fn schedule_notification(&self, task_id: Uuid, fire_at: i64, title: &str) -> CoreResult<()> {
        self.reminder_store
            .schedule_notification(task_id, fire_at, title)
    }

    fn cancel_notification(&self, task_id: Uuid) -> CoreResult<()> {
        self.reminder_store.cancel_notification(task_id)
    }

    fn network_available(&self) -> bool {
        !self.offline
    }
}

#[derive(Debug, Clone)]
enum KeyStore {
    File { dir: PathBuf },
    Unsupported { reason: &'static str },
}

impl KeyStore {
    fn from_env() -> Self {
        if let Some(dir) = env::var_os(INSECURE_KEY_DIR_ENV) {
            return Self::File {
                dir: PathBuf::from(dir),
            };
        }

        if let Some(home) = env::var_os("HOME") {
            return Self::File {
                dir: PathBuf::from(home).join(".taskmanager").join("keys"),
            };
        }

        Self::Unsupported {
            reason: "no HOME directory found for CLI key store; set TASKMANAGER_INSECURE_KEY_DIR for explicit file-backed test storage",
        }
    }

    fn store_key(&self, id: &str, bytes: &[u8]) -> CoreResult<()> {
        match self {
            Self::File { dir } => {
                ensure_private_dir(dir)?;
                let path = key_path(dir, id);
                fs::write(&path, bytes).map_err(key_store_io_error)?;
                set_private_file_permissions(&path)?;
                Ok(())
            }
            Self::Unsupported { reason } => {
                Err(PlatformError::OperationFailed((*reason).into()).into())
            }
        }
    }

    fn load_key(&self, id: &str) -> CoreResult<Vec<u8>> {
        match self {
            Self::File { dir } => fs::read(key_path(dir, id)).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    PlatformError::KeyNotFound(id.to_string()).into()
                } else {
                    key_store_io_error(error)
                }
            }),
            Self::Unsupported { reason } => {
                Err(PlatformError::OperationFailed((*reason).into()).into())
            }
        }
    }

    fn delete_key(&self, id: &str) -> CoreResult<()> {
        match self {
            Self::File { dir } => match fs::remove_file(key_path(dir, id)) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(key_store_io_error(error)),
            },
            Self::Unsupported { reason } => {
                Err(PlatformError::OperationFailed((*reason).into()).into())
            }
        }
    }
}

#[derive(Debug, Clone)]
enum ReminderStore {
    File { dir: PathBuf },
    Disabled,
}

impl ReminderStore {
    fn from_env() -> Self {
        env::var_os(REMINDER_DIR_ENV)
            .map(|dir| Self::File {
                dir: PathBuf::from(dir),
            })
            .unwrap_or(Self::Disabled)
    }

    fn schedule_notification(&self, task_id: Uuid, fire_at: i64, title: &str) -> CoreResult<()> {
        match self {
            Self::File { dir } => {
                fs::create_dir_all(dir).map_err(reminder_io_error)?;
                let reminder = StoredReminder {
                    task_id,
                    fire_at,
                    title: title.to_string(),
                    canceled: false,
                };
                let bytes = serde_json::to_vec_pretty(&reminder)
                    .map_err(|error| PlatformError::OperationFailed(error.to_string()))?;
                fs::write(reminder_path(dir, task_id), bytes).map_err(reminder_io_error)
            }
            Self::Disabled => Err(PlatformError::OperationFailed(
                "CLI reminders are not configured; set TASKMANAGER_REMINDER_DIR for headless persistence".into(),
            )
            .into()),
        }
    }

    fn cancel_notification(&self, task_id: Uuid) -> CoreResult<()> {
        match self {
            Self::File { dir } => {
                fs::create_dir_all(dir).map_err(reminder_io_error)?;
                let reminder = StoredReminder {
                    task_id,
                    fire_at: 0,
                    title: String::new(),
                    canceled: true,
                };
                let bytes = serde_json::to_vec_pretty(&reminder)
                    .map_err(|error| PlatformError::OperationFailed(error.to_string()))?;
                fs::write(reminder_path(dir, task_id), bytes).map_err(reminder_io_error)
            }
            Self::Disabled => Err(PlatformError::OperationFailed(
                "CLI reminders are not configured; set TASKMANAGER_REMINDER_DIR for headless persistence".into(),
            )
            .into()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
struct StoredReminder {
    task_id: Uuid,
    fire_at: i64,
    title: String,
    canceled: bool,
}

fn key_path(dir: &std::path::Path, id: &str) -> PathBuf {
    dir.join(format!("{}.key", hex_id(id)))
}

fn ensure_private_dir(dir: &std::path::Path) -> CoreResult<()> {
    fs::create_dir_all(dir).map_err(key_store_io_error)?;
    #[cfg(unix)]
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(key_store_io_error)?;
    Ok(())
}

fn set_private_file_permissions(path: &std::path::Path) -> CoreResult<()> {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(key_store_io_error)?;
    Ok(())
}

fn reminder_path(dir: &std::path::Path, task_id: Uuid) -> PathBuf {
    dir.join(format!("{task_id}.json"))
}

fn hex_id(id: &str) -> String {
    id.as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn key_store_io_error(error: std::io::Error) -> taskmanager_core::CoreError {
    PlatformError::OperationFailed(format!("key-store I/O failed: {error}")).into()
}

fn reminder_io_error(error: std::io::Error) -> taskmanager_core::CoreError {
    PlatformError::OperationFailed(format!("reminder I/O failed: {error}")).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskmanager_core::CoreError;

    #[test]
    fn file_backed_test_key_store_round_trips_key_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let platform = CliPlatform::with_insecure_stores(
            false,
            temp.path().join("keys"),
            temp.path().join("reminders"),
        );

        platform.store_key("account/key", b"secret").unwrap();

        assert_eq!(platform.load_key("account/key").unwrap(), b"secret");
    }

    #[test]
    fn missing_key_returns_expected_key_store_error() {
        let temp = tempfile::tempdir().unwrap();
        let platform = CliPlatform::with_insecure_stores(
            false,
            temp.path().join("keys"),
            temp.path().join("reminders"),
        );

        let error = platform.load_key("missing").unwrap_err();

        assert!(matches!(
            error,
            CoreError::Platform(PlatformError::KeyNotFound(id)) if id == "missing"
        ));
    }

    #[test]
    fn delete_key_removes_only_selected_key() {
        let temp = tempfile::tempdir().unwrap();
        let platform = CliPlatform::with_insecure_stores(
            false,
            temp.path().join("keys"),
            temp.path().join("reminders"),
        );

        platform.store_key("one", b"1").unwrap();
        platform.store_key("two", b"2").unwrap();
        platform.delete_key("one").unwrap();

        assert!(matches!(
            platform.load_key("one").unwrap_err(),
            CoreError::Platform(PlatformError::KeyNotFound(_))
        ));
        assert_eq!(platform.load_key("two").unwrap(), b"2");
    }

    #[test]
    fn insecure_key_store_is_never_selected_implicitly() {
        let platform = CliPlatform {
            offline: false,
            key_store: KeyStore::Unsupported { reason: "test" },
            reminder_store: ReminderStore::Disabled,
        };

        let error = platform.store_key("key", b"secret").unwrap_err();

        assert!(matches!(
            error,
            CoreError::Platform(PlatformError::OperationFailed(message)) if message == "test"
        ));
    }

    #[test]
    fn reminder_schedule_and_cancel_persist_expected_records() {
        let temp = tempfile::tempdir().unwrap();
        let reminder_dir = temp.path().join("reminders");
        let platform = CliPlatform::with_insecure_stores(
            false,
            temp.path().join("keys"),
            reminder_dir.clone(),
        );
        let task_id = Uuid::new_v4();

        platform
            .schedule_notification(task_id, 1_717_603_200_000, "Reminder")
            .unwrap();
        let scheduled: StoredReminder =
            serde_json::from_slice(&fs::read(reminder_path(&reminder_dir, task_id)).unwrap())
                .unwrap();
        assert_eq!(
            scheduled,
            StoredReminder {
                task_id,
                fire_at: 1_717_603_200_000,
                title: "Reminder".into(),
                canceled: false,
            }
        );

        platform.cancel_notification(task_id).unwrap();
        let canceled: StoredReminder =
            serde_json::from_slice(&fs::read(reminder_path(&reminder_dir, task_id)).unwrap())
                .unwrap();
        assert!(canceled.canceled);
        assert_eq!(canceled.task_id, task_id);
    }

    #[test]
    fn offline_forces_network_unavailable() {
        let temp = tempfile::tempdir().unwrap();
        let platform = CliPlatform::with_insecure_stores(
            true,
            temp.path().join("keys"),
            temp.path().join("reminders"),
        );

        assert!(!platform.network_available());
    }
}
