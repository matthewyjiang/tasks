use std::fmt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use uuid::Uuid;

use crate::core::TaskManagerCore;
use crate::crypto::{decrypt_blob, encrypt_blob, generate_data_key};
use crate::error::CoreError;
use crate::types::{Blob, Task, TaskFilter, TaskPatch, TaskSort, TaskStatus};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiTaskStatus {
    Inbox,
    InProgress,
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
    pub status: FfiTaskStatus,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
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

pub fn generate_account_data_key() -> Vec<u8> {
    generate_data_key().to_vec()
}

impl From<TaskStatus> for FfiTaskStatus {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Inbox => Self::Inbox,
            TaskStatus::InProgress => Self::InProgress,
            TaskStatus::Done => Self::Done,
        }
    }
}

impl From<FfiTaskStatus> for TaskStatus {
    fn from(status: FfiTaskStatus) -> Self {
        match status {
            FfiTaskStatus::Inbox => Self::Inbox,
            FfiTaskStatus::InProgress => Self::InProgress,
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

impl TryFrom<FfiTaskPatch> for TaskPatch {
    type Error = FfiCoreError;

    fn try_from(patch: FfiTaskPatch) -> Result<Self, Self::Error> {
        if patch.clear_due_at && patch.due_at.is_some() {
            return Err(FfiCoreError::InvalidPatch {
                error_message: "due_at cannot be set and cleared in the same patch".to_owned(),
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
                    status: Some(FfiTaskStatus::InProgress),
                    project_id: Some(project_id.clone()),
                    tags: Some(vec!["work".to_owned(), "urgent".to_owned()]),
                    ..FfiTaskPatch::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "updated");
        assert_eq!(updated.project_id, Some(project_id.clone()));
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
    }

    #[test]
    fn ffi_crypto_maps_byte_arrays_and_nonce_lengths() {
        let key = generate_account_data_key();
        let task = FfiTask {
            id: Uuid::new_v4().to_string(),
            title: "title".to_owned(),
            body: "body".to_owned(),
            due_at: None,
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
            "get_task",
            "update_task",
            "delete_task",
            "list_tasks",
            "search_tasks",
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
            "DeviceKeypair",
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
