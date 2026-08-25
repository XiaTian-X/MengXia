//! Core protocol boundary for MengXia.

#![forbid(unsafe_code)]

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use mengxia_framing::{FrameError, FrameLimit, read_frame, write_frame};
use mengxia_types::{ErrorCode, Id};
use prost::Message;
use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;
use tokio::time::{Instant, timeout_at};

/// Exact protocol version implemented by TASK-003.
pub const PROTOCOL_MAJOR: u32 = 1;
/// Exact protocol minor implemented by TASK-003.
pub const PROTOCOL_MINOR: u32 = 0;

include!(concat!(env!("OUT_DIR"), "/mengxia.core.v1.rs"));

/// Canonical configured decode-depth ceiling.
pub const MAX_DECODE_DEPTH: u8 = 64;
/// Exact minimum capable of decoding every TASK-003 response shape.
pub const TASK_003_MIN_DECODE_DEPTH: u8 = DESCRIPTOR_MAX_DEPTH;

/// Minimum accepted TASK-003 handshake budget.
pub const MIN_HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(100);
/// Maximum and default TASK-003 handshake budget.
pub const MAX_HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(5_000);

struct RequestIdentity;
struct CorrelationIdentity;

/// Complete immutable limits for one TASK-003 handshake.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandshakeLimits {
    frame_limit: FrameLimit,
    decode_depth: DecodeDepth,
    timeout: Duration,
}

impl HandshakeLimits {
    /// Validates all handshake limits, including the descriptor-proven depth floor.
    pub const fn new(
        frame_limit: FrameLimit,
        decode_depth: DecodeDepth,
        timeout: Duration,
    ) -> Result<Self, HandshakeFailure> {
        if decode_depth.get() < TASK_003_MIN_DECODE_DEPTH
            || timeout.as_nanos() < MIN_HANDSHAKE_TIMEOUT.as_nanos()
            || timeout.as_nanos() > MAX_HANDSHAKE_TIMEOUT.as_nanos()
        {
            return Err(HandshakeFailure::new(ErrorCode::ValidationError));
        }
        Ok(Self {
            frame_limit,
            decode_depth,
            timeout,
        })
    }

    /// Returns the absolute budget applied to the complete handshake.
    #[must_use]
    pub const fn timeout(self) -> Duration {
        self.timeout
    }
}

/// Redacted terminal TASK-003 handshake result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandshakeFailure {
    code: ErrorCode,
}

impl HandshakeFailure {
    const fn new(code: ErrorCode) -> Self {
        Self { code }
    }

    /// Stable public error classification; no transport or peer details are retained.
    #[must_use]
    pub const fn code(self) -> ErrorCode {
        self.code
    }
}

impl fmt::Display for HandshakeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.as_str())
    }
}

impl std::error::Error for HandshakeFailure {}

/// Private-authority result constructed only after channel authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrincipalContext {
    owner_uid: u32,
}

impl PrincipalContext {
    /// Returns the channel-derived ordinary Client principal UID.
    #[must_use]
    pub const fn owner_uid(self) -> u32 {
        self.owner_uid
    }
}

/// Validated public result of a successful terminal TASK-003 handshake.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NegotiatedHandshake {
    request_id: String,
    correlation_id: String,
}

impl NegotiatedHandshake {
    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    #[must_use]
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }
}

/// Authenticates and serves exactly one terminal protocol-1.0 handshake.
pub async fn serve_handshake(
    stream: &mut UnixStream,
    expected_owner_uid: u32,
    limits: HandshakeLimits,
) -> Result<PrincipalContext, HandshakeFailure> {
    let deadline = Instant::now() + limits.timeout;
    match timeout_at(
        deadline,
        serve_handshake_before_deadline(stream, expected_owner_uid, limits),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(HandshakeFailure::new(ErrorCode::DeadlineExceeded)),
    }
}

