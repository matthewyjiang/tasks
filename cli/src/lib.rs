pub mod args;
pub(crate) mod commands;
pub mod context;
pub mod error;
pub mod output;
pub mod platform;

use std::io::{self, Write};
use std::path::PathBuf;

use chrono::{Duration, Local, NaiveDate, TimeZone};

use base64::Engine;
use serde::{Deserialize, Serialize};

use args::{
    AuthCommands, Cli, Commands, ConfigureArgs, CryptoCommands, SyncCommands, TaskCommands,
};
use clap::{CommandFactory, Parser};
use error::{CliError, CliResult};
use output::{
    AuthOutput, ConfigureOutput, CryptoVerifyOutput, DeleteOutput, LogoutOutput, OutputFormat,
    SecretBytesOutput, SyncResultOutput, VersionOutput, WrappedKeyOutput,
};
use taskmanager_core::{
    decrypt_blob, encrypt_blob, init_account, public_key_from_private_key, sync_pull, sync_push,
    unwrap_data_key, wrap_data_key, Blob, BlobPush, CoreError, LocalDatabase, PlaintextSettings,
    Platform, PlatformError, PullResponse, PushResponse, RemoteBlob, SyncClient, SyncError,
    TaskFilter, TaskManagerCore, TaskPatch, TaskStatus, ACCOUNT_DATA_KEY_ID, DEVICE_PRIVATE_KEY_ID,
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
        Some(Commands::Configure(args)) => {
            run_configure(args, cli.output, ctx.config_path, &ctx.profile, ctx.offline)
        }
        Some(Commands::Account { command }) => {
            commands::account::run(command, cli.output, ctx.offline)
        }
        Some(Commands::Auth { command }) => run_auth(
            command,
            cli.output,
            ctx.config_path,
            &ctx.profile,
            ctx.server_url,
            ctx.offline,
        ),
        Some(Commands::Device { command }) => {
            commands::device::run(command, cli.output, ctx.offline)
        }
        Some(Commands::Sync { command }) => run_sync(
            command,
            cli.output,
            ctx.db_path,
            &ctx.profile,
            ctx.config_path,
            ctx.server_url,
            ctx.offline,
        ),
        Some(Commands::Settings { command }) => {
            commands::settings::run(command, cli.output, ctx.config_path, &ctx.profile)
        }
        Some(Commands::Crypto { command }) => run_crypto(
            command,
            cli.output,
            ctx.db_path,
            &ctx.profile,
            ctx.offline,
            cli.dangerously_print_secrets,
        ),
        Some(Commands::Task { command }) => {
            run_task(command, cli.output, ctx.db_path, &ctx.profile)
        }
        Some(Commands::Generate { command }) => commands::generate::run(command),
        None => {
            let mut command = Cli::command();
            Ok(Some(command.render_long_help().to_string()))
        }
    }
}

const AUTH_ACCESS_TOKEN_ID: &str = "auth_access_token";
const AUTH_REFRESH_TOKEN_ID: &str = "auth_refresh_token";

impl From<taskmanager_core::SyncResult> for SyncResultOutput {
    fn from(result: taskmanager_core::SyncResult) -> Self {
        Self {
            pushed: result.pushed,
            pulled: result.pulled,
            failed: result.failed,
            cursor: result.cursor,
        }
    }
}

fn run_configure(
    args: ConfigureArgs,
    output_format: OutputFormat,
    config_path: Option<PathBuf>,
    profile: &str,
    offline: bool,
) -> CliResult<Option<String>> {
    if offline {
        return Err(CliError::Input(
            "configure requires network access; remove --offline to authenticate with the server"
                .into(),
        ));
    }

    let platform = platform::CliPlatform::new(offline);
    let (account_initialized, public_key) = if key_exists(&platform, DEVICE_PRIVATE_KEY_ID)?
        && key_exists(&platform, ACCOUNT_DATA_KEY_ID)?
    {
        let private_key = platform
            .load_key(DEVICE_PRIVATE_KEY_ID)
            .map_err(CliError::from)?;
        (
            false,
            public_key_from_private_key(&private_key).map_err(CliError::from)?,
        )
    } else {
        (true, init_account(&platform).map_err(CliError::from)?)
    };

    let server_url = match args.server_url {
        Some(value) => value,
        None => prompt("Server URL (for example http://127.0.0.1:18080): ")?,
    };
    let email = match args.email {
        Some(value) => value,
        None => prompt("Email: ")?,
    };
    let password = match args.password {
        Some(value) => value,
        None => prompt_password("Password: ")?,
    };
    let tokens = configure_server_auth(&server_url, &email, &password, &public_key, args.register)?;

    let settings_path = resolve_config_path(config_path, profile)?;
    let mut settings = PlaintextSettings::read_from_file(&settings_path).map_err(CliError::from)?;
    commands::settings::set_plaintext_setting(&mut settings, "server_url", &server_url)?;
    settings
        .write_to_file(&settings_path)
        .map_err(CliError::from)?;

    platform
        .store_key(AUTH_ACCESS_TOKEN_ID, tokens.jwt.as_bytes())
        .map_err(CliError::from)?;
    platform
        .store_key(AUTH_REFRESH_TOKEN_ID, tokens.refresh_token.as_bytes())
        .map_err(CliError::from)?;

    output::format_command_result(
        output_format,
        &ConfigureOutput {
            account_initialized,
            server_url,
            access_token_stored: true,
            refresh_token_stored: true,
            auth_method: tokens.method,
        },
    )
    .map(Some)
}

