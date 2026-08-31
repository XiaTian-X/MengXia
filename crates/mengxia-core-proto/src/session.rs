use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use mengxia_framing::{FrameLimit, read_frame, write_frame};
use mengxia_types::{ErrorCode, Id};
use prost::Message;
use tokio::io::AsyncReadExt as _;
use tokio::net::UnixStream;
use tokio::time::{Instant, timeout_at};

use super::{
    ClientHello, ClientIntent, CoreRequest, CoreResponse, DecodeDepth, HandshakeLimits,
    HandshakeResponse, PROTOCOL_MAJOR, PrincipalContext, SINGLE_COMMAND_PROTOCOL_MINOR,
    ServerHello, TASK_007_MIN_OPERATION_DECODE_DEPTH, error_response, handshake_response,
    preflight_core_request, preflight_core_response, preflight_handshake_response,
};

struct SessionRequestIdentity;
struct SessionCorrelationIdentity;

/// Complete immutable wire limits for one TASK-007 operation frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationLimits {
    frame_limit: FrameLimit,
    decode_depth: DecodeDepth,
}

impl OperationLimits {
    pub const fn new(
        frame_limit: FrameLimit,
        decode_depth: DecodeDepth,
    ) -> Result<Self, OperationFailure> {
        if decode_depth.get() < TASK_007_MIN_OPERATION_DECODE_DEPTH {
            return Err(OperationFailure::new(ErrorCode::ValidationError));
        }
        Ok(Self {
            frame_limit,
            decode_depth,
        })
    }
}

/// Redacted session/operation transport failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationFailure {
    code: ErrorCode,
}

impl OperationFailure {
    pub(crate) const fn new(code: ErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> ErrorCode {
        self.code
    }
}

impl fmt::Display for OperationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.as_str())
    }
}

impl std::error::Error for OperationFailure {}

/// Opaque authority available only after authenticated protocol-1.1 negotiation.
pub struct ServerSessionContext {
    principal: PrincipalContext,
    request_id: String,
    correlation_id: String,
}

impl ServerSessionContext {
    #[must_use]
    pub const fn principal(&self) -> PrincipalContext {
        self.principal
    }

    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    #[must_use]
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }
}

/// Opaque client proof that protocol 1.1 and the canonical correlation were selected.
pub struct NegotiatedClientSession {
    request_id: String,
    correlation_id: String,
}

pub enum ServerNegotiation {
    HandshakeOnly(PrincipalContext),
    SingleCommand(ServerSessionContext),
}

/// Authenticates once and dispatches the retained 1.0 terminal or exact 1.1 session intent.
pub async fn serve_daemon_handshake(
    stream: &mut UnixStream,
    expected_owner_uid: u32,
    limits: HandshakeLimits,
) -> Result<ServerNegotiation, OperationFailure> {
    let deadline = Instant::now() + limits.timeout;
    timeout_at(deadline, async {
        let peer = stream
            .peer_cred()
            .map_err(|_| OperationFailure::new(ErrorCode::AuthenticationError))?;
        if peer.uid() != expected_owner_uid {
            return Err(OperationFailure::new(ErrorCode::AuthenticationError));
        }
        let payload = read_frame(stream, limits.frame_limit)
            .await
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        super::preflight_client_hello(&payload, limits.decode_depth)
            .map_err(|_| OperationFailure::new(ErrorCode::ValidationError))?;
        let hello = ClientHello::decode(payload.as_slice())
            .map_err(|_| OperationFailure::new(ErrorCode::ValidationError))?;
        let request_id = Id::<SessionRequestIdentity>::from_str(&hello.request_id)
            .map_err(|_| OperationFailure::new(ErrorCode::ValidationError))?;
        let legacy = hello.protocol_major == PROTOCOL_MAJOR
            && hello.min_protocol_minor == super::PROTOCOL_MINOR
            && hello.max_protocol_minor == super::PROTOCOL_MINOR
            && matches!(
                ClientIntent::try_from(hello.intent),
                Ok(ClientIntent::Unspecified | ClientIntent::HandshakeOnly)
            );
        let single = hello.protocol_major == PROTOCOL_MAJOR
            && hello.min_protocol_minor == SINGLE_COMMAND_PROTOCOL_MINOR
            && hello.max_protocol_minor == SINGLE_COMMAND_PROTOCOL_MINOR
            && hello.intent == ClientIntent::SingleCommand as i32;
        if !legacy && !single {
            let response = error_response(ErrorCode::ProtocolVersionUnsupported);
            write_frame(stream, &response.encode_to_vec(), limits.frame_limit)
                .await
                .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
            tokio::io::AsyncWriteExt::shutdown(stream)
                .await
                .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
            return Err(OperationFailure::new(ErrorCode::ProtocolVersionUnsupported));
        }
        let correlation_id = Id::<SessionCorrelationIdentity>::try_new()
            .map_err(|_| OperationFailure::new(ErrorCode::IdGenerationUnavailable))?;
        let selected_minor = if single {
            SINGLE_COMMAND_PROTOCOL_MINOR
        } else {
            super::PROTOCOL_MINOR
        };
        let response = HandshakeResponse {
            response: Some(handshake_response::Response::Hello(ServerHello {
                request_id: request_id.to_string(),
                correlation_id: correlation_id.to_string(),
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: selected_minor,
            })),
        };
        write_frame(stream, &response.encode_to_vec(), limits.frame_limit)
            .await
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        let principal = PrincipalContext {
            owner_uid: expected_owner_uid,
        };
        if legacy {
            tokio::io::AsyncWriteExt::shutdown(stream)
                .await
                .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
            Ok(ServerNegotiation::HandshakeOnly(principal))
        } else {
            Ok(ServerNegotiation::SingleCommand(ServerSessionContext {
                principal,
                request_id: request_id.to_string(),
                correlation_id: correlation_id.to_string(),
            }))
        }
    })
    .await
    .unwrap_or_else(|_| Err(OperationFailure::new(ErrorCode::DeadlineExceeded)))
}

