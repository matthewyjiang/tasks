use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

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
