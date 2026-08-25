//! Bounded framing boundary for MengXia protocols.

#![forbid(unsafe_code)]

use std::fmt;
use std::io::ErrorKind;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Canonical minimum accepted IPC frame limit.
pub const MIN_FRAME_BYTES: u32 = 64 * 1024;
/// Canonical default IPC frame limit.
pub const DEFAULT_FRAME_BYTES: u32 = 4 * 1024 * 1024;
/// Canonical maximum accepted IPC frame limit.
pub const MAX_FRAME_BYTES: u32 = 16 * 1024 * 1024;
/// Exact unsigned big-endian header width.
pub const FRAME_HEADER_BYTES: usize = 4;

/// Validated inclusive payload limit for one framed channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameLimit(u32);

impl FrameLimit {
    /// Validates the canonical 64 KiB through 16 MiB range.
    pub const fn new(bytes: u32) -> Result<Self, FrameError> {
        if bytes < MIN_FRAME_BYTES || bytes > MAX_FRAME_BYTES {
            return Err(FrameError::InvalidLimit);
        }
        Ok(Self(bytes))
    }

    /// Returns the exact inclusive payload limit.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Default for FrameLimit {
    fn default() -> Self {
        Self(DEFAULT_FRAME_BYTES)
    }
}

/// Redacted framing failure that never retains input bytes or OS diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FrameError {
    /// A configured frame limit is outside the canonical range.
    InvalidLimit,
    /// A zero or over-limit payload length was observed.
    InvalidLength,
    /// EOF occurred after a frame began and before it completed.
    Truncated,
    /// The bounded payload allocation could not be reserved.
    AllocationUnavailable,
    /// The underlying local transport failed.
    Transport,
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimit => "invalid frame limit",
            Self::InvalidLength => "invalid frame length",
            Self::Truncated => "truncated frame",
            Self::AllocationUnavailable => "frame allocation unavailable",
            Self::Transport => "frame transport failed",
        })
    }
}

impl std::error::Error for FrameError {}

/// Reads exactly one four-byte-length-prefixed frame.
///
/// Zero and over-limit lengths are rejected before payload reservation. The reader
/// never attempts stream resynchronization after an error.
pub async fn read_frame<R>(reader: &mut R, limit: FrameLimit) -> Result<Vec<u8>, FrameError>
where
    R: AsyncRead + Unpin,
{
    let mut header = [0_u8; FRAME_HEADER_BYTES];
    read_exact(reader, &mut header).await?;
    let length = u32::from_be_bytes(header);
    if length == 0 || length > limit.get() {
        return Err(FrameError::InvalidLength);
    }

    let length = usize::try_from(length).map_err(|_| FrameError::InvalidLength)?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(length)
        .map_err(|_| FrameError::AllocationUnavailable)?;
    payload.resize(length, 0);
    read_exact(reader, &mut payload).await?;
    Ok(payload)
}

/// Writes exactly one four-byte-length-prefixed frame and flushes it.
pub async fn write_frame<W>(
    writer: &mut W,
    payload: &[u8],
    limit: FrameLimit,
) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
{
    if payload.is_empty() || payload.len() > limit.get() as usize {
        return Err(FrameError::InvalidLength);
    }
    let length = u32::try_from(payload.len()).map_err(|_| FrameError::InvalidLength)?;
    writer
        .write_all(&length.to_be_bytes())
        .await
        .map_err(|_| FrameError::Transport)?;
    writer
        .write_all(payload)
        .await
        .map_err(|_| FrameError::Transport)?;
    writer.flush().await.map_err(|_| FrameError::Transport)
}

async fn read_exact<R>(reader: &mut R, bytes: &mut [u8]) -> Result<(), FrameError>
where
    R: AsyncRead + Unpin,
{
    reader.read_exact(bytes).await.map(|_| ()).map_err(|error| {
        if error.kind() == ErrorKind::UnexpectedEof {
            FrameError::Truncated
        } else {
            FrameError::Transport
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_limit_accepts_only_the_canonical_closed_range() {
        assert_eq!(
            FrameLimit::new(MIN_FRAME_BYTES - 1),
            Err(FrameError::InvalidLimit)
        );
        assert_eq!(
            FrameLimit::new(MIN_FRAME_BYTES).map(FrameLimit::get),
            Ok(MIN_FRAME_BYTES)
        );
        assert_eq!(
            FrameLimit::new(MAX_FRAME_BYTES).map(FrameLimit::get),
            Ok(MAX_FRAME_BYTES)
        );
        assert_eq!(
            FrameLimit::new(MAX_FRAME_BYTES + 1),
            Err(FrameError::InvalidLimit)
        );
    }

    #[tokio::test]
    async fn cap_minus_one_and_cap_round_trip_exactly() {
        for length in [MIN_FRAME_BYTES as usize - 1, MIN_FRAME_BYTES as usize] {
            let limit = FrameLimit::new(MIN_FRAME_BYTES).expect("minimum limit is valid");
            let expected = vec![0x5a; length];
            let (mut sender, mut receiver) = tokio::io::duplex(length + FRAME_HEADER_BYTES);
            let send = async { write_frame(&mut sender, &expected, limit).await };
            let receive = async { read_frame(&mut receiver, limit).await };
            let (send_result, receive_result) = tokio::join!(send, receive);
            assert_eq!(send_result, Ok(()));
            assert_eq!(receive_result, Ok(expected));
        }
    }

    #[tokio::test]
    async fn zero_over_limit_and_truncation_fail_closed() {
        let limit = FrameLimit::new(MIN_FRAME_BYTES).expect("minimum limit is valid");
        for header in [0_u32.to_be_bytes(), (MIN_FRAME_BYTES + 1).to_be_bytes()] {
            let mut input = header.as_slice();
            assert_eq!(
                read_frame(&mut input, limit).await,
                Err(FrameError::InvalidLength)
            );
        }

        let bytes = [0, 0, 0, 2, 0xaa];
        let mut truncated = bytes.as_slice();
        assert_eq!(
            read_frame(&mut truncated, limit).await,
            Err(FrameError::Truncated)
        );
        assert_eq!(
            write_frame(&mut tokio::io::sink(), &[], limit).await,
            Err(FrameError::InvalidLength)
        );
    }
}