async fn serve_handshake_before_deadline(
    stream: &mut UnixStream,
    expected_owner_uid: u32,
    limits: HandshakeLimits,
) -> Result<PrincipalContext, HandshakeFailure> {
    let peer = stream
        .peer_cred()
        .map_err(|_| HandshakeFailure::new(ErrorCode::AuthenticationError))?;
    if peer.uid() != expected_owner_uid {
        return Err(HandshakeFailure::new(ErrorCode::AuthenticationError));
    }

    let payload = read_frame(stream, limits.frame_limit)
        .await
        .map_err(map_server_frame_error)?;
    let hello = decode_client_hello(&payload, limits.decode_depth)?;
    let request_id = Id::<RequestIdentity>::from_str(&hello.request_id)
        .map_err(|_| HandshakeFailure::new(ErrorCode::ValidationError))?;

    let (response, terminal_error) = if hello.protocol_major == PROTOCOL_MAJOR
        && hello.min_protocol_minor <= hello.max_protocol_minor
        && hello.min_protocol_minor == PROTOCOL_MINOR
    {
        match Id::<CorrelationIdentity>::try_new() {
            Ok(correlation_id) => (
                HandshakeResponse {
                    response: Some(handshake_response::Response::Hello(ServerHello {
                        request_id: request_id.to_string(),
                        correlation_id: correlation_id.to_string(),
                        protocol_major: PROTOCOL_MAJOR,
                        protocol_minor: PROTOCOL_MINOR,
                    })),
                },
                None,
            ),
            Err(_) => (
                error_response(ErrorCode::IdGenerationUnavailable),
                Some(ErrorCode::IdGenerationUnavailable),
            ),
        }
    } else {
        (
            error_response(ErrorCode::ProtocolVersionUnsupported),
            Some(ErrorCode::ProtocolVersionUnsupported),
        )
    };

    write_frame(stream, &response.encode_to_vec(), limits.frame_limit)
        .await
        .map_err(|_| HandshakeFailure::new(ErrorCode::IpcTransportError))?;
    tokio::io::AsyncWriteExt::shutdown(stream)
        .await
        .map_err(|_| HandshakeFailure::new(ErrorCode::IpcTransportError))?;

    terminal_error.map_or(
        Ok(PrincipalContext {
            owner_uid: expected_owner_uid,
        }),
        |code| Err(HandshakeFailure::new(code)),
    )
}

/// Executes exactly one terminal Client handshake on an already authenticated stream.
pub async fn request_handshake(
    stream: &mut UnixStream,
    request_id: &str,
    limits: HandshakeLimits,
) -> Result<NegotiatedHandshake, HandshakeFailure> {
    let parsed_request = Id::<RequestIdentity>::from_str(request_id)
        .map_err(|_| HandshakeFailure::new(ErrorCode::ValidationError))?;
    let canonical_request = parsed_request.to_string();
    let deadline = Instant::now() + limits.timeout;
    match timeout_at(
        deadline,
        request_handshake_before_deadline(stream, &canonical_request, limits),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(HandshakeFailure::new(ErrorCode::DeadlineExceeded)),
    }
}

async fn request_handshake_before_deadline(
    stream: &mut UnixStream,
    request_id: &str,
    limits: HandshakeLimits,
) -> Result<NegotiatedHandshake, HandshakeFailure> {
    let hello = ClientHello {
        request_id: request_id.to_owned(),
        protocol_major: PROTOCOL_MAJOR,
        min_protocol_minor: PROTOCOL_MINOR,
        max_protocol_minor: PROTOCOL_MINOR,
    };
    write_frame(stream, &hello.encode_to_vec(), limits.frame_limit)
        .await
        .map_err(|_| HandshakeFailure::new(ErrorCode::IpcTransportError))?;
    let payload = read_frame(stream, limits.frame_limit)
        .await
        .map_err(|_| HandshakeFailure::new(ErrorCode::IpcTransportError))?;
    preflight_handshake_response(&payload, limits.decode_depth)
        .map_err(|_| HandshakeFailure::new(ErrorCode::ValidationError))?;
    let response = HandshakeResponse::decode(payload.as_slice())
        .map_err(|_| HandshakeFailure::new(ErrorCode::ValidationError))?;

    let negotiated = match response.response {
        Some(handshake_response::Response::Hello(hello)) => {
            validate_server_hello(hello, request_id)?
        }
        Some(handshake_response::Response::Error(error)) => {
            return Err(validate_error_envelope(error)?);
        }
        None => return Err(HandshakeFailure::new(ErrorCode::ValidationError)),
    };

    let mut trailing = [0_u8; 1];
    match stream.read(&mut trailing).await {
        Ok(0) => Ok(negotiated),
        Ok(_) => Err(HandshakeFailure::new(ErrorCode::ValidationError)),
        Err(_) => Err(HandshakeFailure::new(ErrorCode::IpcTransportError)),
    }
}

fn decode_client_hello(
    payload: &[u8],
    depth: DecodeDepth,
) -> Result<ClientHello, HandshakeFailure> {
    preflight_client_hello(payload, depth)
        .map_err(|_| HandshakeFailure::new(ErrorCode::ValidationError))?;
    ClientHello::decode(payload).map_err(|_| HandshakeFailure::new(ErrorCode::ValidationError))
}

