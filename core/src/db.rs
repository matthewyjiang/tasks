use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::{CoreResult, DbError};
use crate::settings::{VaultSettings, VAULT_SETTINGS_ID};
use crate::types::{
    Blob, SharedTaskInvite, SharedTaskRecipient, SharedTaskState, Task, TaskFilter, TaskList,
    TaskPatch, TaskSort, TaskStatus,
};

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
        self.create_task_with_options(title, body, due_at, None, Vec::new())
    }

    pub fn create_task_with_options(
        &self,
        title: String,
        body: String,
        due_at: Option<i64>,
        project_id: Option<Uuid>,
        tags: Vec<String>,
    ) -> CoreResult<Task> {
        if let Some(list_id) = project_id {
            let list = self.get_list(list_id)?;
            if list.deleted {
                return Err(DbError::InvalidRowData(format!("list is deleted: {list_id}")).into());
            }
        }

        let reminder_offset_ms = self.default_reminder_offset_ms(due_at)?;
        let now = now_ms();
        let task = Task {
            id: Uuid::new_v4(),
            title,
            body,
            due_at,
            reminder_offset_ms,
            status: TaskStatus::Open,
            project_id,
            tags,
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
                "SELECT id, title, body, due_at, reminder_offset_ms, status, project_id, tags, created_at, updated_at, deleted, dirty FROM tasks WHERE id = ?1",
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
        let had_due_at = task.due_at.is_some();
        let has_reminder_patch = patch.reminder_offset_ms.is_some();
        if let Some(due_at) = patch.due_at {
            task.due_at = due_at;
            if due_at.is_some()
                && !had_due_at
                && !has_reminder_patch
                && task.reminder_offset_ms.is_none()
            {
                task.reminder_offset_ms = self.default_reminder_offset_ms(due_at)?;
            }
        }
        if let Some(reminder_offset_ms) = patch.reminder_offset_ms {
            if reminder_offset_ms.is_some_and(|offset| offset < 0) {
                return Err(DbError::InvalidRowData(
                    "reminder offset cannot be negative".to_owned(),
                )
                .into());
            }
            task.reminder_offset_ms = reminder_offset_ms;
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
            "SELECT id, title, body, due_at, reminder_offset_ms, status, project_id, tags, created_at, updated_at, deleted, dirty FROM tasks WHERE id != ?",
        );
        let mut values = vec![Value::Text(Uuid::nil().to_string())];

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
                "SELECT id, title, body, due_at, reminder_offset_ms, status, project_id, tags, created_at, updated_at, deleted, dirty FROM tasks WHERE id = ?1 AND title = ?2",
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

    fn default_reminder_offset_ms(&self, due_at: Option<i64>) -> CoreResult<Option<i64>> {
        if due_at.is_none() {
            return Ok(None);
        }
        let minutes = self.vault_settings()?.default_reminder_minutes;
        if minutes <= 0 {
            return Ok(None);
        }
        Ok(Some(i64::from(minutes) * 60_000))
    }

    pub fn share_task(
        &self,
        task_id: Uuid,
        recipient_id: Uuid,
        task_key: Vec<u8>,
        wrapped_task_key: Blob,
    ) -> CoreResult<SharedTaskRecipient> {
        self.get_task(task_id)?;
        let now = now_ms();
        self.connection.execute(
            "INSERT INTO shared_task_state (task_id, owner_id, task_key, accepted_at, revoked_at)
             VALUES (?1, NULL, ?2, NULL, NULL)
             ON CONFLICT(task_id) DO UPDATE SET revoked_at = NULL",
            params![task_id.to_string(), task_key],
        )?;
        self.connection.execute(
            "INSERT INTO shared_task_recipients (task_id, recipient_id, wrapped_task_key, nonce, created_at, revoked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL)
             ON CONFLICT(task_id, recipient_id) DO UPDATE SET wrapped_task_key = excluded.wrapped_task_key, nonce = excluded.nonce, created_at = excluded.created_at, revoked_at = NULL",
            params![task_id.to_string(), recipient_id.to_string(), wrapped_task_key.ciphertext, wrapped_task_key.nonce.to_vec(), now],
        )?;
        self.mark_task_dirty(task_id)?;
        Ok(SharedTaskRecipient {
            task_id,
            recipient_id,
            wrapped_task_key,
            created_at: now,
            revoked_at: None,
        })
    }

    pub fn accept_shared_task(
        &self,
        invite: SharedTaskInvite,
        task_key: Vec<u8>,
    ) -> CoreResult<SharedTaskState> {
        let now = now_ms();
        self.connection.execute(
            "INSERT INTO shared_task_state (task_id, owner_id, task_key, accepted_at, revoked_at)
             VALUES (?1, ?2, ?3, ?4, NULL)
             ON CONFLICT(task_id) DO UPDATE SET owner_id = excluded.owner_id, task_key = excluded.task_key, accepted_at = excluded.accepted_at, revoked_at = NULL",
            params![invite.task_id.to_string(), invite.owner_id.to_string(), task_key, now],
        )?;
        self.shared_task_state(invite.task_id)
    }

    pub fn revoke_shared_task_recipient(
        &self,
        task_id: Uuid,
        recipient_id: Uuid,
        rotated_task_key: Vec<u8>,
        rewrapped_recipients: Vec<SharedTaskRecipient>,
    ) -> CoreResult<SharedTaskState> {
        self.connection.execute(
            "UPDATE shared_task_state SET task_key = ?2, revoked_at = NULL WHERE task_id = ?1",
            params![task_id.to_string(), rotated_task_key],
        )?;
        let now = now_ms();
        self.connection.execute(
            "UPDATE shared_task_recipients SET revoked_at = ?3 WHERE task_id = ?1 AND recipient_id = ?2 AND revoked_at IS NULL",
            params![task_id.to_string(), recipient_id.to_string(), now],
        )?;
        for recipient in rewrapped_recipients {
            self.connection.execute(
                "UPDATE shared_task_recipients SET wrapped_task_key = ?3, nonce = ?4 WHERE task_id = ?1 AND recipient_id = ?2 AND revoked_at IS NULL",
                params![task_id.to_string(), recipient.recipient_id.to_string(), recipient.wrapped_task_key.ciphertext, recipient.wrapped_task_key.nonce.to_vec()],
            )?;
        }
        self.mark_task_dirty(task_id)?;
        self.shared_task_state(task_id)
    }

    pub fn mark_task_dirty(&self, task_id: Uuid) -> CoreResult<()> {
        let task = self.get_task(task_id)?;
        let updated_at = now_ms().max(task.updated_at + 1);
        self.connection.execute(
            "UPDATE tasks SET updated_at = ?2, dirty = 1 WHERE id = ?1",
            params![task_id.to_string(), updated_at],
        )?;
        Ok(())
    }

    pub fn task_encryption_key(
        &self,
        task_id: Uuid,
        account_data_key: &[u8],
    ) -> CoreResult<Vec<u8>> {
        match self.shared_task_state(task_id) {
            Ok(state) => Ok(state.task_key),
            Err(_) => Ok(account_data_key.to_vec()),
        }
    }

    pub fn shared_task_state(&self, task_id: Uuid) -> CoreResult<SharedTaskState> {
        let (owner_id, task_key, accepted_at, revoked_at): (Option<String>, Vec<u8>, Option<i64>, Option<i64>) = self.connection.query_row(
            "SELECT owner_id, task_key, accepted_at, revoked_at FROM shared_task_state WHERE task_id = ?1",
            params![task_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).optional()?.ok_or_else(|| DbError::TaskNotFound(task_id))?;
        let mut statement = self.connection.prepare(
            "SELECT recipient_id, wrapped_task_key, nonce, created_at, revoked_at FROM shared_task_recipients WHERE task_id = ?1 ORDER BY created_at ASC, recipient_id ASC",
        )?;
        let recipients = statement
            .query_map(params![task_id.to_string()], |row| {
                let recipient_id: String = row.get(0)?;
                let nonce: Vec<u8> = row.get(2)?;
                let nonce: [u8; 12] = nonce
                    .try_into()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(SharedTaskRecipient {
                    task_id,
                    recipient_id: parse_uuid(&recipient_id, "recipient_id")?,
                    wrapped_task_key: Blob {
                        ciphertext: row.get(1)?,
                        nonce,
                    },
                    created_at: row.get(3)?,
                    revoked_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SharedTaskState {
            task_id,
            owner_id: owner_id
                .map(|id| Uuid::parse_str(&id))
                .transpose()
                .map_err(|error| DbError::InvalidRowData(error.to_string()))?,
            task_key,
            recipients,
            accepted_at,
            revoked_at,
        })
    }

    pub fn search_tasks(&self, query: String) -> CoreResult<Vec<Task>> {
        let search = SearchQuery::parse(&query);
        if search.is_empty() {
            return Ok(Vec::new());
        }

        let mut tasks = if let Some(fts_query) = search.fts_query() {
            let mut statement = self.connection.prepare(
                "SELECT t.id, t.title, t.body, t.due_at, t.reminder_offset_ms, t.status, t.project_id, t.tags, t.created_at, t.updated_at, t.deleted, t.dirty
                 FROM tasks_fts f JOIN tasks t ON t.rowid = f.rowid
                 WHERE tasks_fts MATCH ?1 AND t.deleted = 0 AND t.id != ?2
                 ORDER BY rank, t.updated_at DESC, t.id ASC",
            )?;
            let rows =
                statement.query_map(params![fts_query, Uuid::nil().to_string()], read_task)?;
            let tasks = collect_tasks(rows)?;
            if search.phrase {
                tasks
                    .into_iter()
                    .filter(|task| search.matches_task(task))
                    .collect()
            } else {
                tasks
            }
        } else {
            Vec::new()
        };

        let mut statement = self.connection.prepare(
            "SELECT id, title, body, due_at, reminder_offset_ms, status, project_id, tags, created_at, updated_at, deleted, dirty
             FROM tasks
             WHERE deleted = 0
               AND id != ?1
               AND EXISTS (SELECT 1 FROM json_each(tasks.tags) WHERE lower(value) LIKE ?2 ESCAPE '\\')
             ORDER BY updated_at DESC, id ASC",
        )?;
        let tag_pattern = search.tag_like_pattern();
        for task in collect_tasks(
            statement.query_map(params![Uuid::nil().to_string(), tag_pattern], read_task)?,
        )? {
            if !tasks.iter().any(|existing| existing.id == task.id) {
                tasks.push(task);
            }
        }

        if search.requires_literal_fallback() {
            let mut statement = self.connection.prepare(
                "SELECT id, title, body, due_at, reminder_offset_ms, status, project_id, tags, created_at, updated_at, deleted, dirty
                 FROM tasks
                 WHERE deleted = 0 AND id != ?1
                 ORDER BY updated_at DESC, id ASC",
            )?;
            for task in
                collect_tasks(statement.query_map(params![Uuid::nil().to_string()], read_task)?)?
            {
                if search.matches_task(&task)
                    && !tasks.iter().any(|existing| existing.id == task.id)
                {
                    tasks.push(task);
                }
            }
        }

        Ok(tasks)
    }

    fn initialize_schema(&self) -> CoreResult<()> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id          TEXT PRIMARY KEY,
                title       TEXT NOT NULL,
                body        TEXT NOT NULL DEFAULT '',
                due_at      INTEGER,
                reminder_offset_ms INTEGER,
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

            CREATE TABLE IF NOT EXISTS shared_task_state (
                task_id     TEXT PRIMARY KEY,
                owner_id    TEXT,
                task_key    BLOB NOT NULL,
                accepted_at INTEGER,
                revoked_at  INTEGER
            );

            CREATE TABLE IF NOT EXISTS shared_task_recipients (
                task_id          TEXT NOT NULL,
                recipient_id     TEXT NOT NULL,
                wrapped_task_key BLOB NOT NULL,
                nonce            BLOB NOT NULL,
                created_at       INTEGER NOT NULL,
                revoked_at       INTEGER,
                PRIMARY KEY (task_id, recipient_id)
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
        self.add_column_if_missing("tasks", "reminder_offset_ms", "INTEGER")?;
        Ok(())
    }

    fn add_column_if_missing(&self, table: &str, column: &str, definition: &str) -> CoreResult<()> {
        let mut statement = self
            .connection
            .prepare(&format!("PRAGMA table_info({table})"))?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
        for existing in columns {
            if existing? == column {
                return Ok(());
            }
        }
        self.connection.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
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
            "SELECT id, title, body, due_at, reminder_offset_ms, status, project_id, tags, created_at, updated_at, deleted, dirty FROM tasks WHERE dirty = 1 ORDER BY updated_at ASC, id ASC",
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

    pub fn retry_queue_entries(&self) -> CoreResult<Vec<(Uuid, i64, i64)>> {
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

    pub fn queue_retry(&self, task_id: Uuid, now: i64) -> CoreResult<()> {
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

    pub fn clear_retry(&self, task_id: Uuid) -> CoreResult<()> {
        self.connection.execute(
            "DELETE FROM sync_queue WHERE task_id = ?1",
            params![task_id.to_string()],
        )?;
        Ok(())
    }

    fn upsert_task(&self, task: &Task) -> CoreResult<()> {
        if task.reminder_offset_ms.is_some_and(|offset| offset < 0) {
            return Err(
                DbError::InvalidRowData("reminder offset cannot be negative".to_owned()).into(),
            );
        }
        self.connection.execute(
            "INSERT INTO tasks
             (id, title, body, due_at, reminder_offset_ms, status, project_id, tags, created_at, updated_at, deleted, dirty)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                body = excluded.body,
                due_at = excluded.due_at,
                reminder_offset_ms = excluded.reminder_offset_ms,
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
                task.reminder_offset_ms,
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
    let status_text: String = row.get(5)?;
    let project_id_text: Option<String> = row.get(6)?;
    let tags_text: String = row.get(7)?;

    let id = parse_uuid(&id_text, "id")?;
    let project_id = project_id_text
        .as_deref()
        .map(|value| parse_uuid(value, "project_id"))
        .transpose()?;
    let tags = serde_json::from_str(&tags_text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            7,
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
        reminder_offset_ms: row.get(4)?,
        status: status_from_db(&status_text)?,
        project_id,
        tags,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        deleted: db_to_bool(row.get(10)?),
        dirty: db_to_bool(row.get(11)?),
    })
}

struct SearchQuery {
    text: String,
    phrase: bool,
}

impl SearchQuery {
    fn parse(query: &str) -> Self {
        let query = query.trim();
        if query.len() >= 2 && query.starts_with('"') && query.ends_with('"') {
            Self {
                text: query[1..query.len() - 1].replace("\"\"", "\""),
                phrase: true,
            }
        } else {
            Self {
                text: query.to_owned(),
                phrase: false,
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    fn terms(&self) -> Vec<String> {
        if self.phrase {
            vec![self.text.to_lowercase()]
        } else {
            self.text
                .split_whitespace()
                .map(str::to_lowercase)
                .collect()
        }
    }

    fn fts_query(&self) -> Option<String> {
        if !self
            .text
            .chars()
            .all(|character| character.is_alphanumeric() || character.is_whitespace())
        {
            return None;
        }
        if self.phrase {
            Some(format!("\"{}\"", self.text.replace('"', "\"\"")))
        } else {
            let terms = self.terms();
            (!terms.is_empty()).then(|| terms.join(" "))
        }
    }

    fn tag_like_pattern(&self) -> String {
        let value = if self.phrase {
            self.text.to_lowercase()
        } else {
            self.terms().join("%")
        };
        format!("%{}%", escape_like_query(&value))
    }

    fn requires_literal_fallback(&self) -> bool {
        self.fts_query().is_none()
    }

    fn matches_task(&self, task: &Task) -> bool {
        let title = task.title.to_lowercase();
        let body = task.body.to_lowercase();
        let tags = task
            .tags
            .iter()
            .map(|tag| tag.to_lowercase())
            .collect::<Vec<_>>();
        if self.phrase {
            let phrase = self.text.to_lowercase();
            return title.contains(&phrase)
                || body.contains(&phrase)
                || tags.iter().any(|tag| tag.contains(&phrase));
        }
        self.terms().iter().all(|term| {
            title.contains(term) || body.contains(term) || tags.iter().any(|tag| tag.contains(term))
        })
    }
}

fn escape_like_query(query: &str) -> String {
    query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
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
    fn task_schema_migrates_missing_reminder_column() {
        let path = temporary_database_path("task_schema_migrates_missing_reminder_column");
        let task_id = Uuid::new_v4();
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute(
                    "CREATE TABLE tasks (
                        id TEXT PRIMARY KEY,
                        title TEXT NOT NULL,
                        body TEXT NOT NULL DEFAULT '',
                        due_at INTEGER,
                        status TEXT NOT NULL DEFAULT 'open',
                        project_id TEXT,
                        tags TEXT NOT NULL DEFAULT '[]',
                        created_at INTEGER NOT NULL,
                        updated_at INTEGER NOT NULL,
                        deleted INTEGER NOT NULL DEFAULT 0,
                        dirty INTEGER NOT NULL DEFAULT 1
                    )",
                    [],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO tasks (id, title, body, status, tags, created_at, updated_at) VALUES (?1, 'old', '', 'open', '[]', 1, 1)",
                    params![task_id.to_string()],
                )
                .unwrap();
        }

        let database = LocalDatabase::open(&path).unwrap();
        let task = database.get_task(task_id).unwrap();

        assert_eq!(task.reminder_offset_ms, None);
        let _ = std::fs::remove_file(path);
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
    fn create_task_applies_default_reminder_for_due_tasks() {
        let database = db();
        let due_task = create_named(&database, "due", "body", Some(10));
        let undated_task = create_named(&database, "undated", "body", None);

        assert_eq!(due_task.reminder_offset_ms, Some(30 * 60_000));
        assert_eq!(undated_task.reminder_offset_ms, None);
    }

    #[test]
    fn create_task_skips_default_reminder_when_disabled() {
        let database = db();
        let settings = VaultSettings {
            default_reminder_minutes: 0,
            ..VaultSettings::default()
        };
        database.update_vault_settings(&settings).unwrap();

        let task = create_named(&database, "due", "body", Some(10));

        assert_eq!(task.reminder_offset_ms, None);
    }

    #[test]
    fn update_task_applies_default_reminder_when_due_date_is_added() {
        let database = db();
        let task = create_named(&database, "due", "body", None);

        let updated = database
            .update_task(
                task.id,
                TaskPatch {
                    due_at: Some(Some(10)),
                    ..TaskPatch::default()
                },
            )
            .unwrap();

        assert_eq!(updated.reminder_offset_ms, Some(30 * 60_000));
    }

    #[test]
    fn update_task_respects_explicit_reminder_patch_when_due_date_is_added() {
        let database = db();
        let task = create_named(&database, "due", "body", None);

        let updated = database
            .update_task(
                task.id,
                TaskPatch {
                    due_at: Some(Some(10)),
                    reminder_offset_ms: Some(None),
                    ..TaskPatch::default()
                },
            )
            .unwrap();

        assert_eq!(updated.reminder_offset_ms, None);
    }

    #[test]
    fn update_task_keeps_disabled_reminder_when_due_date_changes() {
        let database = db();
        let task = create_named(&database, "due", "body", Some(10));
        let disabled = database
            .update_task(
                task.id,
                TaskPatch {
                    reminder_offset_ms: Some(None),
                    ..TaskPatch::default()
                },
            )
            .unwrap();
        assert_eq!(disabled.reminder_offset_ms, None);

        let updated = database
            .update_task(
                task.id,
                TaskPatch {
                    due_at: Some(Some(20)),
                    ..TaskPatch::default()
                },
            )
            .unwrap();

        assert_eq!(updated.reminder_offset_ms, None);
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
                    reminder_offset_ms: Some(Some(600_000)),
                    project_id: Some(Some(project_id)),
                    tags: Some(vec!["tag".to_owned()]),
                    ..TaskPatch::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "new");
        assert_eq!(updated.body, task.body);
        assert_eq!(updated.due_at, task.due_at);
        assert_eq!(updated.reminder_offset_ms, Some(600_000));
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
    fn list_tasks_excludes_reserved_vault_settings_task() {
        let database = db();
        let task = create_named(&database, "visible", "body", None);
        database
            .update_vault_settings(&VaultSettings::default())
            .unwrap();

        let tasks = database
            .list_tasks(
                TaskFilter {
                    include_deleted: true,
                    ..TaskFilter::default()
                },
                TaskSort::CreatedAtAsc,
            )
            .unwrap();

        assert_eq!(
            tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
            vec![task.id]
        );
        assert!(database.vault_settings().is_ok());
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
        let tag_match = create_named(&database, "ordinary", "ordinary", None);
        let punctuation_match = create_named(&database, "foo-bar", "ordinary", None);
        let phrase_match = create_named(&database, "foo bar", "ordinary", None);
        create_named(&database, "foo", "contains bar", None);
        database
            .update_task(
                tag_match.id,
                TaskPatch {
                    tags: Some(vec!["urgent".to_owned()]),
                    ..TaskPatch::default()
                },
            )
            .unwrap();

        assert_eq!(
            database.search_tasks("alpha".to_owned()).unwrap()[0].id,
            title_match.id
        );
        assert_eq!(
            database.search_tasks("beta".to_owned()).unwrap()[0].id,
            body_match.id
        );
        assert_eq!(
            database.search_tasks("urgent".to_owned()).unwrap()[0].id,
            tag_match.id
        );
        assert_eq!(
            database.search_tasks("foo-bar".to_owned()).unwrap()[0].id,
            punctuation_match.id
        );
        let phrase_results = database.search_tasks("\"foo bar\"".to_owned()).unwrap();
        assert_eq!(phrase_results.len(), 1);
        assert_eq!(phrase_results[0].id, phrase_match.id);

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