impl NegotiatedClientSession {
    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    #[must_use]
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }
}

/// Authenticates and negotiates exactly one protocol-1.1 single-command session.
pub async fn serve_single_command_handshake(
    stream: &mut UnixStream,
    expected_owner_uid: u32,
    limits: HandshakeLimits,
) -> Result<ServerSessionContext, OperationFailure> {
    let deadline = Instant::now() + limits.timeout;
    timeout_at(deadline, async {
        let peer = stream
            .peer_cred()
            .map_err(|_| OperationFailure::new(ErrorCode::AuthenticationError))?;
        if peer.uid() != expected_owner_uid {
            return Err(OperationFailure::new(ErrorCode::AuthenticationError));
        }
        let payload = read_frame(stream, limits.frame_limit)
            .await
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        super::preflight_client_hello(&payload, limits.decode_depth)
            .map_err(|_| OperationFailure::new(ErrorCode::ValidationError))?;
        let hello = ClientHello::decode(payload.as_slice())
            .map_err(|_| OperationFailure::new(ErrorCode::ValidationError))?;
        let request_id = Id::<SessionRequestIdentity>::from_str(&hello.request_id)
            .map_err(|_| OperationFailure::new(ErrorCode::ValidationError))?;
        if hello.protocol_major != PROTOCOL_MAJOR
            || hello.min_protocol_minor != SINGLE_COMMAND_PROTOCOL_MINOR
            || hello.max_protocol_minor != SINGLE_COMMAND_PROTOCOL_MINOR
            || hello.intent != ClientIntent::SingleCommand as i32
        {
            return Err(OperationFailure::new(ErrorCode::ProtocolVersionUnsupported));
        }
        let correlation_id = Id::<SessionCorrelationIdentity>::try_new()
            .map_err(|_| OperationFailure::new(ErrorCode::IdGenerationUnavailable))?;
        let response = HandshakeResponse {
            response: Some(handshake_response::Response::Hello(ServerHello {
                request_id: request_id.to_string(),
                correlation_id: correlation_id.to_string(),
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: SINGLE_COMMAND_PROTOCOL_MINOR,
            })),
        };
        write_frame(stream, &response.encode_to_vec(), limits.frame_limit)
            .await
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        Ok(ServerSessionContext {
            principal: PrincipalContext {
                owner_uid: expected_owner_uid,
            },
            request_id: request_id.to_string(),
            correlation_id: correlation_id.to_string(),
        })
    })
    .await
    .unwrap_or_else(|_| Err(OperationFailure::new(ErrorCode::DeadlineExceeded)))
}

