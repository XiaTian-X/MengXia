use mengxia_framing::{FrameError, FrameLimit, read_frame, write_frame};
use prost::Message;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::wire::{MessageKind, validate_message};
use crate::{
    DecodeDepth, HostEnvelope, PluginEnvelope, ProtocolCodecError, SESSION_CHALLENGE_BYTES,
    host_envelope, plugin_envelope,
};

pub fn decode_host_envelope(
    payload: &[u8],
    depth: DecodeDepth,
) -> Result<HostEnvelope, ProtocolCodecError> {
    validate_message(payload, MessageKind::HostEnvelope, depth)?;
    let envelope = HostEnvelope::decode(payload).map_err(|_| ProtocolCodecError::Truncated)?;
    validate_host_semantics(&envelope)?;
    Ok(envelope)
}

pub fn decode_plugin_envelope(
    payload: &[u8],
    depth: DecodeDepth,
) -> Result<PluginEnvelope, ProtocolCodecError> {
    validate_message(payload, MessageKind::PluginEnvelope, depth)?;
    let envelope = PluginEnvelope::decode(payload).map_err(|_| ProtocolCodecError::Truncated)?;
    validate_plugin_semantics(&envelope)?;
    Ok(envelope)
}

#[must_use]
pub fn encode_host_envelope(envelope: &HostEnvelope) -> Vec<u8> {
    envelope.encode_to_vec()
}

#[must_use]
pub fn encode_plugin_envelope(envelope: &PluginEnvelope) -> Vec<u8> {
    envelope.encode_to_vec()
}

pub async fn read_host_envelope<R: AsyncRead + Unpin>(
    reader: &mut R,
    frame_limit: FrameLimit,
    depth: DecodeDepth,
) -> Result<HostEnvelope, ProtocolCodecError> {
    let payload = read_frame(reader, frame_limit)
        .await
        .map_err(map_frame_error)?;
    decode_host_envelope(&payload, depth)
}

pub async fn read_plugin_envelope<R: AsyncRead + Unpin>(
    reader: &mut R,
    frame_limit: FrameLimit,
    depth: DecodeDepth,
) -> Result<PluginEnvelope, ProtocolCodecError> {
    let payload = read_frame(reader, frame_limit)
        .await
        .map_err(map_frame_error)?;
    decode_plugin_envelope(&payload, depth)
}

pub async fn write_host_envelope<W: AsyncWrite + Unpin>(
    writer: &mut W,
    envelope: &HostEnvelope,
    frame_limit: FrameLimit,
) -> Result<(), ProtocolCodecError> {
    write_frame(writer, &envelope.encode_to_vec(), frame_limit)
        .await
        .map_err(map_outbound_frame_error)
}

pub async fn write_plugin_envelope<W: AsyncWrite + Unpin>(
    writer: &mut W,
    envelope: &PluginEnvelope,
    frame_limit: FrameLimit,
) -> Result<(), ProtocolCodecError> {
    write_frame(writer, &envelope.encode_to_vec(), frame_limit)
        .await
        .map_err(map_outbound_frame_error)
}

fn validate_host_semantics(envelope: &HostEnvelope) -> Result<(), ProtocolCodecError> {
    match envelope.body.as_ref() {
        Some(host_envelope::Body::Hello(hello))
            if hello.session_challenge.len() == SESSION_CHALLENGE_BYTES
                && hello.max_frame_bytes != 0
                && hello.max_in_flight_requests != 0 =>
        {
            Ok(())
        }
        Some(host_envelope::Body::Ping(_)) | Some(host_envelope::Body::Shutdown(_)) => Ok(()),
        _ => Err(ProtocolCodecError::InvalidSemanticValue),
    }
}

fn validate_plugin_semantics(envelope: &PluginEnvelope) -> Result<(), ProtocolCodecError> {
    match envelope.body.as_ref() {
        Some(plugin_envelope::Body::Hello(hello))
            if hello.session_challenge.len() == SESSION_CHALLENGE_BYTES =>
        {
            Ok(())
        }
        Some(plugin_envelope::Body::Ping(_)) | Some(plugin_envelope::Body::Shutdown(_)) => Ok(()),
        Some(plugin_envelope::Body::Failure(failure)) if (1..=3).contains(&failure.code) => Ok(()),
        _ => Err(ProtocolCodecError::InvalidSemanticValue),
    }
}

fn map_frame_error(error: FrameError) -> ProtocolCodecError {
    match error {
        FrameError::InvalidLimit => ProtocolCodecError::InvalidLimit,
        FrameError::InvalidLength => ProtocolCodecError::InvalidFrameLength,
        FrameError::Truncated => ProtocolCodecError::Transport,
        FrameError::AllocationUnavailable => ProtocolCodecError::AllocationUnavailable,
        FrameError::Transport => ProtocolCodecError::Transport,
        _ => ProtocolCodecError::Transport,
    }
}

fn map_outbound_frame_error(error: FrameError) -> ProtocolCodecError {
    match error {
        FrameError::Transport => ProtocolCodecError::Transport,
        FrameError::AllocationUnavailable => ProtocolCodecError::AllocationUnavailable,
        FrameError::InvalidLimit | FrameError::InvalidLength | FrameError::Truncated => {
            ProtocolCodecError::InvalidLimit
        }
        _ => ProtocolCodecError::InvalidLimit,
    }
}
