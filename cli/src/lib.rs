pub mod args;
pub mod context;
pub mod error;
pub mod output;
pub mod platform;

use std::path::PathBuf;

use args::{Cli, Commands, TaskCommands};
use clap::Parser;
use error::{CliError, CliResult};
use output::{DeleteOutput, OutputFormat, VersionOutput};
use taskmanager_core::{TaskFilter, TaskManagerCore, TaskPatch, TaskStatus};
use uuid::Uuid;

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
        Some(Commands::Task { command }) => {
            run_task(command, cli.output, ctx.db_path, &ctx.profile)
        }
        None => Ok(None),
    }
}

fn run_task(
    command: TaskCommands,
    output_format: OutputFormat,
    db_path: Option<PathBuf>,
    profile: &str,
) -> CliResult<Option<String>> {
    let db_path = resolve_db_path(db_path, profile)?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            CliError::LocalStorage(format!("failed to create DB directory: {error}"))
        })?;
    }
    let core = TaskManagerCore::open(&db_path).map_err(CliError::from)?;

    match command {
        TaskCommands::Create(args) => {
            let title = args
                .title
                .or(args.title_arg)
                .ok_or_else(|| CliError::Input("task title is required".into()))?;
            let mut task = core
                .create_task(title, args.body, args.due_at)
                .map_err(CliError::from)?;

            if args.project_id.is_some() || !args.tags.is_empty() {
                task = core
                    .update_task(
                        task.id,
                        TaskPatch {
                            project_id: args.project_id.map(Some),
                            tags: if args.tags.is_empty() {
                                None
                            } else {
                                Some(args.tags)
                            },
                            ..TaskPatch::default()
                        },
                    )
                    .map_err(CliError::from)?;
            }

            output::format_command_result(output_format, &task).map(Some)
        }
        TaskCommands::Get(args) => core
            .get_task(args.id)
            .map_err(CliError::from)
            .and_then(|task| output::format_command_result(output_format, &task))
            .map(Some),
        TaskCommands::Update(args) => {
            let patch = TaskPatch {
                title: args.title,
                body: args.body,
                due_at: if args.clear_due_at {
                    Some(None)
                } else {
                    args.due_at.map(Some)
                },
                status: args.status.map(Into::into),
                project_id: if args.clear_project_id {
                    Some(None)
                } else {
                    args.project_id.map(Some)
                },
                tags: args.tags,
            };
            core.update_task(args.id, patch)
                .map_err(CliError::from)
                .and_then(|task| output::format_command_result(output_format, &task))
                .map(Some)
        }
        TaskCommands::Delete(args) => {
            core.delete_task(args.id).map_err(CliError::from)?;
            output::format_command_result(
                output_format,
                &DeleteOutput {
                    id: args.id,
                    deleted: true,
                },
            )
            .map(Some)
        }
        TaskCommands::List(args) => {
            let filter = TaskFilter {
                status: args.status.map(Into::into),
                project_id: args.project_id,
                due_after: args.due_after,
                due_before: args.due_before,
                include_deleted: args.include_deleted,
            };
            core.list_tasks(filter, args.sort.into())
                .map_err(CliError::from)
                .and_then(|tasks| output::format_command_result(output_format, &tasks))
                .map(Some)
        }
        TaskCommands::Search(args) => core
            .search_tasks(fts_literal_query(&args.query))
            .map_err(CliError::from)
            .and_then(|tasks| output::format_command_result(output_format, &tasks))
            .map(Some),
        TaskCommands::Complete(args) => {
            update_status(core, args.id, TaskStatus::Done, output_format)
        }
        TaskCommands::Reopen(args) => {
            update_status(core, args.id, TaskStatus::Inbox, output_format)
        }
    }
}

fn fts_literal_query(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

fn update_status(
    core: TaskManagerCore,
    task_id: Uuid,
    status: TaskStatus,
    output_format: OutputFormat,
) -> CliResult<Option<String>> {
    core.update_task(
        task_id,
        TaskPatch {
            status: Some(status),
            ..TaskPatch::default()
        },
    )
    .map_err(CliError::from)
    .and_then(|task| output::format_command_result(output_format, &task))
    .map(Some)
}

fn resolve_db_path(db_path: Option<PathBuf>, profile: &str) -> CliResult<PathBuf> {
    if let Some(db_path) = db_path {
        return Ok(db_path);
    }

    let home = std::env::var_os("HOME")
        .ok_or_else(|| CliError::LocalStorage("HOME is not set; pass --db explicitly".into()))?;
    Ok(PathBuf::from(home)
        .join(".taskmanager")
        .join("profiles")
        .join(profile)
        .join("tasks.db"))
}

pub fn print_error(error: &CliError, output: OutputFormat) {
    match output {
        OutputFormat::Json | OutputFormat::Jsonl => eprintln!("{}", error.to_json_string()),
        OutputFormat::Table => eprintln!("{error}"),
    }
}
