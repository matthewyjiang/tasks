use clap::CommandFactory;

use crate::args::{Cli, GenerateCommands};
use crate::error::{CliError, CliResult};

pub(crate) fn run(command: GenerateCommands) -> CliResult<Option<String>> {
    match command {
        GenerateCommands::Completion(args) => {
            let mut command = Cli::command();
            let mut buffer = Vec::new();
            clap_complete::generate(args.shell, &mut command, "tsk", &mut buffer);
            String::from_utf8(buffer).map(Some).map_err(|error| {
                CliError::Input(format!("generated completion is not UTF-8: {error}"))
            })
        }
        GenerateCommands::Man => {
            let command = Cli::command();
            let mut buffer = Vec::new();
            clap_mangen::Man::new(command)
                .render(&mut buffer)
                .map_err(|error| {
                    CliError::LocalStorage(format!("failed to render man page: {error}"))
                })?;
            String::from_utf8(buffer).map(Some).map_err(|error| {
                CliError::Input(format!("generated man page is not UTF-8: {error}"))
            })
        }
    }
}