fn validate_server_hello(
    hello: ServerHello,
    expected_request_id: &str,
) -> Result<NegotiatedHandshake, HandshakeFailure> {
    if hello.request_id != expected_request_id
        || hello.protocol_major != PROTOCOL_MAJOR
        || hello.protocol_minor != PROTOCOL_MINOR
    {
        return Err(HandshakeFailure::new(ErrorCode::ValidationError));
    }
    Id::<RequestIdentity>::from_str(&hello.request_id)
        .map_err(|_| HandshakeFailure::new(ErrorCode::ValidationError))?;
    Id::<CorrelationIdentity>::from_str(&hello.correlation_id)
        .map_err(|_| HandshakeFailure::new(ErrorCode::ValidationError))?;
    Ok(NegotiatedHandshake {
        request_id: hello.request_id,
        correlation_id: hello.correlation_id,
    })
}

fn validate_error_envelope(error: ErrorEnvelope) -> Result<HandshakeFailure, HandshakeFailure> {
    if !error.safe_details.is_empty() || error.correlation_id.is_some() {
        return Err(HandshakeFailure::new(ErrorCode::ValidationError));
    }
    let code = ErrorCode::from_str(&error.code)
        .map_err(|_| HandshakeFailure::new(ErrorCode::ValidationError))?;
    if error.safe_message != safe_message(code) || error.retryable {
        return Err(HandshakeFailure::new(ErrorCode::ValidationError));
    }
    Ok(HandshakeFailure::new(code))
}

fn error_response(code: ErrorCode) -> HandshakeResponse {
    HandshakeResponse {
        response: Some(handshake_response::Response::Error(ErrorEnvelope {
            code: code.as_str().to_owned(),
            safe_message: safe_message(code).to_owned(),
            retryable: false,
            correlation_id: None,
            safe_details: Default::default(),
        })),
    }
}

const fn safe_message(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::ProtocolVersionUnsupported => "protocol version is unsupported",
        ErrorCode::ValidationError => "request validation failed",
        ErrorCode::IdGenerationUnavailable => "identifier generation is unavailable",
        _ => "operation failed",
    }
}

fn map_server_frame_error(error: FrameError) -> HandshakeFailure {
    match error {
        FrameError::Transport => HandshakeFailure::new(ErrorCode::IpcTransportError),
        FrameError::InvalidLimit
        | FrameError::InvalidLength
        | FrameError::Truncated
        | FrameError::AllocationUnavailable => HandshakeFailure::new(ErrorCode::ValidationError),
        _ => HandshakeFailure::new(ErrorCode::InternalError),
    }
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum TestAuthorityDomain {
    OrdinaryClient,
    Admin,
}

#[cfg(test)]
fn test_authorize_domain<T>(
    domain: TestAuthorityDomain,
    continuation: impl FnOnce() -> T,
) -> Result<T, HandshakeFailure> {
    match domain {
        TestAuthorityDomain::OrdinaryClient => Ok(continuation()),
        TestAuthorityDomain::Admin => Err(HandshakeFailure::new(ErrorCode::AdminAuthUnavailable)),
    }
}

/// Validated protobuf decode-depth ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeDepth(u8);

impl DecodeDepth {
    /// Accepts the canonical tightening-only range 1 through 64.
    pub const fn new(depth: u8) -> Result<Self, WirePreflightError> {
        if depth == 0 || depth > MAX_DECODE_DEPTH {
            return Err(WirePreflightError::InvalidLimit);
        }
        Ok(Self(depth))
    }

    /// Returns the configured root-inclusive depth.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Redacted failure from the allocation-free protobuf wire preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WirePreflightError {
    /// The configured depth is outside 1 through 64.
    InvalidLimit,
    /// A known embedded message would exceed the configured depth.
    DepthExceeded,
    /// Tags, varints, lengths or wire types are malformed/non-canonical for TASK-003.
    Malformed,
}

impl fmt::Display for WirePreflightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimit => "invalid decode depth",
            Self::DepthExceeded => "protobuf decode depth exceeded",
            Self::Malformed => "malformed protobuf wire data",
        })
    }
}

impl std::error::Error for WirePreflightError {}

/// Validates a ClientHello wire payload before Prost allocation/decoding.
pub fn preflight_client_hello(
    payload: &[u8],
    limit: DecodeDepth,
) -> Result<(), WirePreflightError> {
    preflight(payload, MessageKind::ClientHello, limit)
}

