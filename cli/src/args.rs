use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
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

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Print machine-readable version information.
    Version,
    /// Manage local tasks.
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },
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
