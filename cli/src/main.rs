use std::ffi::OsString;

use clap::Parser;
use taskmanager_cli::{args::Cli, error::CliError, output::OutputFormat, print_error, run};

fn main() {
    let raw_args: Vec<OsString> = std::env::args_os().collect();
    let cli = match Cli::try_parse_from(&raw_args) {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.exit_code() == 0 { 0 } else { 1 };
            if code == 0 {
                let _ = error.print();
            } else if let Some(output) = requested_output_format(&raw_args) {
                print_error(&CliError::from(error), output);
            } else {
                let _ = error.print();
            }
            std::process::exit(code);
        }
    };
    let output = cli.output;

    match run(cli) {
        Ok(Some(stdout)) => println!("{stdout}"),
        Ok(None) => {}
        Err(error) => {
            print_error(&error, output);
            std::process::exit(error.exit_code());
        }
    }
}

fn requested_output_format(args: &[OsString]) -> Option<OutputFormat> {
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--output" {
            return iter.next().and_then(output_format_from_os);
        }

        if let Some(value) = arg.to_string_lossy().strip_prefix("--output=") {
            return output_format_from_str(value);
        }
    }

    None
}

fn output_format_from_os(value: &OsString) -> Option<OutputFormat> {
    output_format_from_str(&value.to_string_lossy())
}

fn output_format_from_str(value: &str) -> Option<OutputFormat> {
    match value {
        "json" => Some(OutputFormat::Json),
        "jsonl" => Some(OutputFormat::Jsonl),
        "table" => Some(OutputFormat::Table),
        _ => None,
    }
}
