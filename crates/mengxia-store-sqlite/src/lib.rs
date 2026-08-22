//! SQLite persistence adapter boundary for MengXia.
//!
//! Runtime integration of the SQLite 3.53.4 pin is owned by TASK-004.

#![forbid(unsafe_code)]

mod config;
mod error;
// Composed into the public opened-Library context by the next TASK-004 path/lock slice.
#[allow(dead_code)]
mod migration;
#[allow(dead_code)]
mod runtime;

pub use config::{ConfigSource, LibraryRoot, ResolvedStoreConfig, StoreConfig};
pub use error::StoreError;
