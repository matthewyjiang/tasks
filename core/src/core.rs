use std::path::Path;

use uuid::Uuid;

use crate::db::LocalDatabase;
use crate::error::CoreResult;
use crate::types::{RetryQueueEntry, SyncStatus, Task, TaskFilter, TaskList, TaskPatch, TaskSort};

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

    pub fn get_task(&self, task_id: Uuid) -> CoreResult<Task> {
        self.database.get_task(task_id)
    }

    pub fn update_task(&self, task_id: Uuid, patch: TaskPatch) -> CoreResult<Task> {
        self.database.update_task(task_id, patch)
    }

    pub fn delete_task(&self, task_id: Uuid) -> CoreResult<()> {
        self.database.delete_task(task_id)
    }

    pub fn list_tasks(&self, filter: TaskFilter, sort: TaskSort) -> CoreResult<Vec<Task>> {
        self.database.list_tasks(filter, sort)
    }

    pub fn search_tasks(&self, query: String) -> CoreResult<Vec<Task>> {
        self.database.search_tasks(query)
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
