pub mod args;
pub mod context;
pub mod error;
pub mod output;
pub mod platform;

use std::path::PathBuf;

use args::{AccountCommands, AuthCommands, Cli, Commands, DeviceCommands, TaskCommands};
use clap::Parser;
use error::{CliError, CliResult};
use output::{
    AuthOutput, DeleteOutput, LogoutOutput, OutputFormat, PublicKeyOutput, UnwrappedKeyOutput,
    VersionOutput, WrappedKeyOutput,
};
use taskmanager_core::{
    init_account, init_device_keypair, unwrap_data_key, wrap_data_key, Blob, CoreError, Platform,
    PlatformError, TaskFilter, TaskManagerCore, TaskPatch, TaskStatus, ACCOUNT_DATA_KEY_ID,
    DEVICE_PRIVATE_KEY_ID,
};
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
        Some(Commands::Account { command }) => run_account(command, cli.output, ctx.offline),
        Some(Commands::Auth { command }) => run_auth(command, cli.output, ctx.offline),
        Some(Commands::Device { command }) => run_device(command, cli.output, ctx.offline),
        Some(Commands::Task { command }) => {
            run_task(command, cli.output, ctx.db_path, &ctx.profile)
        }
        None => Ok(None),
    }
}

const AUTH_ACCESS_TOKEN_ID: &str = "auth_access_token";
const AUTH_REFRESH_TOKEN_ID: &str = "auth_refresh_token";

fn run_account(
    command: AccountCommands,
    output_format: OutputFormat,
    offline: bool,
) -> CliResult<Option<String>> {
    match command {
        AccountCommands::Init => {
            let platform = platform::CliPlatform::new(offline);
            if key_exists(&platform, DEVICE_PRIVATE_KEY_ID)?
                || key_exists(&platform, ACCOUNT_DATA_KEY_ID)?
            {
                return Err(CliError::Conflict("account already exists".into()));
            }

            let public_key = init_account(&platform).map_err(CliError::from)?;
            output::format_command_result(
                output_format,
                &PublicKeyOutput {
                    public_key: to_hex(&public_key),
                },
            )
            .map(Some)
        }
    }
}

fn run_auth(
    command: AuthCommands,
    output_format: OutputFormat,
    offline: bool,
) -> CliResult<Option<String>> {
    let platform = platform::CliPlatform::new(offline);
    match command {
        AuthCommands::Login(args) => {
            platform
                .store_key(AUTH_ACCESS_TOKEN_ID, args.access_token.as_bytes())
                .map_err(CliError::from)?;
            if let Some(refresh_token) = args.refresh_token {
                platform
                    .store_key(AUTH_REFRESH_TOKEN_ID, refresh_token.as_bytes())
                    .map_err(CliError::from)?;
            }
            output::format_command_result(output_format, &AuthOutput { stored: true }).map(Some)
        }
        AuthCommands::Refresh => Err(CliError::UnsupportedPlatform(
            "auth refresh is not implemented until server auth is wired".into(),
        )),
        AuthCommands::Logout => {
            platform
                .delete_key(AUTH_ACCESS_TOKEN_ID)
                .map_err(CliError::from)?;
            platform
                .delete_key(AUTH_REFRESH_TOKEN_ID)
                .map_err(CliError::from)?;
            output::format_command_result(output_format, &LogoutOutput { logged_out: true })
                .map(Some)
        }
    }
}

fn run_device(
    command: DeviceCommands,
    output_format: OutputFormat,
    offline: bool,
) -> CliResult<Option<String>> {
    let platform = platform::CliPlatform::new(offline);
    match command {
        DeviceCommands::InitKeypair => {
            let public_key = init_device_keypair(&platform).map_err(CliError::from)?;
            output::format_command_result(
                output_format,
                &PublicKeyOutput {
                    public_key: to_hex(&public_key),
                },
            )
            .map(Some)
        }
        DeviceCommands::Register => Err(CliError::UnsupportedPlatform(
            "device register is not implemented until server auth is wired".into(),
        )),
        DeviceCommands::List => Err(CliError::UnsupportedPlatform(
            "device list is not implemented until server auth is wired".into(),
        )),
        DeviceCommands::WrapKey(args) => {
            let target_public_key = from_hex(&args.target)?;
            let data_key = platform
                .load_key(ACCOUNT_DATA_KEY_ID)
                .map_err(CliError::from)?;
            let private_key = platform
                .load_key(DEVICE_PRIVATE_KEY_ID)
                .map_err(CliError::from)?;
            let wrapped = wrap_data_key(&data_key, &target_public_key, &private_key)
                .map_err(CliError::from)?;
            output::format_command_result(
                output_format,
                &WrappedKeyOutput {
                    ciphertext: to_hex(&wrapped.ciphertext),
                    nonce: to_hex(&wrapped.nonce),
                },
            )
            .map(Some)
        }
        DeviceCommands::UnwrapKey(args) => {
            let from_public_key = from_hex(&args.from_device)?;
            let ciphertext = from_hex(&args.ciphertext)?;
            let nonce_bytes = from_hex(&args.nonce)?;
            let nonce: [u8; 12] = nonce_bytes.try_into().map_err(|bytes: Vec<u8>| {
                CliError::Crypto(format!(
                    "bad nonce length: expected 12 bytes, got {}",
                    bytes.len()
                ))
            })?;
            let private_key = platform
                .load_key(DEVICE_PRIVATE_KEY_ID)
                .map_err(CliError::from)?;
            let data_key =
                unwrap_data_key(&Blob { ciphertext, nonce }, &from_public_key, &private_key)
                    .map_err(CliError::from)?;
            platform
                .store_key(ACCOUNT_DATA_KEY_ID, &data_key)
                .map_err(CliError::from)?;
            output::format_command_result(output_format, &UnwrappedKeyOutput { stored: true })
                .map(Some)
        }
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

fn key_exists(platform: &dyn Platform, id: &str) -> CliResult<bool> {
    match platform.load_key(id) {
        Ok(_) => Ok(true),
        Err(CoreError::Platform(PlatformError::KeyNotFound(_))) => Ok(false),
        Err(error) => Err(CliError::from(error)),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex(value: &str) -> CliResult<Vec<u8>> {
    if !value.is_ascii() {
        return Err(CliError::Input(
            "hex value must contain ASCII characters only".into(),
        ));
    }
    if !value.len().is_multiple_of(2) {
        return Err(CliError::Input("hex value must have an even length".into()));
    }

    value
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let text = std::str::from_utf8(chunk)
                .map_err(|_| CliError::Input("hex value contains invalid characters".into()))?;
            u8::from_str_radix(text, 16)
                .map_err(|_| CliError::Input("hex value contains invalid characters".into()))
        })
        .collect()
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
