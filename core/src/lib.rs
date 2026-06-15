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

use crate::ffi::schedulable_notification_at;

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
    account_data_key_id, decrypt_task_blob, device_private_key_id,
    device_public_key_from_private_key, encrypt_task_blob, generate_account_data_key,
    generate_ffi_device_keypair, generate_local_account_bootstrap, read_plaintext_settings,
    unwrap_account_data_key, wrap_account_data_key, write_plaintext_settings, FfiAuthMethod,
    FfiBlob, FfiCoreError, FfiCoreErrorKind, FfiDefaultSort, FfiDeviceKeypair, FfiDisplayDensity,
    FfiKeybindings, FfiLocalAccountBootstrap, FfiPlaintextSettings, FfiRetryQueueEntry,
    FfiSharedTaskInvite, FfiSharedTaskRecipient, FfiSharedTaskRecipientKey, FfiSharedTaskState,
    FfiSyncStatus, FfiTagColor, FfiTask, FfiTaskFilter, FfiTaskList, FfiTaskManagerCore,
    FfiTaskPatch, FfiTaskSort, FfiTaskStatus, FfiTheme, FfiVaultSettings,
};
pub use platform::{
    init_account, init_device_keypair, MockPlatform, Platform, ScheduledNotification,
    ACCOUNT_DATA_KEY_ID, DEVICE_PRIVATE_KEY_ID,
};
pub use settings::{
    AuthMethod, DefaultSort, DisplayDensity, Keybindings, PlaintextSettings,
    PlaintextSettingsSyncPayload, Theme, VaultSettings, VaultSettingsBlob,
    PLAINTEXT_SETTINGS_SCHEMA_VERSION, VAULT_SETTINGS_ID, VAULT_SETTINGS_SCHEMA_VERSION,
};
pub use sync::{
    resolve_conflict, sync_pull, sync_push, BlobPush, PullResponse, PushResponse, RemoteBlob,
    SyncClient,
};
pub use types::{
    Blob, RetryQueueEntry, SharedTaskInvite, SharedTaskRecipient, SharedTaskState, SyncResult,
    SyncStatus, Task, TaskFilter, TaskList, TaskPatch, TaskSort, TaskStatus,
};
