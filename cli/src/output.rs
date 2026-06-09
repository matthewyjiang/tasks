use serde::Serialize;
use taskmanager_core::{
    Blob, PlaintextSettings, PlaintextSettingsSyncPayload, RetryQueueEntry, SyncStatus, Task,
    TaskStatus,
};
use uuid::Uuid;

use crate::error::CliResult;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    Table,
    Json,
    Jsonl,
}

#[derive(Debug, Serialize)]
pub struct CommandResult<T: Serialize> {
    pub result: T,
}

impl<T: Serialize> CommandResult<T> {
    pub fn new(result: T) -> Self {
        Self { result }
    }
}

#[derive(Debug, Serialize)]
pub struct VersionOutput {
    pub name: &'static str,
    pub version: &'static str,
}

impl VersionOutput {
    pub fn to_table(&self) -> String {
        format!("{} {}", self.name, self.version)
    }
}

#[derive(Debug, Serialize)]
pub struct DeleteOutput {
    pub id: Uuid,
    pub deleted: bool,
}

#[derive(Debug, Serialize)]
pub struct PublicKeyOutput {
    pub public_key: String,
}

#[derive(Debug, Serialize)]
pub struct AuthOutput {
    pub stored: bool,
}

#[derive(Debug, Serialize)]
pub struct LogoutOutput {
    pub logged_out: bool,
}

#[derive(Debug, Serialize)]
pub struct WrappedKeyOutput {
    pub ciphertext: String,
    pub nonce: String,
}

#[derive(Debug, Serialize)]
pub struct UnwrappedKeyOutput {
    pub stored: bool,
}

#[derive(Debug, Serialize)]
pub struct SecretBytesOutput {
    pub hex: String,
}

#[derive(Debug, Serialize)]
pub struct CryptoVerifyOutput {
    pub data_key_present: bool,
    pub device_private_key_present: bool,
    pub encrypt_decrypt_ok: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncResultOutput {
    pub pushed: usize,
    pub pulled: usize,
    pub failed: usize,
    pub cursor: Option<i64>,
}

pub trait TableOutput {
    fn to_table(&self) -> String;
}

impl TableOutput for VersionOutput {
    fn to_table(&self) -> String {
        self.to_table()
    }
}

impl TableOutput for Task {
    fn to_table(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}",
            self.id,
            status_label(self.status),
            self.due_at
                .map(|due_at| due_at.to_string())
                .unwrap_or_else(|| "-".to_string()),
            self.title
        )
    }
}

impl TableOutput for Vec<Task> {
    fn to_table(&self) -> String {
        if self.is_empty() {
            return "No tasks".to_string();
        }

        let mut lines = vec!["ID\tSTATUS\tDUE\tTITLE".to_string()];
        lines.extend(self.iter().map(TableOutput::to_table));
        lines.join("\n")
    }
}

impl TableOutput for DeleteOutput {
    fn to_table(&self) -> String {
        format!("Deleted {}", self.id)
    }
}

impl TableOutput for PublicKeyOutput {
    fn to_table(&self) -> String {
        self.public_key.clone()
    }
}

impl TableOutput for AuthOutput {
    fn to_table(&self) -> String {
        "Auth tokens stored".to_string()
    }
}

impl TableOutput for LogoutOutput {
    fn to_table(&self) -> String {
        "Logged out".to_string()
    }
}

impl TableOutput for WrappedKeyOutput {
    fn to_table(&self) -> String {
        format!("ciphertext\t{}\nnonce\t{}", self.ciphertext, self.nonce)
    }
}

impl TableOutput for UnwrappedKeyOutput {
    fn to_table(&self) -> String {
        "Data key stored".to_string()
    }
}

impl TableOutput for SecretBytesOutput {
    fn to_table(&self) -> String {
        self.hex.clone()
    }
}

impl TableOutput for SyncResultOutput {
    fn to_table(&self) -> String {
        format!(
            "pushed\t{}\npulled\t{}\nfailed\t{}\ncursor\t{}",
            self.pushed,
            self.pulled,
            self.failed,
            self.cursor
                .map(|cursor| cursor.to_string())
                .unwrap_or_else(|| "-".to_string())
        )
    }
}

impl TableOutput for CryptoVerifyOutput {
    fn to_table(&self) -> String {
        format!(
            "data_key_present\t{}\ndevice_private_key_present\t{}\nencrypt_decrypt_ok\t{}",
            self.data_key_present, self.device_private_key_present, self.encrypt_decrypt_ok
        )
    }
}

impl TableOutput for Blob {
    fn to_table(&self) -> String {
        format!(
            "ciphertext\t{}\nnonce\t{}",
            hex_encode(&self.ciphertext),
            hex_encode(&self.nonce)
        )
    }
}

impl TableOutput for SyncStatus {
    fn to_table(&self) -> String {
        format!(
            "dirty_count\t{}\nretry_queue_depth\t{}\ncursor\t{}",
            self.dirty_count, self.retry_queue_depth, self.cursor
        )
    }
}

impl TableOutput for RetryQueueEntry {
    fn to_table(&self) -> String {
        format!(
            "task_id\t{}\nattempt\t{}\nnext_retry\t{}",
            self.task_id, self.attempt, self.next_retry
        )
    }
}

impl TableOutput for PlaintextSettings {
    fn to_table(&self) -> String {
        format!(
            "schema_version\t{}\nserver_url\t{}\nauth_method\t{:?}\nlanguage\t{}\nlast_sync_cursor\t{}",
            self.schema_version, self.server_url, self.auth_method, self.language, self.last_sync_cursor
        )
    }
}

impl TableOutput for PlaintextSettingsSyncPayload {
    fn to_table(&self) -> String {
        format!(
            "schema_version\t{}\nserver_url\t{}\nauth_method\t{:?}\nlanguage\t{}",
            self.schema_version, self.server_url, self.auth_method, self.language
        )
    }
}

impl TableOutput for serde_json::Value {
    fn to_table(&self) -> String {
        match self {
            serde_json::Value::String(value) => value.clone(),
            other => other.to_string(),
        }
    }
}

pub fn format_command_result<T>(format: OutputFormat, value: &T) -> CliResult<String>
where
    T: Serialize + TableOutput,
{
    match format {
        OutputFormat::Json => Ok(serde_json::to_string_pretty(&CommandResult::new(value))?),
        OutputFormat::Jsonl => Ok(serde_json::to_string(&CommandResult::new(value))?),
        OutputFormat::Table => Ok(value.to_table()),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Inbox => "inbox",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::Done => "done",
    }
}
