use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::crypto::{generate_data_key, generate_device_keypair};
use crate::error::{CoreResult, PlatformError};

pub const DEVICE_PRIVATE_KEY_ID: &str = "device_private_key";
pub const ACCOUNT_DATA_KEY_ID: &str = "account_data_key";

pub trait Platform: Send + Sync {
    fn store_key(&self, id: &str, bytes: &[u8]) -> CoreResult<()>;
    fn load_key(&self, id: &str) -> CoreResult<Vec<u8>>;
    fn delete_key(&self, id: &str) -> CoreResult<()>;
    fn schedule_notification(&self, task_id: Uuid, fire_at: i64, title: &str) -> CoreResult<()>;
    fn cancel_notification(&self, task_id: Uuid) -> CoreResult<()>;
    fn network_available(&self) -> bool;
}

pub fn init_device_keypair(platform: &dyn Platform) -> CoreResult<Vec<u8>> {
    let keypair = generate_device_keypair();
    platform.store_key(DEVICE_PRIVATE_KEY_ID, &keypair.private_key)?;
    Ok(keypair.public_key)
}

pub fn init_account(platform: &dyn Platform) -> CoreResult<Vec<u8>> {
    let public_key = init_device_keypair(platform)?;
    let data_key = generate_data_key();
    platform.store_key(ACCOUNT_DATA_KEY_ID, &data_key)?;
    Ok(public_key)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledNotification {
    pub task_id: Uuid,
    pub fire_at: i64,
    pub title: String,
}

#[derive(Clone, Default)]
pub struct MockPlatform {
    keys: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    scheduled_notifications: Arc<Mutex<Vec<ScheduledNotification>>>,
    canceled_notifications: Arc<Mutex<Vec<Uuid>>>,
    network_available: bool,
}

impl MockPlatform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_network_available(network_available: bool) -> Self {
        Self {
            network_available,
            ..Self::default()
        }
    }

    pub fn scheduled_notifications(&self) -> Vec<ScheduledNotification> {
        self.scheduled_notifications.lock().unwrap().clone()
    }

    pub fn canceled_notifications(&self) -> Vec<Uuid> {
        self.canceled_notifications.lock().unwrap().clone()
    }
}

impl Platform for MockPlatform {
    fn store_key(&self, id: &str, bytes: &[u8]) -> CoreResult<()> {
        self.keys
            .lock()
            .unwrap()
            .insert(id.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn load_key(&self, id: &str) -> CoreResult<Vec<u8>> {
        self.keys
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| PlatformError::KeyNotFound(id.to_owned()).into())
    }

    fn delete_key(&self, id: &str) -> CoreResult<()> {
        self.keys.lock().unwrap().remove(id);
        Ok(())
    }

    fn schedule_notification(&self, task_id: Uuid, fire_at: i64, title: &str) -> CoreResult<()> {
        self.scheduled_notifications
            .lock()
            .unwrap()
            .push(ScheduledNotification {
                task_id,
                fire_at,
                title: title.to_owned(),
            });
        Ok(())
    }

    fn cancel_notification(&self, task_id: Uuid) -> CoreResult<()> {
        self.canceled_notifications.lock().unwrap().push(task_id);
        Ok(())
    }

    fn network_available(&self) -> bool {
        self.network_available
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;

    #[test]
    fn mock_platform_stores_loads_and_deletes_keys() {
        let platform = MockPlatform::new();

        platform.store_key("key", b"secret").unwrap();
        assert_eq!(platform.load_key("key").unwrap(), b"secret");

        platform.delete_key("key").unwrap();
        let error = platform.load_key("key").unwrap_err();
        assert!(matches!(
            error,
            CoreError::Platform(PlatformError::KeyNotFound(id)) if id == "key"
        ));
    }

    #[test]
    fn loading_missing_key_returns_clear_error() {
        let platform = MockPlatform::new();

        let error = platform.load_key("missing").unwrap_err();

        assert!(matches!(
            error,
            CoreError::Platform(PlatformError::KeyNotFound(id)) if id == "missing"
        ));
    }

    #[test]
    fn init_device_keypair_stores_private_key_and_returns_public_key() {
        let platform = MockPlatform::new();

        let public_key = init_device_keypair(&platform).unwrap();
        let private_key = platform.load_key(DEVICE_PRIVATE_KEY_ID).unwrap();

        assert!(!public_key.is_empty());
        assert_eq!(private_key.len(), 32);
        assert_ne!(public_key, private_key);
    }

    #[test]
    fn init_account_stores_data_key_and_returns_device_public_key() {
        let platform = MockPlatform::new();

        let public_key = init_account(&platform).unwrap();
        let private_key = platform.load_key(DEVICE_PRIVATE_KEY_ID).unwrap();
        let data_key = platform.load_key(ACCOUNT_DATA_KEY_ID).unwrap();

        assert!(!public_key.is_empty());
        assert_eq!(private_key.len(), 32);
        assert_eq!(data_key.len(), 32);
    }

    #[test]
    fn notification_calls_are_recorded_with_expected_values() {
        let platform = MockPlatform::new();
        let task_id = Uuid::new_v4();

        platform
            .schedule_notification(task_id, 1_717_603_200_000, "Reminder")
            .unwrap();
        platform.cancel_notification(task_id).unwrap();

        assert_eq!(
            platform.scheduled_notifications(),
            vec![ScheduledNotification {
                task_id,
                fire_at: 1_717_603_200_000,
                title: "Reminder".to_owned(),
            }]
        );
        assert_eq!(platform.canceled_notifications(), vec![task_id]);
    }

    #[test]
    fn network_available_returns_configured_value() {
        assert!(MockPlatform::with_network_available(true).network_available());
        assert!(!MockPlatform::with_network_available(false).network_available());
    }
}
