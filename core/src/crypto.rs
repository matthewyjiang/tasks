use std::fmt;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use p256::ecdh::diffie_hellman;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::{EncodedPoint, PublicKey, SecretKey};
use rand_core::{OsRng, RngCore};
use sha2::Sha256;

use crate::error::{CoreResult, CryptoError};
use crate::types::{Blob, Task};

const DATA_KEY_LENGTH: usize = 32;
const AES_GCM_NONCE_LENGTH: usize = 12;
const DEK_WRAP_INFO: &[u8] = b"dek-wrap";

#[derive(Clone, PartialEq, Eq)]
pub struct DeviceKeypair {
    pub private_key: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl fmt::Debug for DeviceKeypair {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeviceKeypair")
            .field("private_key", &"<redacted>")
            .field("public_key", &self.public_key)
            .finish()
    }
}

pub fn generate_data_key() -> [u8; DATA_KEY_LENGTH] {
    let mut key = [0_u8; DATA_KEY_LENGTH];
    OsRng.fill_bytes(&mut key);
    key
}

pub fn generate_device_keypair() -> DeviceKeypair {
    let private_key = SecretKey::random(&mut OsRng);
    let public_key = private_key.public_key();

    DeviceKeypair {
        private_key: private_key.to_bytes().to_vec(),
        public_key: public_key.to_encoded_point(false).as_bytes().to_vec(),
    }
}

pub fn public_key_from_private_key(private_key: &[u8]) -> CoreResult<Vec<u8>> {
    let private_key = secret_key_from_bytes(private_key)?;
    Ok(private_key
        .public_key()
        .to_encoded_point(false)
        .as_bytes()
        .to_vec())
}

pub fn encrypt_blob(task: &Task, key: &[u8]) -> CoreResult<Blob> {
    let plaintext = serde_json::to_vec(task)?;
    encrypt_bytes(&plaintext, key)
}

pub fn decrypt_blob(blob: &Blob, key: &[u8]) -> CoreResult<Task> {
    let plaintext = decrypt_bytes(blob, key)?;
    serde_json::from_slice(&plaintext).map_err(|error| CryptoError::DeserFailed(error).into())
}

pub fn wrap_data_key(
    data_key: &[u8],
    peer_public_key: &[u8],
    own_private_key: &[u8],
) -> CoreResult<Blob> {
    validate_data_key(data_key)?;
    let wrap_key = derive_wrap_key(peer_public_key, own_private_key)?;
    encrypt_bytes(data_key, &wrap_key)
}

pub fn unwrap_data_key(
    wrapped: &Blob,
    peer_public_key: &[u8],
    own_private_key: &[u8],
) -> CoreResult<[u8; DATA_KEY_LENGTH]> {
    let wrap_key = derive_wrap_key(peer_public_key, own_private_key)?;
    let data_key = decrypt_bytes(wrapped, &wrap_key)?;
    validate_data_key(&data_key)?;

    let mut fixed_key = [0_u8; DATA_KEY_LENGTH];
    fixed_key.copy_from_slice(&data_key);
    Ok(fixed_key)
}

fn encrypt_bytes(plaintext: &[u8], key: &[u8]) -> CoreResult<Blob> {
    validate_data_key(key)?;

    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::BadKeyLength(key.len()))?;
    let mut nonce = [0_u8; AES_GCM_NONCE_LENGTH];
    OsRng.fill_bytes(&mut nonce);

    let nonce_value = Nonce::from(nonce);
    let ciphertext = cipher
        .encrypt(&nonce_value, plaintext)
        .map_err(|_| CryptoError::DecryptFailed)?;

    Ok(Blob { ciphertext, nonce })
}

fn decrypt_bytes(blob: &Blob, key: &[u8]) -> CoreResult<Vec<u8>> {
    validate_data_key(key)?;

    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::BadKeyLength(key.len()))?;
    let nonce: [u8; AES_GCM_NONCE_LENGTH] = blob
        .nonce
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::DecryptFailed)?;
    let nonce_value = Nonce::from(nonce);
    cipher
        .decrypt(&nonce_value, blob.ciphertext.as_ref())
        .map_err(|_| CryptoError::DecryptFailed.into())
}

fn validate_data_key(key: &[u8]) -> CoreResult<()> {
    if key.len() != DATA_KEY_LENGTH {
        return Err(CryptoError::BadKeyLength(key.len()).into());
    }

    Ok(())
}

fn derive_wrap_key(
    peer_public_key: &[u8],
    own_private_key: &[u8],
) -> CoreResult<[u8; DATA_KEY_LENGTH]> {
    let own_private_key = secret_key_from_bytes(own_private_key)?;
    let peer_public_key = public_key_from_bytes(peer_public_key)?;

    let shared_secret = diffie_hellman(
        own_private_key.to_nonzero_scalar(),
        peer_public_key.as_affine(),
    );

    let raw_secret = shared_secret.raw_secret_bytes();
    let hkdf = Hkdf::<Sha256>::new(None, &raw_secret[..]);
    let mut wrap_key = [0_u8; DATA_KEY_LENGTH];
    hkdf.expand(DEK_WRAP_INFO, &mut wrap_key)
        .map_err(|_| CryptoError::KeyAgreementFailed)?;

    Ok(wrap_key)
}

fn secret_key_from_bytes(bytes: &[u8]) -> CoreResult<SecretKey> {
    SecretKey::from_slice(bytes).map_err(|_| CryptoError::KeyAgreementFailed.into())
}

