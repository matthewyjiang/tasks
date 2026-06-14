use std::collections::HashSet;

use uuid::Uuid;

use crate::crypto::{decrypt_blob, encrypt_blob};
use crate::db::LocalDatabase;
use crate::error::{CoreResult, SyncError};
use crate::platform::Platform;
use crate::types::{Blob, SyncResult, Task};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteBlob {
    pub task_id: Uuid,
    pub blob: Blob,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobPush {
    pub task_id: Uuid,
    pub blob: Blob,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushResponse {
    pub accepted_task_ids: Vec<Uuid>,
    pub failed_task_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullResponse {
    pub blobs: Vec<RemoteBlob>,
    pub cursor: i64,
}

pub trait SyncClient {
    fn push_blobs(&self, blobs: Vec<BlobPush>) -> CoreResult<PushResponse>;
    fn delete_blobs(&self, task_ids: Vec<Uuid>) -> CoreResult<PushResponse>;
    fn pull_blobs(&self, since: i64) -> CoreResult<PullResponse>;
}

pub fn sync_push(
    database: &LocalDatabase,
    platform: &dyn Platform,
    client: &dyn SyncClient,
    data_key: &[u8],
) -> CoreResult<SyncResult> {
    let dirty_tasks = database.dirty_tasks()?;
    if dirty_tasks.is_empty() {
        return Ok(SyncResult {
            pushed: 0,
            pulled: 0,
            failed: 0,
            cursor: None,
        });
    }

    if !platform.network_available() {
        let now = now_ms();
        for task in dirty_tasks {
            database.queue_retry(task.id, now)?;
        }
        return Err(SyncError::NetworkUnavailable.into());
    }

    let mut blob_pushes = Vec::new();
    let mut tombstones = Vec::new();
    for task in dirty_tasks {
        if task.deleted {
            tombstones.push(task.id);
        } else {
            let encryption_key = database.task_encryption_key(task.id, data_key)?;
            blob_pushes.push(BlobPush {
                task_id: task.id,
                blob: encrypt_blob(&task, &encryption_key)?,
            });
        }
    }

    let mut confirmed = HashSet::new();
    let mut failed = HashSet::new();
    if !blob_pushes.is_empty() {
        let attempted: Vec<Uuid> = blob_pushes.iter().map(|push| push.task_id).collect();
        match client.push_blobs(blob_pushes) {
            Ok(response) => {
                confirmed.extend(response.accepted_task_ids);
                failed.extend(response.failed_task_ids);
            }
            Err(error) => {
                queue_failed(database, attempted)?;
                return Err(error);
            }
        }
    }
    if !tombstones.is_empty() {
        let attempted = tombstones.clone();
        match client.delete_blobs(tombstones) {
            Ok(response) => {
                confirmed.extend(response.accepted_task_ids);
                failed.extend(response.failed_task_ids);
            }
            Err(error) => {
                queue_failed(database, attempted)?;
                return Err(error);
            }
        }
    }

    for task_id in &confirmed {
        database.clear_dirty(*task_id)?;
        database.clear_retry(*task_id)?;
    }
    queue_failed(database, failed.iter().copied())?;

    Ok(SyncResult {
        pushed: confirmed.len(),
        pulled: 0,
        failed: failed.len(),
        cursor: None,
    })
}

pub fn sync_pull(
    database: &LocalDatabase,
    client: &dyn SyncClient,
    data_key: &[u8],
) -> CoreResult<SyncResult> {
    let since = database.last_pull_cursor()?;
    let response = client.pull_blobs(since)?;
    let mut pulled = 0;
    let mut conflicts = 0;

    for remote in response.blobs {
        let encryption_key = database.task_encryption_key(remote.task_id, data_key)?;
        let mut remote_task = decrypt_blob(&remote.blob, &encryption_key)?;
        remote_task.dirty = false;

        let task_to_store = match database.get_task(remote.task_id) {
            Ok(local) if local.dirty => {
                conflicts += 1;
                resolve_conflict(&local, &remote_task)
            }
            Ok(local) => resolve_conflict(&local, &remote_task),
            Err(_) => remote_task,
        };
        database.upsert_synced_task(&task_to_store)?;
        pulled += 1;
    }

    database.set_last_pull_cursor(response.cursor)?;
    Ok(SyncResult {
        pushed: 0,
        pulled,
        failed: conflicts,
        cursor: Some(response.cursor),
    })
}

pub fn resolve_conflict(local: &Task, remote: &Task) -> Task {
    if remote.updated_at > local.updated_at
        || (remote.updated_at == local.updated_at && remote.id < local.id)
    {
        remote.clone()
    } else {
        local.clone()
    }
}

fn queue_failed(
    database: &LocalDatabase,
    task_ids: impl IntoIterator<Item = Uuid>,
) -> CoreResult<()> {
    let now = now_ms();
    for task_id in task_ids {
        database.queue_retry(task_id, now)?;
    }
    Ok(())
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::generate_data_key;
    use crate::platform::MockPlatform;
    use std::cell::RefCell;

    #[derive(Default)]
    struct FakeSyncClient {
        pushed: RefCell<Vec<BlobPush>>,
        deleted: RefCell<Vec<Uuid>>,
        pull_response: RefCell<Option<PullResponse>>,
        push_error: RefCell<Option<SyncError>>,
        push_response: RefCell<Option<PushResponse>>,
    }

    impl SyncClient for FakeSyncClient {
        fn push_blobs(&self, blobs: Vec<BlobPush>) -> CoreResult<PushResponse> {
            if let Some(error) = self.push_error.borrow_mut().take() {
                return Err(error.into());
            }
            let default_accepted_task_ids = blobs.iter().map(|blob| blob.task_id).collect();
            self.pushed.borrow_mut().extend(blobs);
            Ok(self
                .push_response
                .borrow_mut()
                .take()
                .unwrap_or(PushResponse {
                    accepted_task_ids: default_accepted_task_ids,
                    failed_task_ids: Vec::new(),
                }))
        }
        fn delete_blobs(&self, task_ids: Vec<Uuid>) -> CoreResult<PushResponse> {
            let accepted_task_ids = task_ids.clone();
            self.deleted.borrow_mut().extend(task_ids);
            Ok(PushResponse {
                accepted_task_ids,
                failed_task_ids: Vec::new(),
            })
        }
        fn pull_blobs(&self, _since: i64) -> CoreResult<PullResponse> {
            Ok(self
                .pull_response
                .borrow_mut()
                .take()
                .unwrap_or(PullResponse {
                    blobs: Vec::new(),
                    cursor: 0,
                }))
        }
    }

    #[test]
    fn sync_push_sends_dirty_non_deleted_tasks_as_encrypted_blobs() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let task = db
            .create_task("a".to_owned(), "b".to_owned(), None)
            .unwrap();
        let key = generate_data_key();
        let client = FakeSyncClient::default();
        let platform = MockPlatform::with_network_available(true);

        let result = sync_push(&db, &platform, &client, &key).unwrap();

        assert_eq!(result.pushed, 1);
        assert_eq!(client.pushed.borrow().len(), 1);
        let decrypted = decrypt_blob(&client.pushed.borrow()[0].blob, &key).unwrap();
        assert_eq!(decrypted.id, task.id);
    }

    #[test]
    fn successful_push_clears_dirty_only_for_confirmed_tasks() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let task = db
            .create_task("a".to_owned(), "b".to_owned(), None)
            .unwrap();
        let key = generate_data_key();
        let client = FakeSyncClient::default();
        let platform = MockPlatform::with_network_available(true);

        sync_push(&db, &platform, &client, &key).unwrap();

        assert!(!db.get_task(task.id).unwrap().dirty);
    }

    #[test]
    fn partial_batch_failure_leaves_failed_tasks_dirty() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let accepted = db
            .create_task("accepted".to_owned(), "b".to_owned(), None)
            .unwrap();
        let failed = db
            .create_task("failed".to_owned(), "b".to_owned(), None)
            .unwrap();
        let client = FakeSyncClient::default();
        *client.push_response.borrow_mut() = Some(PushResponse {
            accepted_task_ids: vec![accepted.id],
            failed_task_ids: vec![failed.id],
        });
        let platform = MockPlatform::with_network_available(true);

        let result = sync_push(&db, &platform, &client, &generate_data_key()).unwrap();

        assert_eq!(result.pushed, 1);
        assert!(!db.get_task(accepted.id).unwrap().dirty);
        assert!(db.get_task(failed.id).unwrap().dirty);
        assert_eq!(db.retry_queue_entries().unwrap()[0].0, failed.id);
    }

    #[test]
    fn server_error_returns_status_and_body_and_keeps_dirty() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let task = db
            .create_task("a".to_owned(), "b".to_owned(), None)
            .unwrap();
        let client = FakeSyncClient::default();
        *client.push_error.borrow_mut() = Some(SyncError::ServerError {
            status: 500,
            body: "oops".to_owned(),
        });
        let platform = MockPlatform::with_network_available(true);

        let error = sync_push(&db, &platform, &client, &generate_data_key()).unwrap_err();

        assert!(matches!(
            error,
            crate::error::CoreError::Sync(SyncError::ServerError { status: 500, body }) if body == "oops"
        ));
        assert!(db.get_task(task.id).unwrap().dirty);
    }

    #[test]
    fn sync_push_sends_tombstones_through_delete_path() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let task = db
            .create_task("a".to_owned(), "b".to_owned(), None)
            .unwrap();
        db.delete_task(task.id).unwrap();
        let client = FakeSyncClient::default();
        let platform = MockPlatform::with_network_available(true);

        sync_push(&db, &platform, &client, &generate_data_key()).unwrap();

        assert_eq!(*client.deleted.borrow(), vec![task.id]);
        assert!(client.pushed.borrow().is_empty());
    }

    #[test]
    fn network_unavailable_queues_retry_and_keeps_dirty() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let task = db
            .create_task("a".to_owned(), "b".to_owned(), None)
            .unwrap();
        let client = FakeSyncClient::default();
        let platform = MockPlatform::with_network_available(false);

        let error = sync_push(&db, &platform, &client, &generate_data_key()).unwrap_err();

        assert!(matches!(
            error,
            crate::error::CoreError::Sync(SyncError::NetworkUnavailable)
        ));
        assert!(db.get_task(task.id).unwrap().dirty);
    }

    #[test]
    fn retry_queue_persists_and_backoff_increases() {
        let path = temporary_database_path("retry_queue_persists_and_backoff_increases");
        let db = LocalDatabase::open(&path).unwrap();
        let task = db
            .create_task("a".to_owned(), "b".to_owned(), None)
            .unwrap();

        db.queue_retry(task.id, 1_000).unwrap();
        let first = db.retry_queue_entries().unwrap()[0];
        db.queue_retry(task.id, 2_000).unwrap();
        let second = db.retry_queue_entries().unwrap()[0];
        drop(db);

        let reopened = LocalDatabase::open(&path).unwrap();
        let persisted = reopened.retry_queue_entries().unwrap()[0];

        assert_eq!(first.1, 1);
        assert_eq!(second.1, 2);
        assert!(second.2 > first.2);
        assert_eq!(persisted, second);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn auth_failure_keeps_dirty_flags() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let task = db
            .create_task("a".to_owned(), "b".to_owned(), None)
            .unwrap();
        let client = FakeSyncClient::default();
        *client.push_error.borrow_mut() = Some(SyncError::AuthExpired);
        let platform = MockPlatform::with_network_available(true);

        let error = sync_push(&db, &platform, &client, &generate_data_key()).unwrap_err();

        assert!(matches!(
            error,
            crate::error::CoreError::Sync(SyncError::AuthExpired)
        ));
        assert!(db.get_task(task.id).unwrap().dirty);
    }

    #[test]
    fn sync_push_uses_per_task_key_for_shared_tasks() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let task = db
            .create_task("shared".to_owned(), "body".to_owned(), None)
            .unwrap();
        let account_key = generate_data_key();
        let task_key = generate_data_key();
        let wrapped = Blob {
            ciphertext: vec![1, 2, 3],
            nonce: [7; 12],
        };
        db.share_task(task.id, Uuid::new_v4(), task_key.to_vec(), wrapped)
            .unwrap();
        let client = FakeSyncClient::default();
        let platform = MockPlatform::with_network_available(true);

        sync_push(&db, &platform, &client, &account_key).unwrap();

        let pushed = &client.pushed.borrow()[0].blob;
        assert!(decrypt_blob(pushed, &account_key).is_err());
        assert_eq!(decrypt_blob(pushed, &task_key).unwrap().id, task.id);
    }

    #[test]
    fn sync_pull_uses_per_task_key_for_accepted_shared_tasks() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let task_key = generate_data_key();
        let account_key = generate_data_key();
        let task = Task {
            id: Uuid::new_v4(),
            title: "shared remote".to_owned(),
            body: "body".to_owned(),
            due_at: None,
            reminder_offset_ms: None,
            status: crate::types::TaskStatus::Open,
            project_id: None,
            tags: Vec::new(),
            created_at: 1,
            updated_at: 2,
            deleted: false,
            dirty: false,
        };
        let invite = crate::types::SharedTaskInvite {
            task_id: task.id,
            owner_id: Uuid::new_v4(),
            recipient_id: Uuid::new_v4(),
            wrapped_task_key: Blob {
                ciphertext: vec![1],
                nonce: [1; 12],
            },
        };
        db.accept_shared_task(invite, task_key.to_vec()).unwrap();
        let client = FakeSyncClient::default();
        *client.pull_response.borrow_mut() = Some(PullResponse {
            blobs: vec![RemoteBlob {
                task_id: task.id,
                blob: encrypt_blob(&task, &task_key).unwrap(),
                updated_at: task.updated_at,
            }],
            cursor: 3,
        });

        let result = sync_pull(&db, &client, &account_key).unwrap();

        assert_eq!(result.pulled, 1);
        assert_eq!(db.get_task(task.id).unwrap().title, "shared remote");
    }

    #[test]
    fn sync_pull_decrypts_remote_blobs_and_advances_cursor() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let key = generate_data_key();
        let mut remote = db
            .create_task("r".to_owned(), "body".to_owned(), None)
            .unwrap();
        remote.dirty = false;
        let blob = encrypt_blob(&remote, &key).unwrap();
        let client = FakeSyncClient::default();
        *client.pull_response.borrow_mut() = Some(PullResponse {
            blobs: vec![RemoteBlob {
                task_id: remote.id,
                blob,
                updated_at: remote.updated_at,
            }],
            cursor: 55,
        });

        let result = sync_pull(&db, &client, &key).unwrap();

        assert_eq!(result.pulled, 1);
        assert_eq!(db.get_task(remote.id).unwrap().title, "r");
        assert_eq!(db.last_pull_cursor().unwrap(), 55);
    }

    #[test]
    fn pull_decryption_failure_does_not_advance_cursor() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let client = FakeSyncClient::default();
        *client.pull_response.borrow_mut() = Some(PullResponse {
            blobs: vec![RemoteBlob {
                task_id: Uuid::new_v4(),
                blob: Blob {
                    ciphertext: vec![1, 2, 3],
                    nonce: [0; 12],
                },
                updated_at: 1,
            }],
            cursor: 55,
        });

        assert!(sync_pull(&db, &client, &generate_data_key()).is_err());
        assert_eq!(db.last_pull_cursor().unwrap(), 0);
    }

    #[test]
    fn last_write_wins_and_equal_timestamp_is_deterministic() {
        let mut local = LocalDatabase::open_in_memory()
            .unwrap()
            .create_task("l".to_owned(), "b".to_owned(), None)
            .unwrap();
        let mut remote = local.clone();
        remote.title = "r".to_owned();
        remote.updated_at = local.updated_at + 1;
        assert_eq!(resolve_conflict(&local, &remote).title, "r");
        local.updated_at = remote.updated_at;
        assert_eq!(
            resolve_conflict(&local, &remote),
            resolve_conflict(&local, &remote)
        );
    }

    fn temporary_database_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "taskmanager-core-{name}-{}-{}.sqlite3",
            std::process::id(),
            uuid::Uuid::new_v4()
        ))
    }
}
