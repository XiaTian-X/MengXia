//! Closed, capability-free private Plugin protocol for MengXia.

#![forbid(unsafe_code)]

mod codec;
mod wire;

pub use codec::{
    decode_host_envelope, decode_plugin_envelope, encode_host_envelope, encode_plugin_envelope,
    read_host_envelope, read_plugin_envelope, write_host_envelope, write_plugin_envelope,
};
pub use mengxia_framing::FrameLimit;
pub use wire::{DecodeDepth, ProtocolCodecError};

/// Exact TASK-011 private protocol major version.
pub const PROTOCOL_MAJOR: u32 = 1;
/// Exact TASK-011 private protocol minor version.
pub const PROTOCOL_MINOR: u32 = 0;
/// Exact challenge width used to detect crossed or stale private streams.
pub const SESSION_CHALLENGE_BYTES: usize = 32;
/// Protocol v1.0 permits exactly one application request in flight.
pub const MAX_IN_FLIGHT_REQUESTS: u32 = 1;

include!(concat!(env!("OUT_DIR"), "/mengxia.plugin.v1.rs"));
