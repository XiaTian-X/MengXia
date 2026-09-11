//! Pure MengXia domain model boundary.

#![forbid(unsafe_code)]

mod asset;
mod creative;
mod error;

pub use asset::*;
pub use creative::*;
pub use error::DomainError;
