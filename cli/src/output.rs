use serde::Serialize;

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

pub trait TableOutput {
    fn to_table(&self) -> String;
}

impl TableOutput for VersionOutput {
    fn to_table(&self) -> String {
        self.to_table()
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
