use serde::Serialize;

use crate::error::CliResult;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    Table,
    Json,
    Jsonl,
}

#[derive(Debug, Serialize)]
pub struct VersionOutput {
    pub name: &'static str,
    pub version: &'static str,
}

pub fn format_value<T: Serialize>(format: OutputFormat, value: &T) -> CliResult<String> {
    match format {
        OutputFormat::Json => Ok(serde_json::to_string_pretty(value)?),
        OutputFormat::Jsonl => Ok(serde_json::to_string(value)?),
        OutputFormat::Table => Ok(serde_json::to_string_pretty(value)?),
    }
}
