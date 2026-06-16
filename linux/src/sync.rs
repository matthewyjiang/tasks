use base64::Engine;
use std::path::Path;

use serde::{Deserialize, Serialize};
use taskmanager_core::{
    load_access_token, normalize_sync_server_url, refresh_auth, sync_auth_configured, sync_pull,
    sync_push, AuthClient, Blob, BlobPush, CoreError, CoreResult, DeleteSessionRequest,
    LocalDatabase, LoginRequest, Platform, PlatformError, PullResponse, PushResponse,
    PutCurrentDeviceKeyRequest, RefreshTokenRequest, RegisterRequest, RemoteBlob, SharedTaskInvite,
    SyncClient, SyncError, TaskManagerCore, TokenResponse, ACCOUNT_DATA_KEY_ID,
    DEVICE_PRIVATE_KEY_ID,
};
use uuid::Uuid;

use crate::platform::LinuxPlatform;
use crate::ui::settings::read_settings;

pub struct LinuxHttpSyncClient {
    base_url: String,
    token: String,
    client: reqwest::blocking::Client,
}

impl LinuxHttpSyncClient {
    pub fn new(base_url: &str, token: String) -> CoreResult<Self> {
        Ok(Self {
            base_url: normalize_sync_server_url(base_url)?,
            token,
            client: reqwest::blocking::Client::new(),
        })
    }

    fn auth(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        request.bearer_auth(&self.token)
    }

    pub(crate) fn create_share(
        &self,
        task_id: Uuid,
        recipient_id: Uuid,
        wrapped_task_key: Blob,
    ) -> CoreResult<()> {
        self.auth(
            self.client
                .post(format!("{}/share/{}", self.base_url, task_id)),
        )
        .json(&LinuxShareCreateRequest {
            recipient_id,
            wrapped_dek: wrapped_task_key.ciphertext,
            nonce: wrapped_task_key.nonce,
        })
        .send()
        .map_err(|_| SyncError::NetworkUnavailable)?
        .error_for_status()
        .map_err(linux_http_error)?;
        Ok(())
    }

    pub(crate) fn revoke_share(&self, task_id: Uuid, recipient_id: Uuid) -> CoreResult<()> {
        self.auth(self.client.delete(format!(
            "{}/share/{}/{}",
            self.base_url, task_id, recipient_id
        )))
        .send()
        .map_err(|_| SyncError::NetworkUnavailable)?
        .error_for_status()
        .map_err(linux_http_error)?;
        Ok(())
    }

    pub(crate) fn share_inbox(&self) -> CoreResult<Vec<LinuxSharedTaskWire>> {
        Ok(self
            .auth(self.client.get(format!("{}/share/inbox", self.base_url)))
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(linux_http_error)?
            .json::<LinuxShareInboxResponse>()
            .map_err(|error| SyncError::ServerError {
                status: 0,
                body: error.to_string(),
            })?
            .shared)
    }

    pub(crate) fn user_public_keys(&self, user_id: Uuid) -> CoreResult<Vec<Vec<u8>>> {
        Ok(self
            .auth(
                self.client
                    .get(format!("{}/keys/{}", self.base_url, user_id)),
            )
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(linux_http_error)?
            .json::<LinuxKeysResponse>()
            .map_err(|error| SyncError::ServerError {
                status: 0,
                body: error.to_string(),
            })?
            .keys
            .into_iter()
            .map(|key| key.pub_key)
            .collect())
    }

