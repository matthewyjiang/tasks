//! Client core for the local-first encrypted task manager.
//!
//! This crate owns domain types, local persistence, crypto, and sync logic.

pub mod core;
pub mod crypto;
pub mod db;
pub mod error;
pub mod platform;
pub mod types;

pub use core::TaskManagerCore;
pub use crypto::{
    decrypt_blob, encrypt_blob, generate_data_key, generate_device_keypair, unwrap_data_key,
    wrap_data_key, DeviceKeypair,
};
pub use db::LocalDatabase;
pub use error::{CoreError, CoreResult, CryptoError, DbError, PlatformError, SyncError};
pub use platform::{
    init_account, init_device_keypair, MockPlatform, Platform, ScheduledNotification,
    ACCOUNT_DATA_KEY_ID, DEVICE_PRIVATE_KEY_ID,
};
pub use types::{Blob, SyncResult, Task, TaskFilter, TaskPatch, TaskSort, TaskStatus};
