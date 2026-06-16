use std::path::Path;

use uuid::Uuid;

use crate::crypto::{generate_data_key, unwrap_data_key, wrap_data_key};
use crate::db::LocalDatabase;
use crate::error::CoreResult;
use crate::platform::Platform;
use crate::settings::VaultSettings;
use crate::sync::{sync_session, SyncClient};
use crate::types::{
    RetryQueueEntry, SharedTaskInvite, SharedTaskRecipient, SharedTaskState, SyncStatus, Task,
    TaskFilter, TaskList, TaskPatch, TaskSort,
};

pub struct TaskManagerCore {
    database: LocalDatabase,
}

impl TaskManagerCore {
    pub fn open(database_path: impl AsRef<Path>) -> CoreResult<Self> {
        Ok(Self {
            database: LocalDatabase::open(database_path)?,
        })
    }

    pub fn open_in_memory() -> CoreResult<Self> {
        Ok(Self {
            database: LocalDatabase::open_in_memory()?,
        })
    }

    pub fn create_list(&self, name: String) -> CoreResult<TaskList> {
        self.database.create_list(name)
    }

    pub fn list_task_lists(&self) -> CoreResult<Vec<TaskList>> {
        self.database.list_task_lists()
    }

    pub fn update_list(&self, list_id: Uuid, name: String) -> CoreResult<TaskList> {
        self.database.update_list(list_id, name)
    }

    pub fn delete_list(&self, list_id: Uuid) -> CoreResult<()> {
        self.database.delete_list(list_id)
    }

    pub fn create_task(
        &self,
        title: String,
        body: String,
        due_at: Option<i64>,
    ) -> CoreResult<Task> {
        self.database.create_task(title, body, due_at)
    }

    pub fn create_task_with_options(
        &self,
        title: String,
        body: String,
        due_at: Option<i64>,
        project_id: Option<Uuid>,
        tags: Vec<String>,
    ) -> CoreResult<Task> {
        self.database
            .create_task_with_options(title, body, due_at, project_id, tags)
    }

    pub fn get_task(&self, task_id: Uuid) -> CoreResult<Task> {
        self.database.get_task(task_id)
    }

    pub fn update_task(&self, task_id: Uuid, patch: TaskPatch) -> CoreResult<Task> {
        self.database.update_task(task_id, patch)
    }

    pub fn delete_task(&self, task_id: Uuid) -> CoreResult<()> {
        self.database.delete_task(task_id)
    }

    pub fn restore_task(&self, task_id: Uuid) -> CoreResult<Task> {
        self.database.restore_task(task_id)
    }

    pub fn list_tasks(&self, filter: TaskFilter, sort: TaskSort) -> CoreResult<Vec<Task>> {
        self.database.list_tasks(filter, sort)
    }

    pub fn search_tasks(&self, query: String) -> CoreResult<Vec<Task>> {
        self.database.search_tasks(query)
    }

    pub fn share_task_with_recipient(
        &self,
        task_id: Uuid,
        recipient_id: Uuid,
        recipient_public_key: &[u8],
        owner_private_key: &[u8],
    ) -> CoreResult<SharedTaskRecipient> {
        self.database.get_task(task_id)?;
        let task_key = self
            .database
            .shared_task_state(task_id)
            .map(|state| state.task_key)
            .unwrap_or_else(|_| generate_data_key().to_vec());
        let wrapped_task_key = wrap_data_key(&task_key, recipient_public_key, owner_private_key)?;
        self.database
            .share_task(task_id, recipient_id, task_key, wrapped_task_key)
    }

    pub fn accept_shared_task_invite(
        &self,
        invite: SharedTaskInvite,
        owner_public_key: &[u8],
        recipient_private_key: &[u8],
    ) -> CoreResult<SharedTaskState> {
        let task_key = unwrap_data_key(
            &invite.wrapped_task_key,
            owner_public_key,
            recipient_private_key,
        )?;
        self.database.accept_shared_task(invite, task_key.to_vec())
    }