/// Validates a HandshakeResponse wire payload before Prost allocation/decoding.
pub fn preflight_handshake_response(
    payload: &[u8],
    limit: DecodeDepth,
) -> Result<(), WirePreflightError> {
    preflight(payload, MessageKind::HandshakeResponse, limit)
}

#[derive(Clone, Copy)]
enum MessageKind {
    ClientHello,
    HandshakeResponse,
    ServerHello,
    ErrorEnvelope,
    SafeDetailsEntry,
}

include!(concat!(env!("OUT_DIR"), "/mengxia.depth.rs"));

#[derive(Clone, Copy)]
struct ScanFrame<'a> {
    bytes: &'a [u8],
    offset: usize,
    kind: MessageKind,
}

fn preflight(
    payload: &[u8],
    root: MessageKind,
    limit: DecodeDepth,
) -> Result<(), WirePreflightError> {
    let empty = ScanFrame {
        bytes: &[],
        offset: 0,
        kind: MessageKind::ClientHello,
    };
    let mut stack = [empty; MAX_DECODE_DEPTH as usize];
    stack[0] = ScanFrame {
        bytes: payload,
        offset: 0,
        kind: root,
    };
    let mut depth = 1_usize;

    while depth != 0 {
        let index = depth - 1;
        if stack[index].offset == stack[index].bytes.len() {
            depth -= 1;
            continue;
        }

        let tag = read_varint(stack[index].bytes, &mut stack[index].offset)?;
        let field_number = tag >> 3;
        let wire_type = (tag & 0x07) as u8;
        if field_number == 0 || wire_type == 3 || wire_type == 4 {
            return Err(WirePreflightError::Malformed);
        }

        match wire_type {
            0 => {
                read_varint(stack[index].bytes, &mut stack[index].offset)?;
            }
            1 => skip_fixed(&mut stack[index], 8)?,
            2 => {
                let length = read_varint(stack[index].bytes, &mut stack[index].offset)?;
                let length = usize::try_from(length).map_err(|_| WirePreflightError::Malformed)?;
                let start = stack[index].offset;
                let end = start
                    .checked_add(length)
                    .filter(|end| *end <= stack[index].bytes.len())
                    .ok_or(WirePreflightError::Malformed)?;
                stack[index].offset = end;
                if let Some(kind) = descriptor_embedded_message(stack[index].kind, field_number) {
                    if depth >= limit.get() as usize {
                        return Err(WirePreflightError::DepthExceeded);
                    }
                    stack[depth] = ScanFrame {
                        bytes: &stack[index].bytes[start..end],
                        offset: 0,
                        kind,
                    };
                    depth += 1;
                }
            }
            5 => skip_fixed(&mut stack[index], 4)?,
            _ => return Err(WirePreflightError::Malformed),
        }
    }
    Ok(())
}

fn read_varint(bytes: &[u8], offset: &mut usize) -> Result<u64, WirePreflightError> {
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let byte = *bytes.get(*offset).ok_or(WirePreflightError::Malformed)?;
        *offset += 1;
        if shift == 63 && byte > 1 {
            return Err(WirePreflightError::Malformed);
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            if shift != 0 && byte == 0 {
                return Err(WirePreflightError::Malformed);
            }
            return Ok(value);
        }
    }
    Err(WirePreflightError::Malformed)
}