#[derive(Deserialize)]
struct TokenResponse {
    jwt: String,
    refresh_token: String,
}

struct ConfigureTokens {
    jwt: String,
    refresh_token: String,
    method: &'static str,
}

#[derive(Serialize)]
struct AuthRequest<'a> {
    email: &'a str,
    password: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub_key: Option<String>,
}

fn configure_server_auth(
    server_url: &str,
    email: &str,
    password: &str,
    public_key: &[u8],
    register: bool,
) -> CliResult<ConfigureTokens> {
    let base_url = server_url.trim_end_matches('/');
    let client = reqwest::blocking::Client::new();
    if register {
        let register_request = AuthRequest {
            email,
            password,
            pub_key: Some(base64::engine::general_purpose::STANDARD.encode(public_key)),
        };
        match client
            .post(format!("{base_url}/auth/register"))
            .json(&register_request)
            .send()
        {
            Ok(response) if response.status().is_success() => {
                let tokens: TokenResponse = response.json().map_err(|error| {
                    CliError::Network(format!("failed to decode auth register response: {error}"))
                })?;
                return Ok(ConfigureTokens {
                    jwt: tokens.jwt,
                    refresh_token: tokens.refresh_token,
                    method: "register",
                });
            }
            Ok(response) if response.status().is_client_error() => {
                // Existing account or validation mismatch: try normal login below.
            }
            Ok(response) => {
                return Err(CliError::Network(format!(
                    "server auth register failed: HTTP {}",
                    response.status()
                )));
            }
            Err(error) => {
                return Err(CliError::Network(format!(
                    "server auth register failed: {error}"
                )))
            }
        }
    }

    login_server_auth(server_url, email, password)
}

fn login_server_auth(server_url: &str, email: &str, password: &str) -> CliResult<ConfigureTokens> {
    let base_url = server_url.trim_end_matches('/');
    let client = reqwest::blocking::Client::new();
    let login_request = AuthRequest {
        email,
        password,
        pub_key: None,
    };
    let response = client
        .post(format!("{base_url}/auth/login"))
        .json(&login_request)
        .send()
        .map_err(|error| CliError::Network(format!("server auth login failed: {error}")))?
        .error_for_status()
        .map_err(|error| CliError::Network(format!("server auth login failed: {error}")))?;
    let tokens: TokenResponse = response.json().map_err(|error| {
        CliError::Network(format!("failed to decode auth login response: {error}"))
    })?;
    Ok(ConfigureTokens {
        jwt: tokens.jwt,
        refresh_token: tokens.refresh_token,
        method: "login",
    })
}

fn prompt(label: &str) -> CliResult<String> {
    eprint!("{label}");
    io::stderr()
        .flush()
        .map_err(|error| CliError::LocalStorage(format!("failed to write prompt: {error}")))?;
    let mut value = String::new();
    io::stdin()
        .read_line(&mut value)
        .map_err(|error| CliError::Input(format!("failed to read input: {error}")))?;
    non_empty_prompt_value(value)
}

fn prompt_password(label: &str) -> CliResult<String> {
    match rpassword::prompt_password(label) {
        Ok(value) => non_empty_prompt_value(value),
        Err(_) => prompt(label),
    }
}

fn non_empty_prompt_value(value: String) -> CliResult<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        Err(CliError::Input("required configure value was empty".into()))
    } else {
        Ok(value)
    }
}

