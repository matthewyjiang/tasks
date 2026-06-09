use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use taskmanager_core::{TaskSort, TaskStatus};
use uuid::Uuid;

use crate::output::OutputFormat;

#[derive(Debug, Parser)]
#[command(
    name = "taskmanager",
    version,
    about = "Local-first encrypted task manager CLI"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = "default")]
    pub profile: String,

    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    #[arg(long, global = true)]
    pub server: Option<String>,

    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,

    #[arg(long, global = true)]
    pub quiet: bool,

    #[arg(long, global = true)]
    pub yes: bool,

    #[arg(long, global = true)]
    pub offline: bool,

    #[arg(long, global = true)]
    pub trace: bool,

    #[arg(long, global = true)]
    pub dangerously_print_secrets: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Print machine-readable version information.
    Version,
    /// Manage account bootstrap.
    Account {
        #[command(subcommand)]
        command: AccountCommands,
    },
    /// Manage auth tokens.
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    /// Manage device keys and data-key wrapping.
    Device {
        #[command(subcommand)]
        command: DeviceCommands,
    },
    /// Inspect and manage sync state.
    Sync {
        #[command(subcommand)]
        command: SyncCommands,
    },
    /// Manage plaintext settings.
    Settings {
        #[command(subcommand)]
        command: SettingsCommands,
    },
    /// Run developer crypto diagnostics for fixtures, troubleshooting, and E2E validation.
    Crypto {
        #[command(subcommand)]
        command: CryptoCommands,
    },
    /// Manage local tasks.
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },
    /// Generate packaging artifacts like shell completions and man pages.
    Generate {
        #[command(subcommand)]
        command: GenerateCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum AccountCommands {
    /// Initialize account keys locally.
    Init,
}

#[derive(Debug, Subcommand)]
pub enum AuthCommands {
    /// Store access and optional refresh tokens in the platform key store.
    Login(AuthLoginArgs),
    /// Refresh tokens are not implemented until server auth is wired.
    Refresh,
    /// Remove stored auth tokens from the platform key store.
    Logout,
}

#[derive(Debug, Args)]
pub struct AuthLoginArgs {
    #[arg(long)]
    pub access_token: String,

    #[arg(long)]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum DeviceCommands {
    /// Generate and store this device's private key, printing only the public key.
    InitKeypair,
    /// Register this device with the server. Not implemented until server auth is wired.
    Register,
    /// List registered devices. Not implemented until server auth is wired.
    List,
    /// Wrap the local account data key for a target device public key.
    WrapKey(DeviceWrapKeyArgs),
    /// Unwrap and store an account data key from another device.
    UnwrapKey(DeviceUnwrapKeyArgs),
}

#[derive(Debug, Args)]
pub struct DeviceWrapKeyArgs {
    #[arg(long)]
    pub target: String,
}

#[derive(Debug, Args)]
pub struct DeviceUnwrapKeyArgs {
    #[arg(long = "from")]
    pub from_device: String,

    #[arg(long)]
    pub ciphertext: String,

    #[arg(long)]
    pub nonce: String,
}

#[derive(Debug, Subcommand)]
pub enum SyncCommands {
    /// Report local sync diagnostics.
    Status,
    /// Queue a task for sync retry.
    Retry(TaskIdArgs),
    /// Push local encrypted changes to the server.
    Push,
    /// Pull remote encrypted changes from the server.
    Pull,
    /// Pull remote changes, then push remaining local changes.
    Run,
    /// List conflicts. Not implemented until conflict persistence is wired.
    Conflicts,
    /// Resolve a conflict. Not implemented until conflict persistence is wired.
    Resolve(TaskIdArgs),
}

#[derive(Debug, Subcommand)]
pub enum SettingsCommands {
    /// Get all settings or a single setting by key.
    Get(SettingsGetArgs),
    /// Set a plaintext setting.
    Set(SettingsSetArgs),
    /// Print syncable plaintext settings, excluding device-local cursor.
    PullPlaintext,
    /// Store syncable plaintext settings from JSON.
    PushPlaintext(SettingsPushPlaintextArgs),
    /// Migrate/create the plaintext settings file using the current schema.
    Migrate,
}

#[derive(Debug, Args)]
pub struct SettingsGetArgs {
    pub key: Option<String>,
}

#[derive(Debug, Args)]
pub struct SettingsSetArgs {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Args)]
pub struct SettingsPushPlaintextArgs {
    pub json: String,
}

#[derive(Debug, Subcommand)]
pub enum CryptoCommands {
    /// Dev diagnostic: encrypt a local task to inspect or fixture the sync blob shape.
    EncryptTask(TaskIdArgs),
    /// Dev diagnostic: decrypt a JSON blob fixture while debugging crypto/sync failures.
    DecryptBlob(CryptoBlobFileArgs),
    /// Dev diagnostic: wrap the local data key without registering or mutating devices.
    WrapDataKey(DeviceWrapKeyArgs),
    /// Dev diagnostic: print an unwrapped data key for troubleshooting; requires --dangerously-print-secrets.
    UnwrapDataKey(DeviceUnwrapKeyArgs),
    /// Dev diagnostic: verify stored local keys can encrypt/decrypt a fixture task.
    VerifyLocal,
}

