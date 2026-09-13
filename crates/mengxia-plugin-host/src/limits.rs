use std::time::Duration;

use mengxia_plugin_proto::{DecodeDepth, FrameLimit};
use mengxia_types::ErrorCode;

use crate::PluginHostError;

pub const DEFAULT_CONTROL_FRAME_BYTES: u32 = 262_144;
pub const DEFAULT_DECODE_DEPTH: u8 = 16;
pub const DEFAULT_INBOUND_FRAMES: usize = 16;
pub const DEFAULT_OUTBOUND_FRAMES: usize = 16;
pub const DEFAULT_ACTIVE_SESSIONS: usize = 4;
pub const STDERR_BUFFER_BYTES: usize = 8_192;
pub const DEFAULT_STDERR_TOTAL_BYTES: usize = 1_048_576;
pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(5_000);
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_millis(30_000);
pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(2_000);
pub const MAX_AGGREGATE_PAYLOAD_BYTES: usize = 37_781_504;

/// Complete immutable limits for the TASK-011 private Plugin host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluginHostLimits {
    frame_limit: FrameLimit,
    decode_depth: DecodeDepth,
    inbound_frames: usize,
    outbound_frames: usize,
    active_sessions: usize,
    stderr_total_bytes: usize,
    handshake_timeout: Duration,
    request_timeout: Duration,
    shutdown_timeout: Duration,
    aggregate_payload_bytes: usize,
}

impl PluginHostLimits {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        control_frame_bytes: u32,
        decode_depth: u8,
        inbound_frames: usize,
        outbound_frames: usize,
        active_sessions: usize,
        stderr_total_bytes: usize,
        handshake_timeout: Duration,
        request_timeout: Duration,
        shutdown_timeout: Duration,
    ) -> Result<Self, PluginHostError> {
        if !(65_536..=DEFAULT_CONTROL_FRAME_BYTES).contains(&control_frame_bytes)
            || !(1..=DEFAULT_INBOUND_FRAMES).contains(&inbound_frames)
            || !(1..=DEFAULT_OUTBOUND_FRAMES).contains(&outbound_frames)
            || !(1..=DEFAULT_ACTIVE_SESSIONS).contains(&active_sessions)
            || !(65_536..=DEFAULT_STDERR_TOTAL_BYTES).contains(&stderr_total_bytes)
            || !duration_in_range(handshake_timeout, DEFAULT_HANDSHAKE_TIMEOUT)
            || !duration_in_range(request_timeout, DEFAULT_REQUEST_TIMEOUT)
            || !duration_in_range(shutdown_timeout, DEFAULT_SHUTDOWN_TIMEOUT)
        {
            return Err(PluginHostError::new(ErrorCode::ValidationError));
        }
        let frame_limit = FrameLimit::new(control_frame_bytes)
            .map_err(|_| PluginHostError::new(ErrorCode::ValidationError))?;
        let decode_depth = DecodeDepth::new(decode_depth)
            .map_err(|_| PluginHostError::new(ErrorCode::ValidationError))?;
        let queued = inbound_frames
            .checked_add(outbound_frames)
            .and_then(|frames| frames.checked_add(4))
            .and_then(|frames| frames.checked_mul(control_frame_bytes as usize))
            .and_then(|bytes| bytes.checked_add(STDERR_BUFFER_BYTES))
            .and_then(|bytes| bytes.checked_mul(active_sessions))
            .ok_or_else(|| PluginHostError::new(ErrorCode::ValidationError))?;
        if queued > MAX_AGGREGATE_PAYLOAD_BYTES {
            return Err(PluginHostError::new(ErrorCode::ValidationError));
        }
        Ok(Self {
            frame_limit,
            decode_depth,
            inbound_frames,
            outbound_frames,
            active_sessions,
            stderr_total_bytes,
            handshake_timeout,
            request_timeout,
            shutdown_timeout,
            aggregate_payload_bytes: queued,
        })
    }

    #[must_use]
    pub const fn frame_limit(self) -> FrameLimit {
        self.frame_limit
    }

    #[must_use]
    pub const fn decode_depth(self) -> DecodeDepth {
        self.decode_depth
    }

    #[must_use]
    pub const fn inbound_frames(self) -> usize {
        self.inbound_frames
    }

    #[must_use]
    pub const fn outbound_frames(self) -> usize {
        self.outbound_frames
    }

    #[must_use]
    pub const fn active_sessions(self) -> usize {
        self.active_sessions
    }

    #[must_use]
    pub const fn stderr_total_bytes(self) -> usize {
        self.stderr_total_bytes
    }

    #[must_use]
    pub const fn handshake_timeout(self) -> Duration {
        self.handshake_timeout
    }

    #[must_use]
    pub const fn request_timeout(self) -> Duration {
        self.request_timeout
    }

    #[must_use]
    pub const fn shutdown_timeout(self) -> Duration {
        self.shutdown_timeout
    }

    #[must_use]
    pub const fn aggregate_payload_bytes(self) -> usize {
        self.aggregate_payload_bytes
    }
}

impl Default for PluginHostLimits {
    fn default() -> Self {
        Self::new(
            DEFAULT_CONTROL_FRAME_BYTES,
            DEFAULT_DECODE_DEPTH,
            DEFAULT_INBOUND_FRAMES,
            DEFAULT_OUTBOUND_FRAMES,
            DEFAULT_ACTIVE_SESSIONS,
            DEFAULT_STDERR_TOTAL_BYTES,
            DEFAULT_HANDSHAKE_TIMEOUT,
            DEFAULT_REQUEST_TIMEOUT,
            DEFAULT_SHUTDOWN_TIMEOUT,
        )
        .expect("canonical Plugin host defaults are valid")
    }
}

fn duration_in_range(value: Duration, maximum: Duration) -> bool {
    value >= Duration::from_millis(100) && value <= maximum
}