    pub(crate) fn user_public_keys_by_email(
        &self,
        email: &str,
    ) -> CoreResult<(Uuid, Vec<Vec<u8>>)> {
        let response = self
            .auth(
                self.client
                    .get(format!("{}/keys/by-email", self.base_url))
                    .query(&[("email", email)]),
            )
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(linux_http_error)?
            .json::<LinuxKeysResponse>()
            .map_err(|error| SyncError::ServerError {
                status: 0,
                body: error.to_string(),
            })?;
        Ok((
            response.user_id,
            response.keys.into_iter().map(|key| key.pub_key).collect(),
        ))
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
        let (accepted, failed): (Vec<_>, Vec<_>) = response
            .results
            .into_iter()
            .partition(|result| result.status == "ok");
        Ok(PushResponse {
            accepted_task_ids: accepted.into_iter().map(|result| result.task_id).collect(),
            failed_task_ids: failed.into_iter().map(|result| result.task_id).collect(),
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
struct LinuxShareCreateRequest {
    recipient_id: Uuid,
    #[serde(serialize_with = "serialize_base64_bytes")]
    wrapped_dek: Vec<u8>,
    #[serde(serialize_with = "serialize_base64_nonce")]
    nonce: [u8; 12],
}

#[derive(Deserialize)]
pub(crate) struct LinuxShareInboxResponse {
    shared: Vec<LinuxSharedTaskWire>,
}

#[derive(Deserialize)]
pub(crate) struct LinuxSharedTaskWire {
    task_id: Uuid,
    owner_id: Uuid,
    recipient_id: Uuid,
    wrapped_dek: String,
    nonce: String,
}

impl LinuxSharedTaskWire {
    fn into_invite(self) -> Option<SharedTaskInvite> {
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(self.wrapped_dek)
            .ok()?;
        let nonce = base64::engine::general_purpose::STANDARD
            .decode(self.nonce)
            .ok()?
            .try_into()
            .ok()?;
        Some(SharedTaskInvite {
            task_id: self.task_id,
            owner_id: self.owner_id,
            recipient_id: self.recipient_id,
            wrapped_task_key: Blob { ciphertext, nonce },
        })
    }
}

#[derive(Deserialize)]
struct LinuxKeysResponse {
    user_id: Uuid,
    keys: Vec<LinuxKeyResponse>,
}

#[derive(Deserialize)]
struct LinuxKeyResponse {
    #[serde(deserialize_with = "deserialize_base64_bytes")]
    pub_key: Vec<u8>,
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
        return Some(RemoteBlob {
            task_id: blob.task_id,
            blob: None,
            updated_at: blob.updated_at,
            deleted: true,
        });
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
        blob: Some(Blob { ciphertext, nonce }),
        updated_at: blob.updated_at,
        deleted: false,
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

fn deserialize_base64_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(serde::de::Error::custom)
}

#[derive(Default)]
pub(crate) struct LinuxSyncSummary {
    pub(crate) pushed: usize,
    pub(crate) pulled: usize,
    pub(crate) failed: usize,
    pub(crate) pending_retries: usize,
    pub(crate) conflicts: usize,
}

impl LinuxSyncSummary {
    pub(crate) fn changed(&self) -> bool {
        self.pushed > 0
            || self.pulled > 0
            || self.failed > 0
            || self.pending_retries > 0
            || self.conflicts > 0
    }
}

fn sync_auth_configured_for_settings(
    platform: &LinuxPlatform,
    settings: &crate::ui::settings::LinuxSettings,
) -> bool {
    sync_auth_configured(platform, &settings.server_url)
}

pub(crate) struct LinuxAuthClient {
    client: reqwest::blocking::Client,
}

impl LinuxAuthClient {
    pub(crate) fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct LinuxRegisterRequest {
    email: String,
    password: String,
    pub_key: String,
}

#[derive(Serialize)]
struct LinuxLoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
struct LinuxRefreshTokenRequest {
    refresh_token: String,
}

#[derive(Serialize)]
struct LinuxPutCurrentDeviceKeyRequest {
    pub_key: String,
}

#[derive(Deserialize)]
struct LinuxTokenResponse {
    jwt: String,
    refresh_token: String,
    #[serde(default)]
    user_id: Option<String>,
}

impl From<LinuxTokenResponse> for TokenResponse {
    fn from(response: LinuxTokenResponse) -> Self {
        Self {
            jwt: response.jwt,
            refresh_token: response.refresh_token,
            user_id: response.user_id,
        }
    }
}

impl AuthClient for LinuxAuthClient {
    fn register(&self, server_url: &str, request: RegisterRequest) -> CoreResult<TokenResponse> {
        Ok(self
            .client
            .post(format!("{server_url}/auth/register"))
            .json(&LinuxRegisterRequest {
                email: request.email,
                password: request.password,
                pub_key: request.pub_key,
            })
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(linux_http_error)?
            .json::<LinuxTokenResponse>()
            .map_err(|error| SyncError::ServerError {
                status: 0,
                body: error.to_string(),
            })?
            .into())
    }

    fn login(&self, server_url: &str, request: LoginRequest) -> CoreResult<TokenResponse> {
        Ok(self
            .client
            .post(format!("{server_url}/auth/login"))
            .json(&LinuxLoginRequest {
                email: request.email,
                password: request.password,
            })
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(linux_http_error)?
            .json::<LinuxTokenResponse>()
            .map_err(|error| SyncError::ServerError {
                status: 0,
                body: error.to_string(),
            })?
            .into())
    }

    fn refresh(&self, server_url: &str, request: RefreshTokenRequest) -> CoreResult<TokenResponse> {
        Ok(self
            .client
            .post(format!("{server_url}/auth/refresh"))
            .json(&LinuxRefreshTokenRequest {
                refresh_token: request.refresh_token,
            })
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(linux_http_error)?
            .json::<LinuxTokenResponse>()
            .map_err(|error| SyncError::ServerError {
                status: 0,
                body: error.to_string(),
            })?
            .into())
    }

    fn delete_session(&self, server_url: &str, request: DeleteSessionRequest) -> CoreResult<()> {
        self.client
            .delete(format!("{server_url}/auth/session"))
            .json(&LinuxRefreshTokenRequest {
                refresh_token: request.refresh_token,
            })
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(linux_http_error)?;
        Ok(())
    }

    fn put_current_device_key(
        &self,
        server_url: &str,
        access_token: &str,
        request: PutCurrentDeviceKeyRequest,
    ) -> CoreResult<()> {
        self.client
            .put(format!("{server_url}/keys/me"))
            .bearer_auth(access_token)
            .json(&LinuxPutCurrentDeviceKeyRequest {
                pub_key: request.pub_key,
            })
            .send()
            .map_err(|_| SyncError::NetworkUnavailable)?
            .error_for_status()
            .map_err(linux_http_error)?;
        Ok(())
    }
}

pub(crate) fn linux_sync_configured(settings_path: &Path) -> bool {
    let settings = read_settings(settings_path).unwrap_or_default();
    sync_auth_configured_for_settings(&LinuxPlatform::new(), &settings)
}

pub(crate) fn share_linux_task(
    core: &TaskManagerCore,
    settings_path: &Path,
    task_id: Uuid,
    recipient_email: &str,
) -> CoreResult<()> {
    let settings = read_settings(settings_path).unwrap_or_default();
    let platform = LinuxPlatform::new();
    let server_url = normalize_sync_server_url(&settings.server_url)?;
    let token = load_access_token(&platform)?;
    let client = LinuxHttpSyncClient::new(&server_url, token)?;
    let (recipient_id, recipient_keys) = client.user_public_keys_by_email(recipient_email)?;
    let recipient_public_key = recipient_keys.into_iter().next().ok_or_else(|| {
        PlatformError::OperationFailed("recipient has no registered public key".to_owned())
    })?;
    let owner_private_key = platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    let recipient = core.share_task_with_recipient(
        task_id,
        recipient_id,
        &recipient_public_key,
        &owner_private_key,
    )?;
    client.create_share(task_id, recipient_id, recipient.wrapped_task_key)?;
    Ok(())
}

pub(crate) fn revoke_linux_share(
    core: &TaskManagerCore,
    settings_path: &Path,
    task_id: Uuid,
    recipient_id: Uuid,
) -> CoreResult<()> {
    let settings = read_settings(settings_path).unwrap_or_default();
    let platform = LinuxPlatform::new();
    let server_url = normalize_sync_server_url(&settings.server_url)?;
    let token = load_access_token(&platform)?;
    let client = LinuxHttpSyncClient::new(&server_url, token)?;
    let current_state = core.shared_task_state(task_id)?;
    let mut remaining_public_keys = Vec::new();
    for recipient in current_state
        .active_recipients()
        .filter(|recipient| recipient.recipient_id != recipient_id)
    {
        let public_key = client
            .user_public_keys(recipient.recipient_id)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                PlatformError::OperationFailed(format!(
                    "recipient {} has no registered public key",
                    recipient.recipient_id
                ))
            })?;
        remaining_public_keys.push((recipient.recipient_id, public_key));
    }
    let owner_private_key = platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    let updated_state = core.revoke_shared_task_recipient(
        task_id,
        recipient_id,
        remaining_public_keys,
        &owner_private_key,
    )?;
    for recipient in updated_state.active_recipients() {
        client.create_share(
            task_id,
            recipient.recipient_id,
            recipient.wrapped_task_key.clone(),
        )?;
    }
    client.revoke_share(task_id, recipient_id)?;
    Ok(())
}

pub(crate) fn run_linux_sync(db_path: &Path, settings_path: &Path) -> CoreResult<LinuxSyncSummary> {
    let settings = read_settings(settings_path).unwrap_or_default();
    if !sync_auth_configured_for_settings(&LinuxPlatform::new(), &settings) {
        return Ok(LinuxSyncSummary::default());
    }
    let platform = LinuxPlatform::new();
    let server_url = normalize_sync_server_url(&settings.server_url)?;
    let data_key = platform.load_key(ACCOUNT_DATA_KEY_ID)?;
    let database = LocalDatabase::open(db_path)?;

    let mut summary = match run_linux_sync_once(&database, &platform, &server_url, &data_key) {
        Err(CoreError::Sync(SyncError::AuthExpired)) => {
            refresh_linux_auth(&platform, &server_url)?;
            run_linux_sync_once(&database, &platform, &server_url, &data_key)?
        }
        result => result?,
    };
    summary.pending_retries = TaskManagerCore::open(db_path)?
        .sync_status()?
        .retry_queue_depth;
    Ok(summary)
}

pub(crate) fn run_linux_sync_once(
    database: &LocalDatabase,
    platform: &LinuxPlatform,
    server_url: &str,
    data_key: &[u8],
) -> CoreResult<LinuxSyncSummary> {
    let token = load_access_token(platform)?;
    let client = LinuxHttpSyncClient::new(server_url, token)?;
    accept_share_inbox(database, platform, &client)?;
    let pull = sync_pull(database, &client, data_key)?;
    let push = sync_push(database, platform, &client, data_key)?;
    Ok(LinuxSyncSummary {
        pushed: push.pushed,
        pulled: pull.pulled,
        failed: push.failed,
        pending_retries: 0,
        conflicts: pull.failed,
    })
}

fn accept_share_inbox(
    database: &LocalDatabase,
    platform: &LinuxPlatform,
    client: &LinuxHttpSyncClient,
) -> CoreResult<()> {
    let recipient_private_key = platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    for item in client.share_inbox()? {
        let Some(invite) = item.into_invite() else {
            continue;
        };
        if database.shared_task_state(invite.task_id).is_ok() {
            continue;
        }
        let Some(owner_public_key) = client.user_public_keys(invite.owner_id)?.into_iter().next()
        else {
            continue;
        };
        let task_key = taskmanager_core::unwrap_data_key(
            &invite.wrapped_task_key,
            &owner_public_key,
            &recipient_private_key,
        )?;
        database.accept_shared_task(invite, task_key.to_vec())?;
    }
    Ok(())
}

fn refresh_linux_auth(platform: &LinuxPlatform, server_url: &str) -> CoreResult<()> {
    refresh_auth(platform, &LinuxAuthClient::new(), server_url).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleted_wire_blob_maps_to_core_tombstone() {
        let task_id = Uuid::new_v4();
        let remote = linux_remote_blob_from_wire(LinuxPullWireBlob {
            task_id,
            ciphertext: None,
            nonce: None,
            updated_at: 123,
            deleted: true,
        })
        .expect("deleted wire blobs should be preserved as tombstones");

        assert_eq!(remote.task_id, task_id);
        assert_eq!(remote.updated_at, 123);
        assert!(remote.deleted);
        assert!(remote.blob.is_none());
    }
}
