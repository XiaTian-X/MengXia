//! Plugin process host boundary for MengXia.

#![forbid(unsafe_code)]

mod error;
mod limits;
mod session;

pub use error::{
    PluginHostError, PluginRequestFailure, RequestOutcome, SessionClose, TerminationRequired,
};
pub use limits::{
    DEFAULT_ACTIVE_SESSIONS, DEFAULT_CONTROL_FRAME_BYTES, DEFAULT_DECODE_DEPTH,
    DEFAULT_HANDSHAKE_TIMEOUT, DEFAULT_INBOUND_FRAMES, DEFAULT_OUTBOUND_FRAMES,
    DEFAULT_REQUEST_TIMEOUT, DEFAULT_SHUTDOWN_TIMEOUT, DEFAULT_STDERR_TOTAL_BYTES,
    MAX_AGGREGATE_PAYLOAD_BYTES, PluginHostLimits, STDERR_BUFFER_BYTES,
};
pub use session::{ExpectedPluginSession, PluginHostAdmission, PluginSession, SessionDriver};