fn skip_fixed(frame: &mut ScanFrame<'_>, width: usize) -> Result<(), WirePreflightError> {
    frame.offset = frame
        .offset
        .checked_add(width)
        .filter(|end| *end <= frame.bytes.len())
        .ok_or(WirePreflightError::Malformed)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use prost::Message;
    use tokio::net::UnixStream;

    use super::*;

    #[test]
    fn decode_depth_range_is_exact() {
        assert_eq!(DecodeDepth::new(0), Err(WirePreflightError::InvalidLimit));
        assert_eq!(DecodeDepth::new(1).map(DecodeDepth::get), Ok(1));
        assert_eq!(DecodeDepth::new(64).map(DecodeDepth::get), Ok(64));
        assert_eq!(DecodeDepth::new(65), Err(WirePreflightError::InvalidLimit));
    }

    #[test]
    fn handshake_timeout_range_is_exact_without_submillisecond_truncation() {
        let frame = FrameLimit::default();
        let depth = DecodeDepth::new(TASK_003_MIN_DECODE_DEPTH).unwrap();
        assert!(HandshakeLimits::new(frame, depth, MIN_HANDSHAKE_TIMEOUT).is_ok());
        assert!(HandshakeLimits::new(frame, depth, MAX_HANDSHAKE_TIMEOUT).is_ok());
        assert_eq!(
            HandshakeLimits::new(
                frame,
                depth,
                MIN_HANDSHAKE_TIMEOUT - Duration::from_nanos(1),
            )
            .map_err(HandshakeFailure::code),
            Err(ErrorCode::ValidationError)
        );
        assert_eq!(
            HandshakeLimits::new(
                frame,
                depth,
                MAX_HANDSHAKE_TIMEOUT + Duration::from_nanos(1),
            )
            .map_err(HandshakeFailure::code),
            Err(ErrorCode::ValidationError)
        );
    }

    #[test]
    fn published_response_graph_requires_exactly_three_levels() {
        let response = HandshakeResponse {
            response: Some(handshake_response::Response::Error(ErrorEnvelope {
                code: "VALIDATION_ERROR".to_owned(),
                safe_message: "validation failed".to_owned(),
                retryable: false,
                correlation_id: None,
                safe_details: [("future".to_owned(), "value".to_owned())]
                    .into_iter()
                    .collect(),
            })),
        }
        .encode_to_vec();

        assert_eq!(
            preflight_handshake_response(&response, DecodeDepth::new(2).unwrap()),
            Err(WirePreflightError::DepthExceeded)
        );
        assert_eq!(
            preflight_handshake_response(&response, DecodeDepth::new(3).unwrap()),
            Ok(())
        );
    }

    #[test]
    fn unknown_length_delimited_fields_are_opaque_but_groups_are_rejected() {
        let deeply_nested_unknown = [0x52, 0x06, 0x52, 0x04, 0x52, 0x02, 0x08, 0x01];
        assert_eq!(
            preflight_client_hello(&deeply_nested_unknown, DecodeDepth::new(1).unwrap()),
            Ok(())
        );
        assert_eq!(
            preflight_client_hello(&[0x53, 0x54], DecodeDepth::new(64).unwrap()),
            Err(WirePreflightError::Malformed)
        );
    }

    #[test]
    fn malformed_tags_varints_lengths_and_fixed_values_fail() {
        for malformed in [
            &[0_u8][..],
            &[0x80][..],
            &[0x88, 0x00, 0x01][..],
            &[0x08, 0x80, 0x00][..],
            &[0x0a, 0x02, 0x01][..],
            &[0x09, 0, 0, 0][..],
            &[0x0d, 0, 0, 0][..],
            &[
                0x08, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02,
            ][..],
        ] {
            assert_eq!(
                preflight_client_hello(malformed, DecodeDepth::new(64).unwrap()),
                Err(WirePreflightError::Malformed)
            );
        }
    }

    #[tokio::test]
    async fn same_peer_negotiates_once_and_requires_terminal_eof() {
        let limits = HandshakeLimits::new(
            FrameLimit::default(),
            DecodeDepth::new(TASK_003_MIN_DECODE_DEPTH).unwrap(),
            MAX_HANDSHAKE_TIMEOUT,
        )
        .unwrap();
        let request_id = Id::<RequestIdentity>::try_new().unwrap().to_string();
        let (mut server, mut client) = UnixStream::pair().unwrap();
        let expected_uid = server.peer_cred().unwrap().uid();

        let server_task = async {
            serve_handshake(&mut server, expected_uid, limits)
                .await
                .map(PrincipalContext::owner_uid)
        };
        let client_task = request_handshake(&mut client, &request_id, limits);
        let (server_result, client_result) = tokio::join!(server_task, client_task);

        assert_eq!(server_result, Ok(expected_uid));
        let negotiated = client_result.unwrap();
        assert_eq!(negotiated.request_id(), request_id);
        Id::<CorrelationIdentity>::from_str(negotiated.correlation_id()).unwrap();
    }

    #[tokio::test]
    async fn peer_mismatch_is_rejected_before_any_frame() {
        let limits = HandshakeLimits::new(
            FrameLimit::default(),
            DecodeDepth::new(TASK_003_MIN_DECODE_DEPTH).unwrap(),
            MAX_HANDSHAKE_TIMEOUT,
        )
        .unwrap();
        let (mut server, _client) = UnixStream::pair().unwrap();
        let actual = server.peer_cred().unwrap().uid();
        let mismatched = actual.checked_add(1).unwrap_or(actual - 1);

        assert_eq!(
            serve_handshake(&mut server, mismatched, limits)
                .await
                .map_err(HandshakeFailure::code),
            Err(ErrorCode::AuthenticationError)
        );
    }

    #[tokio::test]
    async fn incompatible_version_returns_the_exact_safe_error() {
        let limits = HandshakeLimits::new(
            FrameLimit::default(),
            DecodeDepth::new(TASK_003_MIN_DECODE_DEPTH).unwrap(),
            MAX_HANDSHAKE_TIMEOUT,
        )
        .unwrap();
        let request_id = Id::<RequestIdentity>::try_new().unwrap().to_string();
        let (mut server, mut client) = UnixStream::pair().unwrap();
        let expected_uid = server.peer_cred().unwrap().uid();
        let bad_hello = ClientHello {
            request_id,
            protocol_major: 2,
            min_protocol_minor: 0,
            max_protocol_minor: 0,
        };

        let server_task = serve_handshake(&mut server, expected_uid, limits);
        let client_task = async {
            write_frame(
                &mut client,
                &bad_hello.encode_to_vec(),
                FrameLimit::default(),
            )
            .await
            .unwrap();
            let payload = read_frame(&mut client, FrameLimit::default())
                .await
                .unwrap();
            let response = HandshakeResponse::decode(payload.as_slice()).unwrap();
            match response.response.unwrap() {
                handshake_response::Response::Error(error) => {
                    validate_error_envelope(error).unwrap().code()
                }
                handshake_response::Response::Hello(_) => panic!("unexpected hello"),
            }
        };
        let (server_result, client_code) = tokio::join!(server_task, client_task);

        assert_eq!(
            server_result.map_err(HandshakeFailure::code),
            Err(ErrorCode::ProtocolVersionUnsupported)
        );
        assert_eq!(client_code, ErrorCode::ProtocolVersionUnsupported);
    }

    #[tokio::test]
    async fn auth_reserved_actor_tag_never_changes_channel_derived_principal() {
        let limits = HandshakeLimits::new(
            FrameLimit::default(),
            DecodeDepth::new(TASK_003_MIN_DECODE_DEPTH).unwrap(),
            MAX_HANDSHAKE_TIMEOUT,
        )
        .unwrap();
        let request_id = Id::<RequestIdentity>::try_new().unwrap().to_string();
        let (mut server, mut client) = UnixStream::pair().unwrap();
        let expected_uid = server.peer_cred().unwrap().uid();
        let mut wire = ClientHello {
            request_id,
            protocol_major: PROTOCOL_MAJOR,
            min_protocol_minor: PROTOCOL_MINOR,
            max_protocol_minor: PROTOCOL_MINOR,
        }
        .encode_to_vec();
        wire.extend_from_slice(&[0x1a, 0x05, b'a', b'd', b'm', b'i', b'n']);

        let server_task = serve_handshake(&mut server, expected_uid, limits);
        let client_task = async {
            write_frame(&mut client, &wire, FrameLimit::default())
                .await
                .unwrap();
            read_frame(&mut client, FrameLimit::default())
                .await
                .unwrap();
        };
        let (server_result, ()) = tokio::join!(server_task, client_task);
        assert_eq!(server_result.unwrap().owner_uid(), expected_uid);
    }

    #[tokio::test]
    async fn handshake_one_absolute_deadline_bounds_a_silent_peer() {
        let limits = HandshakeLimits::new(
            FrameLimit::default(),
            DecodeDepth::new(TASK_003_MIN_DECODE_DEPTH).unwrap(),
            MIN_HANDSHAKE_TIMEOUT,
        )
        .unwrap();
        let (mut server, _silent_client) = UnixStream::pair().unwrap();
        let expected_uid = server.peer_cred().unwrap().uid();
        assert_eq!(
            serve_handshake(&mut server, expected_uid, limits)
                .await
                .map_err(HandshakeFailure::code),
            Err(ErrorCode::DeadlineExceeded)
        );
    }

    #[test]
    fn auth_admin_test_seam_fails_before_continuation() {
        let result = test_authorize_domain(TestAuthorityDomain::Admin, || {
            panic!("Admin continuation must remain unreachable")
        });
        assert_eq!(
            result.map_err(HandshakeFailure::code),
            Err(ErrorCode::AdminAuthUnavailable)
        );
        assert_eq!(
            test_authorize_domain(TestAuthorityDomain::OrdinaryClient, || 7),
            Ok(7)
        );
    }
}