fn run_auth(
    command: AuthCommands,
    output_format: OutputFormat,
    config_path: Option<PathBuf>,
    profile: &str,
    server_url: Option<String>,
    offline: bool,
) -> CliResult<Option<String>> {
    let platform = platform::CliPlatform::new(offline);
    match command {
        AuthCommands::Login(args) => {
            let (access_token, refresh_token) = match (args.email, args.access_token) {
                (Some(email), None) => {
                    if offline {
                        return Err(CliError::Input(
                            "auth login requires network access; remove --offline to authenticate with the server"
                                .into(),
                        ));
                    }
                    let password = match args.password {
                        Some(value) => value,
                        None => prompt_password("Password: ")?,
                    };
                    let server_url = args
                        .server_url
                        .or(server_url)
                        .map(Ok)
                        .unwrap_or_else(|| resolve_server_url(None, config_path, profile))?;
                    let tokens = login_server_auth(&server_url, &email, &password)?;
                    (tokens.jwt, Some(tokens.refresh_token))
                }
                (None, Some(access_token)) => (access_token, args.refresh_token),
                (Some(_), Some(_)) => {
                    return Err(CliError::Input(
                        "use either --email/--password or --access-token, not both".into(),
                    ));
                }
                (None, None) => {
                    return Err(CliError::Input(
                        "auth login requires --email or --access-token".into(),
                    ));
                }
            };

            platform
                .store_key(AUTH_ACCESS_TOKEN_ID, access_token.as_bytes())
                .map_err(CliError::from)?;
            if let Some(refresh_token) = refresh_token {
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

fn run_sync(
    command: SyncCommands,
    output_format: OutputFormat,
    db_path: Option<PathBuf>,
    profile: &str,
    config_path: Option<PathBuf>,
    server_url: Option<String>,
    offline: bool,
) -> CliResult<Option<String>> {
    match command {
        SyncCommands::Status => open_core(db_path, profile)?
            .sync_status()
            .map_err(CliError::from)
            .and_then(|status| output::format_command_result(output_format, &status))
            .map(Some),
        SyncCommands::Retry(args) => open_core(db_path, profile)?
            .queue_sync_retry(args.id, now_ms())
            .map_err(CliError::from)
            .and_then(|entry| output::format_command_result(output_format, &entry))
            .map(Some),
        SyncCommands::Push => {
            let (database, platform, client, data_key) =
                sync_runtime(db_path, profile, config_path.clone(), server_url, offline)?;
            sync_push(&database, &platform, &client, &data_key)
                .map(SyncResultOutput::from)
                .map_err(CliError::from)
                .and_then(|result| output::format_command_result(output_format, &result))
                .map(Some)
        }
        SyncCommands::Pull => {
            let (database, _platform, client, data_key) =
                sync_runtime(db_path, profile, config_path.clone(), server_url, offline)?;
            sync_pull(&database, &client, &data_key)
                .map(SyncResultOutput::from)
                .map_err(CliError::from)
                .and_then(|result| output::format_command_result(output_format, &result))
                .map(Some)
        }
        SyncCommands::Run => {
            let (database, platform, client, data_key) =
                sync_runtime(db_path, profile, config_path, server_url, offline)?;
            let pull = sync_pull(&database, &client, &data_key).map_err(CliError::from)?;
            let push =
                sync_push(&database, &platform, &client, &data_key).map_err(CliError::from)?;
            output::format_command_result(
                output_format,
                &SyncResultOutput {
                    pushed: push.pushed,
                    pulled: pull.pulled,
                    failed: push.failed + pull.failed,
                    cursor: pull.cursor,
                },
            )
            .map(Some)
        }
        SyncCommands::Conflicts => Err(CliError::UnsupportedPlatform(
            "sync conflicts is not implemented until conflict persistence is wired".into(),
        )),
        SyncCommands::Resolve(_) => Err(CliError::UnsupportedPlatform(
            "sync resolve is not implemented until conflict persistence is wired".into(),
        )),
    }
}

fn resolve_server_url(
    server_url: Option<String>,
    config_path: Option<PathBuf>,
    profile: &str,
) -> CliResult<String> {
    if let Some(server_url) = server_url {
        return Ok(server_url);
    }

    let settings_path = resolve_config_path(config_path, profile)?;
    let settings = PlaintextSettings::read_from_file(&settings_path).map_err(CliError::from)?;
    if settings.server_url.is_empty() {
        Err(CliError::Input(
            "--server is required for sync push/pull/run until settings server_url is configured"
                .into(),
        ))
    } else {
        Ok(settings.server_url)
    }
}

fn sync_runtime(
    db_path: Option<PathBuf>,
    profile: &str,
    config_path: Option<PathBuf>,
    server_url: Option<String>,
    offline: bool,
) -> CliResult<(
    LocalDatabase,
    platform::CliPlatform,
    HttpSyncClient,
    Vec<u8>,
)> {
    let server_url = resolve_server_url(server_url, config_path, profile)?;
    let platform = platform::CliPlatform::new(offline);
    let token = String::from_utf8(
        platform
            .load_key(AUTH_ACCESS_TOKEN_ID)
            .map_err(CliError::from)?,
    )
    .map_err(|error| {
        CliError::LocalStorage(format!("stored access token is not UTF-8: {error}"))
    })?;
    let data_key = platform
        .load_key(ACCOUNT_DATA_KEY_ID)
        .map_err(CliError::from)?;
    let db_path = resolve_db_path(db_path, profile)?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            CliError::LocalStorage(format!("failed to create DB directory: {error}"))
        })?;
    }
    let database = LocalDatabase::open(&db_path).map_err(CliError::from)?;
    Ok((
        database,
        platform,
        HttpSyncClient::new(server_url, token),
        data_key,
    ))
}

struct HttpSyncClient {
    base_url: String,
    token: String,
    client: reqwest::blocking::Client,
}

impl HttpSyncClient {
    fn new(base_url: String, token: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            token,
            client: reqwest::blocking::Client::new(),
        }
    }

    fn auth(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        request.bearer_auth(&self.token)
    }
}