async fn request_single_command_handshake(
    stream: &mut UnixStream,
    request_id: &str,
    limits: HandshakeLimits,
) -> Result<NegotiatedClientSession, OperationFailure> {
    let parsed = Id::<SessionRequestIdentity>::from_str(request_id)
        .map_err(|_| OperationFailure::new(ErrorCode::ValidationError))?;
    let canonical = parsed.to_string();
    let deadline = Instant::now() + limits.timeout;
    timeout_at(deadline, async {
        let hello = ClientHello {
            request_id: canonical.clone(),
            protocol_major: PROTOCOL_MAJOR,
            min_protocol_minor: SINGLE_COMMAND_PROTOCOL_MINOR,
            max_protocol_minor: SINGLE_COMMAND_PROTOCOL_MINOR,
            intent: ClientIntent::SingleCommand as i32,
        };
        write_frame(stream, &hello.encode_to_vec(), limits.frame_limit)
            .await
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        let payload = read_frame(stream, limits.frame_limit)
            .await
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        preflight_handshake_response(&payload, limits.decode_depth)
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        let response = HandshakeResponse::decode(payload.as_slice())
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        let hello = match response.response {
            Some(handshake_response::Response::Hello(hello)) => hello,
            _ => return Err(OperationFailure::new(ErrorCode::ProtocolVersionUnsupported)),
        };
        if hello.request_id != canonical
            || hello.protocol_major != PROTOCOL_MAJOR
            || hello.protocol_minor != SINGLE_COMMAND_PROTOCOL_MINOR
        {
            return Err(OperationFailure::new(ErrorCode::IpcTransportError));
        }
        Id::<SessionCorrelationIdentity>::from_str(&hello.correlation_id)
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        Ok(NegotiatedClientSession {
            request_id: canonical,
            correlation_id: hello.correlation_id,
        })
    })
    .await
    .unwrap_or_else(|_| Err(OperationFailure::new(ErrorCode::DeadlineExceeded)))
}

/// Reads, preflights and decodes one CoreRequest before the absolute deadline.
pub async fn read_core_request(
    stream: &mut UnixStream,
    limits: OperationLimits,
    deadline: Instant,
) -> Result<CoreRequest, OperationFailure> {
    timeout_at(deadline, async {
        let payload = read_frame(stream, limits.frame_limit)
            .await
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        preflight_core_request(&payload, limits.decode_depth)
            .map_err(|_| OperationFailure::new(ErrorCode::ValidationError))?;
        CoreRequest::decode(payload.as_slice())
            .map_err(|_| OperationFailure::new(ErrorCode::ValidationError))
    })
    .await
    .unwrap_or_else(|_| Err(OperationFailure::new(ErrorCode::DeadlineExceeded)))
}

/// Encodes and writes one terminal CoreResponse before the absolute deadline.
pub async fn write_core_response(
    stream: &mut UnixStream,
    response: &CoreResponse,
    limits: OperationLimits,
    deadline: Instant,
) -> Result<(), OperationFailure> {
    timeout_at(deadline, async {
        write_frame(stream, &response.encode_to_vec(), limits.frame_limit)
            .await
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        tokio::io::AsyncWriteExt::shutdown(stream)
            .await
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))
    })
    .await
    .unwrap_or_else(|_| Err(OperationFailure::new(ErrorCode::DeadlineExceeded)))
}

