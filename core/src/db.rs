use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::{CoreResult, DbError};
use crate::settings::{VaultSettings, VAULT_SETTINGS_ID};
use crate::types::{Task, TaskFilter, TaskList, TaskPatch, TaskSort, TaskStatus};

pub struct LocalDatabase {
    connection: Connection,
}

impl LocalDatabase {
    pub fn open(path: impl AsRef<Path>) -> CoreResult<Self> {
        let connection = Connection::open(path)?;
        let database = Self { connection };
        database.initialize_schema()?;
        Ok(database)
    }

    pub fn open_in_memory() -> CoreResult<Self> {
        let connection = Connection::open_in_memory()?;
        let database = Self { connection };
        database.initialize_schema()?;
        Ok(database)
    }

    pub fn create_list(&self, name: String) -> CoreResult<TaskList> {
        let now = now_ms();
        let list = TaskList {
            id: Uuid::new_v4(),
            name,
            created_at: now,
            updated_at: now,
            deleted: false,
            dirty: true,
        };
        self.upsert_list(&list)?;
        Ok(list)
    }

    pub fn list_task_lists(&self) -> CoreResult<Vec<TaskList>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, created_at, updated_at, deleted, dirty FROM task_lists WHERE deleted = 0 ORDER BY name COLLATE NOCASE ASC, id ASC",
        )?;
        let lists = statement.query_map([], read_task_list)?;
        collect_task_lists(lists)
    }

    pub fn update_list(&self, list_id: Uuid, name: String) -> CoreResult<TaskList> {
        let mut list = self.get_list(list_id)?;
        list.name = name;
        list.updated_at = now_ms().max(list.updated_at + 1);
        list.dirty = true;
        self.upsert_list(&list)?;
        Ok(list)
    }

    pub fn delete_list(&self, list_id: Uuid) -> CoreResult<()> {
        let mut list = self.get_list(list_id)?;
        list.deleted = true;
        list.updated_at = now_ms().max(list.updated_at + 1);
        list.dirty = true;
        self.upsert_list(&list)?;
        self.connection.execute(
            "UPDATE tasks SET project_id = NULL, updated_at = ?2, dirty = 1 WHERE project_id = ?1",
            params![list_id.to_string(), now_ms()],
        )?;
        Ok(())
    }

    fn get_list(&self, list_id: Uuid) -> CoreResult<TaskList> {
        self.connection
            .query_row(
                "SELECT id, name, created_at, updated_at, deleted, dirty FROM task_lists WHERE id = ?1",
                params![list_id.to_string()],
                read_task_list,
            )
            .optional()?
            .ok_or_else(|| DbError::InvalidRowData(format!("list not found: {list_id}")).into())
    }

    pub fn create_task(
        &self,
        title: String,
        body: String,
        due_at: Option<i64>,
    ) -> CoreResult<Task> {
        let now = now_ms();
        let task = Task {
            id: Uuid::new_v4(),
            title,
            body,
            due_at,
            status: TaskStatus::Open,
            project_id: None,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            deleted: false,
            dirty: true,
        };

        self.upsert_task(&task)?;
        Ok(task)
    }

    pub fn get_task(&self, task_id: Uuid) -> CoreResult<Task> {
        self.connection
            .query_row(
                "SELECT id, title, body, due_at, status, project_id, tags, created_at, updated_at, deleted, dirty FROM tasks WHERE id = ?1",
                params![task_id.to_string()],
                read_task,
            )
            .optional()?
            .ok_or_else(|| DbError::TaskNotFound(task_id).into())
    }

    pub fn update_task(&self, task_id: Uuid, patch: TaskPatch) -> CoreResult<Task> {
        let mut task = self.get_task(task_id)?;

        if let Some(title) = patch.title {
            task.title = title;
        }
        if let Some(body) = patch.body {
            task.body = body;
        }
        if let Some(due_at) = patch.due_at {
            task.due_at = due_at;
        }
        if let Some(status) = patch.status {
            task.status = status;
        }
        if let Some(project_id) = patch.project_id {
            task.project_id = project_id;
        }
        if let Some(tags) = patch.tags {
            task.tags = tags;
        }

        task.updated_at = now_ms().max(task.updated_at + 1);
        task.dirty = true;
        self.upsert_task(&task)?;
        Ok(task)
    }

    pub fn delete_task(&self, task_id: Uuid) -> CoreResult<()> {
        let mut task = self.get_task(task_id)?;
        task.deleted = true;
        task.dirty = true;
        task.updated_at = now_ms().max(task.updated_at + 1);
        self.upsert_task(&task)
    }

    pub fn list_tasks(&self, filter: TaskFilter, sort: TaskSort) -> CoreResult<Vec<Task>> {
        let mut sql = String::from(
            "SELECT id, title, body, due_at, status, project_id, tags, created_at, updated_at, deleted, dirty FROM tasks WHERE 1 = 1",
        );
        let mut values = Vec::new();

        if !filter.include_deleted {
            sql.push_str(" AND deleted = 0");
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            values.push(Value::Text(status_to_db(status).to_owned()));
        }
        if let Some(project_id) = filter.project_id {
            sql.push_str(" AND project_id = ?");
            values.push(Value::Text(project_id.to_string()));
        }
        for tag in filter.tags {
            sql.push_str(" AND EXISTS (SELECT 1 FROM json_each(tasks.tags) WHERE value = ?)");
            values.push(Value::Text(tag));
        }
        if let Some(due_after) = filter.due_after {
            sql.push_str(" AND due_at IS NOT NULL AND due_at >= ?");
            values.push(Value::Integer(due_after));
        }
        if let Some(due_before) = filter.due_before {
            sql.push_str(" AND due_at IS NOT NULL AND due_at <= ?");
            values.push(Value::Integer(due_before));
        }

        sql.push_str(match sort {
            TaskSort::UpdatedAtDesc => " ORDER BY updated_at DESC, id ASC",
            TaskSort::UpdatedAtAsc => " ORDER BY updated_at ASC, id ASC",
            TaskSort::DueAtAsc => " ORDER BY due_at IS NULL ASC, due_at ASC, id ASC",
            TaskSort::DueAtDesc => " ORDER BY due_at IS NULL ASC, due_at DESC, id ASC",
            TaskSort::CreatedAtAsc => " ORDER BY created_at ASC, id ASC",
            TaskSort::CreatedAtDesc => " ORDER BY created_at DESC, id ASC",
        });

        let mut statement = self.connection.prepare(&sql)?;
        let tasks = statement.query_map(params_from_iter(values), read_task)?;
        collect_tasks(tasks)
    }

    pub fn vault_settings(&self) -> CoreResult<VaultSettings> {
        let task = self
            .connection
            .query_row(
                "SELECT id, title, body, due_at, status, project_id, tags, created_at, updated_at, deleted, dirty FROM tasks WHERE id = ?1 AND title = ?2",
                params![Uuid::nil().to_string(), VAULT_SETTINGS_ID],
                read_task,
            )
            .optional()?;
        task.as_ref()
            .map(VaultSettings::from_reserved_task)
            .unwrap_or_else(|| Ok(VaultSettings::default()))
    }

    pub fn update_vault_settings(&self, settings: &VaultSettings) -> CoreResult<VaultSettings> {
        let task = settings.to_reserved_task(now_ms())?;
        self.upsert_task(&task)?;
        Ok(settings.clone())
    }

    pub fn search_tasks(&self, query: String) -> CoreResult<Vec<Task>> {
        let mut statement = self.connection.prepare(
            "SELECT t.id, t.title, t.body, t.due_at, t.status, t.project_id, t.tags, t.created_at, t.updated_at, t.deleted, t.dirty
             FROM tasks_fts f JOIN tasks t ON t.rowid = f.rowid
             WHERE tasks_fts MATCH ?1 AND t.deleted = 0 AND t.id != ?2
             ORDER BY rank, t.updated_at DESC, t.id ASC",
        )?;
        let tasks = statement.query_map(params![query, Uuid::nil().to_string()], read_task)?;
        collect_tasks(tasks)
    }

    fn initialize_schema(&self) -> CoreResult<()> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id          TEXT PRIMARY KEY,
                title       TEXT NOT NULL,
                body        TEXT NOT NULL DEFAULT '',
                due_at      INTEGER,
                status      TEXT NOT NULL DEFAULT 'open',
                project_id  TEXT,
                tags        TEXT NOT NULL DEFAULT '[]',
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL,
                deleted     INTEGER NOT NULL DEFAULT 0,
                dirty       INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS task_lists (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL,
                deleted     INTEGER NOT NULL DEFAULT 0,
                dirty       INTEGER NOT NULL DEFAULT 1
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS tasks_fts USING fts5(
                title, body,
                content='tasks', content_rowid='rowid'
            );

            CREATE TRIGGER IF NOT EXISTS tasks_ai AFTER INSERT ON tasks BEGIN
                INSERT INTO tasks_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body);
            END;
            CREATE TRIGGER IF NOT EXISTS tasks_ad AFTER DELETE ON tasks BEGIN
                INSERT INTO tasks_fts(tasks_fts, rowid, title, body) VALUES('delete', old.rowid, old.title, old.body);
            END;
            CREATE TRIGGER IF NOT EXISTS tasks_au AFTER UPDATE ON tasks BEGIN
                INSERT INTO tasks_fts(tasks_fts, rowid, title, body) VALUES('delete', old.rowid, old.title, old.body);
                INSERT INTO tasks_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body);
            END;

            CREATE TABLE IF NOT EXISTS sync_cursor (
                id          INTEGER PRIMARY KEY DEFAULT 1,
                last_pull   INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS sync_queue (
                task_id     TEXT PRIMARY KEY,
                queued_at   INTEGER NOT NULL,
                attempt     INTEGER NOT NULL DEFAULT 0,
                next_retry  INTEGER NOT NULL DEFAULT 0
            );

            DELETE FROM sync_queue
            WHERE rowid NOT IN (
                SELECT keep.rowid
                FROM sync_queue AS keep
                WHERE keep.task_id = sync_queue.task_id
                ORDER BY keep.attempt DESC, keep.next_retry DESC, keep.rowid DESC
                LIMIT 1
            );

            CREATE UNIQUE INDEX IF NOT EXISTS sync_queue_task_id_unique
            ON sync_queue(task_id);",
        )?;
        Ok(())
    }

    fn upsert_list(&self, list: &TaskList) -> CoreResult<()> {
        self.connection.execute(
            "INSERT INTO task_lists (id, name, created_at, updated_at, deleted, dirty)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                deleted = excluded.deleted,
                dirty = excluded.dirty",
            params![
                list.id.to_string(),
                list.name,
                list.created_at,
                list.updated_at,
                bool_to_db(list.deleted),
                bool_to_db(list.dirty),
            ],
        )?;
        Ok(())
    }

    pub(crate) fn upsert_synced_task(&self, task: &Task) -> CoreResult<()> {
        self.upsert_task(task)
    }

    pub(crate) fn dirty_tasks(&self) -> CoreResult<Vec<Task>> {
        let mut statement = self.connection.prepare(
            "SELECT id, title, body, due_at, status, project_id, tags, created_at, updated_at, deleted, dirty FROM tasks WHERE dirty = 1 ORDER BY updated_at ASC, id ASC",
        )?;
        let tasks = statement.query_map([], read_task)?;
        collect_tasks(tasks)
    }

    pub(crate) fn clear_dirty(&self, task_id: Uuid) -> CoreResult<()> {
        self.connection.execute(
            "UPDATE tasks SET dirty = 0 WHERE id = ?1",
            params![task_id.to_string()],
        )?;
        Ok(())
    }

    pub(crate) fn last_pull_cursor(&self) -> CoreResult<i64> {
        Ok(self
            .connection
            .query_row(
                "SELECT last_pull FROM sync_cursor WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }

    pub(crate) fn set_last_pull_cursor(&self, cursor: i64) -> CoreResult<()> {
        self.connection.execute(
            "INSERT INTO sync_cursor (id, last_pull) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET last_pull = excluded.last_pull",
            params![cursor],
        )?;
        Ok(())
    }

    pub(crate) fn retry_queue_entries(&self) -> CoreResult<Vec<(Uuid, i64, i64)>> {
        let mut statement = self
            .connection
            .prepare("SELECT task_id, attempt, next_retry FROM sync_queue ORDER BY task_id ASC")?;
        let rows = statement.query_map([], |row| {
            let task_id: String = row.get(0)?;
            Ok((parse_uuid(&task_id, "task_id")?, row.get(1)?, row.get(2)?))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub(crate) fn queue_retry(&self, task_id: Uuid, now: i64) -> CoreResult<()> {
        let current_attempt: Option<i64> = self
            .connection
            .query_row(
                "SELECT attempt FROM sync_queue WHERE task_id = ?1",
                params![task_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let attempt = current_attempt.unwrap_or(0) + 1;
        let delay_ms = 1_000_i64.saturating_mul(2_i64.saturating_pow((attempt - 1).min(10) as u32));
        self.connection.execute(
            "INSERT INTO sync_queue (task_id, queued_at, attempt, next_retry) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(task_id) DO UPDATE SET attempt = excluded.attempt, next_retry = excluded.next_retry",
            params![task_id.to_string(), now, attempt, now + delay_ms],
        )?;
        Ok(())
    }

    fn upsert_task(&self, task: &Task) -> CoreResult<()> {
        self.connection.execute(
            "INSERT INTO tasks
             (id, title, body, due_at, status, project_id, tags, created_at, updated_at, deleted, dirty)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                body = excluded.body,
                due_at = excluded.due_at,
                status = excluded.status,
                project_id = excluded.project_id,
                tags = excluded.tags,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                deleted = excluded.deleted,
                dirty = excluded.dirty",
            params![
                task.id.to_string(),
                task.title,
                task.body,
                task.due_at,
                status_to_db(task.status),
                task.project_id.map(|id| id.to_string()),
                serde_json::to_string(&task.tags)?,
                task.created_at,
                task.updated_at,
                bool_to_db(task.deleted),
                bool_to_db(task.dirty),
            ],
        )?;
        Ok(())
    }
}

fn collect_task_lists(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<TaskList>>,
) -> CoreResult<Vec<TaskList>> {
    let mut lists = Vec::new();
    for list in rows {
        lists.push(list?);
    }
    Ok(lists)
}

fn read_task_list(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskList> {
    let id_text: String = row.get(0)?;
    Ok(TaskList {
        id: parse_uuid(&id_text, "id")?,
        name: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
        deleted: db_to_bool(row.get(4)?),
        dirty: db_to_bool(row.get(5)?),
    })
}

fn collect_tasks(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Task>>,
) -> CoreResult<Vec<Task>> {
    let mut tasks = Vec::new();
    for task in rows {
        tasks.push(task?);
    }
    Ok(tasks)
}

fn read_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let id_text: String = row.get(0)?;
    let status_text: String = row.get(4)?;
    let project_id_text: Option<String> = row.get(5)?;
    let tags_text: String = row.get(6)?;

    let id = parse_uuid(&id_text, "id")?;
    let project_id = project_id_text
        .as_deref()
        .map(|value| parse_uuid(value, "project_id"))
        .transpose()?;
    let tags = serde_json::from_str(&tags_text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            6,
            rusqlite::types::Type::Text,
            Box::new(DbError::InvalidRowData(format!(
                "invalid tags JSON: {error}"
            ))),
        )
    })?;

    Ok(Task {
        id,
        title: row.get(1)?,
        body: row.get(2)?,
        due_at: row.get(3)?,
        status: status_from_db(&status_text)?,
        project_id,
        tags,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        deleted: db_to_bool(row.get(9)?),
        dirty: db_to_bool(row.get(10)?),
    })
}

fn parse_uuid(text: &str, column: &'static str) -> rusqlite::Result<Uuid> {
    Uuid::parse_str(text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(DbError::InvalidRowData(format!(
                "invalid {column}: {error}"
            ))),
        )
    })
}

fn status_to_db(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "open",
        TaskStatus::Done => "done",
    }
}

fn status_from_db(text: &str) -> rusqlite::Result<TaskStatus> {
    match text {
        "open" | "inbox" | "in_progress" => Ok(TaskStatus::Open),
        "done" => Ok(TaskStatus::Done),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            Box::new(DbError::InvalidTaskStatus(other.to_owned())),
        )),
    }
}

