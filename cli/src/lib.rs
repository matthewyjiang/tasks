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
    let ctx = context::CliContext::from_cli(&cli)?;
    if ctx.trace {
        eprintln!("trace: profile={} offline={}", ctx.profile, ctx.offline);
    }

    match cli.command {
        Some(Commands::Version) => output::format_command_result(
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
    match output {
        OutputFormat::Json | OutputFormat::Jsonl => eprintln!("{}", error.to_json_string()),
        OutputFormat::Table => eprintln!("{error}"),
    }
}
