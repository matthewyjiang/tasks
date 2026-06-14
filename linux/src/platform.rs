use taskmanager_core::{CoreError, CoreResult, Platform, PlatformError, Task};
use uuid::Uuid;

use crate::notifications::{
    reconcile_task_notification, NotificationScheduler, SystemdUserNotificationScheduler,
};

#[derive(Clone, Debug)]
pub struct LinuxPlatform {
    service_name: String,
    offline: bool,
}

impl Default for LinuxPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxPlatform {
    pub fn new() -> Self {
        Self {
            service_name: "tsk".to_owned(),
            offline: false,
        }
    }

    #[cfg(test)]
    pub fn offline() -> Self {
        Self {
            service_name: "tsk-test".to_owned(),
            offline: true,
        }
    }

    fn entry(&self, id: &str) -> CoreResult<keyring::Entry> {
        keyring::Entry::new(&self.service_name, id).map_err(|error| {
            PlatformError::OperationFailed(format!("failed to open keyring entry {id}: {error}"))
                .into()
        })
    }

    pub fn sync_task_notification(
        &self,
        task: &Task,
        now_ms: i64,
        database_path: Option<std::path::PathBuf>,
    ) -> CoreResult<()> {
        let scheduler = SystemdUserNotificationScheduler::new(database_path)?;
        reconcile_task_notification(&scheduler, task, now_ms)
    }

    pub fn cancel_task_notification(
        &self,
        task_id: Uuid,
        database_path: Option<std::path::PathBuf>,
    ) -> CoreResult<()> {
        SystemdUserNotificationScheduler::new(database_path)?.cancel(task_id)
    }
}

impl Platform for LinuxPlatform {
    fn store_key(&self, id: &str, bytes: &[u8]) -> CoreResult<()> {
        let encoded = hex_encode(bytes);
        self.entry(id)?.set_password(&encoded).map_err(|error| {
            PlatformError::OperationFailed(format!("failed to store key {id}: {error}")).into()
        })
    }

    fn load_key(&self, id: &str) -> CoreResult<Vec<u8>> {
        let encoded = self.entry(id)?.get_password().map_err(|error| {
            let platform_error = match error {
                keyring::Error::NoEntry => PlatformError::KeyNotFound(id.to_owned()),
                other => {
                    PlatformError::OperationFailed(format!("failed to load key {id}: {other}"))
                }
            };
            CoreError::from(platform_error)
        })?;
        hex_decode(&encoded).map_err(|error| {
            PlatformError::OperationFailed(format!("stored key {id} is invalid: {error}")).into()
        })
    }

    fn delete_key(&self, id: &str) -> CoreResult<()> {
        match self.entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(PlatformError::OperationFailed(format!(
                "failed to delete key {id}: {error}"
            ))
            .into()),
        }
    }

    fn schedule_notification(&self, task_id: Uuid, fire_at: i64, _title: &str) -> CoreResult<()> {
        SystemdUserNotificationScheduler::new(None)?.schedule(task_id, fire_at)
    }

    fn cancel_notification(&self, task_id: Uuid) -> CoreResult<()> {
        SystemdUserNotificationScheduler::new(None)?.cancel(task_id)
    }

    fn network_available(&self) -> bool {
        !self.offline
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(input: &str) -> Result<Vec<u8>, String> {
    if !input.len().is_multiple_of(2) {
        return Err("hex string has odd length".to_owned());
    }
    (0..input.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&input[index..index + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip_preserves_bytes() {
        let bytes = [0, 1, 2, 15, 16, 254, 255];
        assert_eq!(hex_decode(&hex_encode(&bytes)).unwrap(), bytes);
    }

    #[test]
    fn offline_platform_reports_no_network() {
        assert!(!LinuxPlatform::offline().network_available());
    }
}