fn bool_to_db(value: bool) -> i64 {
    i64::from(value)
}

fn db_to_bool(value: i64) -> bool {
    value != 0
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch")
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;

    fn db() -> LocalDatabase {
        LocalDatabase::open_in_memory().unwrap()
    }

    fn temporary_database_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "taskmanager-core-{name}-{}-{}.sqlite3",
            std::process::id(),
            Uuid::new_v4()
        ))
    }

    fn create_named(
        database: &LocalDatabase,
        title: &str,
        body: &str,
        due_at: Option<i64>,
    ) -> Task {
        database
            .create_task(title.to_owned(), body.to_owned(), due_at)
            .unwrap()
    }

    #[test]
    fn opening_database_initializes_required_tables() {
        let database = db();
        for table in ["tasks", "tasks_fts", "sync_cursor", "sync_queue"] {
            let count: i64 = database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table {table}");
        }
    }

    #[test]
    fn schema_initialization_is_idempotent() {
        let database = db();
        database.initialize_schema().unwrap();
        database.initialize_schema().unwrap();
    }

    #[test]
    fn sync_queue_migrates_old_non_unique_schema() {
        let path = temporary_database_path("sync_queue_migrates_old_non_unique_schema");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE sync_queue (
                        task_id     TEXT NOT NULL,
                        queued_at   INTEGER NOT NULL,
                        attempt     INTEGER NOT NULL DEFAULT 0,
                        next_retry  INTEGER NOT NULL DEFAULT 0
                    );",
                )
                .unwrap();
        }

        let database = LocalDatabase::open(&path).unwrap();
        let task_id = Uuid::new_v4();
        database.queue_retry(task_id, 1_000).unwrap();
        database.queue_retry(task_id, 2_000).unwrap();

        let entries = database.retry_queue_entries().unwrap();
        assert_eq!(entries, vec![(task_id, 2, 4_000)]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn create_and_get_task_round_trip() {
        let database = db();
        let task = create_named(&database, "title", "body", Some(10));
        let loaded = database.get_task(task.id).unwrap();

        assert_eq!(loaded, task);
        assert_eq!(loaded.status, TaskStatus::Open);
        assert!(loaded.dirty);
        assert!(!loaded.deleted);
    }

    #[test]
    fn update_task_changes_only_patched_fields() {
        let database = db();
        let task = create_named(&database, "old", "body", Some(10));
        let project_id = Uuid::new_v4();

        let updated = database
            .update_task(
                task.id,
                TaskPatch {
                    title: Some("new".to_owned()),
                    status: Some(TaskStatus::Done),
                    project_id: Some(Some(project_id)),
                    tags: Some(vec!["tag".to_owned()]),
                    ..TaskPatch::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "new");
        assert_eq!(updated.body, task.body);
        assert_eq!(updated.due_at, task.due_at);
        assert_eq!(updated.status, TaskStatus::Done);
        assert_eq!(updated.project_id, Some(project_id));
        assert_eq!(updated.tags, vec!["tag"]);
        assert!(updated.updated_at > task.updated_at);
        assert!(updated.dirty);
    }

    #[test]
    fn delete_task_creates_tombstone() {
        let database = db();
        let task = create_named(&database, "delete", "body", None);

        database.delete_task(task.id).unwrap();
        let deleted = database.get_task(task.id).unwrap();

        assert!(deleted.deleted);
        assert!(deleted.dirty);
        assert!(deleted.updated_at > task.updated_at);
    }

    #[test]
    fn list_tasks_excludes_deleted_by_default_and_can_include_them() {
        let database = db();
        let active = create_named(&database, "active", "body", None);
        let deleted = create_named(&database, "deleted", "body", None);
        database.delete_task(deleted.id).unwrap();

        let default_list = database
            .list_tasks(TaskFilter::default(), TaskSort::CreatedAtAsc)
            .unwrap();
        assert_eq!(
            default_list.iter().map(|task| task.id).collect::<Vec<_>>(),
            vec![active.id]
        );

        let with_deleted = database
            .list_tasks(
                TaskFilter {
                    include_deleted: true,
                    ..TaskFilter::default()
                },
                TaskSort::CreatedAtAsc,
            )
            .unwrap();
        assert_eq!(with_deleted.len(), 2);
    }

    #[test]
    fn list_tasks_filters_by_status_project_tags_and_due_range() {
        let database = db();
        let project_id = Uuid::new_v4();
        let matching = create_named(&database, "matching", "body", Some(50));
        database
            .update_task(
                matching.id,
                TaskPatch {
                    status: Some(TaskStatus::Open),
                    project_id: Some(Some(project_id)),
                    tags: Some(vec!["work".to_owned(), "urgent".to_owned()]),
                    ..TaskPatch::default()
                },
            )
            .unwrap();
        let wrong_tag = create_named(&database, "wrong tag", "body", Some(50));
        database
            .update_task(
                wrong_tag.id,
                TaskPatch {
                    status: Some(TaskStatus::Open),
                    project_id: Some(Some(project_id)),
                    tags: Some(vec!["work".to_owned()]),
                    ..TaskPatch::default()
                },
            )
            .unwrap();
        create_named(&database, "wrong due", "body", Some(500));
        create_named(&database, "wrong status", "body", Some(50));

        let tasks = database
            .list_tasks(
                TaskFilter {
                    status: Some(TaskStatus::Open),
                    project_id: Some(project_id),
                    tags: vec!["work".to_owned(), "urgent".to_owned()],
                    due_after: Some(40),
                    due_before: Some(60),
                    include_deleted: false,
                },
                TaskSort::UpdatedAtDesc,
            )
            .unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, matching.id);
    }

    #[test]
    fn list_tasks_sort_modes_are_deterministic() {
        let database = db();
        let later_due = create_named(&database, "b", "body", Some(20));
        let earlier_due = create_named(&database, "a", "body", Some(10));

        let due_asc = database
            .list_tasks(TaskFilter::default(), TaskSort::DueAtAsc)
            .unwrap();
        assert_eq!(
            due_asc.iter().map(|task| task.id).collect::<Vec<_>>(),
            vec![earlier_due.id, later_due.id]
        );

        let due_desc = database
            .list_tasks(TaskFilter::default(), TaskSort::DueAtDesc)
            .unwrap();
        assert_eq!(
            due_desc.iter().map(|task| task.id).collect::<Vec<_>>(),
            vec![later_due.id, earlier_due.id]
        );
    }

    #[test]
    fn search_tasks_finds_title_and_body_matches_and_updates_after_changes() {
        let database = db();
        let title_match = create_named(&database, "alpha project", "ordinary", None);
        let body_match = create_named(&database, "ordinary", "contains beta", None);

        assert_eq!(
            database.search_tasks("alpha".to_owned()).unwrap()[0].id,
            title_match.id
        );
        assert_eq!(
            database.search_tasks("beta".to_owned()).unwrap()[0].id,
            body_match.id
        );

        database
            .update_task(
                body_match.id,
                TaskPatch {
                    body: Some("contains gamma".to_owned()),
                    ..TaskPatch::default()
                },
            )
            .unwrap();
        assert!(database.search_tasks("beta".to_owned()).unwrap().is_empty());
        assert_eq!(
            database.search_tasks("gamma".to_owned()).unwrap()[0].id,
            body_match.id
        );

        database
            .connection
            .execute(
                "INSERT INTO tasks_fts(tasks_fts) VALUES('integrity-check')",
                [],
            )
            .unwrap();

        database.delete_task(title_match.id).unwrap();
        assert!(database
            .search_tasks("alpha".to_owned())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn tags_are_stored_as_json_arrays() {
        let database = db();
        let task = create_named(&database, "tags", "body", None);
        database
            .update_task(
                task.id,
                TaskPatch {
                    tags: Some(vec!["work".to_owned(), "urgent".to_owned()]),
                    ..TaskPatch::default()
                },
            )
            .unwrap();

        let tags_json: String = database
            .connection
            .query_row(
                "SELECT tags FROM tasks WHERE id = ?1",
                params![task.id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tags_json, "[\"work\",\"urgent\"]");
        assert_eq!(
            database.get_task(task.id).unwrap().tags,
            vec!["work", "urgent"]
        );
    }

    #[test]
    fn invalid_status_row_returns_clear_error() {
        let database = db();
        database.connection.execute(
            "INSERT INTO tasks (id, title, body, status, tags, created_at, updated_at) VALUES (?1, 't', 'b', 'bad', '[]', 1, 1)",
            params![Uuid::new_v4().to_string()],
        ).unwrap();

        let error = database
            .list_tasks(
                TaskFilter {
                    include_deleted: true,
                    ..TaskFilter::default()
                },
                TaskSort::UpdatedAtDesc,
            )
            .unwrap_err();
        assert!(matches!(error, CoreError::Database(DbError::Sqlite(_))));
    }

    #[test]
    fn invalid_uuid_row_returns_clear_error() {
        let database = db();
        database.connection.execute(
            "INSERT INTO tasks (id, title, body, status, tags, created_at, updated_at) VALUES ('not-a-uuid', 't', 'b', 'open', '[]', 1, 1)",
            [],
        ).unwrap();

        let error = database
            .list_tasks(
                TaskFilter {
                    include_deleted: true,
                    ..TaskFilter::default()
                },
                TaskSort::UpdatedAtDesc,
            )
            .unwrap_err();
        assert!(matches!(error, CoreError::Database(DbError::Sqlite(_))));
    }

    #[test]
    fn invalid_tags_json_row_returns_clear_error() {
        let database = db();
        database.connection.execute(
            "INSERT INTO tasks (id, title, body, status, tags, created_at, updated_at) VALUES (?1, 't', 'b', 'open', 'not-json', 1, 1)",
            params![Uuid::new_v4().to_string()],
        ).unwrap();

        let error = database
            .list_tasks(
                TaskFilter {
                    include_deleted: true,
                    ..TaskFilter::default()
                },
                TaskSort::UpdatedAtDesc,
            )
            .unwrap_err();
        assert!(matches!(error, CoreError::Database(DbError::Sqlite(_))));
    }
}
