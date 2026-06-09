use taskmanager_core::{CoreResult, Platform, PlatformError};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CliPlatform {
    offline: bool,
}

impl CliPlatform {
    pub fn new(offline: bool) -> Self {
        Self { offline }
    }
}

impl Platform for CliPlatform {
    fn store_key(&self, _id: &str, _bytes: &[u8]) -> CoreResult<()> {
        Err(
            PlatformError::OperationFailed("CLI key store is not implemented yet".to_string())
                .into(),
        )
    }

    fn load_key(&self, id: &str) -> CoreResult<Vec<u8>> {
        Err(PlatformError::KeyNotFound(id.to_string()).into())
    }

    fn delete_key(&self, _id: &str) -> CoreResult<()> {
        Err(
            PlatformError::OperationFailed("CLI key store is not implemented yet".to_string())
                .into(),
        )
    }

    fn schedule_notification(&self, _task_id: Uuid, _fire_at: i64, _title: &str) -> CoreResult<()> {
        Err(
            PlatformError::OperationFailed("CLI notifications are not implemented yet".to_string())
                .into(),
        )
    }

    fn cancel_notification(&self, _task_id: Uuid) -> CoreResult<()> {
        Err(
            PlatformError::OperationFailed("CLI notifications are not implemented yet".to_string())
                .into(),
        )
    }

    fn network_available(&self) -> bool {
        !self.offline
    }
}