impl SyncClient for HttpSyncClient {
    fn push_blobs(&self, blobs: Vec<BlobPush>) -> taskmanager_core::CoreResult<PushResponse> {
        let request = BatchRequest {
            blobs: blobs.into_iter().map(BlobRequest::from).collect(),
        };
        let response: BatchResponse = self
            .auth(self.client.post(format!("{}/blobs/batch", self.base_url)))
            .json(&request)
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(http_error)?
            .json()
            .map_err(|error| SyncError::ServerError {
                status: 0,
                body: error.to_string(),
            })?;
        Ok(push_response_from_batch(response))
    }

    fn delete_blobs(&self, task_ids: Vec<Uuid>) -> taskmanager_core::CoreResult<PushResponse> {
        let mut accepted_task_ids = Vec::new();
        for task_id in task_ids {
            self.auth(
                self.client
                    .delete(format!("{}/blobs/{}", self.base_url, task_id)),
            )
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(http_error)?;
            accepted_task_ids.push(task_id);
        }
        Ok(PushResponse {
            accepted_task_ids,
            failed_task_ids: Vec::new(),
        })
    }

    fn pull_blobs(&self, since: i64) -> taskmanager_core::CoreResult<PullResponse> {
        let response: PullWireResponse = self
            .auth(
                self.client
                    .get(format!("{}/blobs", self.base_url))
                    .query(&[("since", since)]),
            )
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(http_error)?
            .json()
            .map_err(|error| SyncError::ServerError {
                status: 0,
                body: error.to_string(),
            })?;
        Ok(PullResponse {
            blobs: response
                .blobs
                .into_iter()
                .map(remote_blob_from_wire)
                .collect::<Result<Vec<_>, _>>()?,
            cursor: response.cursor,
        })
    }
}

fn http_error(error: reqwest::Error) -> SyncError {
    if error.status() == Some(reqwest::StatusCode::UNAUTHORIZED) {
        SyncError::AuthExpired
    } else {
        SyncError::ServerError {
            status: error.status().map_or(0, |status| status.as_u16()),
            body: error.to_string(),
        }
    }
}

#[derive(Serialize)]
struct BatchRequest {
    blobs: Vec<BlobRequest>,
}

#[derive(Serialize)]
struct BlobRequest {
    task_id: Uuid,
    #[serde(with = "base64_bytes")]
    ciphertext: Vec<u8>,
    #[serde(with = "base64_nonce")]
    nonce: [u8; 12],
}

impl From<BlobPush> for BlobRequest {
    fn from(push: BlobPush) -> Self {
        Self {
            task_id: push.task_id,
            ciphertext: push.blob.ciphertext,
            nonce: push.blob.nonce,
        }
    }
}

#[derive(Deserialize)]
struct BatchResponse {
    results: Vec<BatchResult>,
}

