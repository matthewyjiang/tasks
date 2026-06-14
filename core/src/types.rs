use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskList {
    pub id: Uuid,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted: bool,
    pub dirty: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub due_at: Option<i64>,
    #[serde(default)]
    pub reminder_offset_ms: Option<i64>,
    pub status: TaskStatus,
    pub project_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted: bool,
    pub dirty: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Done,
}

impl Task {
    pub fn notification_at(&self) -> Option<i64> {
        let due_at = self.due_at?;
        let offset = self.reminder_offset_ms?;
        if offset < 0 {
            return None;
        }
        due_at.checked_sub(offset)
    }

    pub fn notification_enabled(&self) -> bool {
        !self.deleted && self.status != TaskStatus::Done
    }

    pub fn schedulable_notification_at(&self, now_ms: i64) -> Option<i64> {
        if !self.notification_enabled() {
            return None;
        }
        self.notification_at().filter(|fire_at| *fire_at > now_ms)
    }

    pub fn notification_due(&self, now_ms: i64) -> bool {
        self.notification_enabled()
            && self
                .notification_at()
                .is_some_and(|fire_at| fire_at <= now_ms)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub body: Option<String>,
    pub due_at: Option<Option<i64>>,
    pub reminder_offset_ms: Option<Option<i64>>,
    pub status: Option<TaskStatus>,
    pub project_id: Option<Option<Uuid>>,
    pub tags: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub project_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub due_after: Option<i64>,
    pub due_before: Option<i64>,
    pub include_deleted: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskSort {
    #[default]
    UpdatedAtDesc,
    UpdatedAtAsc,
    DueAtAsc,
    DueAtDesc,
    CreatedAtAsc,
    CreatedAtDesc,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blob {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncStatus {
    pub dirty_count: usize,
    pub retry_queue_depth: usize,
    pub cursor: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryQueueEntry {
    pub task_id: Uuid,
    pub attempt: i64,
    pub next_retry: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedTaskRecipient {
    pub task_id: Uuid,
    pub recipient_id: Uuid,
    pub wrapped_task_key: Blob,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedTaskState {
    pub task_id: Uuid,
    pub owner_id: Option<Uuid>,
    pub task_key: Vec<u8>,
    pub recipients: Vec<SharedTaskRecipient>,
    pub accepted_at: Option<i64>,
    pub revoked_at: Option<i64>,
}

impl fmt::Debug for SharedTaskState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedTaskState")
            .field("task_id", &self.task_id)
            .field("owner_id", &self.owner_id)
            .field("task_key", &"<redacted>")
            .field("recipients", &self.recipients)
            .field("accepted_at", &self.accepted_at)
            .field("revoked_at", &self.revoked_at)
            .finish()
    }
}

impl SharedTaskState {
    pub fn active_recipients(&self) -> impl Iterator<Item = &SharedTaskRecipient> {
        self.recipients
            .iter()
            .filter(|recipient| recipient.revoked_at.is_none())
    }

    pub fn is_shared(&self) -> bool {
        self.accepted_at.is_some() || self.active_recipients().next().is_some()
    }

    pub fn revocation_notice() -> &'static str {
        "Revocation stops future shared sync by rotating the task key, but cannot erase plaintext or keys already synced to a recipient device."
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedTaskInvite {
    pub task_id: Uuid,
    pub owner_id: Uuid,
    pub recipient_id: Uuid,
    pub wrapped_task_key: Blob,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncResult {
    pub pushed: usize,
    pub pulled: usize,
    pub failed: usize,
    pub cursor: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_task() -> Task {
        Task {
            id: Uuid::parse_str("018f6f4a-c9f4-7724-91ef-2f7b38a62600").unwrap(),
            title: "Write readable core".to_owned(),
            body: "Start with stable domain types.".to_owned(),
            due_at: Some(1_717_603_200_000),
            reminder_offset_ms: Some(600_000),
            status: TaskStatus::Open,
            project_id: Some(Uuid::parse_str("018f6f4a-c9f4-7724-91ef-2f7b38a62601").unwrap()),
            tags: vec!["core".to_owned(), "rust".to_owned()],
            created_at: 1_717_600_000_000,
            updated_at: 1_717_600_001_000,
            deleted: false,
            dirty: true,
        }
    }

    #[test]
    fn task_json_round_trip_preserves_every_field() {
        let task = sample_task();

        let json = serde_json::to_string(&task).unwrap();
        let decoded: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, task);
    }

    #[test]
    fn task_status_uses_stable_snake_case_wire_values() {
        assert_eq!(
            serde_json::to_string(&TaskStatus::Open).unwrap(),
            "\"open\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Done).unwrap(),
            "\"done\""
        );

        assert_eq!(
            serde_json::from_str::<TaskStatus>("\"open\"").unwrap(),
            TaskStatus::Open
        );
        assert_eq!(
            serde_json::from_str::<TaskStatus>("\"done\"").unwrap(),
            TaskStatus::Done
        );
    }

    #[test]
    fn optional_fields_round_trip_when_none() {
        let mut task = sample_task();
        task.due_at = None;
        task.project_id = None;

        let json = serde_json::to_string(&task).unwrap();
        let decoded: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.due_at, None);
        assert_eq!(decoded.project_id, None);
        assert_eq!(decoded, task);
    }

    #[test]
    fn missing_reminder_offset_defaults_to_none_for_old_payloads() {
        let json = r#"{
            "id":"018f6f4a-c9f4-7724-91ef-2f7b38a62600",
            "title":"old",
            "body":"payload",
            "due_at":1717603200000,
            "status":"open",
            "project_id":null,
            "tags":[],
            "created_at":1,
            "updated_at":2,
            "deleted":false,
            "dirty":false
        }"#;

        let task: Task = serde_json::from_str(json).unwrap();

        assert_eq!(task.reminder_offset_ms, None);
    }

    #[test]
    fn notification_helpers_apply_core_reminder_semantics() {
        let mut task = sample_task();
        task.due_at = Some(10_000);
        task.reminder_offset_ms = Some(1_000);
        assert_eq!(task.notification_at(), Some(9_000));
        assert_eq!(task.schedulable_notification_at(8_999), Some(9_000));
        assert_eq!(task.schedulable_notification_at(9_000), None);
        assert!(task.notification_due(9_000));

        task.status = TaskStatus::Done;
        assert_eq!(task.schedulable_notification_at(8_000), None);
        assert!(!task.notification_due(10_000));

        task.status = TaskStatus::Open;
        task.reminder_offset_ms = Some(-1);
        assert_eq!(task.notification_at(), None);
    }

    #[test]
    fn tag_lists_round_trip_when_empty_and_populated() {
        let mut task = sample_task();
        task.tags = Vec::new();
        let decoded_empty: Task =
            serde_json::from_str(&serde_json::to_string(&task).unwrap()).unwrap();
        assert_eq!(decoded_empty.tags, Vec::<String>::new());

        task.tags = vec!["work".to_owned(), "urgent".to_owned(), "offline".to_owned()];
        let decoded_populated: Task =
            serde_json::from_str(&serde_json::to_string(&task).unwrap()).unwrap();
        assert_eq!(decoded_populated.tags, task.tags);
    }

    #[test]
    fn blob_preserves_ciphertext_and_nonce() {
        let blob = Blob {
            ciphertext: vec![1, 2, 3, 4, 5],
            nonce: [9; 12],
        };

        let cloned = blob.clone();
        let debug_text = format!("{blob:?}");

        assert_eq!(cloned, blob);
        assert!(debug_text.contains("ciphertext"));
        assert!(debug_text.contains("nonce"));
    }

    #[test]
    fn defaults_are_spec_compliant() {
        assert_eq!(
            TaskPatch::default(),
            TaskPatch {
                title: None,
                body: None,
                due_at: None,
                reminder_offset_ms: None,
                status: None,
                project_id: None,
                tags: None,
            }
        );

        assert_eq!(
            TaskFilter::default(),
            TaskFilter {
                status: None,
                project_id: None,
                tags: Vec::new(),
                due_after: None,
                due_before: None,
                include_deleted: false,
            }
        );

        assert_eq!(TaskSort::default(), TaskSort::UpdatedAtDesc);
        assert_eq!(
            SyncResult::default(),
            SyncResult {
                pushed: 0,
                pulled: 0,
                failed: 0,
                cursor: None,
            }
        );
    }
}
