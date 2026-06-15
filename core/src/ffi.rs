use std::fmt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use uuid::Uuid;

use crate::core::TaskManagerCore;
use crate::crypto::{
    decrypt_blob, encrypt_blob, generate_data_key, generate_device_keypair,
    public_key_from_private_key, unwrap_data_key, wrap_data_key, DeviceKeypair,
};
use crate::error::CoreError;
use crate::platform::{ACCOUNT_DATA_KEY_ID, DEVICE_PRIVATE_KEY_ID};
use crate::settings::{
    AuthMethod, DefaultSort, DisplayDensity, Keybindings, PlaintextSettings, Theme, VaultSettings,
};
use crate::types::{
    Blob, RetryQueueEntry, SharedTaskInvite, SharedTaskRecipient, SharedTaskState, SyncStatus,
    Task, TaskFilter, TaskList, TaskPatch, TaskSort, TaskStatus,
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
            "retry_queue_entries",
            "generate_local_account_bootstrap",
            "wrap_account_data_key",
            "unwrap_account_data_key",
            "encrypt_task_blob",
            "decrypt_task_blob",
            "generate_account_data_key",
            "InvalidPatch",
        ] {
            assert!(udl.contains(expected), "UDL missing {expected}");
        }
        for internal in [
            "LocalDatabase",
            "SyncClient",
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
