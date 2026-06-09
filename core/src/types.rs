use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub due_at: Option<i64>,
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
    Inbox,
    InProgress,
    Done,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub body: Option<String>,
    pub due_at: Option<Option<i64>>,
    pub status: Option<TaskStatus>,
    pub project_id: Option<Option<Uuid>>,
    pub tags: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub project_id: Option<Uuid>,
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
            status: TaskStatus::InProgress,
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
            serde_json::to_string(&TaskStatus::Inbox).unwrap(),
            "\"inbox\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::InProgress).unwrap(),
            "\"in_progress\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Done).unwrap(),
            "\"done\""
        );

        assert_eq!(
            serde_json::from_str::<TaskStatus>("\"inbox\"").unwrap(),
            TaskStatus::Inbox
        );
        assert_eq!(
            serde_json::from_str::<TaskStatus>("\"in_progress\"").unwrap(),
            TaskStatus::InProgress
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
