//! Client core for the local-first encrypted task manager.
//!
//! This crate owns domain types, local persistence, crypto, and sync logic.

pub mod auth;
pub mod core;
pub mod crypto;
pub mod db;
pub mod enrollment;
pub mod error;
pub mod ffi;
pub mod platform;
pub mod settings;
pub mod sync;
pub mod types;

use crate::ffi::schedulable_notification_at;

uniffi::include_scaffolding!("core");

pub use auth::{
    clear_sync_auth, configure_sync_auth, device_public_key_base64_from_platform,
    load_access_token, logout_sync_auth, normalize_sync_server_url, refresh_auth,
    sync_auth_configured, sync_auth_state, sync_server_origin, AuthClient, AuthCredentials,
    ConfigureSyncAuthResult, DeleteSessionRequest, LoginRequest, PutCurrentDeviceKeyRequest,
    RefreshTokenRequest, RegisterRequest, SyncAuthState, TokenResponse, AUTH_ACCESS_TOKEN_ID,
    AUTH_ACCOUNT_ID_ID, AUTH_REFRESH_TOKEN_ID, AUTH_SYNC_ORIGIN_ID,
};
pub use core::TaskManagerCore;
pub use crypto::{
    decrypt_blob, encrypt_blob, generate_data_key, generate_device_keypair,
    public_key_from_private_key, unwrap_data_key, wrap_data_key, DeviceKeypair,
};
pub use db::LocalDatabase;
pub use enrollment::{
    accept_wrapped_account_data_key_payload, begin_existing_account_enrollment,
    create_wrapped_account_data_key_payload, existing_account_enrollment_state, EnrollmentState,
    WrappedAccountDataKeyPayload,
};
pub use error::{
    CoreError, CoreResult, CryptoError, DbError, PlatformError, SettingsError, SyncError,
};
pub use ffi::{
    accept_ffi_wrapped_account_data_key_payload, account_data_key_id, auth_access_token_id,
    auth_account_id_id, auth_refresh_token_id, auth_sync_origin_id,
    create_ffi_wrapped_account_data_key_payload, decrypt_task_blob, device_private_key_id,
    device_public_key_from_private_key, encrypt_task_blob, ffi_begin_existing_account_enrollment,
    ffi_existing_account_enrollment_state, ffi_sync_auth_state, ffi_sync_server_origin,
    generate_account_data_key, generate_ffi_device_keypair, generate_local_account_bootstrap,
    normalize_ffi_sync_server_url, read_plaintext_settings, unwrap_account_data_key,
    wrap_account_data_key, write_plaintext_settings, FfiAuthMethod, FfiBlob, FfiBlobPush,
    FfiCoreError, FfiCoreErrorKind, FfiDefaultSort, FfiDeviceKeypair, FfiDisplayDensity,
    FfiEnrollmentState, FfiKeybindings, FfiLocalAccountBootstrap, FfiPlaintextSettings,
    FfiPlatform, FfiPullResponse, FfiPushResponse, FfiRemoteBlob, FfiRetryQueueEntry,
    FfiSharedTaskInvite, FfiSharedTaskRecipient, FfiSharedTaskRecipientKey, FfiSharedTaskState,
    FfiSyncAuthState, FfiSyncClient, FfiSyncResult, FfiSyncStatus, FfiTagColor, FfiTask,
    FfiTaskFilter, FfiTaskList, FfiTaskManagerCore, FfiTaskPatch, FfiTaskSort, FfiTaskStatus,
    FfiTheme, FfiVaultSettings, FfiWrappedAccountDataKeyPayload,
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
    resolve_conflict, sync_pull, sync_push, sync_session, BlobPush, PullResponse, PushResponse,
    RemoteBlob, SyncClient,
};
pub use types::{
    Blob, RetryQueueEntry, SharedTaskInvite, SharedTaskRecipient, SharedTaskState, SyncResult,
    SyncStatus, Task, TaskFilter, TaskList, TaskPatch, TaskSort, TaskStatus,
};