    pub fn revoke_shared_task_recipient(
        &self,
        task_id: Uuid,
        recipient_id: Uuid,
        remaining_recipient_public_keys: Vec<(Uuid, Vec<u8>)>,
        owner_private_key: &[u8],
    ) -> CoreResult<SharedTaskState> {
        let current_state = self.database.shared_task_state(task_id)?;
        let remaining_recipient_ids: Vec<Uuid> = current_state
            .active_recipients()
            .filter(|recipient| recipient.recipient_id != recipient_id)
            .map(|recipient| recipient.recipient_id)
            .collect();
        let rotated_task_key = generate_data_key();
        let mut rewrapped_recipients = Vec::with_capacity(remaining_recipient_ids.len());
        for remaining_recipient_id in remaining_recipient_ids {
            let public_key = remaining_recipient_public_keys
                .iter()
                .find(|(id, _)| *id == remaining_recipient_id)
                .map(|(_, public_key)| public_key.as_slice())
                .ok_or_else(|| {
                    crate::error::PlatformError::OperationFailed(format!(
                        "missing public key for remaining recipient {remaining_recipient_id}"
                    ))
                })?;
            let wrapped_task_key = wrap_data_key(&rotated_task_key, public_key, owner_private_key)?;
            rewrapped_recipients.push(SharedTaskRecipient {
                task_id,
                recipient_id: remaining_recipient_id,
                wrapped_task_key,
                created_at: 0,
                revoked_at: None,
            });
        }
        self.database.revoke_shared_task_recipient(
            task_id,
            recipient_id,
            rotated_task_key.to_vec(),
            rewrapped_recipients,
        )
    }

    pub fn shared_task_state(&self, task_id: Uuid) -> CoreResult<SharedTaskState> {
        self.database.shared_task_state(task_id)
    }

