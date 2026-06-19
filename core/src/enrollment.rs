use crate::auth::{clear_enrollment_pending, AUTH_ENROLLMENT_PENDING_ID};
use crate::crypto::{public_key_from_private_key, unwrap_data_key, wrap_data_key};
use crate::error::{CoreError, CoreResult, PlatformError};
use crate::platform::{Platform, ACCOUNT_DATA_KEY_ID, DEVICE_PRIVATE_KEY_ID};
use crate::types::Blob;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnrollmentState {
    LocalOnlyReady,
    ExistingAccountPending,
    SyncReady,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WrappedAccountDataKeyPayload {
    pub sender_public_key: Vec<u8>,
    pub recipient_public_key: Vec<u8>,
    pub wrapped_account_data_key: Blob,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingEnrollmentRequest {
    pub request_id: String,
    pub recipient_public_key: Vec<u8>,
    pub device_name: String,
    pub platform: String,
    pub created_at: String,
}

pub fn public_key_fingerprint(public_key: &[u8]) -> String {
    public_key
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalDataEnrollmentStrategy {
    RequireEmptyLocalKey,
    MergeLocalData,
    ReplaceLocalData,
}

pub trait EnrollmentClient {
    fn create_request(
        &self,
        public_key: &[u8],
        device_name: &str,
        platform: &str,
    ) -> CoreResult<String>;
    fn list_pending_requests(&self) -> CoreResult<Vec<PendingEnrollmentRequest>>;
    fn approve_request(
        &self,
        request_id: &str,
        payload: &WrappedAccountDataKeyPayload,
    ) -> CoreResult<()>;
    fn reject_request(&self, request_id: &str) -> CoreResult<()>;
    fn approved_payload(
        &self,
        public_key: &[u8],
    ) -> CoreResult<Option<WrappedAccountDataKeyPayload>>;
}

pub fn existing_account_enrollment_state(platform: &dyn Platform) -> EnrollmentState {
    match (
        platform.load_key(DEVICE_PRIVATE_KEY_ID),
        platform.load_key(ACCOUNT_DATA_KEY_ID),
    ) {
        (Ok(_), Ok(_)) if platform.load_key(AUTH_ENROLLMENT_PENDING_ID).is_err() => {
            EnrollmentState::SyncReady
        }
        (Ok(_), _) => EnrollmentState::ExistingAccountPending,
        (Err(_), _) => EnrollmentState::LocalOnlyReady,
    }
}

pub fn begin_existing_account_enrollment(platform: &dyn Platform) -> CoreResult<EnrollmentState> {
    platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    if platform.load_key(ACCOUNT_DATA_KEY_ID).is_ok()
        && platform.load_key(AUTH_ENROLLMENT_PENDING_ID).is_err()
    {
        Ok(EnrollmentState::SyncReady)
    } else {
        platform.store_key(AUTH_ENROLLMENT_PENDING_ID, b"true")?;
        Ok(EnrollmentState::ExistingAccountPending)
    }
}

pub fn announce_existing_account_enrollment(
    platform: &dyn Platform,
    client: &dyn EnrollmentClient,
    device_name: &str,
    platform_name: &str,
) -> CoreResult<String> {
    platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    if platform.load_key(ACCOUNT_DATA_KEY_ID).is_ok()
        && platform.load_key(AUTH_ENROLLMENT_PENDING_ID).is_err()
    {
        return Ok(String::new());
    }
    let private_key = platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    let public_key = public_key_from_private_key(&private_key)?;
    let request_id = client.create_request(&public_key, device_name, platform_name)?;
    platform.store_key(AUTH_ENROLLMENT_PENDING_ID, b"true")?;
    Ok(request_id)
}

pub fn approve_pending_enrollment_request(
    platform: &dyn Platform,
    client: &dyn EnrollmentClient,
    request: &PendingEnrollmentRequest,
) -> CoreResult<()> {
    if existing_account_enrollment_state(platform) != EnrollmentState::SyncReady {
        return Err(PlatformError::OperationFailed(
            "only a sync-ready enrolled device can approve enrollment requests".to_owned(),
        )
        .into());
    }
    let account_data_key = platform.load_key(ACCOUNT_DATA_KEY_ID)?;
    let sender_private_key = platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    let payload = create_wrapped_account_data_key_payload(
        &account_data_key,
        &request.recipient_public_key,
        &sender_private_key,
    )?;
    client.approve_request(&request.request_id, &payload)
}

pub fn complete_pending_enrollment(
    platform: &dyn Platform,
    client: &dyn EnrollmentClient,
) -> CoreResult<EnrollmentState> {
    complete_pending_enrollment_with_strategy(
        platform,
        client,
        LocalDataEnrollmentStrategy::RequireEmptyLocalKey,
    )
}

pub fn complete_pending_enrollment_with_strategy(
    platform: &dyn Platform,
    client: &dyn EnrollmentClient,
    strategy: LocalDataEnrollmentStrategy,
) -> CoreResult<EnrollmentState> {
    match approved_payload_for_current_device(platform, client)? {
        Some(payload) => {
            accept_wrapped_account_data_key_payload_with_strategy(platform, &payload, strategy)
        }
        None => Ok(existing_account_enrollment_state(platform)),
    }
}

pub fn approved_payload_for_current_device(
    platform: &dyn Platform,
    client: &dyn EnrollmentClient,
) -> CoreResult<Option<WrappedAccountDataKeyPayload>> {
    let private_key = platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    let public_key = public_key_from_private_key(&private_key)?;
    client.approved_payload(&public_key)
}

pub fn create_wrapped_account_data_key_payload(
    account_data_key: &[u8],
    recipient_public_key: &[u8],
    sender_private_key: &[u8],
) -> CoreResult<WrappedAccountDataKeyPayload> {
    let sender_public_key = public_key_from_private_key(sender_private_key)?;
    Ok(WrappedAccountDataKeyPayload {
        sender_public_key,
        recipient_public_key: recipient_public_key.to_vec(),
        wrapped_account_data_key: wrap_data_key(
            account_data_key,
            recipient_public_key,
            sender_private_key,
        )?,
    })
}

pub fn accept_wrapped_account_data_key_payload(
    platform: &dyn Platform,
    payload: &WrappedAccountDataKeyPayload,
) -> CoreResult<EnrollmentState> {
    accept_wrapped_account_data_key_payload_with_strategy(
        platform,
        payload,
        LocalDataEnrollmentStrategy::RequireEmptyLocalKey,
    )
}

pub fn accept_wrapped_account_data_key_payload_with_strategy(
    platform: &dyn Platform,
    payload: &WrappedAccountDataKeyPayload,
    strategy: LocalDataEnrollmentStrategy,
) -> CoreResult<EnrollmentState> {
    let account_data_key =
        unwrap_pending_account_data_key_with_strategy(platform, payload, strategy)?;
    store_completed_account_data_key(platform, &account_data_key)
}

pub fn unwrap_pending_account_data_key_with_strategy(
    platform: &dyn Platform,
    payload: &WrappedAccountDataKeyPayload,
    strategy: LocalDataEnrollmentStrategy,
) -> CoreResult<Vec<u8>> {
    if let Err(error) = platform.load_key(AUTH_ENROLLMENT_PENDING_ID) {
        match error {
            CoreError::Platform(PlatformError::KeyNotFound(_)) => {
                return Err(PlatformError::OperationFailed(
                    "enrollment is not pending; refusing wrapped account data key".to_owned(),
                )
                .into());
            }
            error => return Err(error),
        }
    }
    match (platform.load_key(ACCOUNT_DATA_KEY_ID), strategy) {
        (Ok(_), LocalDataEnrollmentStrategy::RequireEmptyLocalKey) => {
            return Err(PlatformError::OperationFailed(
                "local account data key already exists; choose merge local data or replace local data to continue enrollment".to_owned(),
            )
            .into());
        }
        (
            Ok(_),
            LocalDataEnrollmentStrategy::MergeLocalData
            | LocalDataEnrollmentStrategy::ReplaceLocalData,
        ) => {}
        (Err(CoreError::Platform(PlatformError::KeyNotFound(_))), _) => {}
        (Err(error), _) => return Err(error),
    }
    let recipient_private_key = platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    let recipient_public_key = public_key_from_private_key(&recipient_private_key)?;
    if recipient_public_key != payload.recipient_public_key {
        return Err(PlatformError::OperationFailed(
            "wrapped account data key is not addressed to this device".to_owned(),
        )
        .into());
    }
    unwrap_data_key(
        &payload.wrapped_account_data_key,
        &payload.sender_public_key,
        &recipient_private_key,
    )
    .map(|key| key.to_vec())
}

pub fn store_completed_account_data_key(
    platform: &dyn Platform,
    account_data_key: &[u8],
) -> CoreResult<EnrollmentState> {
    platform.store_key(ACCOUNT_DATA_KEY_ID, account_data_key)?;
    clear_enrollment_pending(platform)?;
    Ok(EnrollmentState::SyncReady)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{generate_data_key, generate_device_keypair};
    use crate::platform::{init_device_keypair, MockPlatform};
    use std::sync::Mutex;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct CreatedEnrollmentRequest {
        public_key: Vec<u8>,
        device_name: String,
        platform: String,
    }

    #[derive(Default)]
    struct FakeEnrollmentClient {
        request_id: Mutex<Option<String>>,
        create_error: Mutex<Option<String>>,
        created: Mutex<Vec<CreatedEnrollmentRequest>>,
        pending: Mutex<Vec<PendingEnrollmentRequest>>,
        approved: Mutex<Option<WrappedAccountDataKeyPayload>>,
        rejected: Mutex<Vec<String>>,
    }

    impl EnrollmentClient for FakeEnrollmentClient {
        fn create_request(
            &self,
            public_key: &[u8],
            device_name: &str,
            platform: &str,
        ) -> CoreResult<String> {
            if let Some(message) = self.create_error.lock().unwrap().take() {
                return Err(PlatformError::OperationFailed(message).into());
            }
            self.created.lock().unwrap().push(CreatedEnrollmentRequest {
                public_key: public_key.to_vec(),
                device_name: device_name.to_owned(),
                platform: platform.to_owned(),
            });
            Ok(self
                .request_id
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| "request-1".to_owned()))
        }

        fn list_pending_requests(&self) -> CoreResult<Vec<PendingEnrollmentRequest>> {
            Ok(self.pending.lock().unwrap().clone())
        }

        fn approve_request(
            &self,
            _request_id: &str,
            payload: &WrappedAccountDataKeyPayload,
        ) -> CoreResult<()> {
            *self.approved.lock().unwrap() = Some(payload.clone());
            Ok(())
        }

        fn reject_request(&self, request_id: &str) -> CoreResult<()> {
            self.rejected.lock().unwrap().push(request_id.to_owned());
            Ok(())
        }

        fn approved_payload(
            &self,
            _public_key: &[u8],
        ) -> CoreResult<Option<WrappedAccountDataKeyPayload>> {
            Ok(self.approved.lock().unwrap().clone())
        }
    }

    #[test]
    fn existing_account_without_data_key_is_pending_and_does_not_create_key() {
        let platform = MockPlatform::new();
        init_device_keypair(&platform).unwrap();
        assert_eq!(
            begin_existing_account_enrollment(&platform).unwrap(),
            EnrollmentState::ExistingAccountPending
        );
        assert!(platform.load_key(ACCOUNT_DATA_KEY_ID).is_err());
        assert_eq!(
            platform.load_key(AUTH_ENROLLMENT_PENDING_ID).unwrap(),
            b"true"
        );
    }

    #[test]
    fn accepting_wrapped_payload_stores_account_data_key() {
        let sender = generate_device_keypair();
        let recipient = generate_device_keypair();
        let account_data_key = generate_data_key();
        let payload = create_wrapped_account_data_key_payload(
            &account_data_key,
            &recipient.public_key,
            &sender.private_key,
        )
        .unwrap();
        let platform = MockPlatform::new();
        platform
            .store_key(DEVICE_PRIVATE_KEY_ID, &recipient.private_key)
            .unwrap();
        platform
            .store_key(AUTH_ENROLLMENT_PENDING_ID, b"true")
            .unwrap();

        assert_eq!(
            accept_wrapped_account_data_key_payload(&platform, &payload).unwrap(),
            EnrollmentState::SyncReady
        );
        assert_eq!(
            platform.load_key(ACCOUNT_DATA_KEY_ID).unwrap(),
            account_data_key.to_vec()
        );
    }

    #[test]
    fn accepting_wrapped_payload_fails_without_replacing_existing_account_data_key() {
        let sender = generate_device_keypair();
        let recipient = generate_device_keypair();
        let existing_account_data_key = generate_data_key();
        let replacement_account_data_key = generate_data_key();
        let payload = create_wrapped_account_data_key_payload(
            &replacement_account_data_key,
            &recipient.public_key,
            &sender.private_key,
        )
        .unwrap();
        let platform = MockPlatform::new();
        platform
            .store_key(DEVICE_PRIVATE_KEY_ID, &recipient.private_key)
            .unwrap();
        platform
            .store_key(ACCOUNT_DATA_KEY_ID, &existing_account_data_key)
            .unwrap();

        assert!(accept_wrapped_account_data_key_payload(&platform, &payload).is_err());
        assert_eq!(
            platform.load_key(ACCOUNT_DATA_KEY_ID).unwrap(),
            existing_account_data_key.to_vec()
        );
    }

    #[test]
    fn accepting_wrapped_payload_refuses_to_replace_local_key_even_when_pending() {
        let sender = generate_device_keypair();
        let recipient = generate_device_keypair();
        let local_bootstrap_key = generate_data_key();
        let wrapped_account_data_key = generate_data_key();
        let payload = create_wrapped_account_data_key_payload(
            &wrapped_account_data_key,
            &recipient.public_key,
            &sender.private_key,
        )
        .unwrap();
        let platform = MockPlatform::new();
        platform
            .store_key(DEVICE_PRIVATE_KEY_ID, &recipient.private_key)
            .unwrap();
        platform
            .store_key(ACCOUNT_DATA_KEY_ID, &local_bootstrap_key)
            .unwrap();
        platform
            .store_key(AUTH_ENROLLMENT_PENDING_ID, b"true")
            .unwrap();

        assert!(accept_wrapped_account_data_key_payload(&platform, &payload).is_err());
        assert_eq!(
            platform.load_key(ACCOUNT_DATA_KEY_ID).unwrap(),
            local_bootstrap_key.to_vec()
        );
        assert!(platform.load_key(AUTH_ENROLLMENT_PENDING_ID).is_ok());
    }

    #[test]
    fn accepting_wrapped_payload_with_explicit_merge_strategy_replaces_local_key() {
        let sender = generate_device_keypair();
        let recipient = generate_device_keypair();
        let local_bootstrap_key = generate_data_key();
        let wrapped_account_data_key = generate_data_key();
        let payload = create_wrapped_account_data_key_payload(
            &wrapped_account_data_key,
            &recipient.public_key,
            &sender.private_key,
        )
        .unwrap();
        let platform = MockPlatform::new();
        platform
            .store_key(DEVICE_PRIVATE_KEY_ID, &recipient.private_key)
            .unwrap();
        platform
            .store_key(ACCOUNT_DATA_KEY_ID, &local_bootstrap_key)
            .unwrap();
        platform
            .store_key(AUTH_ENROLLMENT_PENDING_ID, b"true")
            .unwrap();

        assert_eq!(
            accept_wrapped_account_data_key_payload_with_strategy(
                &platform,
                &payload,
                LocalDataEnrollmentStrategy::MergeLocalData,
            )
            .unwrap(),
            EnrollmentState::SyncReady
        );
        assert_eq!(
            platform.load_key(ACCOUNT_DATA_KEY_ID).unwrap(),
            wrapped_account_data_key.to_vec()
        );
        assert!(platform.load_key(AUTH_ENROLLMENT_PENDING_ID).is_err());
    }

    #[test]
    fn announce_existing_account_enrollment_creates_request_and_marks_pending() {
        let platform = MockPlatform::new();
        let public_key = init_device_keypair(&platform).unwrap();
        let client = FakeEnrollmentClient::default();

        assert_eq!(
            announce_existing_account_enrollment(&platform, &client, "laptop", "linux").unwrap(),
            "request-1"
        );

        assert_eq!(
            platform.load_key(AUTH_ENROLLMENT_PENDING_ID).unwrap(),
            b"true".to_vec()
        );
        assert_eq!(
            client.created.lock().unwrap().as_slice(),
            &[CreatedEnrollmentRequest {
                public_key,
                device_name: "laptop".to_owned(),
                platform: "linux".to_owned(),
            }]
        );
    }

    #[test]
    fn failed_announce_does_not_mark_enrollment_pending() {
        let platform = MockPlatform::new();
        init_device_keypair(&platform).unwrap();
        let client = FakeEnrollmentClient::default();
        *client.create_error.lock().unwrap() = Some("network unavailable".to_owned());

        assert!(
            announce_existing_account_enrollment(&platform, &client, "laptop", "linux").is_err()
        );
        assert!(platform.load_key(AUTH_ENROLLMENT_PENDING_ID).is_err());
    }

    #[test]
    fn sync_ready_announce_does_not_create_request_or_mark_pending() {
        let platform = MockPlatform::new();
        init_device_keypair(&platform).unwrap();
        platform
            .store_key(ACCOUNT_DATA_KEY_ID, &generate_data_key())
            .unwrap();
        let client = FakeEnrollmentClient::default();

        assert_eq!(
            announce_existing_account_enrollment(&platform, &client, "laptop", "linux").unwrap(),
            ""
        );
        assert!(client.created.lock().unwrap().is_empty());
        assert!(platform.load_key(AUTH_ENROLLMENT_PENDING_ID).is_err());
        assert_eq!(
            existing_account_enrollment_state(&platform),
            EnrollmentState::SyncReady
        );
    }

    #[test]
    fn pending_device_cannot_approve_enrollment_request() {
        let pending_platform = MockPlatform::new();
        let recipient_public_key = init_device_keypair(&pending_platform).unwrap();
        pending_platform
            .store_key(ACCOUNT_DATA_KEY_ID, &generate_data_key())
            .unwrap();
        pending_platform
            .store_key(AUTH_ENROLLMENT_PENDING_ID, b"true")
            .unwrap();
        let client = FakeEnrollmentClient::default();
        let request = PendingEnrollmentRequest {
            request_id: "request-1".to_owned(),
            recipient_public_key,
            device_name: "laptop".to_owned(),
            platform: "linux".to_owned(),
            created_at: "now".to_owned(),
        };

        assert!(approve_pending_enrollment_request(&pending_platform, &client, &request).is_err());
        assert!(client.approved.lock().unwrap().is_none());
    }

    #[test]
    fn announced_approved_payload_completes_existing_account_enrollment() {
        let enrolled_platform = MockPlatform::new();
        let recipient_platform = MockPlatform::new();
        let recipient_public_key = init_device_keypair(&recipient_platform).unwrap();
        let account_data_key = generate_data_key();
        let sender = generate_device_keypair();
        enrolled_platform
            .store_key(DEVICE_PRIVATE_KEY_ID, &sender.private_key)
            .unwrap();
        enrolled_platform
            .store_key(ACCOUNT_DATA_KEY_ID, &account_data_key)
            .unwrap();
        let client = FakeEnrollmentClient::default();
        let request = PendingEnrollmentRequest {
            request_id: "request-1".to_owned(),
            recipient_public_key,
            device_name: "laptop".to_owned(),
            platform: "linux".to_owned(),
            created_at: "now".to_owned(),
        };

        approve_pending_enrollment_request(&enrolled_platform, &client, &request).unwrap();
        recipient_platform
            .store_key(AUTH_ENROLLMENT_PENDING_ID, b"true")
            .unwrap();

        assert_eq!(
            complete_pending_enrollment(&recipient_platform, &client).unwrap(),
            EnrollmentState::SyncReady
        );
        assert_eq!(
            recipient_platform.load_key(ACCOUNT_DATA_KEY_ID).unwrap(),
            account_data_key.to_vec()
        );
        assert!(recipient_platform
            .load_key(AUTH_ENROLLMENT_PENDING_ID)
            .is_err());
    }

    #[test]
    fn accepting_payload_for_another_device_fails_without_storing_key() {
        let sender = generate_device_keypair();
        let intended_recipient = generate_device_keypair();
        let wrong_recipient = generate_device_keypair();
        let payload = create_wrapped_account_data_key_payload(
            &generate_data_key(),
            &intended_recipient.public_key,
            &sender.private_key,
        )
        .unwrap();
        let platform = MockPlatform::new();
        platform
            .store_key(DEVICE_PRIVATE_KEY_ID, &wrong_recipient.private_key)
            .unwrap();

        assert!(accept_wrapped_account_data_key_payload(&platform, &payload).is_err());
        assert!(platform.load_key(ACCOUNT_DATA_KEY_ID).is_err());
    }
}
