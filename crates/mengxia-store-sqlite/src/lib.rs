//! SQLite persistence adapter boundary for MengXia.
//!
//! Runtime integration of the SQLite 3.53.4 pin is owned by TASK-004.

#![forbid(unsafe_code)]

#[allow(dead_code)]
mod bootstrap;
mod config;
mod error;
#[allow(dead_code)]
mod intent;
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod migration;
#[allow(dead_code)]
mod path_authority;
#[allow(dead_code)]
mod runtime;
#[allow(dead_code)]
mod stock_sqlite_open;
mod wal;

pub use config::{ConfigSource, LibraryRoot, ResolvedStoreConfig, StoreConfig};
pub use error::StoreError;