/// Negotiates protocol 1.1, sends one request and reads one terminal response.
pub async fn request_single_command(
    stream: &mut UnixStream,
    request_id: &str,
    request: &CoreRequest,
    handshake_limits: HandshakeLimits,
    operation_limits: OperationLimits,
    operation_timeout: Duration,
) -> Result<(NegotiatedClientSession, CoreResponse), OperationFailure> {
    let session = request_single_command_handshake(stream, request_id, handshake_limits).await?;
    let operation_deadline = Instant::now() + operation_timeout;
    timeout_at(operation_deadline, async {
        write_frame(
            stream,
            &request.encode_to_vec(),
            operation_limits.frame_limit,
        )
        .await
        .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        let payload = read_frame(stream, operation_limits.frame_limit)
            .await
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        preflight_core_response(&payload, operation_limits.decode_depth)
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        let response = CoreResponse::decode(payload.as_slice())
            .map_err(|_| OperationFailure::new(ErrorCode::IpcTransportError))?;
        let mut trailing = [0_u8; 1];
        match stream.read(&mut trailing).await {
            Ok(0) => {}
            Ok(_) | Err(_) => {
                return Err(OperationFailure::new(ErrorCode::IpcTransportError));
            }
        }
        Ok((session, response))
    })
    .await
    .unwrap_or_else(|_| Err(OperationFailure::new(ErrorCode::DeadlineExceeded)))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mengxia_framing::FrameLimit;
    use mengxia_types::{ErrorCode, Id};
    use tokio::net::UnixStream;

    use super::*;
    use crate::{
        CoreRequest, DecodeDepth, HandshakeLimits, IngestAssetCopyRequest, IngestMode, RetryAction,
        core_request, core_response, operation_error_response, request_handshake,
    };

    #[tokio::test]
    async fn daemon_dispatch_preserves_legacy_terminal_handshake() {
        let limits = HandshakeLimits::new(
            FrameLimit::default(),
            DecodeDepth::new(crate::TASK_003_MIN_DECODE_DEPTH).unwrap(),
            Duration::from_secs(1),
        )
        .unwrap();
        let request_id = Id::<SessionRequestIdentity>::try_new().unwrap().to_string();
        let (mut server, mut client) = UnixStream::pair().unwrap();
        let uid = server.peer_cred().unwrap().uid();
        let (served, requested) = tokio::join!(
            serve_daemon_handshake(&mut server, uid, limits),
            request_handshake(&mut client, &request_id, limits),
        );
        assert!(matches!(
            served.unwrap(),
            ServerNegotiation::HandshakeOnly(_)
        ));
        assert_eq!(requested.unwrap().request_id(), request_id);
    }

    #[tokio::test]
    async fn single_command_uses_distinct_limits_preflight_and_correlation() {
        let frame = FrameLimit::default();
        let depth = DecodeDepth::new(crate::MAX_DECODE_DEPTH).unwrap();
        let handshake = HandshakeLimits::new(frame, depth, Duration::from_secs(1)).unwrap();
        let operation = OperationLimits::new(frame, depth).unwrap();
        let request_id = Id::<SessionRequestIdentity>::try_new().unwrap().to_string();
        let request = CoreRequest {
            operation: Some(core_request::Operation::IngestAssetCopy(
                IngestAssetCopyRequest {
                    command_id: request_id.clone(),
                    source_path: b"/private/tmp/source".to_vec(),
                    mode: IngestMode::Copy as i32,
                    asset_kind: "file".to_owned(),
                    content_kind: "binary".to_owned(),
                    representation_purpose: "original".to_owned(),
                    resource_kind: "blob".to_owned(),
                    logical_name: "source".to_owned(),
                    expected_sha256: None,
                    operation_timeout_ms: 100,
                },
            )),
        };
        let (mut server, mut client) = UnixStream::pair().unwrap();
        let uid = server.peer_cred().unwrap().uid();
        let server_task = async {
            let session = match serve_daemon_handshake(&mut server, uid, handshake)
                .await
                .unwrap()
            {
                ServerNegotiation::SingleCommand(session) => session,
                ServerNegotiation::HandshakeOnly(_) => panic!("wrong intent"),
            };
            let decoded = read_core_request(
                &mut server,
                operation,
                Instant::now() + Duration::from_secs(1),
            )
            .await
            .unwrap();
            assert!(matches!(
                decoded.operation,
                Some(core_request::Operation::IngestAssetCopy(_))
            ));
            let response = operation_error_response(
                ErrorCode::Backpressure,
                RetryAction::SameCommand,
                session.correlation_id(),
            )
            .unwrap();
            write_core_response(
                &mut server,
                &response,
                operation,
                Instant::now() + Duration::from_secs(1),
            )
            .await
            .unwrap();
            session.correlation_id().to_owned()
        };
        let client_task = request_single_command(
            &mut client,
            &request_id,
            &request,
            handshake,
            operation,
            Duration::from_secs(1),
        );
        let (correlation, response) = tokio::join!(server_task, client_task);
        let (session, response) = response.unwrap();
        assert_eq!(session.correlation_id(), correlation);
        assert!(matches!(
            response.response,
            Some(core_response::Response::Error(_))
        ));
    }

    #[tokio::test]
    async fn client_operation_timeout_starts_after_handshake_without_unused_budget() {
        let frame = FrameLimit::default();
        let depth = DecodeDepth::new(crate::MAX_DECODE_DEPTH).unwrap();
        let handshake = HandshakeLimits::new(frame, depth, Duration::from_millis(500)).unwrap();
        let operation = OperationLimits::new(frame, depth).unwrap();
        let request_id = Id::<SessionRequestIdentity>::try_new().unwrap().to_string();
        let request = CoreRequest {
            operation: Some(core_request::Operation::IngestAssetCopy(
                IngestAssetCopyRequest {
                    command_id: request_id.clone(),
                    source_path: b"/private/tmp/source".to_vec(),
                    mode: IngestMode::Copy as i32,
                    asset_kind: "file".to_owned(),
                    content_kind: "binary".to_owned(),
                    representation_purpose: "original".to_owned(),
                    resource_kind: "blob".to_owned(),
                    logical_name: "source".to_owned(),
                    expected_sha256: None,
                    operation_timeout_ms: 50,
                },
            )),
        };
        let (mut server, mut client) = UnixStream::pair().unwrap();
        let uid = server.peer_cred().unwrap().uid();
        let server_task = async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let session = match serve_daemon_handshake(&mut server, uid, handshake)
                .await
                .unwrap()
            {
                ServerNegotiation::SingleCommand(session) => session,
                ServerNegotiation::HandshakeOnly(_) => panic!("wrong intent"),
            };
            let _ = read_core_request(
                &mut server,
                operation,
                Instant::now() + Duration::from_millis(500),
            )
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
            let response = operation_error_response(
                ErrorCode::Backpressure,
                RetryAction::SameCommand,
                session.correlation_id(),
            )
            .unwrap();
            let _ = write_core_response(
                &mut server,
                &response,
                operation,
                Instant::now() + Duration::from_millis(100),
            )
            .await;
        };
        let client_task = request_single_command(
            &mut client,
            &request_id,
            &request,
            handshake,
            operation,
            Duration::from_millis(50),
        );
        let (_, result) = tokio::join!(server_task, client_task);
        assert_eq!(
            result.map(|_| ()).map_err(OperationFailure::code),
            Err(ErrorCode::DeadlineExceeded)
        );
    }

    #[tokio::test]
    async fn client_requires_terminal_eof_and_rejects_trailing_response_bytes() {
        use tokio::io::AsyncWriteExt as _;

        let frame = FrameLimit::default();
        let depth = DecodeDepth::new(crate::MAX_DECODE_DEPTH).unwrap();
        let handshake = HandshakeLimits::new(frame, depth, Duration::from_secs(1)).unwrap();
        let operation = OperationLimits::new(frame, depth).unwrap();
        let request_id = Id::<SessionRequestIdentity>::try_new().unwrap().to_string();
        let request = CoreRequest {
            operation: Some(core_request::Operation::IngestAssetCopy(
                IngestAssetCopyRequest {
                    command_id: request_id.clone(),
                    source_path: b"/private/tmp/source".to_vec(),
                    mode: IngestMode::Copy as i32,
                    asset_kind: "file".to_owned(),
                    content_kind: "binary".to_owned(),
                    representation_purpose: "original".to_owned(),
                    resource_kind: "blob".to_owned(),
                    logical_name: "source".to_owned(),
                    expected_sha256: None,
                    operation_timeout_ms: 100,
                },
            )),
        };
        let (mut server, mut client) = UnixStream::pair().unwrap();
        let uid = server.peer_cred().unwrap().uid();
        let server_task = async {
            let session = match serve_daemon_handshake(&mut server, uid, handshake)
                .await
                .unwrap()
            {
                ServerNegotiation::SingleCommand(session) => session,
                ServerNegotiation::HandshakeOnly(_) => panic!("wrong intent"),
            };
            let _ = read_core_request(
                &mut server,
                operation,
                Instant::now() + Duration::from_secs(1),
            )
            .await
            .unwrap();
            let response = operation_error_response(
                ErrorCode::Backpressure,
                RetryAction::SameCommand,
                session.correlation_id(),
            )
            .unwrap();
            write_frame(&mut server, &response.encode_to_vec(), frame)
                .await
                .unwrap();
            server.write_all(b"trailing-byte").await.unwrap();
            server.shutdown().await.unwrap();
        };
        let client_task = request_single_command(
            &mut client,
            &request_id,
            &request,
            handshake,
            operation,
            Duration::from_secs(1),
        );
        let (_, result) = tokio::join!(server_task, client_task);
        assert_eq!(
            result.map(|_| ()).map_err(OperationFailure::code),
            Err(ErrorCode::IpcTransportError)
        );
    }

    #[test]
    fn operation_depth_floor_is_independent_and_exact() {
        let frame = FrameLimit::default();
        let below = crate::TASK_007_MIN_OPERATION_DECODE_DEPTH.saturating_sub(1);
        if below != 0 {
            assert!(OperationLimits::new(frame, DecodeDepth::new(below).unwrap()).is_err());
        }
        assert!(
            OperationLimits::new(
                frame,
                DecodeDepth::new(crate::TASK_007_MIN_OPERATION_DECODE_DEPTH).unwrap()
            )
            .is_ok()
        );
    }
}
