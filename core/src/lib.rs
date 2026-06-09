//! Client core for the local-first encrypted task manager.
//!
//! This crate owns domain types, local persistence, crypto, and sync logic.

pub mod crypto;
pub mod db;
pub mod error;
pub mod types;

pub use crypto::{
    decrypt_blob, encrypt_blob, generate_data_key, generate_device_keypair, unwrap_data_key,
    wrap_data_key, DeviceKeypair,
};
pub use db::LocalDatabase;
pub use error::{CoreError, CoreResult, CryptoError, DbError, SyncError};
pub use types::{Blob, SyncResult, Task, TaskFilter, TaskPatch, TaskSort, TaskStatus};