fn public_key_from_bytes(bytes: &[u8]) -> CoreResult<PublicKey> {
    let encoded_point =
        EncodedPoint::from_bytes(bytes).map_err(|_| CryptoError::KeyAgreementFailed)?;
    PublicKey::from_sec1_bytes(encoded_point.as_bytes())
        .map_err(|_| CryptoError::KeyAgreementFailed.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TaskStatus;
    use uuid::Uuid;

    fn sample_task() -> Task {
        Task {
            id: Uuid::new_v4(),
            title: "Encrypt this task".to_owned(),
            body: "The server must never see this text.".to_owned(),
            due_at: Some(1_717_603_200_000),
            status: TaskStatus::Inbox,
            project_id: None,
            tags: vec!["private".to_owned(), "local-first".to_owned()],
            created_at: 1_717_600_000_000,
            updated_at: 1_717_600_001_000,
            deleted: false,
            dirty: true,
        }
    }

    #[test]
    fn generated_data_key_is_32_bytes() {
        assert_eq!(generate_data_key().len(), 32);
    }

    #[test]
    fn device_keypair_debug_redacts_private_key() {
        let keypair = generate_device_keypair();
        let debug_text = format!("{keypair:?}");
        let private_key_text = format!("{:?}", keypair.private_key);

        assert!(debug_text.contains("<redacted>"));
        assert!(debug_text.contains("public_key"));
        assert!(!debug_text.contains(&private_key_text));
    }

    #[test]
    fn consecutive_data_keys_are_different() {
        assert_ne!(generate_data_key(), generate_data_key());
    }

    #[test]
    fn encrypt_blob_returns_ciphertext_and_12_byte_nonce() {
        let blob = encrypt_blob(&sample_task(), &generate_data_key()).unwrap();

        assert!(!blob.ciphertext.is_empty());
        assert_eq!(blob.nonce.len(), 12);
    }

    #[test]
    fn encrypting_same_task_twice_produces_different_blobs() {
        let task = sample_task();
        let key = generate_data_key();

        let first = encrypt_blob(&task, &key).unwrap();
        let second = encrypt_blob(&task, &key).unwrap();

        assert_ne!(first.nonce, second.nonce);
        assert_ne!(first.ciphertext, second.ciphertext);
    }

    #[test]
    fn decrypt_blob_returns_original_task() {
        let task = sample_task();
        let key = generate_data_key();

        let blob = encrypt_blob(&task, &key).unwrap();
        let decrypted = decrypt_blob(&blob, &key).unwrap();

        assert_eq!(decrypted, task);
    }

    #[test]
    fn encrypt_blob_rejects_bad_key_lengths() {
        let error = encrypt_blob(&sample_task(), &[0_u8; 31]).unwrap_err();
        assert!(matches!(
            error,
            crate::error::CoreError::Crypto(CryptoError::BadKeyLength(31))
        ));
    }

    #[test]
    fn decrypt_blob_rejects_bad_key_lengths() {
        let blob = Blob {
            ciphertext: vec![1, 2, 3],
            nonce: [0; 12],
        };

        let error = decrypt_blob(&blob, &[0_u8; 33]).unwrap_err();
        assert!(matches!(
            error,
            crate::error::CoreError::Crypto(CryptoError::BadKeyLength(33))
        ));
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let blob = encrypt_blob(&sample_task(), &generate_data_key()).unwrap();

        let error = decrypt_blob(&blob, &generate_data_key()).unwrap_err();
        assert!(matches!(
            error,
            crate::error::CoreError::Crypto(CryptoError::DecryptFailed)
        ));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = generate_data_key();
        let mut blob = encrypt_blob(&sample_task(), &key).unwrap();
        blob.ciphertext[0] ^= 1;

        let error = decrypt_blob(&blob, &key).unwrap_err();
        assert!(matches!(
            error,
            crate::error::CoreError::Crypto(CryptoError::DecryptFailed)
        ));
    }

    #[test]
    fn tampered_nonce_fails() {
        let key = generate_data_key();
        let mut blob = encrypt_blob(&sample_task(), &key).unwrap();
        blob.nonce[0] ^= 1;

        let error = decrypt_blob(&blob, &key).unwrap_err();
        assert!(matches!(
            error,
            crate::error::CoreError::Crypto(CryptoError::DecryptFailed)
        ));
    }

    #[test]
    fn device_public_key_can_be_used_for_wrapping() {
        let sender = generate_device_keypair();
        let recipient = generate_device_keypair();
        let data_key = generate_data_key();

        let wrapped = wrap_data_key(&data_key, &recipient.public_key, &sender.private_key).unwrap();
        let unwrapped =
            unwrap_data_key(&wrapped, &sender.public_key, &recipient.private_key).unwrap();

        assert_eq!(unwrapped, data_key);
    }

    #[test]
    fn unwrap_with_wrong_private_key_fails() {
        let sender = generate_device_keypair();
        let recipient = generate_device_keypair();
        let wrong_recipient = generate_device_keypair();
        let data_key = generate_data_key();

        let wrapped = wrap_data_key(&data_key, &recipient.public_key, &sender.private_key).unwrap();
        let error = unwrap_data_key(&wrapped, &sender.public_key, &wrong_recipient.private_key)
            .unwrap_err();

        assert!(matches!(
            error,
            crate::error::CoreError::Crypto(CryptoError::DecryptFailed)
        ));
    }

    #[test]
    fn malformed_public_key_fails_cleanly() {
        let device = generate_device_keypair();
        let data_key = generate_data_key();

        let error = wrap_data_key(&data_key, b"not a public key", &device.private_key).unwrap_err();

        assert!(matches!(
            error,
            crate::error::CoreError::Crypto(CryptoError::KeyAgreementFailed)
        ));
    }
}
