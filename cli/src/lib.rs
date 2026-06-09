pub mod args;
pub mod context;
pub mod error;
pub mod output;
pub mod platform;

use args::{Cli, Commands};
use clap::Parser;
use error::{CliError, CliResult};
use output::{OutputFormat, VersionOutput};

pub fn run_from<I, T>(itr: I) -> CliResult<Option<String>>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::try_parse_from(itr).map_err(CliError::from)?;
    run(cli)
}

pub fn run(cli: Cli) -> CliResult<Option<String>> {
    let _ctx = context::CliContext::from_cli(&cli)?;
    match cli.command {
        Some(Commands::Version) => output::format_value(
            cli.output,
            &VersionOutput {
                name: env!("CARGO_PKG_NAME"),
                version: env!("CARGO_PKG_VERSION"),
            },
        )
        .map(Some),
        None => Ok(None),
    }
}

pub fn print_error(error: &CliError, output: OutputFormat) {
    if output == OutputFormat::Json {
        eprintln!("{}", error.to_json_string());
    } else {
        eprintln!("{error}");
    }
}
