//! Shared, dependency-neutral value contracts for MengXia.

#![forbid(unsafe_code)]

mod digest;
mod error;
mod id;
mod revision;
mod timestamp;

pub use digest::Sha256Digest;
pub use error::{ErrorCode, IdGenerationError, RevisionOverflow, ValueError};
pub use id::Id;
pub use revision::RevisionNo;
pub use timestamp::Timestamp;
