use clap::Parser;
use taskmanager_cli::{args::Cli, print_error, run};

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.exit_code() == 0 { 0 } else { 1 };
            let _ = error.print();
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
