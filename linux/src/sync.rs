use base64::Engine;
use std::path::Path;

use serde::{Deserialize, Serialize};
use taskmanager_core::{
    sync_pull, sync_push, Blob, BlobPush, CoreError, CoreResult, LocalDatabase, Platform,
    PullResponse, PushResponse, RemoteBlob, SyncClient, SyncError, ACCOUNT_DATA_KEY_ID,
    DEVICE_PRIVATE_KEY_ID,
};
use uuid::Uuid;

use crate::platform::LinuxPlatform;
use crate::ui::settings::read_settings;

pub(crate) const AUTH_ACCESS_TOKEN_ID: &str = "auth_access_token";
pub(crate) const AUTH_REFRESH_TOKEN_ID: &str = "auth_refresh_token";

pub struct LinuxHttpSyncClient {
    base_url: String,
    token: String,
    client: reqwest::blocking::Client,
}

impl LinuxHttpSyncClient {
    pub fn new(base_url: String, token: String) -> Self {
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

impl SyncClient for LinuxHttpSyncClient {
    fn push_blobs(&self, blobs: Vec<BlobPush>) -> CoreResult<PushResponse> {
        let request = LinuxBatchRequest {
            blobs: blobs.into_iter().map(LinuxBlobRequest::from).collect(),
        };
        let response: LinuxBatchResponse = self
            .auth(self.client.post(format!("{}/blobs/batch", self.base_url)))
            .json(&request)
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(linux_http_error)?
            .json()
            .map_err(|error| SyncError::ServerError {
                status: 0,
                body: error.to_string(),
            })?;
        Ok(PushResponse {
            accepted_task_ids: response
                .results
                .into_iter()
                .filter(|result| result.status == "ok")
                .map(|result| result.task_id)
                .collect(),
            failed_task_ids: Vec::new(),
        })
    }

    fn delete_blobs(&self, task_ids: Vec<Uuid>) -> CoreResult<PushResponse> {
        let mut accepted_task_ids = Vec::new();
        for task_id in task_ids {
            self.auth(
                self.client
                    .delete(format!("{}/blobs/{}", self.base_url, task_id)),
            )
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(linux_http_error)?;
            accepted_task_ids.push(task_id);
        }
        Ok(PushResponse {
            accepted_task_ids,
            failed_task_ids: Vec::new(),
        })
    }

    fn pull_blobs(&self, since: i64) -> CoreResult<PullResponse> {
        let response: LinuxPullWireResponse = self
            .auth(
                self.client
                    .get(format!("{}/blobs", self.base_url))
                    .query(&[("since", since)]),
            )
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(linux_http_error)?
            .json()
            .map_err(|error| SyncError::ServerError {
                status: 0,
                body: error.to_string(),
            })?;
        Ok(PullResponse {
            blobs: response
                .blobs
                .into_iter()
                .filter_map(linux_remote_blob_from_wire)
                .collect(),
            cursor: response.cursor,
        })
    }
}

pub(crate) fn linux_http_error(error: reqwest::Error) -> SyncError {
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
struct LinuxBatchRequest {
    blobs: Vec<LinuxBlobRequest>,
}

#[derive(Serialize)]
struct LinuxBlobRequest {
    task_id: Uuid,
    #[serde(serialize_with = "serialize_base64_bytes")]
    ciphertext: Vec<u8>,
    #[serde(serialize_with = "serialize_base64_nonce")]
    nonce: [u8; 12],
}

impl From<BlobPush> for LinuxBlobRequest {
    fn from(push: BlobPush) -> Self {
        Self {
            task_id: push.task_id,
            ciphertext: push.blob.ciphertext,
            nonce: push.blob.nonce,
        }
    }
}

#[derive(Deserialize)]
struct LinuxBatchResponse {
    results: Vec<LinuxBatchResult>,
}

#[derive(Deserialize)]
struct LinuxBatchResult {
    task_id: Uuid,
    status: String,
}

#[derive(Deserialize)]
struct LinuxPullWireResponse {
    blobs: Vec<LinuxPullWireBlob>,
    cursor: i64,
}

#[derive(Deserialize)]
struct LinuxPullWireBlob {
    task_id: Uuid,
    ciphertext: Option<String>,
    nonce: Option<String>,
    updated_at: i64,
    deleted: bool,
}

fn linux_remote_blob_from_wire(blob: LinuxPullWireBlob) -> Option<RemoteBlob> {
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

fn serialize_base64_bytes<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn serialize_base64_nonce<S>(bytes: &[u8; 12], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[derive(Default)]
pub(crate) struct LinuxSyncSummary {
    pub(crate) pushed: usize,
    pub(crate) pulled: usize,
    failed: usize,
}

impl LinuxSyncSummary {
    pub(crate) fn changed(&self) -> bool {
        self.pushed > 0 || self.pulled > 0 || self.failed > 0
    }
}

fn sync_auth_configured(
    platform: &LinuxPlatform,
    settings: &crate::ui::settings::LinuxSettings,
) -> bool {
    !settings.server_url.trim().is_empty()
        && platform.load_key(AUTH_ACCESS_TOKEN_ID).is_ok()
        && platform.load_key(AUTH_REFRESH_TOKEN_ID).is_ok()
        && platform.load_key(ACCOUNT_DATA_KEY_ID).is_ok()
        && platform.load_key(DEVICE_PRIVATE_KEY_ID).is_ok()
}

#[derive(Serialize)]
struct RefreshTokenRequest {
    refresh_token: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    jwt: String,
    refresh_token: String,
}

pub(crate) fn linux_sync_configured(settings_path: &Path) -> bool {
    let settings = read_settings(settings_path).unwrap_or_default();
    sync_auth_configured(&LinuxPlatform::new(), &settings)
}

pub(crate) fn run_linux_sync(db_path: &Path, settings_path: &Path) -> CoreResult<LinuxSyncSummary> {
    let settings = read_settings(settings_path).unwrap_or_default();
    if !sync_auth_configured(&LinuxPlatform::new(), &settings) {
        return Ok(LinuxSyncSummary::default());
    }
    let platform = LinuxPlatform::new();
    let data_key = platform.load_key(ACCOUNT_DATA_KEY_ID)?;
    let database = LocalDatabase::open(db_path)?;

    match run_linux_sync_once(&database, &platform, &settings.server_url, &data_key) {
        Err(CoreError::Sync(SyncError::AuthExpired)) => {
            refresh_linux_auth(&platform, &settings.server_url)?;
            run_linux_sync_once(&database, &platform, &settings.server_url, &data_key)
        }
        result => result,
    }
}

pub(crate) fn run_linux_sync_once(
    database: &LocalDatabase,
    platform: &LinuxPlatform,
    server_url: &str,
    data_key: &[u8],
) -> CoreResult<LinuxSyncSummary> {
    let token = load_utf8_key(platform, AUTH_ACCESS_TOKEN_ID, "access token")?;
    let client = LinuxHttpSyncClient::new(server_url.to_owned(), token);
    let pull = sync_pull(database, &client, data_key)?;
    let push = sync_push(database, platform, &client, data_key)?;
    Ok(LinuxSyncSummary {
        pushed: push.pushed,
        pulled: pull.pulled,
        failed: pull.failed + push.failed,
    })
}

fn refresh_linux_auth(platform: &LinuxPlatform, server_url: &str) -> CoreResult<()> {
    let refresh_token = load_utf8_key(platform, AUTH_REFRESH_TOKEN_ID, "refresh token")?;
    let client = reqwest::blocking::Client::new();
    let tokens = client
        .post(format!("{}/auth/refresh", server_url.trim_end_matches('/')))
        .json(&RefreshTokenRequest { refresh_token })
        .send()
        .map_err(|_| SyncError::NetworkUnavailable)?
        .error_for_status()
        .map_err(linux_http_error)?
        .json::<TokenResponse>()
        .map_err(|error| SyncError::ServerError {
            status: 0,
            body: error.to_string(),
        })?;

    platform.store_key(AUTH_ACCESS_TOKEN_ID, tokens.jwt.as_bytes())?;
    platform.store_key(AUTH_REFRESH_TOKEN_ID, tokens.refresh_token.as_bytes())?;
    Ok(())
}

fn load_utf8_key(platform: &LinuxPlatform, key_id: &str, label: &str) -> CoreResult<String> {
    String::from_utf8(platform.load_key(key_id)?).map_err(|error| {
        taskmanager_core::PlatformError::OperationFailed(format!(
            "stored {label} is not UTF-8: {error}"
        ))
        .into()
    })
}
