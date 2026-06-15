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

pub fn existing_account_enrollment_state(platform: &dyn Platform) -> EnrollmentState {
    match (
        platform.load_key(DEVICE_PRIVATE_KEY_ID),
        platform.load_key(ACCOUNT_DATA_KEY_ID),
    ) {
        (Ok(_), Ok(_)) => EnrollmentState::SyncReady,
        (Ok(_), Err(_)) => EnrollmentState::ExistingAccountPending,
        (Err(_), _) => EnrollmentState::LocalOnlyReady,
    }
}

pub fn begin_existing_account_enrollment(platform: &dyn Platform) -> CoreResult<EnrollmentState> {
    platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    if platform.load_key(ACCOUNT_DATA_KEY_ID).is_ok() {
        Ok(EnrollmentState::SyncReady)
    } else {
        Ok(EnrollmentState::ExistingAccountPending)
    }
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
    match platform.load_key(ACCOUNT_DATA_KEY_ID) {
        Ok(_) => {
            return Err(PlatformError::OperationFailed(
                "account data key already exists; refusing to replace it".to_owned(),
            )
            .into());
        }
        Err(CoreError::Platform(PlatformError::KeyNotFound(_))) => {}
        Err(error) => return Err(error),
    }
    let recipient_private_key = platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    let recipient_public_key = public_key_from_private_key(&recipient_private_key)?;
    if recipient_public_key != payload.recipient_public_key {
        return Err(PlatformError::OperationFailed(
            "wrapped account data key is not addressed to this device".to_owned(),
        )
        .into());
    }
    let account_data_key = unwrap_data_key(
        &payload.wrapped_account_data_key,
        &payload.sender_public_key,
        &recipient_private_key,
    )?;
    platform.store_key(ACCOUNT_DATA_KEY_ID, &account_data_key)?;
    Ok(EnrollmentState::SyncReady)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{generate_data_key, generate_device_keypair};
    use crate::platform::{init_device_keypair, MockPlatform};

    #[test]
    fn existing_account_without_data_key_is_pending_and_does_not_create_key() {
        let platform = MockPlatform::new();
        init_device_keypair(&platform).unwrap();
        assert_eq!(
            begin_existing_account_enrollment(&platform).unwrap(),
            EnrollmentState::ExistingAccountPending
        );
        assert!(platform.load_key(ACCOUNT_DATA_KEY_ID).is_err());
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