#[derive(Deserialize)]
struct BatchResult {
    task_id: Uuid,
    status: String,
}

#[derive(Deserialize)]
struct PullWireResponse {
    blobs: Vec<PullWireBlob>,
    cursor: i64,
}

#[derive(Deserialize)]
struct PullWireBlob {
    task_id: Uuid,
    ciphertext: Option<String>,
    nonce: Option<String>,
    updated_at: i64,
    deleted: bool,
}

fn remote_blob_from_wire(blob: PullWireBlob) -> taskmanager_core::CoreResult<RemoteBlob> {
    if blob.deleted {
        return Ok(RemoteBlob {
            task_id: blob.task_id,
            blob: None,
            updated_at: blob.updated_at,
            deleted: true,
        });
    }
    let ciphertext = blob
        .ciphertext
        .and_then(|ciphertext| {
            base64::engine::general_purpose::STANDARD
                .decode(ciphertext)
                .ok()
        })
        .ok_or_else(|| SyncError::ServerError {
            status: 502,
            body: "malformed blob payload".to_owned(),
        })?;
    let nonce = blob
        .nonce
        .and_then(|nonce| base64::engine::general_purpose::STANDARD.decode(nonce).ok())
        .and_then(|nonce| nonce.try_into().ok())
        .ok_or_else(|| SyncError::ServerError {
            status: 502,
            body: "malformed blob payload".to_owned(),
        })?;
    Ok(RemoteBlob {
        task_id: blob.task_id,
        blob: Some(Blob { ciphertext, nonce }),
        updated_at: blob.updated_at,
        deleted: false,
    })
}

mod base64_bytes {
    use base64::Engine;
    use serde::Serializer;

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
    }
}

mod base64_nonce {
    use base64::Engine;
    use serde::Serializer;

    pub fn serialize<S>(nonce: &[u8; 12], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(nonce))
    }
}

fn push_response_from_batch(response: BatchResponse) -> PushResponse {
    let mut accepted_task_ids = Vec::new();
    let mut failed_task_ids = Vec::new();
    for result in response.results {
        if result.status == "ok" {
            accepted_task_ids.push(result.task_id);
        } else {
            failed_task_ids.push(result.task_id);
        }
    }
    PushResponse {
        accepted_task_ids,
        failed_task_ids,
    }
}

