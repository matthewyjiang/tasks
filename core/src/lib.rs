//! Client core for the local-first encrypted task manager.
//!
//! This crate owns domain types, local persistence, crypto, and sync logic.

pub mod error;
pub mod types;

pub use error::{CoreError, CoreResult, CryptoError, DbError, SyncError};
pub use types::{Blob, SyncResult, Task, TaskFilter, TaskPatch, TaskSort, TaskStatus};