#[derive(Debug, Args)]
pub struct CryptoBlobFileArgs {
    pub file: PathBuf,
}

#[derive(Debug, Subcommand)]
pub enum GenerateCommands {
    /// Generate shell completions for bash, zsh, fish, PowerShell, or elvish.
    Completion(GenerateCompletionArgs),
    /// Generate a roff man page for taskmanager(1).
    Man,
}

#[derive(Debug, Args)]
pub struct GenerateCompletionArgs {
    #[arg(value_enum)]
    pub shell: Shell,
}

#[derive(Debug, Subcommand)]
pub enum TaskCommands {
    /// Create a local task.
    Create(TaskCreateArgs),
    /// Get a task by id.
    Get(TaskIdArgs),
    /// Update a task by id.
    Update(TaskUpdateArgs),
    /// Delete a task by tombstoning it.
    Delete(TaskIdArgs),
    /// List local tasks.
    List(TaskListArgs),
    /// Search local tasks by title/body.
    Search(TaskSearchArgs),
    /// Mark a task done.
    Complete(TaskIdArgs),
    /// Reopen a task into inbox status.
    Reopen(TaskIdArgs),
}

#[derive(Debug, Args)]
pub struct TaskCreateArgs {
    #[arg(long, required_unless_present = "title_arg")]
    pub title: Option<String>,

    #[arg(value_name = "TITLE")]
    pub title_arg: Option<String>,

    #[arg(long, default_value = "")]
    pub body: String,

    #[arg(long, alias = "due")]
    pub due_at: Option<i64>,

    #[arg(long)]
    pub project_id: Option<Uuid>,

    #[arg(long = "tag")]
    pub tags: Vec<String>,
}

#[derive(Debug, Args)]
pub struct TaskIdArgs {
    pub id: Uuid,
}

#[derive(Debug, Args)]
pub struct TaskUpdateArgs {
    pub id: Uuid,

    #[arg(long)]
    pub title: Option<String>,

    #[arg(long)]
    pub body: Option<String>,

    #[arg(long)]
    pub due_at: Option<i64>,

    #[arg(long, conflicts_with = "due_at")]
    pub clear_due_at: bool,

    #[arg(long, value_enum)]
    pub status: Option<CliTaskStatus>,

    #[arg(long)]
    pub project_id: Option<Uuid>,

    #[arg(long, conflicts_with = "project_id")]
    pub clear_project_id: bool,

    #[arg(long, value_delimiter = ',')]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Args)]
pub struct TaskListArgs {
    #[arg(long, value_enum)]
    pub status: Option<CliTaskStatus>,

    #[arg(long)]
    pub project_id: Option<Uuid>,

    #[arg(long = "tag")]
    pub tags: Vec<String>,

    #[arg(long)]
    pub due_after: Option<i64>,

    #[arg(long)]
    pub due_before: Option<i64>,

    #[arg(long)]
    pub include_deleted: bool,

    #[arg(long, value_enum, default_value_t = CliTaskSort::UpdatedAtDesc)]
    pub sort: CliTaskSort,
}

#[derive(Debug, Args)]
pub struct TaskSearchArgs {
    pub query: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliTaskStatus {
    Inbox,
    InProgress,
    Done,
}

impl From<CliTaskStatus> for TaskStatus {
    fn from(status: CliTaskStatus) -> Self {
        match status {
            CliTaskStatus::Inbox => Self::Inbox,
            CliTaskStatus::InProgress => Self::InProgress,
            CliTaskStatus::Done => Self::Done,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliTaskSort {
    UpdatedAtDesc,
    UpdatedAtAsc,
    DueAtAsc,
    DueAtDesc,
    CreatedAtAsc,
    CreatedAtDesc,
}

impl From<CliTaskSort> for TaskSort {
    fn from(sort: CliTaskSort) -> Self {
        match sort {
            CliTaskSort::UpdatedAtDesc => Self::UpdatedAtDesc,
            CliTaskSort::UpdatedAtAsc => Self::UpdatedAtAsc,
            CliTaskSort::DueAtAsc => Self::DueAtAsc,
            CliTaskSort::DueAtDesc => Self::DueAtDesc,
            CliTaskSort::CreatedAtAsc => Self::CreatedAtAsc,
            CliTaskSort::CreatedAtDesc => Self::CreatedAtDesc,
        }
    }
}

impl ValueEnum for OutputFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Table, Self::Json, Self::Jsonl]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            Self::Table => Some(clap::builder::PossibleValue::new("table")),
            Self::Json => Some(clap::builder::PossibleValue::new("json")),
            Self::Jsonl => Some(clap::builder::PossibleValue::new("jsonl")),
        }
    }
}