fn run_crypto(
    command: CryptoCommands,
    output_format: OutputFormat,
    db_path: Option<PathBuf>,
    profile: &str,
    offline: bool,
    dangerously_print_secrets: bool,
) -> CliResult<Option<String>> {
    let platform = platform::CliPlatform::new(offline);
    match command {
        CryptoCommands::EncryptTask(args) => {
            let task = open_core(db_path, profile)?
                .get_task(args.id)
                .map_err(CliError::from)?;
            let data_key = platform
                .load_key(ACCOUNT_DATA_KEY_ID)
                .map_err(CliError::from)?;
            let blob = encrypt_blob(&task, &data_key).map_err(CliError::from)?;
            output::format_command_result(output_format, &blob).map(Some)
        }
        CryptoCommands::DecryptBlob(args) => {
            let data_key = platform
                .load_key(ACCOUNT_DATA_KEY_ID)
                .map_err(CliError::from)?;
            let contents = std::fs::read_to_string(&args.file).map_err(|error| {
                CliError::LocalStorage(format!("failed to read {}: {error}", args.file.display()))
            })?;
            let blob = parse_blob_file(&contents)?;
            let task = decrypt_blob(&blob, &data_key).map_err(CliError::from)?;
            output::format_command_result(output_format, &task).map(Some)
        }
        CryptoCommands::WrapDataKey(args) => {
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
        CryptoCommands::UnwrapDataKey(args) => {
            if !dangerously_print_secrets {
                return Err(CliError::Input(
                    "refusing to print data key without --dangerously-print-secrets".into(),
                ));
            }
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
            output::format_command_result(
                output_format,
                &SecretBytesOutput {
                    hex: to_hex(&data_key),
                },
            )
            .map(Some)
        }
        CryptoCommands::VerifyLocal => {
            let data_key = platform
                .load_key(ACCOUNT_DATA_KEY_ID)
                .map_err(CliError::from)?;
            let private_key = platform
                .load_key(DEVICE_PRIVATE_KEY_ID)
                .map_err(CliError::from)?;
            if data_key.len() != 32 {
                return Err(CliError::Crypto(format!(
                    "bad account data key length: expected 32 bytes, got {}",
                    data_key.len()
                )));
            }
            if private_key.len() != 32 {
                return Err(CliError::Crypto(format!(
                    "bad device private key length: expected 32 bytes, got {}",
                    private_key.len()
                )));
            }
            let task = taskmanager_core::Task {
                id: Uuid::nil(),
                title: "crypto verify".to_owned(),
                body: String::new(),
                due_at: None,
                reminder_offset_ms: None,
                status: TaskStatus::Open,
                project_id: None,
                tags: Vec::new(),
                created_at: 0,
                updated_at: 0,
                deleted: false,
                dirty: false,
            };
            let blob = encrypt_blob(&task, &data_key).map_err(CliError::from)?;
            decrypt_blob(&blob, &data_key).map_err(CliError::from)?;
            output::format_command_result(
                output_format,
                &CryptoVerifyOutput {
                    data_key_present: true,
                    device_private_key_present: true,
                    encrypt_decrypt_ok: true,
                },
            )
            .map(Some)
        }
    }
}

fn parse_blob_file(contents: &str) -> CliResult<Blob> {
    let value: serde_json::Value = serde_json::from_str(contents).map_err(CliError::from)?;
    if let Some(result) = value.get("result") {
        serde_json::from_value(result.clone()).map_err(CliError::from)
    } else {
        serde_json::from_value(value).map_err(CliError::from)
    }
}

fn parse_due_at(value: Option<&str>) -> CliResult<Option<i64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::Input("due date cannot be empty".into()));
    }
    if let Ok(epoch_ms) = value.parse::<i64>() {
        return Ok(Some(epoch_ms));
    }

    let lower = value.to_ascii_lowercase();
    let date = match lower.as_str() {
        "today" => Local::now().date_naive(),
        "tomorrow" => Local::now().date_naive() + Duration::days(1),
        _ => NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
            CliError::Input(format!(
                "unsupported due date '{value}'; use epoch milliseconds, YYYY-MM-DD, today, or tomorrow"
            ))
        })?,
    };
    let due = date.and_hms_opt(0, 0, 0).ok_or_else(|| {
        CliError::Input(format!(
            "unsupported due date '{value}'; invalid midnight time"
        ))
    })?;
    let local_due = Local.from_local_datetime(&due).single().ok_or_else(|| {
        CliError::Input(format!(
            "unsupported due date '{value}'; local time is ambiguous"
        ))
    })?;
    Ok(Some(local_due.timestamp_millis()))
}

fn run_task(
    command: TaskCommands,
    output_format: OutputFormat,
    db_path: Option<PathBuf>,
    profile: &str,
) -> CliResult<Option<String>> {
    let core = open_core(db_path, profile)?;

    match command {
        TaskCommands::Create(args) => {
            let title = args
                .title
                .or(args.title_arg)
                .ok_or_else(|| CliError::Input("task title is required".into()))?;
            let due_at = parse_due_at(args.due_at.as_deref())?;
            let mut task = core
                .create_task(title, args.body, due_at)
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
                    parse_due_at(args.due_at.as_deref())?.map(Some)
                },
                status: args.status.map(Into::into),
                project_id: if args.clear_project_id {
                    Some(None)
                } else {
                    args.project_id.map(Some)
                },
                tags: args.tags,
                ..TaskPatch::default()
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
                tags: args.tags,
                due_after: parse_due_at(args.due_after.as_deref())?,
                due_before: parse_due_at(args.due_before.as_deref())?,
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
        TaskCommands::Reopen(args) => update_status(core, args.id, TaskStatus::Open, output_format),
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

fn open_core(db_path: Option<PathBuf>, profile: &str) -> CliResult<TaskManagerCore> {
    let db_path = resolve_db_path(db_path, profile)?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            CliError::LocalStorage(format!("failed to create DB directory: {error}"))
        })?;
    }
    TaskManagerCore::open(&db_path).map_err(CliError::from)
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
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

fn resolve_config_path(config_path: Option<PathBuf>, profile: &str) -> CliResult<PathBuf> {
    if let Some(config_path) = config_path {
        return Ok(config_path);
    }

    let home = std::env::var_os("HOME").ok_or_else(|| {
        CliError::LocalStorage("HOME is not set; pass --config explicitly".into())
    })?;
    Ok(PathBuf::from(home)
        .join(".taskmanager")
        .join("profiles")
        .join(profile)
        .join("settings.json"))
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
