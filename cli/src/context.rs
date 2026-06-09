use std::path::PathBuf;

use crate::{args::Cli, error::CliResult, output::OutputFormat};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CliContext {
    pub profile: String,
    pub config_path: Option<PathBuf>,
    pub db_path: Option<PathBuf>,
    pub server_url: Option<String>,
    pub output: OutputFormat,
    pub quiet: bool,
    pub yes: bool,
    pub offline: bool,
    pub trace: bool,
}

impl CliContext {
    pub fn from_cli(cli: &Cli) -> CliResult<Self> {
        Ok(Self {
            profile: cli.profile.clone(),
            config_path: cli.config.clone(),
            db_path: cli.db.clone(),
            server_url: cli.server.clone(),
            output: cli.output,
            quiet: cli.quiet,
            yes: cli.yes,
            offline: cli.offline,
            trace: cli.trace,
        })
    }
}
