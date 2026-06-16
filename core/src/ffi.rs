use std::fmt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use uuid::Uuid;

use crate::auth::{
    configure_sync_auth, device_public_key_base64_from_platform, load_access_token,
    logout_sync_auth, normalize_sync_server_url, refresh_auth, sync_auth_state, sync_server_origin,
    AuthClient, AuthCredentials, ConfigureSyncAuthResult, DeleteSessionRequest, LoginRequest,
    PutCurrentDeviceKeyRequest, RefreshTokenRequest, RegisterRequest, SyncAuthState, TokenResponse,
    AUTH_ACCESS_TOKEN_ID, AUTH_ACCOUNT_ID_ID, AUTH_REFRESH_TOKEN_ID, AUTH_SYNC_ORIGIN_ID,
};
use crate::core::TaskManagerCore;
use crate::crypto::{
    decrypt_blob, encrypt_blob, generate_data_key, generate_device_keypair,
    public_key_from_private_key, unwrap_data_key, wrap_data_key, DeviceKeypair,
};
use crate::enrollment::{
    accept_wrapped_account_data_key_payload, begin_existing_account_enrollment,
    create_wrapped_account_data_key_payload, existing_account_enrollment_state, EnrollmentState,
    WrappedAccountDataKeyPayload,
};
use crate::error::{CoreError, CoreResult, SyncError};
use crate::platform::{Platform, ACCOUNT_DATA_KEY_ID, DEVICE_PRIVATE_KEY_ID};
use crate::settings::{
    AuthMethod, DefaultSort, DisplayDensity, Keybindings, PlaintextSettings, Theme, VaultSettings,
};
use crate::sync::{BlobPush, PullResponse, PushResponse, RemoteBlob, SyncClient};
use crate::types::{
    Blob, RetryQueueEntry, SharedTaskInvite, SharedTaskRecipient, SharedTaskState, SyncResult,
    SyncStatus, Task, TaskFilter, TaskList, TaskPatch, TaskSort, TaskStatus,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiTaskStatus {
    Open,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiTaskSort {
    UpdatedAtDesc,
    UpdatedAtAsc,
    DueAtAsc,
    DueAtDesc,
    CreatedAtAsc,
    CreatedAtDesc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiTask {
    pub id: String,
    pub title: String,
    pub body: String,
    pub due_at: Option<i64>,
    pub reminder_offset_ms: Option<i64>,
    pub status: FfiTaskStatus,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted: bool,
    pub dirty: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiTaskList {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted: bool,
    pub dirty: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FfiTaskPatch {
    pub title: Option<String>,
    pub body: Option<String>,
    pub due_at: Option<i64>,
    pub clear_due_at: bool,
    pub reminder_offset_ms: Option<i64>,
    pub clear_reminder_offset_ms: bool,
    pub status: Option<FfiTaskStatus>,
    pub project_id: Option<String>,
    pub clear_project_id: bool,
    pub tags: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FfiTaskFilter {
    pub status: Option<FfiTaskStatus>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub due_after: Option<i64>,
    pub due_before: Option<i64>,
    pub include_deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiBlob {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiAuthMethod {
    Biometric,
    Pin,
    Password,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiSyncAuthState {
    LocalOnlyReady,
    AuthenticatedEnrollmentPending,
    SyncReady,
    AuthRequired,
    MisconfiguredOrigin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiEnrollmentState {
    LocalOnlyReady,
    ExistingAccountPending,
    SyncReady,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiTheme {
    Light,
    Dark,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiDefaultSort {
    DueAtAsc,
    UpdatedAtDesc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiDisplayDensity {
    Compact,
    Comfortable,
    Spacious,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiTagColor {
    pub tag: String,
    pub color: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiKeybindings {
    pub add_task: String,
    pub search: String,
    pub close_overlay: String,
    pub confirm_rename: String,
    pub delete_task: String,
    pub toggle_done: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiPlaintextSettings {
    pub schema_version: i32,
    pub server_url: String,
    pub auth_method: FfiAuthMethod,
    pub language: String,
    pub last_sync_cursor: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiVaultSettings {
    pub schema_version: i32,
    pub theme: FfiTheme,
    pub default_sort: FfiDefaultSort,
    pub show_completed: bool,
    pub default_reminder_minutes: i32,
    pub tag_colors: Vec<FfiTagColor>,
    pub display_density: FfiDisplayDensity,
    pub first_day_of_week: i32,
    pub notification_sound: String,
    pub keybindings: FfiKeybindings,
    pub show_share_revocation_warning: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiSharedTaskRecipient {
    pub task_id: String,
    pub recipient_id: String,
    pub wrapped_task_key: FfiBlob,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FfiSharedTaskState {
    pub task_id: String,
    pub owner_id: Option<String>,
    pub task_key: Vec<u8>,
    pub recipients: Vec<FfiSharedTaskRecipient>,
    pub accepted_at: Option<i64>,
    pub revoked_at: Option<i64>,
}

impl fmt::Debug for FfiSharedTaskState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FfiSharedTaskState")
            .field("task_id", &self.task_id)
            .field("owner_id", &self.owner_id)
            .field("task_key", &"<redacted>")
            .field("recipients", &self.recipients)
            .field("accepted_at", &self.accepted_at)
            .field("revoked_at", &self.revoked_at)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiSharedTaskInvite {
    pub task_id: String,
    pub owner_id: String,
    pub recipient_id: String,
    pub wrapped_task_key: FfiBlob,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiSharedTaskRecipientKey {
    pub recipient_id: String,
    pub public_key: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiSyncStatus {
    pub dirty_count: u64,
    pub retry_queue_depth: u64,
    pub cursor: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiRetryQueueEntry {
    pub task_id: String,
    pub attempt: i64,
    pub next_retry: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiBlobPush {
    pub task_id: String,
    pub blob: FfiBlob,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiRemoteBlob {
    pub task_id: String,
    pub blob: Option<FfiBlob>,
    pub updated_at: i64,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiPushResponse {
    pub accepted_task_ids: Vec<String>,
    pub failed_task_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiPullResponse {
    pub blobs: Vec<FfiRemoteBlob>,
    pub cursor: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiSyncResult {
    pub pushed: u64,
    pub pulled: u64,
    pub failed: u64,
    pub cursor: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiAuthCredentials {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiRegisterRequest {
    pub email: String,
    pub password: String,
    pub pub_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiLoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiRefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiDeleteSessionRequest {
    pub refresh_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiPutCurrentDeviceKeyRequest {
    pub pub_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiTokenResponse {
    pub jwt: String,
    pub refresh_token: String,
    pub user_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiConfigureSyncAuthResult {
    pub server_url: String,
    pub sync_origin: String,
    pub account_id: Option<String>,
    pub state: FfiSyncAuthState,
}

pub trait FfiAuthClient: Send + Sync {
    fn register_account(
        &self,
        server_url: String,
        request: FfiRegisterRequest,
    ) -> Result<FfiTokenResponse, FfiCoreError>;
    fn login(
        &self,
        server_url: String,
        request: FfiLoginRequest,
    ) -> Result<FfiTokenResponse, FfiCoreError>;
    fn refresh(
        &self,
        server_url: String,
        request: FfiRefreshTokenRequest,
    ) -> Result<FfiTokenResponse, FfiCoreError>;
    fn delete_session(
        &self,
        server_url: String,
        request: FfiDeleteSessionRequest,
    ) -> Result<(), FfiCoreError>;
    fn put_current_device_key(
        &self,
        server_url: String,
        access_token: String,
        request: FfiPutCurrentDeviceKeyRequest,
    ) -> Result<(), FfiCoreError>;
}

pub trait FfiSyncClient: Send + Sync {
    fn push_blobs(&self, blobs: Vec<FfiBlobPush>) -> Result<FfiPushResponse, FfiCoreError>;
    fn delete_blobs(&self, task_ids: Vec<String>) -> Result<FfiPushResponse, FfiCoreError>;
    fn pull_blobs(&self, since: i64) -> Result<FfiPullResponse, FfiCoreError>;
}

#[derive(Clone, PartialEq, Eq)]
pub struct FfiDeviceKeypair {
    pub private_key: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl fmt::Debug for FfiDeviceKeypair {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FfiDeviceKeypair")
            .field("private_key", &"<redacted>")
            .field("public_key", &self.public_key)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FfiLocalAccountBootstrap {
    pub device_private_key: Vec<u8>,
    pub device_public_key: Vec<u8>,
    pub account_data_key: Vec<u8>,
}

impl fmt::Debug for FfiLocalAccountBootstrap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FfiLocalAccountBootstrap")
            .field("device_private_key", &"<redacted>")
            .field("device_public_key", &self.device_public_key)
            .field("account_data_key", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiWrappedAccountDataKeyPayload {
    pub sender_public_key: Vec<u8>,
    pub recipient_public_key: Vec<u8>,
    pub wrapped_account_data_key: FfiBlob,
}

pub trait FfiPlatform: Send + Sync {
    fn store_key(&self, id: String, bytes: Vec<u8>) -> Result<(), FfiCoreError>;
    fn load_key(&self, id: String) -> Result<Vec<u8>, FfiCoreError>;
    fn delete_key(&self, id: String) -> Result<(), FfiCoreError>;
    fn network_available(&self) -> bool;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiCoreErrorKind {
    CryptoError,
    DatabaseError,
    SyncError,
    PlatformError,
    SettingsError,
    SerializationError,
    InvalidUuid,
    InvalidNonce,
    InvalidPatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiCoreError {
    CryptoError { error_message: String },
    DatabaseError { error_message: String },
    SyncError { error_message: String },
    PlatformError { error_message: String },
    SettingsError { error_message: String },
    SerializationError { error_message: String },
    InvalidUuid { error_message: String },
    InvalidNonce { error_message: String },
    InvalidPatch { error_message: String },
}

impl FfiCoreError {
    pub fn kind(&self) -> FfiCoreErrorKind {
        match self {
            Self::CryptoError { .. } => FfiCoreErrorKind::CryptoError,
            Self::DatabaseError { .. } => FfiCoreErrorKind::DatabaseError,
            Self::SyncError { .. } => FfiCoreErrorKind::SyncError,
            Self::PlatformError { .. } => FfiCoreErrorKind::PlatformError,
            Self::SettingsError { .. } => FfiCoreErrorKind::SettingsError,
            Self::SerializationError { .. } => FfiCoreErrorKind::SerializationError,
            Self::InvalidUuid { .. } => FfiCoreErrorKind::InvalidUuid,
            Self::InvalidNonce { .. } => FfiCoreErrorKind::InvalidNonce,
            Self::InvalidPatch { .. } => FfiCoreErrorKind::InvalidPatch,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::CryptoError { error_message }
            | Self::DatabaseError { error_message }
            | Self::SyncError { error_message }
            | Self::PlatformError { error_message }
            | Self::SettingsError { error_message }
            | Self::SerializationError { error_message }
            | Self::InvalidUuid { error_message }
            | Self::InvalidNonce { error_message }
            | Self::InvalidPatch { error_message } => error_message,
        }
    }
}

impl fmt::Display for FfiCoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message())
    }
}

impl std::error::Error for FfiCoreError {}

pub struct FfiTaskManagerCore {
    inner: Mutex<TaskManagerCore>,
}

impl FfiTaskManagerCore {
    pub fn new(database_path: String) -> Result<Self, FfiCoreError> {
        let inner = TaskManagerCore::open(Path::new(&database_path)).map_err(FfiCoreError::from)?;
        Ok(Self {
            inner: Mutex::new(inner),
        })
    }

    pub fn create_list(&self, name: String) -> Result<FfiTaskList, FfiCoreError> {
        self.inner()?
            .create_list(name)
            .map(FfiTaskList::from)
            .map_err(FfiCoreError::from)
    }

    pub fn list_task_lists(&self) -> Result<Vec<FfiTaskList>, FfiCoreError> {
        self.inner()?
            .list_task_lists()
            .map(|lists| lists.into_iter().map(FfiTaskList::from).collect())
            .map_err(FfiCoreError::from)
    }

    pub fn update_list(&self, list_id: String, name: String) -> Result<FfiTaskList, FfiCoreError> {
        self.inner()?
            .update_list(parse_uuid(&list_id)?, name)
            .map(FfiTaskList::from)
            .map_err(FfiCoreError::from)
    }

    pub fn delete_list(&self, list_id: String) -> Result<(), FfiCoreError> {
        self.inner()?
            .delete_list(parse_uuid(&list_id)?)
            .map_err(FfiCoreError::from)
    }

    pub fn create_task(
        &self,
        title: String,
        body: String,
        due_at: Option<i64>,
    ) -> Result<FfiTask, FfiCoreError> {
        self.inner()?
            .create_task(title, body, due_at)
            .map(FfiTask::from)
            .map_err(FfiCoreError::from)
    }

    pub fn create_task_with_options(
        &self,
        title: String,
        body: String,
        due_at: Option<i64>,
        project_id: Option<String>,
        tags: Vec<String>,
    ) -> Result<FfiTask, FfiCoreError> {
        let patch = FfiTaskPatch {
            project_id,
            tags: Some(tags),
            ..FfiTaskPatch::default()
        };
        let validated_patch = TaskPatch::try_from(patch)?;
        self.inner()?
            .create_task_with_options(
                title,
                body,
                due_at,
                validated_patch.project_id.unwrap_or(None),
                validated_patch.tags.unwrap_or_default(),
            )
            .map(FfiTask::from)
            .map_err(FfiCoreError::from)
    }

    pub fn get_task(&self, task_id: String) -> Result<FfiTask, FfiCoreError> {
        self.inner()?
            .get_task(parse_uuid(&task_id)?)
            .map(FfiTask::from)
            .map_err(FfiCoreError::from)
    }

    pub fn update_task(
        &self,
        task_id: String,
        patch: FfiTaskPatch,
    ) -> Result<FfiTask, FfiCoreError> {
        self.inner()?
            .update_task(parse_uuid(&task_id)?, patch.try_into()?)
            .map(FfiTask::from)
            .map_err(FfiCoreError::from)
    }

    pub fn delete_task(&self, task_id: String) -> Result<(), FfiCoreError> {
        self.inner()?
            .delete_task(parse_uuid(&task_id)?)
            .map_err(FfiCoreError::from)
    }

    pub fn list_tasks(
        &self,
        filter: FfiTaskFilter,
        sort: FfiTaskSort,
    ) -> Result<Vec<FfiTask>, FfiCoreError> {
        self.inner()?
            .list_tasks(filter.try_into()?, sort.into())
            .map(|tasks| tasks.into_iter().map(FfiTask::from).collect())
            .map_err(FfiCoreError::from)
    }

    pub fn search_tasks(&self, query: String) -> Result<Vec<FfiTask>, FfiCoreError> {
        self.inner()?
            .search_tasks(query)
            .map(|tasks| tasks.into_iter().map(FfiTask::from).collect())
            .map_err(FfiCoreError::from)
    }

    pub fn share_task_with_recipient(
        &self,
        task_id: String,
        recipient_id: String,
        recipient_public_key: Vec<u8>,
        owner_private_key: Vec<u8>,
    ) -> Result<FfiSharedTaskRecipient, FfiCoreError> {
        self.inner()?
            .share_task_with_recipient(
                parse_uuid(&task_id)?,
                parse_uuid(&recipient_id)?,
                &recipient_public_key,
                &owner_private_key,
            )
            .map(FfiSharedTaskRecipient::from)
            .map_err(FfiCoreError::from)
    }

    pub fn accept_shared_task_invite(
        &self,
        invite: FfiSharedTaskInvite,
        owner_public_key: Vec<u8>,
        recipient_private_key: Vec<u8>,
    ) -> Result<FfiSharedTaskState, FfiCoreError> {
        self.inner()?
            .accept_shared_task_invite(
                invite.try_into()?,
                &owner_public_key,
                &recipient_private_key,
            )
            .map(FfiSharedTaskState::from)
            .map_err(FfiCoreError::from)
    }

    pub fn revoke_shared_task_recipient(
        &self,
        task_id: String,
        recipient_id: String,
        remaining_recipient_public_keys: Vec<FfiSharedTaskRecipientKey>,
        owner_private_key: Vec<u8>,
    ) -> Result<FfiSharedTaskState, FfiCoreError> {
        let remaining_recipient_public_keys = remaining_recipient_public_keys
            .into_iter()
            .map(|key| Ok((parse_uuid(&key.recipient_id)?, key.public_key)))
            .collect::<Result<Vec<_>, FfiCoreError>>()?;
        self.inner()?
            .revoke_shared_task_recipient(
                parse_uuid(&task_id)?,
                parse_uuid(&recipient_id)?,
                remaining_recipient_public_keys,
                &owner_private_key,
            )
            .map(FfiSharedTaskState::from)
            .map_err(FfiCoreError::from)
    }

    pub fn shared_task_state(&self, task_id: String) -> Result<FfiSharedTaskState, FfiCoreError> {
        self.inner()?
            .shared_task_state(parse_uuid(&task_id)?)
            .map(FfiSharedTaskState::from)
            .map_err(FfiCoreError::from)
    }

    pub fn shared_task_revocation_notice(&self) -> String {
        SharedTaskState::revocation_notice().to_owned()
    }

    pub fn vault_settings(&self) -> Result<FfiVaultSettings, FfiCoreError> {
        self.inner()?
            .vault_settings()
            .map(FfiVaultSettings::from)
            .map_err(FfiCoreError::from)
    }

    pub fn update_vault_settings(
        &self,
        settings: FfiVaultSettings,
    ) -> Result<FfiVaultSettings, FfiCoreError> {
        self.inner()?
            .update_vault_settings(settings.try_into()?)
            .map(FfiVaultSettings::from)
            .map_err(FfiCoreError::from)
    }

    pub fn sync_status(&self) -> Result<FfiSyncStatus, FfiCoreError> {
        self.inner()?
            .sync_status()
            .map(FfiSyncStatus::from)
            .map_err(FfiCoreError::from)
    }

    pub fn retry_queue_entries(&self) -> Result<Vec<FfiRetryQueueEntry>, FfiCoreError> {
        self.inner()?
            .retry_queue_entries()
            .map(|entries| entries.into_iter().map(FfiRetryQueueEntry::from).collect())
            .map_err(FfiCoreError::from)
    }

    pub fn queue_sync_retry(
        &self,
        task_id: String,
        now: i64,
    ) -> Result<FfiRetryQueueEntry, FfiCoreError> {
        self.inner()?
            .queue_sync_retry(parse_uuid(&task_id)?, now)
            .map(FfiRetryQueueEntry::from)
            .map_err(FfiCoreError::from)
    }

    pub fn sync_run(
        &self,
        network_available: bool,
        client: Box<dyn FfiSyncClient>,
        data_key: Vec<u8>,
    ) -> Result<FfiSyncResult, FfiCoreError> {
        let platform = FfiSyncPlatform {
            online: network_available,
        };
        let adapter = FfiSyncClientAdapter { client };
        self.inner()?
            .sync_run(&platform, &adapter, &data_key)
            .map(FfiSyncResult::from)
            .map_err(FfiCoreError::from)
    }

    fn inner(&self) -> Result<MutexGuard<'_, TaskManagerCore>, FfiCoreError> {
        self.inner.lock().map_err(|_| FfiCoreError::DatabaseError {
            error_message: "task manager core lock poisoned".to_owned(),
        })
    }
}

pub fn encrypt_task_blob(task: FfiTask, key: Vec<u8>) -> Result<FfiBlob, FfiCoreError> {
    encrypt_blob(&task.try_into()?, &key)
        .map(FfiBlob::from)
        .map_err(FfiCoreError::from)
}

pub fn decrypt_task_blob(blob: FfiBlob, key: Vec<u8>) -> Result<FfiTask, FfiCoreError> {
    decrypt_blob(&blob.try_into()?, &key)
        .map(FfiTask::from)
        .map_err(FfiCoreError::from)
}

pub fn schedulable_notification_at(
    task: FfiTask,
    now_ms: i64,
) -> Result<Option<i64>, FfiCoreError> {
    let task: Task = task.try_into()?;
    Ok(task.schedulable_notification_at(now_ms))
}

pub fn generate_account_data_key() -> Vec<u8> {
    generate_data_key().to_vec()
}

pub fn generate_ffi_device_keypair() -> FfiDeviceKeypair {
    generate_device_keypair().into()
}

pub fn generate_local_account_bootstrap() -> FfiLocalAccountBootstrap {
    let device_keypair = generate_device_keypair();
    FfiLocalAccountBootstrap {
        device_private_key: device_keypair.private_key,
        device_public_key: device_keypair.public_key,
        account_data_key: generate_data_key().to_vec(),
    }
}

pub fn device_public_key_from_private_key(private_key: Vec<u8>) -> Result<Vec<u8>, FfiCoreError> {
    public_key_from_private_key(&private_key).map_err(FfiCoreError::from)
}

pub fn wrap_account_data_key(
    data_key: Vec<u8>,
    peer_public_key: Vec<u8>,
    own_private_key: Vec<u8>,
) -> Result<FfiBlob, FfiCoreError> {
    wrap_data_key(&data_key, &peer_public_key, &own_private_key)
        .map(FfiBlob::from)
        .map_err(FfiCoreError::from)
}

pub fn unwrap_account_data_key(
    wrapped: FfiBlob,
    peer_public_key: Vec<u8>,
    own_private_key: Vec<u8>,
) -> Result<Vec<u8>, FfiCoreError> {
    unwrap_data_key(&wrapped.try_into()?, &peer_public_key, &own_private_key)
        .map(|key| key.to_vec())
        .map_err(FfiCoreError::from)
}

pub fn device_private_key_id() -> String {
    DEVICE_PRIVATE_KEY_ID.to_owned()
}

pub fn account_data_key_id() -> String {
    ACCOUNT_DATA_KEY_ID.to_owned()
}

pub fn auth_access_token_id() -> String {
    AUTH_ACCESS_TOKEN_ID.to_owned()
}

pub fn auth_refresh_token_id() -> String {
    AUTH_REFRESH_TOKEN_ID.to_owned()
}

pub fn auth_sync_origin_id() -> String {
    AUTH_SYNC_ORIGIN_ID.to_owned()
}

pub fn auth_account_id_id() -> String {
    AUTH_ACCOUNT_ID_ID.to_owned()
}

pub fn normalize_ffi_sync_server_url(server_url: String) -> Result<String, FfiCoreError> {
    normalize_sync_server_url(&server_url).map_err(FfiCoreError::from)
}

pub fn ffi_sync_server_origin(server_url: String) -> Result<String, FfiCoreError> {
    sync_server_origin(&server_url).map_err(FfiCoreError::from)
}

pub fn ffi_sync_auth_state(platform: Box<dyn FfiPlatform>, server_url: String) -> FfiSyncAuthState {
    let adapter = FfiPlatformAdapter { platform };
    sync_auth_state(&adapter, &server_url).into()
}

pub fn ffi_configure_sync_auth(
    platform: Box<dyn FfiPlatform>,
    auth_client: Box<dyn FfiAuthClient>,
    server_url: String,
    credentials: FfiAuthCredentials,
    register_public_key_base64: String,
) -> Result<FfiConfigureSyncAuthResult, FfiCoreError> {
    let platform = FfiPlatformAdapter { platform };
    let auth_client = FfiAuthClientAdapter {
        client: auth_client,
    };
    configure_sync_auth(
        &platform,
        &auth_client,
        &server_url,
        credentials.into(),
        register_public_key_base64,
    )
    .map(FfiConfigureSyncAuthResult::from)
    .map_err(FfiCoreError::from)
}

pub fn ffi_refresh_auth(
    platform: Box<dyn FfiPlatform>,
    auth_client: Box<dyn FfiAuthClient>,
    server_url: String,
) -> Result<FfiTokenResponse, FfiCoreError> {
    let platform = FfiPlatformAdapter { platform };
    let auth_client = FfiAuthClientAdapter {
        client: auth_client,
    };
    refresh_auth(&platform, &auth_client, &server_url)
        .map(FfiTokenResponse::from)
        .map_err(FfiCoreError::from)
}

pub fn ffi_logout_sync_auth(
    platform: Box<dyn FfiPlatform>,
    auth_client: Box<dyn FfiAuthClient>,
    server_url: String,
) -> Result<(), FfiCoreError> {
    let platform = FfiPlatformAdapter { platform };
    let auth_client = FfiAuthClientAdapter {
        client: auth_client,
    };
    logout_sync_auth(&platform, &auth_client, &server_url).map_err(FfiCoreError::from)
}

pub fn ffi_load_access_token(platform: Box<dyn FfiPlatform>) -> Result<String, FfiCoreError> {
    let platform = FfiPlatformAdapter { platform };
    load_access_token(&platform).map_err(FfiCoreError::from)
}

pub fn ffi_device_public_key_base64_from_platform(
    platform: Box<dyn FfiPlatform>,
) -> Result<String, FfiCoreError> {
    let platform = FfiPlatformAdapter { platform };
    device_public_key_base64_from_platform(&platform).map_err(FfiCoreError::from)
}

pub fn ffi_existing_account_enrollment_state(platform: Box<dyn FfiPlatform>) -> FfiEnrollmentState {
    let adapter = FfiPlatformAdapter { platform };
    existing_account_enrollment_state(&adapter).into()
}

pub fn ffi_begin_existing_account_enrollment(
    platform: Box<dyn FfiPlatform>,
) -> Result<FfiEnrollmentState, FfiCoreError> {
    let adapter = FfiPlatformAdapter { platform };
    begin_existing_account_enrollment(&adapter)
        .map(FfiEnrollmentState::from)
        .map_err(FfiCoreError::from)
}

pub fn create_ffi_wrapped_account_data_key_payload(
    account_data_key: Vec<u8>,
    recipient_public_key: Vec<u8>,
    sender_private_key: Vec<u8>,
) -> Result<FfiWrappedAccountDataKeyPayload, FfiCoreError> {
    create_wrapped_account_data_key_payload(
        &account_data_key,
        &recipient_public_key,
        &sender_private_key,
    )
    .map(FfiWrappedAccountDataKeyPayload::from)
    .map_err(FfiCoreError::from)
}

pub fn accept_ffi_wrapped_account_data_key_payload(
    platform: Box<dyn FfiPlatform>,
    payload: FfiWrappedAccountDataKeyPayload,
) -> Result<FfiEnrollmentState, FfiCoreError> {
    let adapter = FfiPlatformAdapter { platform };
    let payload = WrappedAccountDataKeyPayload::try_from(payload)?;
    accept_wrapped_account_data_key_payload(&adapter, &payload)
        .map(FfiEnrollmentState::from)
        .map_err(FfiCoreError::from)
}

pub fn read_plaintext_settings(path: String) -> Result<FfiPlaintextSettings, FfiCoreError> {
    PlaintextSettings::read_from_file(Path::new(&path))
        .map(FfiPlaintextSettings::from)
        .map_err(FfiCoreError::from)
}

pub fn write_plaintext_settings(
    path: String,
    settings: FfiPlaintextSettings,
) -> Result<(), FfiCoreError> {
    PlaintextSettings::try_from(settings)?
        .write_to_file(Path::new(&path))
        .map_err(FfiCoreError::from)
}

impl From<TaskStatus> for FfiTaskStatus {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Open => Self::Open,
            TaskStatus::Done => Self::Done,
        }
    }
}

impl From<FfiTaskStatus> for TaskStatus {
    fn from(status: FfiTaskStatus) -> Self {
        match status {
            FfiTaskStatus::Open => Self::Open,
            FfiTaskStatus::Done => Self::Done,
        }
    }
}

impl From<TaskSort> for FfiTaskSort {
    fn from(sort: TaskSort) -> Self {
        match sort {
            TaskSort::UpdatedAtDesc => Self::UpdatedAtDesc,
            TaskSort::UpdatedAtAsc => Self::UpdatedAtAsc,
            TaskSort::DueAtAsc => Self::DueAtAsc,
            TaskSort::DueAtDesc => Self::DueAtDesc,
            TaskSort::CreatedAtAsc => Self::CreatedAtAsc,
            TaskSort::CreatedAtDesc => Self::CreatedAtDesc,
        }
    }
}

impl From<FfiTaskSort> for TaskSort {
    fn from(sort: FfiTaskSort) -> Self {
        match sort {
            FfiTaskSort::UpdatedAtDesc => Self::UpdatedAtDesc,
            FfiTaskSort::UpdatedAtAsc => Self::UpdatedAtAsc,
            FfiTaskSort::DueAtAsc => Self::DueAtAsc,
            FfiTaskSort::DueAtDesc => Self::DueAtDesc,
            FfiTaskSort::CreatedAtAsc => Self::CreatedAtAsc,
            FfiTaskSort::CreatedAtDesc => Self::CreatedAtDesc,
        }
    }
}

impl From<Task> for FfiTask {
    fn from(task: Task) -> Self {
        Self {
            id: task.id.to_string(),
            title: task.title,
            body: task.body,
            due_at: task.due_at,
            reminder_offset_ms: task.reminder_offset_ms,
            status: task.status.into(),
            project_id: task.project_id.map(|id| id.to_string()),
            tags: task.tags,
            created_at: task.created_at,
            updated_at: task.updated_at,
            deleted: task.deleted,
            dirty: task.dirty,
        }
    }
}

impl TryFrom<FfiTask> for Task {
    type Error = FfiCoreError;

    fn try_from(task: FfiTask) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_uuid(&task.id)?,
            title: task.title,
            body: task.body,
            due_at: task.due_at,
            reminder_offset_ms: task.reminder_offset_ms,
            status: task.status.into(),
            project_id: task.project_id.as_deref().map(parse_uuid).transpose()?,
            tags: task.tags,
            created_at: task.created_at,
            updated_at: task.updated_at,
            deleted: task.deleted,
            dirty: task.dirty,
        })
    }
}

impl From<TaskList> for FfiTaskList {
    fn from(list: TaskList) -> Self {
        Self {
            id: list.id.to_string(),
            name: list.name,
            created_at: list.created_at,
            updated_at: list.updated_at,
            deleted: list.deleted,
            dirty: list.dirty,
        }
    }
}

impl TryFrom<FfiTaskList> for TaskList {
    type Error = FfiCoreError;

    fn try_from(list: FfiTaskList) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_uuid(&list.id)?,
            name: list.name,
            created_at: list.created_at,
            updated_at: list.updated_at,
            deleted: list.deleted,
            dirty: list.dirty,
        })
    }
}

impl TryFrom<FfiTaskPatch> for TaskPatch {
    type Error = FfiCoreError;

    fn try_from(patch: FfiTaskPatch) -> Result<Self, Self::Error> {
        if patch.clear_due_at && patch.due_at.is_some() {
            return Err(FfiCoreError::InvalidPatch {
                error_message: "due_at cannot be set and cleared in the same patch".to_owned(),
            });
        }
        if patch.clear_reminder_offset_ms && patch.reminder_offset_ms.is_some() {
            return Err(FfiCoreError::InvalidPatch {
                error_message: "reminder_offset_ms cannot be set and cleared in the same patch"
                    .to_owned(),
            });
        }
        if patch.reminder_offset_ms.is_some_and(|offset| offset < 0) {
            return Err(FfiCoreError::InvalidPatch {
                error_message: "reminder_offset_ms cannot be negative".to_owned(),
            });
        }
        if patch.clear_project_id && patch.project_id.is_some() {
            return Err(FfiCoreError::InvalidPatch {
                error_message: "project_id cannot be set and cleared in the same patch".to_owned(),
            });
        }

        Ok(Self {
            title: patch.title,
            body: patch.body,
            due_at: if patch.clear_due_at {
                Some(None)
            } else {
                patch.due_at.map(Some)
            },
            reminder_offset_ms: if patch.clear_reminder_offset_ms {
                Some(None)
            } else {
                patch.reminder_offset_ms.map(Some)
            },
            status: patch.status.map(TaskStatus::from),
            project_id: if patch.clear_project_id {
                Some(None)
            } else {
                patch
                    .project_id
                    .as_deref()
                    .map(parse_uuid)
                    .transpose()?
                    .map(Some)
            },
            tags: patch.tags,
        })
    }
}

impl TryFrom<FfiTaskFilter> for TaskFilter {
    type Error = FfiCoreError;

    fn try_from(filter: FfiTaskFilter) -> Result<Self, Self::Error> {
        Ok(Self {
            status: filter.status.map(TaskStatus::from),
            project_id: filter.project_id.as_deref().map(parse_uuid).transpose()?,
            tags: filter.tags,
            due_after: filter.due_after,
            due_before: filter.due_before,
            include_deleted: filter.include_deleted,
        })
    }
}

struct FfiSyncPlatform {
    online: bool,
}

impl Platform for FfiSyncPlatform {
    fn store_key(&self, _id: &str, _bytes: &[u8]) -> crate::error::CoreResult<()> {
        Err(crate::error::PlatformError::OperationFailed(
            "sync platform does not store keys".to_owned(),
        )
        .into())
    }

    fn load_key(&self, _id: &str) -> crate::error::CoreResult<Vec<u8>> {
        Err(crate::error::PlatformError::OperationFailed(
            "sync platform does not load keys".to_owned(),
        )
        .into())
    }

    fn delete_key(&self, _id: &str) -> crate::error::CoreResult<()> {
        Err(crate::error::PlatformError::OperationFailed(
            "sync platform does not delete keys".to_owned(),
        )
        .into())
    }

    fn schedule_notification(
        &self,
        _task_id: Uuid,
        _fire_at: i64,
        _title: &str,
    ) -> crate::error::CoreResult<()> {
        Ok(())
    }

    fn cancel_notification(&self, _task_id: Uuid) -> crate::error::CoreResult<()> {
        Ok(())
    }

    fn network_available(&self) -> bool {
        self.online
    }
}

struct FfiPlatformAdapter {
    platform: Box<dyn FfiPlatform>,
}

impl Platform for FfiPlatformAdapter {
    fn store_key(&self, id: &str, bytes: &[u8]) -> CoreResult<()> {
        self.platform
            .store_key(id.to_owned(), bytes.to_vec())
            .map_err(ffi_error_to_core)
    }

    fn load_key(&self, id: &str) -> CoreResult<Vec<u8>> {
        self.platform
            .load_key(id.to_owned())
            .map_err(ffi_error_to_core)
    }

    fn delete_key(&self, id: &str) -> CoreResult<()> {
        self.platform
            .delete_key(id.to_owned())
            .map_err(ffi_error_to_core)
    }

    fn schedule_notification(&self, _task_id: Uuid, _fire_at: i64, _title: &str) -> CoreResult<()> {
        Ok(())
    }

    fn cancel_notification(&self, _task_id: Uuid) -> CoreResult<()> {
        Ok(())
    }

    fn network_available(&self) -> bool {
        self.platform.network_available()
    }
}

struct FfiAuthClientAdapter {
    client: Box<dyn FfiAuthClient>,
}

impl AuthClient for FfiAuthClientAdapter {
    fn register(&self, server_url: &str, request: RegisterRequest) -> CoreResult<TokenResponse> {
        self.client
            .register_account(server_url.to_owned(), request.into())
            .map(TokenResponse::from)
            .map_err(ffi_error_to_core)
    }

    fn login(&self, server_url: &str, request: LoginRequest) -> CoreResult<TokenResponse> {
        self.client
            .login(server_url.to_owned(), request.into())
            .map(TokenResponse::from)
            .map_err(ffi_error_to_core)
    }

    fn refresh(&self, server_url: &str, request: RefreshTokenRequest) -> CoreResult<TokenResponse> {
        self.client
            .refresh(server_url.to_owned(), request.into())
            .map(TokenResponse::from)
            .map_err(ffi_error_to_core)
    }

    fn delete_session(&self, server_url: &str, request: DeleteSessionRequest) -> CoreResult<()> {
        self.client
            .delete_session(server_url.to_owned(), request.into())
            .map_err(ffi_error_to_core)
    }

    fn put_current_device_key(
        &self,
        server_url: &str,
        access_token: &str,
        request: PutCurrentDeviceKeyRequest,
    ) -> CoreResult<()> {
        self.client
            .put_current_device_key(
                server_url.to_owned(),
                access_token.to_owned(),
                request.into(),
            )
            .map_err(ffi_error_to_core)
    }
}

struct FfiSyncClientAdapter {
    client: Box<dyn FfiSyncClient>,
}

impl SyncClient for FfiSyncClientAdapter {
    fn push_blobs(&self, blobs: Vec<BlobPush>) -> crate::error::CoreResult<PushResponse> {
        self.client
            .push_blobs(blobs.into_iter().map(FfiBlobPush::from).collect())
            .and_then(PushResponse::try_from)
            .map_err(ffi_error_to_core)
    }

    fn delete_blobs(&self, task_ids: Vec<Uuid>) -> crate::error::CoreResult<PushResponse> {
        self.client
            .delete_blobs(task_ids.into_iter().map(|id| id.to_string()).collect())
            .and_then(PushResponse::try_from)
            .map_err(ffi_error_to_core)
    }

    fn pull_blobs(&self, since: i64) -> crate::error::CoreResult<PullResponse> {
        self.client
            .pull_blobs(since)
            .and_then(PullResponse::try_from)
            .map_err(ffi_error_to_core)
    }
}

fn ffi_error_to_core(error: FfiCoreError) -> CoreError {
    match error {
        FfiCoreError::SyncError { error_message } => ffi_sync_error_to_core(error_message),
        FfiCoreError::PlatformError { error_message } => {
            if let Some(key_id) = error_message.strip_prefix("missing key ") {
                crate::error::PlatformError::KeyNotFound(key_id.to_owned()).into()
            } else {
                crate::error::PlatformError::OperationFailed(error_message).into()
            }
        }
        other => crate::error::PlatformError::OperationFailed(other.to_string()).into(),
    }
}

fn ffi_sync_error_to_core(error_message: String) -> CoreError {
    let normalized = error_message.to_ascii_lowercase();
    if normalized.contains("auth expired") {
        SyncError::AuthExpired.into()
    } else if normalized.contains("network unavailable") {
        SyncError::NetworkUnavailable.into()
    } else if let Some(task_id) = normalized
        .strip_prefix("blob conflict: ")
        .and_then(|id| Uuid::parse_str(id).ok())
    {
        SyncError::BlobConflict(task_id).into()
    } else {
        let status = normalized
            .strip_prefix("server error ")
            .and_then(|suffix| suffix.split(|c: char| !c.is_ascii_digit()).next())
            .and_then(|digits| digits.parse::<u16>().ok())
            .unwrap_or(500);
        SyncError::ServerError {
            status,
            body: error_message,
        }
        .into()
    }
}

impl From<AuthCredentials> for FfiAuthCredentials {
    fn from(credentials: AuthCredentials) -> Self {
        Self {
            email: credentials.email,
            password: credentials.password,
        }
    }
}

impl From<FfiAuthCredentials> for AuthCredentials {
    fn from(credentials: FfiAuthCredentials) -> Self {
        Self {
            email: credentials.email,
            password: credentials.password,
        }
    }
}

impl From<RegisterRequest> for FfiRegisterRequest {
    fn from(request: RegisterRequest) -> Self {
        Self {
            email: request.email,
            password: request.password,
            pub_key: request.pub_key,
        }
    }
}

impl From<FfiRegisterRequest> for RegisterRequest {
    fn from(request: FfiRegisterRequest) -> Self {
        Self {
            email: request.email,
            password: request.password,
            pub_key: request.pub_key,
        }
    }
}

impl From<LoginRequest> for FfiLoginRequest {
    fn from(request: LoginRequest) -> Self {
        Self {
            email: request.email,
            password: request.password,
        }
    }
}

impl From<FfiLoginRequest> for LoginRequest {
    fn from(request: FfiLoginRequest) -> Self {
        Self {
            email: request.email,
            password: request.password,
        }
    }
}

impl From<RefreshTokenRequest> for FfiRefreshTokenRequest {
    fn from(request: RefreshTokenRequest) -> Self {
        Self {
            refresh_token: request.refresh_token,
        }
    }
}

impl From<FfiRefreshTokenRequest> for RefreshTokenRequest {
    fn from(request: FfiRefreshTokenRequest) -> Self {
        Self {
            refresh_token: request.refresh_token,
        }
    }
}

impl From<DeleteSessionRequest> for FfiDeleteSessionRequest {
    fn from(request: DeleteSessionRequest) -> Self {
        Self {
            refresh_token: request.refresh_token,
        }
    }
}

impl From<FfiDeleteSessionRequest> for DeleteSessionRequest {
    fn from(request: FfiDeleteSessionRequest) -> Self {
        Self {
            refresh_token: request.refresh_token,
        }
    }
}

impl From<PutCurrentDeviceKeyRequest> for FfiPutCurrentDeviceKeyRequest {
    fn from(request: PutCurrentDeviceKeyRequest) -> Self {
        Self {
            pub_key: request.pub_key,
        }
    }
}

impl From<FfiPutCurrentDeviceKeyRequest> for PutCurrentDeviceKeyRequest {
    fn from(request: FfiPutCurrentDeviceKeyRequest) -> Self {
        Self {
            pub_key: request.pub_key,
        }
    }
}

impl From<TokenResponse> for FfiTokenResponse {
    fn from(response: TokenResponse) -> Self {
        Self {
            jwt: response.jwt,
            refresh_token: response.refresh_token,
            user_id: response.user_id,
        }
    }
}

impl From<FfiTokenResponse> for TokenResponse {
    fn from(response: FfiTokenResponse) -> Self {
        Self {
            jwt: response.jwt,
            refresh_token: response.refresh_token,
            user_id: response.user_id,
        }
    }
}

impl From<ConfigureSyncAuthResult> for FfiConfigureSyncAuthResult {
    fn from(result: ConfigureSyncAuthResult) -> Self {
        Self {
            server_url: result.server_url,
            sync_origin: result.sync_origin,
            account_id: result.account_id,
            state: result.state.into(),
        }
    }
}

impl From<SyncAuthState> for FfiSyncAuthState {
    fn from(state: SyncAuthState) -> Self {
        match state {
            SyncAuthState::LocalOnlyReady => Self::LocalOnlyReady,
            SyncAuthState::AuthenticatedEnrollmentPending => Self::AuthenticatedEnrollmentPending,
            SyncAuthState::SyncReady => Self::SyncReady,
            SyncAuthState::AuthRequired => Self::AuthRequired,
            SyncAuthState::MisconfiguredOrigin => Self::MisconfiguredOrigin,
        }
    }
}

impl From<EnrollmentState> for FfiEnrollmentState {
    fn from(state: EnrollmentState) -> Self {
        match state {
            EnrollmentState::LocalOnlyReady => Self::LocalOnlyReady,
            EnrollmentState::ExistingAccountPending => Self::ExistingAccountPending,
            EnrollmentState::SyncReady => Self::SyncReady,
        }
    }
}

impl From<WrappedAccountDataKeyPayload> for FfiWrappedAccountDataKeyPayload {
    fn from(payload: WrappedAccountDataKeyPayload) -> Self {
        Self {
            sender_public_key: payload.sender_public_key,
            recipient_public_key: payload.recipient_public_key,
            wrapped_account_data_key: payload.wrapped_account_data_key.into(),
        }
    }
}

impl TryFrom<FfiWrappedAccountDataKeyPayload> for WrappedAccountDataKeyPayload {
    type Error = FfiCoreError;

    fn try_from(payload: FfiWrappedAccountDataKeyPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            sender_public_key: payload.sender_public_key,
            recipient_public_key: payload.recipient_public_key,
            wrapped_account_data_key: payload.wrapped_account_data_key.try_into()?,
        })
    }
}

impl From<Blob> for FfiBlob {
    fn from(blob: Blob) -> Self {
        Self {
            ciphertext: blob.ciphertext,
            nonce: blob.nonce.to_vec(),
        }
    }
}

impl TryFrom<FfiBlob> for Blob {
    type Error = FfiCoreError;

    fn try_from(blob: FfiBlob) -> Result<Self, Self::Error> {
        let nonce: [u8; 12] =
            blob.nonce
                .try_into()
                .map_err(|nonce: Vec<u8>| FfiCoreError::InvalidNonce {
                    error_message: format!(
                        "invalid nonce length: expected 12 bytes, got {}",
                        nonce.len()
                    ),
                })?;
        Ok(Self {
            ciphertext: blob.ciphertext,
            nonce,
        })
    }
}

impl From<SharedTaskRecipient> for FfiSharedTaskRecipient {
    fn from(recipient: SharedTaskRecipient) -> Self {
        Self {
            task_id: recipient.task_id.to_string(),
            recipient_id: recipient.recipient_id.to_string(),
            wrapped_task_key: recipient.wrapped_task_key.into(),
            created_at: recipient.created_at,
            revoked_at: recipient.revoked_at,
        }
    }
}

impl From<SharedTaskState> for FfiSharedTaskState {
    fn from(state: SharedTaskState) -> Self {
        Self {
            task_id: state.task_id.to_string(),
            owner_id: state.owner_id.map(|id| id.to_string()),
            task_key: state.task_key,
            recipients: state
                .recipients
                .into_iter()
                .map(FfiSharedTaskRecipient::from)
                .collect(),
            accepted_at: state.accepted_at,
            revoked_at: state.revoked_at,
        }
    }
}

impl TryFrom<FfiSharedTaskInvite> for SharedTaskInvite {
    type Error = FfiCoreError;

    fn try_from(invite: FfiSharedTaskInvite) -> Result<Self, Self::Error> {
        Ok(Self {
            task_id: parse_uuid(&invite.task_id)?,
            owner_id: parse_uuid(&invite.owner_id)?,
            recipient_id: parse_uuid(&invite.recipient_id)?,
            wrapped_task_key: invite.wrapped_task_key.try_into()?,
        })
    }
}

impl From<AuthMethod> for FfiAuthMethod {
    fn from(method: AuthMethod) -> Self {
        match method {
            AuthMethod::Biometric => Self::Biometric,
            AuthMethod::Pin => Self::Pin,
            AuthMethod::Password => Self::Password,
        }
    }
}

impl From<FfiAuthMethod> for AuthMethod {
    fn from(method: FfiAuthMethod) -> Self {
        match method {
            FfiAuthMethod::Biometric => Self::Biometric,
            FfiAuthMethod::Pin => Self::Pin,
            FfiAuthMethod::Password => Self::Password,
        }
    }
}

impl From<Theme> for FfiTheme {
    fn from(theme: Theme) -> Self {
        match theme {
            Theme::Light => Self::Light,
            Theme::Dark => Self::Dark,
            Theme::System => Self::System,
        }
    }
}

impl From<FfiTheme> for Theme {
    fn from(theme: FfiTheme) -> Self {
        match theme {
            FfiTheme::Light => Self::Light,
            FfiTheme::Dark => Self::Dark,
            FfiTheme::System => Self::System,
        }
    }
}

impl From<DefaultSort> for FfiDefaultSort {
    fn from(sort: DefaultSort) -> Self {
        match sort {
            DefaultSort::DueAtAsc => Self::DueAtAsc,
            DefaultSort::UpdatedAtDesc => Self::UpdatedAtDesc,
        }
    }
}

impl From<FfiDefaultSort> for DefaultSort {
    fn from(sort: FfiDefaultSort) -> Self {
        match sort {
            FfiDefaultSort::DueAtAsc => Self::DueAtAsc,
            FfiDefaultSort::UpdatedAtDesc => Self::UpdatedAtDesc,
        }
    }
}

impl From<DisplayDensity> for FfiDisplayDensity {
    fn from(density: DisplayDensity) -> Self {
        match density {
            DisplayDensity::Compact => Self::Compact,
            DisplayDensity::Comfortable => Self::Comfortable,
            DisplayDensity::Spacious => Self::Spacious,
        }
    }
}

impl From<FfiDisplayDensity> for DisplayDensity {
    fn from(density: FfiDisplayDensity) -> Self {
        match density {
            FfiDisplayDensity::Compact => Self::Compact,
            FfiDisplayDensity::Comfortable => Self::Comfortable,
            FfiDisplayDensity::Spacious => Self::Spacious,
        }
    }
}

impl From<Keybindings> for FfiKeybindings {
    fn from(keybindings: Keybindings) -> Self {
        Self {
            add_task: keybindings.add_task,
            search: keybindings.search,
            close_overlay: keybindings.close_overlay,
            confirm_rename: keybindings.confirm_rename,
            delete_task: keybindings.delete_task,
            toggle_done: keybindings.toggle_done,
        }
    }
}

impl From<FfiKeybindings> for Keybindings {
    fn from(keybindings: FfiKeybindings) -> Self {
        Self {
            add_task: keybindings.add_task,
            search: keybindings.search,
            close_overlay: keybindings.close_overlay,
            confirm_rename: keybindings.confirm_rename,
            delete_task: keybindings.delete_task,
            toggle_done: keybindings.toggle_done,
        }
    }
}

impl From<PlaintextSettings> for FfiPlaintextSettings {
    fn from(settings: PlaintextSettings) -> Self {
        Self {
            schema_version: settings.schema_version,
            server_url: settings.server_url,
            auth_method: settings.auth_method.into(),
            language: settings.language,
            last_sync_cursor: settings.last_sync_cursor,
        }
    }
}

impl TryFrom<FfiPlaintextSettings> for PlaintextSettings {
    type Error = FfiCoreError;

    fn try_from(settings: FfiPlaintextSettings) -> Result<Self, Self::Error> {
        Ok(Self {
            schema_version: settings.schema_version,
            server_url: settings.server_url,
            auth_method: settings.auth_method.into(),
            language: settings.language,
            last_sync_cursor: settings.last_sync_cursor,
        })
    }
}

impl From<VaultSettings> for FfiVaultSettings {
    fn from(settings: VaultSettings) -> Self {
        Self {
            schema_version: settings.schema_version,
            theme: settings.theme.into(),
            default_sort: settings.default_sort.into(),
            show_completed: settings.show_completed,
            default_reminder_minutes: settings.default_reminder_minutes,
            tag_colors: settings
                .tag_colors
                .into_iter()
                .map(|(tag, color)| FfiTagColor { tag, color })
                .collect(),
            display_density: settings.display_density.into(),
            first_day_of_week: settings.first_day_of_week,
            notification_sound: settings.notification_sound,
            keybindings: settings.keybindings.into(),
            show_share_revocation_warning: settings.show_share_revocation_warning,
        }
    }
}

impl TryFrom<FfiVaultSettings> for VaultSettings {
    type Error = FfiCoreError;

    fn try_from(settings: FfiVaultSettings) -> Result<Self, Self::Error> {
        Ok(Self {
            schema_version: settings.schema_version,
            theme: settings.theme.into(),
            default_sort: settings.default_sort.into(),
            show_completed: settings.show_completed,
            default_reminder_minutes: settings.default_reminder_minutes,
            tag_colors: settings
                .tag_colors
                .into_iter()
                .map(|tag_color| (tag_color.tag, tag_color.color))
                .collect(),
            display_density: settings.display_density.into(),
            first_day_of_week: settings.first_day_of_week,
            notification_sound: settings.notification_sound,
            keybindings: settings.keybindings.into(),
            show_share_revocation_warning: settings.show_share_revocation_warning,
        })
    }
}

impl From<BlobPush> for FfiBlobPush {
    fn from(push: BlobPush) -> Self {
        Self {
            task_id: push.task_id.to_string(),
            blob: push.blob.into(),
        }
    }
}

impl TryFrom<FfiRemoteBlob> for RemoteBlob {
    type Error = FfiCoreError;

    fn try_from(blob: FfiRemoteBlob) -> Result<Self, Self::Error> {
        Ok(Self {
            task_id: parse_uuid(&blob.task_id)?,
            blob: blob.blob.map(TryInto::try_into).transpose()?,
            updated_at: blob.updated_at,
            deleted: blob.deleted,
        })
    }
}

impl TryFrom<FfiPushResponse> for PushResponse {
    type Error = FfiCoreError;

    fn try_from(response: FfiPushResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            accepted_task_ids: response
                .accepted_task_ids
                .iter()
                .map(|id| parse_uuid(id))
                .collect::<Result<Vec<_>, _>>()?,
            failed_task_ids: response
                .failed_task_ids
                .iter()
                .map(|id| parse_uuid(id))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<FfiPullResponse> for PullResponse {
    type Error = FfiCoreError;

    fn try_from(response: FfiPullResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            blobs: response
                .blobs
                .into_iter()
                .map(RemoteBlob::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            cursor: response.cursor,
        })
    }
}

impl From<SyncResult> for FfiSyncResult {
    fn from(result: SyncResult) -> Self {
        Self {
            pushed: result.pushed as u64,
            pulled: result.pulled as u64,
            failed: result.failed as u64,
            cursor: result.cursor,
        }
    }
}

impl From<SyncStatus> for FfiSyncStatus {
    fn from(status: SyncStatus) -> Self {
        Self {
            dirty_count: status.dirty_count as u64,
            retry_queue_depth: status.retry_queue_depth as u64,
            cursor: status.cursor,
        }
    }
}

impl From<RetryQueueEntry> for FfiRetryQueueEntry {
    fn from(entry: RetryQueueEntry) -> Self {
        Self {
            task_id: entry.task_id.to_string(),
            attempt: entry.attempt,
            next_retry: entry.next_retry,
        }
    }
}

impl From<DeviceKeypair> for FfiDeviceKeypair {
    fn from(keypair: DeviceKeypair) -> Self {
        Self {
            private_key: keypair.private_key,
            public_key: keypair.public_key,
        }
    }
}

impl From<CoreError> for FfiCoreError {
    fn from(error: CoreError) -> Self {
        let error_message = error.to_string();
        match error {
            CoreError::Crypto(_) => Self::CryptoError { error_message },
            CoreError::Database(_) => Self::DatabaseError { error_message },
            CoreError::Sync(_) => Self::SyncError { error_message },
            CoreError::Platform(_) => Self::PlatformError { error_message },
            CoreError::Settings(_) => Self::SettingsError { error_message },
            CoreError::Serialization(_) => Self::SerializationError { error_message },
        }
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, FfiCoreError> {
    Uuid::parse_str(value).map_err(|error| FfiCoreError::InvalidUuid {
        error_message: format!("invalid UUID '{value}': {error}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_crud_round_trips_representative_values() {
        let path = temporary_database_path("ffi_crud_round_trips_representative_values");
        let core = FfiTaskManagerCore::new(path.to_string_lossy().into_owned()).unwrap();
        let project_id = Uuid::new_v4().to_string();

        let created = core
            .create_task("title".to_owned(), "body".to_owned(), Some(10))
            .unwrap();
        let updated = core
            .update_task(
                created.id.clone(),
                FfiTaskPatch {
                    title: Some("updated".to_owned()),
                    status: Some(FfiTaskStatus::Open),
                    reminder_offset_ms: Some(5),
                    project_id: Some(project_id.clone()),
                    tags: Some(vec!["work".to_owned(), "urgent".to_owned()]),
                    ..FfiTaskPatch::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "updated");
        assert_eq!(updated.project_id, Some(project_id.clone()));
        assert_eq!(updated.reminder_offset_ms, Some(5));
        assert_eq!(updated.tags, vec!["work", "urgent"]);
        assert_eq!(core.get_task(created.id.clone()).unwrap(), updated);
        assert_eq!(
            core.search_tasks("updated".to_owned()).unwrap()[0].id,
            created.id
        );
        assert_eq!(
            core.list_tasks(
                FfiTaskFilter {
                    project_id: Some(project_id),
                    include_deleted: false,
                    ..FfiTaskFilter::default()
                },
                FfiTaskSort::UpdatedAtDesc
            )
            .unwrap()
            .len(),
            1
        );

        core.delete_task(created.id.clone()).unwrap();
        assert!(core.get_task(created.id).unwrap().deleted);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn ffi_schedulable_notification_at_uses_core_reminder_semantics() {
        let task = FfiTask {
            id: Uuid::new_v4().to_string(),
            title: "Reminder".to_owned(),
            body: String::new(),
            due_at: Some(10_000),
            reminder_offset_ms: Some(1_000),
            status: FfiTaskStatus::Open,
            project_id: None,
            tags: vec![],
            created_at: 0,
            updated_at: 0,
            deleted: false,
            dirty: false,
        };

        assert_eq!(
            schedulable_notification_at(task.clone(), 8_999).unwrap(),
            Some(9_000)
        );
        assert_eq!(
            schedulable_notification_at(task.clone(), 9_000).unwrap(),
            None
        );

        let mut done = task.clone();
        done.status = FfiTaskStatus::Done;
        assert_eq!(schedulable_notification_at(done, 8_999).unwrap(), None);

        let mut deleted = task;
        deleted.deleted = true;
        assert_eq!(schedulable_notification_at(deleted, 8_999).unwrap(), None);
    }

    #[derive(Default)]
    struct FfiFakeSyncClient;

    impl FfiSyncClient for FfiFakeSyncClient {
        fn push_blobs(&self, blobs: Vec<FfiBlobPush>) -> Result<FfiPushResponse, FfiCoreError> {
            Ok(FfiPushResponse {
                accepted_task_ids: blobs.into_iter().map(|blob| blob.task_id).collect(),
                failed_task_ids: Vec::new(),
            })
        }

        fn delete_blobs(&self, task_ids: Vec<String>) -> Result<FfiPushResponse, FfiCoreError> {
            Ok(FfiPushResponse {
                accepted_task_ids: task_ids,
                failed_task_ids: Vec::new(),
            })
        }

        fn pull_blobs(&self, _since: i64) -> Result<FfiPullResponse, FfiCoreError> {
            Ok(FfiPullResponse {
                blobs: Vec::new(),
                cursor: 7,
            })
        }
    }

    #[test]
    fn ffi_sync_run_delegates_to_core_sync_and_clears_dirty() {
        let database_path =
            temporary_database_path("ffi_sync_run_delegates_to_core_sync_and_clears_dirty");
        let core = FfiTaskManagerCore::new(database_path.to_string_lossy().into_owned()).unwrap();
        let task = core
            .create_task("sync me".to_owned(), String::new(), None)
            .unwrap();

        let result = core
            .sync_run(
                true,
                Box::new(FfiFakeSyncClient),
                generate_data_key().to_vec(),
            )
            .unwrap();

        assert_eq!(result.pushed, 1);
        assert_eq!(result.cursor, Some(7));
        assert!(!core.get_task(task.id).unwrap().dirty);
    }

    struct FfiSyncErrorClient;

    impl FfiSyncClient for FfiSyncErrorClient {
        fn push_blobs(&self, _blobs: Vec<FfiBlobPush>) -> Result<FfiPushResponse, FfiCoreError> {
            Ok(FfiPushResponse {
                accepted_task_ids: Vec::new(),
                failed_task_ids: Vec::new(),
            })
        }

        fn delete_blobs(&self, _task_ids: Vec<String>) -> Result<FfiPushResponse, FfiCoreError> {
            Ok(FfiPushResponse {
                accepted_task_ids: Vec::new(),
                failed_task_ids: Vec::new(),
            })
        }

        fn pull_blobs(&self, _since: i64) -> Result<FfiPullResponse, FfiCoreError> {
            Err(FfiCoreError::SyncError {
                error_message: "auth expired".to_owned(),
            })
        }
    }

    #[test]
    fn ffi_sync_client_sync_errors_preserve_kind_through_sync_run() {
        let database_path =
            temporary_database_path("ffi_sync_client_sync_errors_preserve_kind_through_sync_run");
        let core = FfiTaskManagerCore::new(database_path.to_string_lossy().into_owned()).unwrap();

        let error = core
            .sync_run(
                true,
                Box::new(FfiSyncErrorClient),
                generate_data_key().to_vec(),
            )
            .unwrap_err();

        assert_eq!(error.kind(), FfiCoreErrorKind::SyncError);
        assert_eq!(error.message(), "auth expired");
    }

    #[test]
    fn ffi_task_lists_settings_and_sync_status_cover_ios_surface() {
        let path =
            temporary_database_path("ffi_task_lists_settings_and_sync_status_cover_ios_surface");
        let core = FfiTaskManagerCore::new(path.to_string_lossy().into_owned()).unwrap();

        let list = core.create_list("Work".to_owned()).unwrap();
        assert_eq!(list.name, "Work");
        assert_eq!(core.list_task_lists().unwrap(), vec![list.clone()]);

        let task = core
            .create_task_with_options(
                "plan iOS".to_owned(),
                "bind core".to_owned(),
                Some(100),
                Some(list.id.clone()),
                vec!["ios".to_owned(), "ffi".to_owned()],
            )
            .unwrap();
        assert_eq!(task.project_id, Some(list.id.clone()));
        assert_eq!(task.tags, vec!["ios", "ffi"]);

        let retry = core.queue_sync_retry(task.id.clone(), 1_000).unwrap();
        assert_eq!(retry.task_id, task.id);
        assert_eq!(retry.attempt, 1);
        let status = core.sync_status().unwrap();
        assert_eq!(status.dirty_count, 1);
        assert_eq!(status.retry_queue_depth, 1);
        assert_eq!(status.cursor, 0);

        let mut settings = core.vault_settings().unwrap();
        settings.theme = FfiTheme::Dark;
        settings.show_completed = true;
        settings.tag_colors = vec![FfiTagColor {
            tag: "ios".to_owned(),
            color: "#007AFF".to_owned(),
        }];
        let saved = core.update_vault_settings(settings.clone()).unwrap();
        assert_eq!(saved, settings);
        assert_eq!(core.vault_settings().unwrap(), settings);

        let renamed = core
            .update_list(list.id.clone(), "Personal".to_owned())
            .unwrap();
        assert_eq!(renamed.name, "Personal");
        core.delete_list(list.id).unwrap();
        assert!(core.list_task_lists().unwrap().is_empty());
        assert_eq!(core.get_task(task.id).unwrap().project_id, None);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn ffi_plaintext_settings_round_trip_through_file() {
        let path = temporary_database_path("ffi_plaintext_settings_round_trip_through_file")
            .with_extension("json");
        let missing = read_plaintext_settings(path.to_string_lossy().into_owned()).unwrap();
        assert_eq!(missing.server_url, "");
        assert_eq!(missing.auth_method, FfiAuthMethod::Password);

        let settings = FfiPlaintextSettings {
            schema_version: 1,
            server_url: "https://api.example.com".to_owned(),
            auth_method: FfiAuthMethod::Biometric,
            language: "en".to_owned(),
            last_sync_cursor: 42,
        };
        write_plaintext_settings(path.to_string_lossy().into_owned(), settings.clone()).unwrap();

        assert_eq!(
            read_plaintext_settings(path.to_string_lossy().into_owned()).unwrap(),
            settings
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn ffi_auth_and_enrollment_helpers_map_shared_core_types() {
        use std::collections::HashMap;
        use std::sync::Mutex;

        struct TestFfiPlatform {
            keys: Mutex<HashMap<String, Vec<u8>>>,
        }

        impl TestFfiPlatform {
            fn new() -> Self {
                Self {
                    keys: Mutex::new(HashMap::new()),
                }
            }
        }

        impl FfiPlatform for TestFfiPlatform {
            fn store_key(&self, id: String, bytes: Vec<u8>) -> Result<(), FfiCoreError> {
                self.keys.lock().unwrap().insert(id, bytes);
                Ok(())
            }

            fn load_key(&self, id: String) -> Result<Vec<u8>, FfiCoreError> {
                self.keys
                    .lock()
                    .unwrap()
                    .get(&id)
                    .cloned()
                    .ok_or(FfiCoreError::PlatformError {
                        error_message: format!("missing key {id}"),
                    })
            }

            fn delete_key(&self, id: String) -> Result<(), FfiCoreError> {
                self.keys.lock().unwrap().remove(&id);
                Ok(())
            }

            fn network_available(&self) -> bool {
                true
            }
        }

        assert_eq!(auth_access_token_id(), AUTH_ACCESS_TOKEN_ID);
        assert_eq!(auth_refresh_token_id(), AUTH_REFRESH_TOKEN_ID);
        assert_eq!(auth_sync_origin_id(), AUTH_SYNC_ORIGIN_ID);
        assert_eq!(auth_account_id_id(), AUTH_ACCOUNT_ID_ID);
        assert_eq!(
            normalize_ffi_sync_server_url(" https://example.com/ ".to_owned()).unwrap(),
            "https://example.com"
        );
        assert_eq!(
            ffi_sync_server_origin("https://example.com/tasks".to_owned()).unwrap(),
            "https://example.com:443"
        );
        assert!(normalize_ffi_sync_server_url("http://example.com".to_owned()).is_err());

        let bootstrap = generate_local_account_bootstrap();
        let platform = TestFfiPlatform::new();
        platform
            .store_key(
                device_private_key_id(),
                bootstrap.device_private_key.clone(),
            )
            .unwrap();
        assert_eq!(
            ffi_begin_existing_account_enrollment(Box::new(platform)).unwrap(),
            FfiEnrollmentState::ExistingAccountPending
        );

        let recipient = generate_ffi_device_keypair();
        let sender = generate_ffi_device_keypair();
        let payload = create_ffi_wrapped_account_data_key_payload(
            bootstrap.account_data_key.clone(),
            recipient.public_key.clone(),
            sender.private_key,
        )
        .unwrap();
        let platform = TestFfiPlatform::new();
        platform
            .store_key(device_private_key_id(), recipient.private_key)
            .unwrap();
        assert_eq!(
            accept_ffi_wrapped_account_data_key_payload(Box::new(platform), payload).unwrap(),
            FfiEnrollmentState::SyncReady
        );
    }

    #[test]
    fn ffi_device_and_account_key_helpers_support_ios_keychain_storage() {
        let bootstrap = generate_local_account_bootstrap();
        assert_eq!(bootstrap.device_private_key.len(), 32);
        assert!(!bootstrap.device_public_key.is_empty());
        assert_eq!(bootstrap.account_data_key.len(), 32);
        assert_eq!(device_private_key_id(), DEVICE_PRIVATE_KEY_ID);
        assert_eq!(account_data_key_id(), ACCOUNT_DATA_KEY_ID);
        assert_eq!(
            device_public_key_from_private_key(bootstrap.device_private_key.clone()).unwrap(),
            bootstrap.device_public_key
        );

        let another_device = generate_ffi_device_keypair();
        let wrapped = wrap_account_data_key(
            bootstrap.account_data_key.clone(),
            another_device.public_key.clone(),
            bootstrap.device_private_key,
        )
        .unwrap();
        let unwrapped = unwrap_account_data_key(
            wrapped,
            bootstrap.device_public_key,
            another_device.private_key,
        )
        .unwrap();
        assert_eq!(unwrapped, bootstrap.account_data_key);
    }

    #[test]
    fn ffi_error_mapping_preserves_kind_and_message() {
        let path = temporary_database_path("ffi_error_mapping_preserves_kind_and_message");
        let core = FfiTaskManagerCore::new(path.to_string_lossy().into_owned()).unwrap();

        let error = core.get_task("not-a-uuid".to_owned()).unwrap_err();

        assert_eq!(error.kind(), FfiCoreErrorKind::InvalidUuid);
        assert!(error.message().contains("invalid UUID"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn ffi_patch_rejects_conflicting_nullable_field_operations() {
        let due_at_error = TaskPatch::try_from(FfiTaskPatch {
            due_at: Some(10),
            clear_due_at: true,
            ..FfiTaskPatch::default()
        })
        .unwrap_err();
        assert_eq!(due_at_error.kind(), FfiCoreErrorKind::InvalidPatch);

        let project_id_error = TaskPatch::try_from(FfiTaskPatch {
            project_id: Some(Uuid::new_v4().to_string()),
            clear_project_id: true,
            ..FfiTaskPatch::default()
        })
        .unwrap_err();
        assert_eq!(project_id_error.kind(), FfiCoreErrorKind::InvalidPatch);

        let reminder_error = TaskPatch::try_from(FfiTaskPatch {
            reminder_offset_ms: Some(10),
            clear_reminder_offset_ms: true,
            ..FfiTaskPatch::default()
        })
        .unwrap_err();
        assert_eq!(reminder_error.kind(), FfiCoreErrorKind::InvalidPatch);

        let negative_reminder_error = TaskPatch::try_from(FfiTaskPatch {
            reminder_offset_ms: Some(-1),
            ..FfiTaskPatch::default()
        })
        .unwrap_err();
        assert_eq!(
            negative_reminder_error.kind(),
            FfiCoreErrorKind::InvalidPatch
        );
    }

    #[test]
    fn ffi_crypto_maps_byte_arrays_and_nonce_lengths() {
        let key = generate_account_data_key();
        let task = FfiTask {
            id: Uuid::new_v4().to_string(),
            title: "title".to_owned(),
            body: "body".to_owned(),
            due_at: None,
            reminder_offset_ms: None,
            status: FfiTaskStatus::Done,
            project_id: None,
            tags: vec!["tag".to_owned()],
            created_at: 1,
            updated_at: 2,
            deleted: false,
            dirty: true,
        };

        let blob = encrypt_task_blob(task.clone(), key.clone()).unwrap();
        assert_eq!(blob.nonce.len(), 12);
        assert_eq!(decrypt_task_blob(blob, key).unwrap(), task);

        let error = decrypt_task_blob(
            FfiBlob {
                ciphertext: Vec::new(),
                nonce: vec![0; 11],
            },
            vec![0; 32],
        )
        .unwrap_err();
        assert_eq!(error.kind(), FfiCoreErrorKind::InvalidNonce);
    }

    #[test]
    fn udl_mentions_exposed_apis_and_hides_internal_apis() {
        let udl = include_str!("../uniffi/core.udl");
        for expected in [
            "interface FfiTaskManagerCore",
            "create_task",
            "create_task_with_options",
            "get_task",
            "update_task",
            "delete_task",
            "list_tasks",
            "search_tasks",
            "FfiTaskList",
            "create_list",
            "list_task_lists",
            "update_list",
            "delete_list",
            "FfiPlaintextSettings",
            "read_plaintext_settings",
            "write_plaintext_settings",
            "FfiVaultSettings",
            "vault_settings",
            "update_vault_settings",
            "FfiSyncStatus",
            "FfiSyncClient",
            "FfiSyncResult",
            "sync_run",
            "retry_queue_entries",
            "generate_local_account_bootstrap",
            "wrap_account_data_key",
            "unwrap_account_data_key",
            "FfiPlatform",
            "FfiSyncAuthState",
            "FfiEnrollmentState",
            "FfiWrappedAccountDataKeyPayload",
            "normalize_ffi_sync_server_url",
            "ffi_sync_server_origin",
            "ffi_sync_auth_state",
            "ffi_existing_account_enrollment_state",
            "ffi_begin_existing_account_enrollment",
            "create_ffi_wrapped_account_data_key_payload",
            "accept_ffi_wrapped_account_data_key_payload",
            "auth_access_token_id",
            "auth_refresh_token_id",
            "auth_sync_origin_id",
            "auth_account_id_id",
            "encrypt_task_blob",
            "decrypt_task_blob",
            "generate_account_data_key",
            "InvalidPatch",
        ] {
            assert!(udl.contains(expected), "UDL missing {expected}");
        }
        for internal in [
            "LocalDatabase",
            "interface SyncClient",
            "callback interface SyncClient",
            "MockPlatform",
            "interface DeviceKeypair",
            "dictionary DeviceKeypair",
        ] {
            assert!(
                !udl.contains(internal),
                "UDL exposed internal API {internal}"
            );
        }
    }

    fn temporary_database_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "taskmanager-core-{name}-{}-{}.sqlite3",
            std::process::id(),
            uuid::Uuid::new_v4()
        ))
    }
}
