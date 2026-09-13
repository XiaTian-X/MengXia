use std::fmt;

use mengxia_types::ErrorCode;

/// Closed rejection returned by a conforming Plugin for one request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluginRequestFailure {
    UnsupportedRequest,
    InvalidRequest,
    Internal,
}

/// Whether the separately owned Plugin process requires terminal cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminationRequired {
    ProtocolFailure,
    ResourceLimit,
    ShutdownRejected,
    Cancellation,
    InternalFailure,
}

/// Redacted host/session failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluginHostError {
    code: ErrorCode,
    termination: Option<TerminationRequired>,
}

impl PluginHostError {
    pub(crate) const fn new(code: ErrorCode) -> Self {
        Self {
            code,
            termination: None,
        }
    }

    pub(crate) const fn terminal(code: ErrorCode, reason: TerminationRequired) -> Self {
        Self {
            code,
            termination: Some(reason),
        }
    }

    #[must_use]
    pub const fn code(self) -> ErrorCode {
        self.code
    }

    #[must_use]
    pub const fn termination_required(self) -> Option<TerminationRequired> {
        self.termination
    }
}

impl fmt::Display for PluginHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.as_str())
    }
}

impl std::error::Error for PluginHostError {}

/// Result of a legal request/response exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestOutcome<T> {
    Completed(T),
    Rejected(PluginRequestFailure),
}

/// Terminal protocol-session state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionClose {
    CooperativeShutdown,
}
