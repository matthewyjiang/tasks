//! Client core for the local-first encrypted task manager.
//!
//! This crate owns domain types, local persistence, crypto, and sync logic.

pub mod core;
pub mod crypto;
pub mod db;
pub mod error;
pub mod ffi;
pub mod platform;
pub mod settings;
pub mod sync;
pub mod types;

uniffi::include_scaffolding!("core");

pub use core::TaskManagerCore;
pub use crypto::{
    decrypt_blob, encrypt_blob, generate_data_key, generate_device_keypair,
    public_key_from_private_key, unwrap_data_key, wrap_data_key, DeviceKeypair,
};
pub use db::LocalDatabase;
pub use error::{
    CoreError, CoreResult, CryptoError, DbError, PlatformError, SettingsError, SyncError,
};
pub use ffi::{
    decrypt_task_blob, encrypt_task_blob, generate_account_data_key, FfiBlob, FfiCoreError,
    FfiCoreErrorKind, FfiTask, FfiTaskFilter, FfiTaskManagerCore, FfiTaskPatch, FfiTaskSort,
    FfiTaskStatus,
};
pub use platform::{
    init_account, init_device_keypair, MockPlatform, Platform, ScheduledNotification,
    ACCOUNT_DATA_KEY_ID, DEVICE_PRIVATE_KEY_ID,
};
pub use settings::{
    AuthMethod, DefaultSort, DisplayDensity, PlaintextSettings, PlaintextSettingsSyncPayload,
    Theme, VaultSettings, VaultSettingsBlob, PLAINTEXT_SETTINGS_SCHEMA_VERSION, VAULT_SETTINGS_ID,
    VAULT_SETTINGS_SCHEMA_VERSION,
};
pub use sync::{
    resolve_conflict, sync_pull, sync_push, BlobPush, PullResponse, PushResponse, RemoteBlob,
    SyncClient,
};
pub use types::{
    Blob, RetryQueueEntry, SyncResult, SyncStatus, Task, TaskFilter, TaskPatch, TaskSort,
    TaskStatus,
};
