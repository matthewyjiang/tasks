pub mod args;
pub mod context;
pub mod error;
pub mod output;
pub mod platform;

use std::path::PathBuf;

use base64::Engine;
use serde::{Deserialize, Serialize};

use args::{
    AccountCommands, AuthCommands, Cli, Commands, CryptoCommands, DeviceCommands, GenerateCommands,
    SettingsCommands, SyncCommands, TaskCommands,
};
use clap::{CommandFactory, Parser};
use error::{CliError, CliResult};
use output::{
    AuthOutput, CryptoVerifyOutput, DeleteOutput, LogoutOutput, OutputFormat, PublicKeyOutput,
    SecretBytesOutput, SyncResultOutput, UnwrappedKeyOutput, VersionOutput, WrappedKeyOutput,
};
use taskmanager_core::{
    decrypt_blob, encrypt_blob, init_account, init_device_keypair, sync_pull, sync_push,
    unwrap_data_key, wrap_data_key, AuthMethod, Blob, BlobPush, CoreError, LocalDatabase,
    PlaintextSettings, PlaintextSettingsSyncPayload, Platform, PlatformError, PullResponse,
    PushResponse, RemoteBlob, SyncClient, SyncError, TaskFilter, TaskManagerCore, TaskPatch,
    TaskStatus, ACCOUNT_DATA_KEY_ID, DEVICE_PRIVATE_KEY_ID,
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
            run_settings(command, cli.output, ctx.config_path, &ctx.profile)
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
        Some(Commands::Generate { command }) => run_generate(command),
        None => Ok(None),
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

fn run_generate(command: GenerateCommands) -> CliResult<Option<String>> {
    match command {
        GenerateCommands::Completion(args) => {
            let mut command = Cli::command();
            let mut buffer = Vec::new();
            clap_complete::generate(args.shell, &mut command, "taskmanager", &mut buffer);
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
                .filter_map(remote_blob_from_wire)
                .collect(),
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

fn remote_blob_from_wire(blob: PullWireBlob) -> Option<RemoteBlob> {
    if blob.deleted {
        return None;
    }
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(blob.ciphertext?)
        .ok()?;
    let nonce = base64::engine::general_purpose::STANDARD
        .decode(blob.nonce?)
        .ok()?
        .try_into()
        .ok()?;
    Some(RemoteBlob {
        task_id: blob.task_id,
        blob: Blob { ciphertext, nonce },
        updated_at: blob.updated_at,
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

fn run_settings(
    command: SettingsCommands,
    output_format: OutputFormat,
    config_path: Option<PathBuf>,
    profile: &str,
) -> CliResult<Option<String>> {
    let path = resolve_config_path(config_path, profile)?;
    match command {
        SettingsCommands::Get(args) => {
            let settings = PlaintextSettings::read_from_file(&path).map_err(CliError::from)?;
            if let Some(key) = args.key {
                let value = plaintext_setting_value(&settings, &key)?;
                output::format_command_result(output_format, &value).map(Some)
            } else {
                output::format_command_result(output_format, &settings).map(Some)
            }
        }
        SettingsCommands::Set(args) => {
            let mut settings = PlaintextSettings::read_from_file(&path).map_err(CliError::from)?;
            set_plaintext_setting(&mut settings, &args.key, &args.value)?;
            settings.write_to_file(&path).map_err(CliError::from)?;
            output::format_command_result(output_format, &settings).map(Some)
        }
        SettingsCommands::PullPlaintext => {
            let settings = PlaintextSettings::read_from_file(&path).map_err(CliError::from)?;
            output::format_command_result(output_format, &settings.sync_payload()).map(Some)
        }
        SettingsCommands::PushPlaintext(args) => {
            let payload: PlaintextSettingsSyncPayload =
                serde_json::from_str(&args.json).map_err(CliError::from)?;
            let mut settings = PlaintextSettings::read_from_file(&path).map_err(CliError::from)?;
            apply_plaintext_sync_payload(&mut settings, payload)?;
            settings.write_to_file(&path).map_err(CliError::from)?;
            output::format_command_result(output_format, &settings).map(Some)
        }
        SettingsCommands::Migrate => {
            let settings = PlaintextSettings::read_from_file(&path).map_err(CliError::from)?;
            settings.write_to_file(&path).map_err(CliError::from)?;
            output::format_command_result(output_format, &settings).map(Some)
        }
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
                status: TaskStatus::Inbox,
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
                tags: args.tags,
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

fn apply_plaintext_sync_payload(
    settings: &mut PlaintextSettings,
    payload: PlaintextSettingsSyncPayload,
) -> CliResult<()> {
    if payload.schema_version != taskmanager_core::PLAINTEXT_SETTINGS_SCHEMA_VERSION {
        return Err(CliError::Input(format!(
            "unsupported plaintext settings schema_version: {}",
            payload.schema_version
        )));
    }
    set_plaintext_setting(settings, "server_url", &payload.server_url)?;
    settings.auth_method = payload.auth_method;
    set_plaintext_setting(settings, "language", &payload.language)?;
    Ok(())
}

fn plaintext_setting_value(
    settings: &PlaintextSettings,
    key: &str,
) -> CliResult<serde_json::Value> {
    match key {
        "schema_version" => Ok(settings.schema_version.into()),
        "server_url" => Ok(settings.server_url.clone().into()),
        "auth_method" => serde_json::to_value(settings.auth_method).map_err(CliError::from),
        "language" => Ok(settings.language.clone().into()),
        "last_sync_cursor" => Ok(settings.last_sync_cursor.into()),
        _ => Err(CliError::Input(format!("unknown settings key: {key}"))),
    }
}

fn set_plaintext_setting(
    settings: &mut PlaintextSettings,
    key: &str,
    value: &str,
) -> CliResult<()> {
    match key {
        "server_url" => {
            if !(value.is_empty() || value.starts_with("http://") || value.starts_with("https://"))
            {
                return Err(CliError::Input(
                    "server_url must be empty or start with http:// or https://".into(),
                ));
            }
            settings.server_url = value.to_owned();
        }
        "auth_method" => {
            settings.auth_method = match value {
                "biometric" => AuthMethod::Biometric,
                "pin" => AuthMethod::Pin,
                "password" => AuthMethod::Password,
                _ => {
                    return Err(CliError::Input(
                        "auth_method must be one of: biometric, pin, password".into(),
                    ))
                }
            };
        }
        "language" => {
            if value.trim().is_empty() {
                return Err(CliError::Input("language must not be empty".into()));
            }
            settings.language = value.to_owned();
        }
        "last_sync_cursor" => {
            settings.last_sync_cursor = value
                .parse::<i64>()
                .map_err(|_| CliError::Input("last_sync_cursor must be a signed integer".into()))?;
        }
        "schema_version" => {
            return Err(CliError::Input(
                "schema_version is managed by settings migrate".into(),
            ));
        }
        _ => return Err(CliError::Input(format!("unknown settings key: {key}"))),
    }
    Ok(())
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
