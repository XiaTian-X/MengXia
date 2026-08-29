//! Pure MengXia domain model boundary.

#![forbid(unsafe_code)]

mod asset;
mod error;

pub use asset::*;
pub use error::DomainError;