    pub fn shared_task_revocation_notice(&self) -> &'static str {
        SharedTaskState::revocation_notice()
    }

    pub fn vault_settings(&self) -> CoreResult<VaultSettings> {
        self.database.vault_settings()
    }

    pub fn update_vault_settings(&self, settings: VaultSettings) -> CoreResult<VaultSettings> {
        self.database.update_vault_settings(&settings)
    }

    pub fn sync_run(
        &self,
        platform: &dyn Platform,
        client: &dyn SyncClient,
        data_key: &[u8],
    ) -> CoreResult<crate::types::SyncResult> {
        sync_session(&self.database, platform, client, data_key)
    }

    pub fn sync_status(&self) -> CoreResult<SyncStatus> {
        Ok(SyncStatus {
            dirty_count: self.database.dirty_tasks()?.len(),
            retry_queue_depth: self.database.retry_queue_entries()?.len(),
            cursor: self.database.last_pull_cursor()?,
        })
    }

    pub fn retry_queue_entries(&self) -> CoreResult<Vec<RetryQueueEntry>> {
        Ok(self
            .database
            .retry_queue_entries()?
            .into_iter()
            .map(|(task_id, attempt, next_retry)| RetryQueueEntry {
                task_id,
                attempt,
                next_retry,
            })
            .collect())
    }

    pub fn queue_sync_retry(&self, task_id: Uuid, now: i64) -> CoreResult<RetryQueueEntry> {
        self.database.get_task(task_id)?;
        self.database.queue_retry(task_id, now)?;
        self.retry_queue_entries()?
            .into_iter()
            .find(|entry| entry.task_id == task_id)
            .ok_or_else(|| crate::error::DbError::TaskNotFound(task_id).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{CoreError, DbError};
    use crate::types::TaskStatus;

    #[test]
    fn constructor_opens_in_memory_database() {
        let core = TaskManagerCore::open_in_memory().unwrap();
        let tasks = core
            .list_tasks(TaskFilter::default(), TaskSort::UpdatedAtDesc)
            .unwrap();

        assert!(tasks.is_empty());
    }

    #[test]
    fn constructor_opens_or_creates_database_path() {
        let path = temporary_database_path("constructor_opens_or_creates_database_path");
        let core = TaskManagerCore::open(&path).unwrap();

        let created = core
            .create_task("persisted".to_owned(), "body".to_owned(), None)
            .unwrap();
        drop(core);

        let reopened = TaskManagerCore::open(&path).unwrap();
        assert_eq!(reopened.get_task(created.id).unwrap(), created);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn constructor_fails_clearly_for_invalid_database_path() {
        let path = std::env::temp_dir().join(format!(
            "taskmanager-core-missing-dir-{}-{}.sqlite3",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let invalid_path = path.join("db.sqlite3");

        let error = match TaskManagerCore::open(invalid_path) {
            Ok(_) => panic!("opening an invalid database path should fail"),
            Err(error) => error,
        };
        assert!(matches!(error, CoreError::Database(DbError::Sqlite(_))));
    }

    #[test]
    fn facade_list_methods_delegate_to_database() {
        let core = TaskManagerCore::open_in_memory().unwrap();

        assert!(core.list_task_lists().unwrap().is_empty());
        let list = core.create_list("Work".to_owned()).unwrap();
        assert_eq!(list.name, "Work");
        assert_eq!(core.list_task_lists().unwrap(), vec![list.clone()]);

        let updated = core.update_list(list.id, "Personal".to_owned()).unwrap();
        assert_eq!(updated.name, "Personal");
        core.delete_list(list.id).unwrap();
        assert!(core.list_task_lists().unwrap().is_empty());
    }

    #[test]
    fn facade_crud_methods_delegate_to_database() {
        let core = TaskManagerCore::open_in_memory().unwrap();
        let created = core
            .create_task("title".to_owned(), "body".to_owned(), Some(123))
            .unwrap();

        assert_eq!(core.get_task(created.id).unwrap(), created);

        let updated = core
            .update_task(
                created.id,
                TaskPatch {
                    title: Some("updated".to_owned()),
                    status: Some(TaskStatus::Done),
                    ..TaskPatch::default()
                },
            )
            .unwrap();
        assert_eq!(updated.title, "updated");
        assert_eq!(updated.status, TaskStatus::Done);

        let listed = core
            .list_tasks(TaskFilter::default(), TaskSort::UpdatedAtDesc)
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        let searched = core.search_tasks("updated".to_owned()).unwrap();
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].id, created.id);

        core.delete_task(created.id).unwrap();
        assert!(core.get_task(created.id).unwrap().deleted);

        let restored = core.restore_task(created.id).unwrap();
        assert!(!restored.deleted);
        assert!(!core.get_task(created.id).unwrap().deleted);
    }

    #[test]
    fn shared_task_flow_wraps_accepts_and_revokes_recipient_access() {
        let owner = TaskManagerCore::open_in_memory().unwrap();
        let recipient = TaskManagerCore::open_in_memory().unwrap();
        let task = owner
            .create_task("shared".to_owned(), "secret".to_owned(), None)
            .unwrap();
        let owner_keys = crate::crypto::generate_device_keypair();
        let recipient_keys = crate::crypto::generate_device_keypair();
        let recipient_id = Uuid::new_v4();

        let share = owner
            .share_task_with_recipient(
                task.id,
                recipient_id,
                &recipient_keys.public_key,
                &owner_keys.private_key,
            )
            .unwrap();
        let owner_state = owner.shared_task_state(task.id).unwrap();
        assert!(owner_state.is_shared());
        assert_eq!(owner_state.active_recipients().count(), 1);

        let accepted = recipient
            .accept_shared_task_invite(
                crate::types::SharedTaskInvite {
                    task_id: task.id,
                    owner_id: Uuid::new_v4(),
                    recipient_id,
                    wrapped_task_key: share.wrapped_task_key,
                },
                &owner_keys.public_key,
                &recipient_keys.private_key,
            )
            .unwrap();
        assert_eq!(accepted.task_key, owner_state.task_key);
        assert!(accepted.accepted_at.is_some());

        let second_recipient_keys = crate::crypto::generate_device_keypair();
        let second_recipient_id = Uuid::new_v4();
        let second_share = owner
            .share_task_with_recipient(
                task.id,
                second_recipient_id,
                &second_recipient_keys.public_key,
                &owner_keys.private_key,
            )
            .unwrap();
        let second_task_key = crate::crypto::unwrap_data_key(
            &second_share.wrapped_task_key,
            &owner_keys.public_key,
            &second_recipient_keys.private_key,
        )
        .unwrap();
        assert_eq!(second_task_key.to_vec(), owner_state.task_key);

        let revoked = owner
            .revoke_shared_task_recipient(
                task.id,
                recipient_id,
                vec![(
                    second_recipient_id,
                    second_recipient_keys.public_key.clone(),
                )],
                &owner_keys.private_key,
            )
            .unwrap();
        assert_eq!(revoked.active_recipients().count(), 1);
        assert_ne!(revoked.task_key, owner_state.task_key);
        let remaining = revoked.active_recipients().next().unwrap();
        assert_eq!(remaining.recipient_id, second_recipient_id);
        let rewrapped_task_key = crate::crypto::unwrap_data_key(
            &remaining.wrapped_task_key,
            &owner_keys.public_key,
            &second_recipient_keys.private_key,
        )
        .unwrap();
        assert_eq!(rewrapped_task_key.to_vec(), revoked.task_key);
        assert!(owner.get_task(task.id).unwrap().dirty);
        assert!(TaskManagerCore::open_in_memory()
            .unwrap()
            .shared_task_revocation_notice()
            .contains("cannot erase plaintext"));
    }

    #[test]
    fn create_task_with_options_validates_list_before_insert() {
        let core = TaskManagerCore::open_in_memory().unwrap();
        let missing_list = Uuid::new_v4();

        let error = core
            .create_task_with_options(
                "title".to_owned(),
                "body".to_owned(),
                None,
                Some(missing_list),
                vec!["tag".to_owned()],
            )
            .unwrap_err();

        assert!(matches!(
            error,
            CoreError::Database(DbError::InvalidRowData(_))
        ));
        assert!(core
            .list_tasks(TaskFilter::default(), TaskSort::UpdatedAtDesc)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn create_task_with_options_rejects_deleted_lists() {
        let core = TaskManagerCore::open_in_memory().unwrap();
        let list = core.create_list("Work".to_owned()).unwrap();
        core.delete_list(list.id).unwrap();

        let error = core
            .create_task_with_options(
                "title".to_owned(),
                "body".to_owned(),
                None,
                Some(list.id),
                Vec::new(),
            )
            .unwrap_err();

        assert!(matches!(
            error,
            CoreError::Database(DbError::InvalidRowData(_))
        ));
        assert!(core
            .list_tasks(TaskFilter::default(), TaskSort::UpdatedAtDesc)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn facade_preserves_database_error_semantics() {
        let core = TaskManagerCore::open_in_memory().unwrap();
        let missing_id = Uuid::new_v4();

        let error = core.get_task(missing_id).unwrap_err();
        assert!(
            matches!(error, CoreError::Database(DbError::TaskNotFound(id)) if id == missing_id)
        );
    }

    #[test]
    fn multiple_facades_over_same_path_observe_consistent_state() {
        let path =
            temporary_database_path("multiple_facades_over_same_path_observe_consistent_state");
        let first = TaskManagerCore::open(&path).unwrap();
        let second = TaskManagerCore::open(&path).unwrap();

        let created = first
            .create_task("shared".to_owned(), "body".to_owned(), None)
            .unwrap();

        assert_eq!(second.get_task(created.id).unwrap(), created);

        let _ = std::fs::remove_file(path);
    }

    fn temporary_database_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "taskmanager-core-{name}-{}-{}.sqlite3",
            std::process::id(),
            uuid::Uuid::new_v4()
        ))
    }
}
