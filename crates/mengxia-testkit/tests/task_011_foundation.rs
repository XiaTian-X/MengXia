use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{
    io::{Read, Write},
    os::fd::OwnedFd,
    os::unix::net::UnixStream as StdUnixStream,
    process::{Command, Stdio},
};

use mengxia_plugin_host::{
    DEFAULT_ACTIVE_SESSIONS, DEFAULT_CONTROL_FRAME_BYTES, DEFAULT_DECODE_DEPTH,
    DEFAULT_HANDSHAKE_TIMEOUT, DEFAULT_INBOUND_FRAMES, DEFAULT_OUTBOUND_FRAMES,
    DEFAULT_REQUEST_TIMEOUT, DEFAULT_SHUTDOWN_TIMEOUT, DEFAULT_STDERR_TOTAL_BYTES,
    ExpectedPluginSession, MAX_AGGREGATE_PAYLOAD_BYTES, PluginHostAdmission, PluginHostLimits,
    RequestOutcome, SessionClose,
};
use mengxia_plugin_package::inspect_manifest;
use mengxia_plugin_proto::{
    DecodeDepth, HostEnvelope, HostHello, MAX_IN_FLIGHT_REQUESTS, PROTOCOL_MAJOR, PROTOCOL_MINOR,
    PingRequest, PingResponse, PluginEnvelope, PluginHello, ProtocolCodecError, ShutdownRequest,
    ShutdownResponse, decode_plugin_envelope, encode_host_envelope, encode_plugin_envelope,
    host_envelope, plugin_envelope, read_host_envelope, write_plugin_envelope,
};
use prost::Message;
use prost_types::FileDescriptorSet;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWrite;
use tokio::time::Instant;

#[path = "support/plugin_hostile.rs"]
mod plugin_hostile;
#[path = "support/plugin_supply.rs"]
mod plugin_supply;

const PROTO_SHA256: &str = "4e6cd9898db0c3937c53d9bfd51ec5b3c330bfc7afa3914e6006b459090eee84";
const DESCRIPTOR_SHA256: &str = "b5088d5da6e01671322234a1491cf7d69c0e59488be12540301228053ff73a52";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_owned()
}

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn expected_session() -> ExpectedPluginSession {
    let manifest = include_bytes!("fixtures/task_010/manifest-v1.golden.json");
    let package = inspect_manifest(manifest).expect("TASK-010 golden package remains valid");
    ExpectedPluginSession::new(package.digest(), [0x5a; 32]).expect("non-zero challenge")
}

fn minimum_limits() -> PluginHostLimits {
    PluginHostLimits::new(
        65_536,
        2,
        1,
        1,
        1,
        65_536,
        Duration::from_millis(100),
        Duration::from_millis(100),
        Duration::from_millis(100),
    )
    .expect("minimum accepted limits")
}

fn runtime_ping_value() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock must be after the Unix epoch");
    (elapsed.as_nanos() as u64) ^ u64::from(std::process::id())
}

#[test]
fn plugin_descriptor_source_and_authority_graph_are_exact() {
    let proto = fs::read(root().join("proto/plugin/v1/control.proto")).unwrap();
    let descriptor = fs::read(root().join("proto/plugin/v1/control.pb")).unwrap();
    let provenance = fs::read_to_string(root().join("proto/plugin/v1/control.provenance")).unwrap();
    let build = fs::read_to_string(root().join("crates/mengxia-plugin-proto/build.rs")).unwrap();
    assert_eq!(digest_hex(&proto), PROTO_SHA256);
    assert_eq!(digest_hex(&descriptor), DESCRIPTOR_SHA256);
    assert!(provenance.contains(&format!("proto_sha256={PROTO_SHA256}")));
    assert!(provenance.contains(&format!("descriptor_sha256={DESCRIPTOR_SHA256}")));
    assert!(provenance.contains("protoc_version=35.1"));
    assert!(
        build.contains("config.skip_debug([\".\"]);"),
        "generated protocol values must not expose challenge bytes through Debug"
    );

    let set = FileDescriptorSet::decode(descriptor.as_slice()).unwrap();
    assert_eq!(set.file.len(), 1);
    let file = &set.file[0];
    assert_eq!(file.package.as_deref(), Some("mengxia.plugin.v1"));
    assert!(file.dependency.is_empty());
    let source = std::str::from_utf8(&proto).unwrap();
    for forbidden in [
        "mengxia.core",
        "ClientHello",
        "CoreRequest",
        "Admin",
        "actor",
        "credential",
        "database",
        "cas_path",
        "file_path",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden authority: {forbidden}"
        );
    }
}

#[test]
fn protocol_golden_messages_are_byte_exact() {
    fn hex(name: &str) -> Vec<u8> {
        let text = fs::read_to_string(
            root()
                .join("crates/mengxia-testkit/tests/fixtures/task_011")
                .join(name),
        )
        .unwrap();
        let text = text.trim();
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }
    let hello = HostEnvelope {
        sequence: 0,
        body: Some(host_envelope::Body::Hello(HostHello {
            protocol_major: 1,
            protocol_minor: 0,
            session_challenge: vec![0x5a; 32],
            max_frame_bytes: 65_536,
            max_in_flight_requests: MAX_IN_FLIGHT_REQUESTS,
        })),
    };
    assert_eq!(encode_host_envelope(&hello), hex("host-hello-v1.hex"));
    let plugin_hello = PluginEnvelope {
        sequence: 0,
        body: Some(plugin_envelope::Body::Hello(PluginHello {
            protocol_major: 1,
            protocol_minor: 0,
            session_challenge: vec![0x5a; 32],
        })),
    };
    assert_eq!(
        encode_plugin_envelope(&plugin_hello),
        hex("plugin-hello-v1.hex")
    );
    let ping = HostEnvelope {
        sequence: 1,
        body: Some(host_envelope::Body::Ping(PingRequest {
            nonce: 0x0102_0304_0506_0708,
        })),
    };
    assert_eq!(encode_host_envelope(&ping), hex("ping-sequence-1.hex"));
    let shutdown = HostEnvelope {
        sequence: 2,
        body: Some(host_envelope::Body::Shutdown(ShutdownRequest {})),
    };
    assert_eq!(
        encode_host_envelope(&shutdown),
        hex("shutdown-sequence-2.hex")
    );
}

#[test]
fn task_011_dependency_delta_adds_no_third_party_inventory() {
    plugin_supply::validate_task_011_supply(&root()).unwrap();
}

#[test]
fn closed_wire_scanner_rejects_noncanonical_and_unknown_inputs() {
    let depth = DecodeDepth::new(2).unwrap();
    let valid = PluginEnvelope {
        sequence: 0,
        body: Some(plugin_envelope::Body::Hello(PluginHello {
            protocol_major: 1,
            protocol_minor: 0,
            session_challenge: vec![0x5a; 32],
        })),
    }
    .encode_to_vec();
    assert!(decode_plugin_envelope(&valid, depth).is_ok());

    let cases = [
        (vec![0x30, 0x01], ProtocolCodecError::UnknownOrReservedField),
        (
            vec![0x08, 0x01, 0x08, 0x02],
            ProtocolCodecError::DuplicateField,
        ),
        (
            vec![0x08, 0x81, 0x00],
            ProtocolCodecError::NonCanonicalVarint,
        ),
        (vec![0x08, 0x00], ProtocolCodecError::NonCanonicalEncoding),
        (vec![0x0a, 0x00], ProtocolCodecError::WrongWireType),
        (vec![0x0b], ProtocolCodecError::UnsupportedGroup),
        (vec![0x12, 0x02, 0x08], ProtocolCodecError::Truncated),
        (
            vec![0x2a, 0x02, 0x08, 0x04],
            ProtocolCodecError::InvalidSemanticValue,
        ),
        (vec![0x2a, 0x00], ProtocolCodecError::InvalidSemanticValue),
    ];
    for (bytes, expected) in cases {
        assert!(decode_plugin_envelope(&bytes, depth).err() == Some(expected));
    }
    let mut out_of_order = valid.clone();
    out_of_order.extend_from_slice(&[0x08, 0x01]);
    assert!(
        decode_plugin_envelope(&out_of_order, depth).err()
            == Some(ProtocolCodecError::NonCanonicalEncoding)
    );
    assert!(
        decode_plugin_envelope(&valid, DecodeDepth::new(2).unwrap())
            == Ok(PluginEnvelope::decode(valid.as_slice()).unwrap())
    );
}

#[test]
fn limits_are_tightening_only_and_checked() {
    let defaults = PluginHostLimits::default();
    assert_eq!(defaults.frame_limit().get(), DEFAULT_CONTROL_FRAME_BYTES);
    assert_eq!(defaults.decode_depth().get(), DEFAULT_DECODE_DEPTH);
    assert_eq!(defaults.inbound_frames(), DEFAULT_INBOUND_FRAMES);
    assert_eq!(defaults.outbound_frames(), DEFAULT_OUTBOUND_FRAMES);
    assert_eq!(defaults.active_sessions(), DEFAULT_ACTIVE_SESSIONS);
    assert_eq!(defaults.stderr_total_bytes(), DEFAULT_STDERR_TOTAL_BYTES);
    assert_eq!(defaults.handshake_timeout(), DEFAULT_HANDSHAKE_TIMEOUT);
    assert_eq!(defaults.request_timeout(), DEFAULT_REQUEST_TIMEOUT);
    assert_eq!(defaults.shutdown_timeout(), DEFAULT_SHUTDOWN_TIMEOUT);
    assert_eq!(
        defaults.aggregate_payload_bytes(),
        MAX_AGGREGATE_PAYLOAD_BYTES
    );

    for invalid in [
        PluginHostLimits::new(
            65_535,
            2,
            1,
            1,
            1,
            65_536,
            Duration::from_millis(100),
            Duration::from_millis(100),
            Duration::from_millis(100),
        ),
        PluginHostLimits::new(
            65_536,
            1,
            1,
            1,
            1,
            65_536,
            Duration::from_millis(100),
            Duration::from_millis(100),
            Duration::from_millis(100),
        ),
        PluginHostLimits::new(
            65_536,
            2,
            0,
            1,
            1,
            65_536,
            Duration::from_millis(100),
            Duration::from_millis(100),
            Duration::from_millis(100),
        ),
        PluginHostLimits::new(
            65_536,
            2,
            1,
            1,
            5,
            65_536,
            Duration::from_millis(100),
            Duration::from_millis(100),
            Duration::from_millis(100),
        ),
        PluginHostLimits::new(
            262_145,
            16,
            16,
            16,
            4,
            1_048_576,
            Duration::from_millis(5_000),
            Duration::from_millis(30_000),
            Duration::from_millis(2_000),
        ),
    ] {
        assert_eq!(invalid.unwrap_err().code().as_str(), "VALIDATION_ERROR");
    }

    let build =
        |frame, depth, inbound, outbound, sessions, stderr, handshake, request, shutdown| {
            PluginHostLimits::new(
                frame,
                depth,
                inbound,
                outbound,
                sessions,
                stderr,
                Duration::from_millis(handshake),
                Duration::from_millis(request),
                Duration::from_millis(shutdown),
            )
        };
    for frame in [65_536, 262_143, 262_144] {
        assert!(build(frame, 2, 1, 1, 1, 65_536, 100, 100, 100).is_ok());
    }
    for frame in [65_535, 262_145] {
        assert!(build(frame, 2, 1, 1, 1, 65_536, 100, 100, 100).is_err());
    }
    for depth in [2, 15, 16] {
        assert!(build(65_536, depth, 1, 1, 1, 65_536, 100, 100, 100).is_ok());
    }
    for depth in [1, 17] {
        assert!(build(65_536, depth, 1, 1, 1, 65_536, 100, 100, 100).is_err());
    }
    for value in [1, 15, 16] {
        assert!(build(65_536, 2, value, 1, 1, 65_536, 100, 100, 100).is_ok());
        assert!(build(65_536, 2, 1, value, 1, 65_536, 100, 100, 100).is_ok());
    }
    for value in [0, 17] {
        assert!(build(65_536, 2, value, 1, 1, 65_536, 100, 100, 100).is_err());
        assert!(build(65_536, 2, 1, value, 1, 65_536, 100, 100, 100).is_err());
    }
    for sessions in [1, 3, 4] {
        assert!(build(65_536, 2, 1, 1, sessions, 65_536, 100, 100, 100).is_ok());
    }
    for sessions in [0, 5] {
        assert!(build(65_536, 2, 1, 1, sessions, 65_536, 100, 100, 100).is_err());
    }
    for stderr in [65_536, 1_048_575, 1_048_576] {
        assert!(build(65_536, 2, 1, 1, 1, stderr, 100, 100, 100).is_ok());
    }
    for stderr in [65_535, 1_048_577] {
        assert!(build(65_536, 2, 1, 1, 1, stderr, 100, 100, 100).is_err());
    }
    for handshake in [100, 4_999, 5_000] {
        assert!(build(65_536, 2, 1, 1, 1, 65_536, handshake, 100, 100).is_ok());
    }
    for handshake in [99, 5_001] {
        assert!(build(65_536, 2, 1, 1, 1, 65_536, handshake, 100, 100).is_err());
    }
    for request in [100, 29_999, 30_000] {
        assert!(build(65_536, 2, 1, 1, 1, 65_536, 100, request, 100).is_ok());
    }
    for request in [99, 30_001] {
        assert!(build(65_536, 2, 1, 1, 1, 65_536, 100, request, 100).is_err());
    }
    for shutdown in [100, 1_999, 2_000] {
        assert!(build(65_536, 2, 1, 1, 1, 65_536, 100, 100, shutdown).is_ok());
    }
    for shutdown in [99, 2_001] {
        assert!(build(65_536, 2, 1, 1, 1, 65_536, 100, 100, shutdown).is_err());
    }
}

#[test]
fn admission_is_bounded_before_stream_ownership() {
    let admission = PluginHostAdmission::new(minimum_limits());
    let permit = admission.try_acquire().unwrap();
    assert_eq!(admission.available_permits(), 0);
    assert_eq!(
        admission
            .try_acquire()
            .err()
            .expect("second admission must fail")
            .code()
            .as_str(),
        "BACKPRESSURE"
    );
    drop(permit);
    assert_eq!(admission.available_permits(), 1);
}

#[test]
fn dropping_unpolled_driver_releases_streams_and_admission() {
    let admission = PluginHostAdmission::new(minimum_limits());
    let permit = admission.try_acquire().unwrap();
    let (host_write, _peer_read) = tokio::io::duplex(70_000);
    let (_peer_write, host_read) = tokio::io::duplex(70_000);
    let (_peer_stderr, host_stderr) = tokio::io::duplex(8_192);
    let (driver, session) = permit
        .open(
            host_write,
            host_read,
            host_stderr,
            expected_session(),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
    assert_eq!(admission.available_permits(), 0);
    drop(driver);
    drop(session);
    assert_eq!(admission.available_permits(), 1);
}

#[tokio::test]
async fn one_in_flight_request_applies_backpressure_without_a_side_queue() {
    let limits = minimum_limits();
    let admission = PluginHostAdmission::new(limits);
    let permit = admission.try_acquire().unwrap();
    let (host_write, mut plugin_read) = tokio::io::duplex(70_000);
    let (mut plugin_write, host_read) = tokio::io::duplex(70_000);
    let (_plugin_stderr, host_stderr) = tokio::io::duplex(8_192);
    let deadline = Instant::now() + Duration::from_secs(2);
    let (driver, session) = permit
        .open(
            host_write,
            host_read,
            host_stderr,
            expected_session(),
            deadline,
        )
        .unwrap();
    let peer = tokio::spawn(async move {
        let depth = DecodeDepth::new(2).unwrap();
        let hello = read_host_envelope(&mut plugin_read, limits.frame_limit(), depth)
            .await
            .unwrap();
        let Some(host_envelope::Body::Hello(hello)) = hello.body else {
            panic!("hello expected");
        };
        write_plugin_envelope(
            &mut plugin_write,
            &PluginEnvelope {
                sequence: 0,
                body: Some(plugin_envelope::Body::Hello(PluginHello {
                    protocol_major: 1,
                    protocol_minor: 0,
                    session_challenge: hello.session_challenge,
                })),
            },
            limits.frame_limit(),
        )
        .await
        .unwrap();
        let _ping = read_host_envelope(&mut plugin_read, limits.frame_limit(), depth)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
    });
    let driver = tokio::spawn(driver);
    let first_session = session.clone();
    let first_ping_value = runtime_ping_value();
    let second_ping_value = first_ping_value.wrapping_add(1);
    let first = tokio::spawn(async move { first_session.ping(first_ping_value, deadline).await });
    tokio::task::yield_now().await;
    let second = session.ping(second_ping_value, deadline).await.unwrap_err();
    assert_eq!(second.code().as_str(), "BACKPRESSURE");
    session.cancel();
    assert_eq!(
        first.await.unwrap().unwrap_err().code().as_str(),
        "OPERATION_CANCELLED"
    );
    assert_eq!(
        driver.await.unwrap().unwrap_err().code().as_str(),
        "OPERATION_CANCELLED"
    );
    peer.abort();
    let _ = peer.await;
    drop(session);
    assert_eq!(admission.available_permits(), 1);
}

#[tokio::test]
async fn unsolicited_response_is_detected_while_active_and_closes_the_session() {
    let limits = minimum_limits();
    let admission = PluginHostAdmission::new(limits);
    let permit = admission.try_acquire().unwrap();
    let (host_write, mut plugin_read) = tokio::io::duplex(70_000);
    let (mut plugin_write, host_read) = tokio::io::duplex(70_000);
    let (_plugin_stderr, host_stderr) = tokio::io::duplex(8_192);
    let deadline = Instant::now() + Duration::from_secs(2);
    let (driver, session) = permit
        .open(
            host_write,
            host_read,
            host_stderr,
            expected_session(),
            deadline,
        )
        .unwrap();
    let peer = tokio::spawn(async move {
        let depth = DecodeDepth::new(2).unwrap();
        let hello = read_host_envelope(&mut plugin_read, limits.frame_limit(), depth)
            .await
            .unwrap();
        let Some(host_envelope::Body::Hello(hello)) = hello.body else {
            panic!("hello expected");
        };
        write_plugin_envelope(
            &mut plugin_write,
            &PluginEnvelope {
                sequence: 0,
                body: Some(plugin_envelope::Body::Hello(PluginHello {
                    protocol_major: 1,
                    protocol_minor: 0,
                    session_challenge: hello.session_challenge,
                })),
            },
            limits.frame_limit(),
        )
        .await
        .unwrap();
        write_plugin_envelope(
            &mut plugin_write,
            &PluginEnvelope {
                sequence: 1,
                body: Some(plugin_envelope::Body::Ping(PingResponse { nonce: 1 })),
            },
            limits.frame_limit(),
        )
        .await
        .unwrap();
    });
    let error = driver.await.unwrap_err();
    assert_eq!(error.code().as_str(), "PLUGIN_PROTOCOL_VIOLATION");
    drop(session);
    peer.await.unwrap();
    assert_eq!(admission.available_permits(), 1);
}

#[test]
fn hostile_fixture_inherited_socket_transport_is_real() {
    let (mut parent_in, child_in) = StdUnixStream::pair().unwrap();
    let (mut parent_out, child_out) = StdUnixStream::pair().unwrap();
    let (_parent_err, child_err) = StdUnixStream::pair().unwrap();
    parent_out
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_task_011_hostile_plugin"))
        .arg("valid_ping")
        .stdin(Stdio::from(OwnedFd::from(child_in)))
        .stdout(Stdio::from(OwnedFd::from(child_out)))
        .stderr(Stdio::from(OwnedFd::from(child_err)))
        .spawn()
        .unwrap();
    let hex = "0000002c122a08011a205a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a208080042801";
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect::<Vec<_>>();
    let write = parent_in.write_all(&bytes);
    let mut header = [0_u8; 4];
    let read = if write.is_ok() {
        parent_out.read_exact(&mut header)
    } else {
        Err(std::io::Error::other("hostile transport write failed"))
    };
    let reaped = kill_and_reap_bounded(&mut child);
    write.expect("host hello must traverse the inherited stream");
    read.expect("hostile child must answer through inherited streams");
    reaped.expect("transport proof child must be boundedly killed and reaped");
    assert_eq!(u32::from_be_bytes(header), 38);
}

fn kill_and_reap_bounded(child: &mut std::process::Child) -> Result<(), &'static str> {
    if child
        .try_wait()
        .map_err(|_| "initial try_wait failed")?
        .is_none()
        && child.kill().is_err()
        && child
            .try_wait()
            .map_err(|_| "post-kill-race try_wait failed")?
            .is_none()
    {
        return Err("child kill failed");
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if child
            .try_wait()
            .map_err(|_| "bounded reap try_wait failed")?
            .is_some()
        {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err("child reap timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[tokio::test]
async fn handshake_ping_and_shutdown_are_correlated_and_joined() {
    let limits = minimum_limits();
    let admission = PluginHostAdmission::new(limits);
    let permit = admission.try_acquire().unwrap();
    let (host_write, mut plugin_read) = tokio::io::duplex(70_000);
    let (mut plugin_write, host_read) = tokio::io::duplex(70_000);
    let (_plugin_stderr, host_stderr) = tokio::io::duplex(8_192);
    let deadline = Instant::now() + Duration::from_secs(2);
    let (driver, session) = permit
        .open(
            host_write,
            host_read,
            host_stderr,
            expected_session(),
            deadline,
        )
        .unwrap();

    let peer = tokio::spawn(async move {
        let depth = DecodeDepth::new(2).unwrap();
        let hello = read_host_envelope(&mut plugin_read, limits.frame_limit(), depth)
            .await
            .unwrap();
        let Some(host_envelope::Body::Hello(hello)) = hello.body else {
            panic!("host hello expected");
        };
        write_plugin_envelope(
            &mut plugin_write,
            &PluginEnvelope {
                sequence: 0,
                body: Some(plugin_envelope::Body::Hello(PluginHello {
                    protocol_major: PROTOCOL_MAJOR,
                    protocol_minor: PROTOCOL_MINOR,
                    session_challenge: hello.session_challenge,
                })),
            },
            limits.frame_limit(),
        )
        .await
        .unwrap();
        let ping = read_host_envelope(&mut plugin_read, limits.frame_limit(), depth)
            .await
            .unwrap();
        let nonce = match ping.body {
            Some(host_envelope::Body::Ping(ping)) => ping.nonce,
            _ => panic!("ping expected"),
        };
        write_plugin_envelope(
            &mut plugin_write,
            &PluginEnvelope {
                sequence: ping.sequence,
                body: Some(plugin_envelope::Body::Ping(PingResponse { nonce })),
            },
            limits.frame_limit(),
        )
        .await
        .unwrap();
        let shutdown = read_host_envelope(&mut plugin_read, limits.frame_limit(), depth)
            .await
            .unwrap();
        assert!(matches!(
            shutdown.body,
            Some(host_envelope::Body::Shutdown(_))
        ));
        write_plugin_envelope(
            &mut plugin_write,
            &PluginEnvelope {
                sequence: shutdown.sequence,
                body: Some(plugin_envelope::Body::Shutdown(ShutdownResponse {})),
            },
            limits.frame_limit(),
        )
        .await
        .unwrap();
    });
    let driver = tokio::spawn(driver);
    let ping_value = runtime_ping_value();

    assert_eq!(
        session.ping(ping_value, deadline).await.unwrap(),
        RequestOutcome::Completed(ping_value)
    );
    assert_eq!(
        session.shutdown(deadline).await.unwrap(),
        RequestOutcome::Completed(())
    );
    drop(session);
    assert_eq!(
        driver.await.unwrap().unwrap(),
        SessionClose::CooperativeShutdown
    );
    peer.await.unwrap();
    assert_eq!(admission.available_permits(), 1);
}

#[tokio::test]
async fn wrong_challenge_and_timeout_fail_closed_without_disclosure() {
    let limits = minimum_limits();
    let admission = PluginHostAdmission::new(limits);
    let permit = admission.try_acquire().unwrap();
    let (host_write, mut plugin_read) = tokio::io::duplex(70_000);
    let (mut plugin_write, host_read) = tokio::io::duplex(70_000);
    let (_plugin_stderr, host_stderr) = tokio::io::duplex(8_192);
    let deadline = Instant::now() + Duration::from_secs(1);
    let (driver, session) = permit
        .open(
            host_write,
            host_read,
            host_stderr,
            expected_session(),
            deadline,
        )
        .unwrap();
    let peer = tokio::spawn(async move {
        let depth = DecodeDepth::new(2).unwrap();
        let _ = read_host_envelope(&mut plugin_read, limits.frame_limit(), depth)
            .await
            .unwrap();
        write_plugin_envelope(
            &mut plugin_write,
            &PluginEnvelope {
                sequence: 0,
                body: Some(plugin_envelope::Body::Hello(PluginHello {
                    protocol_major: 1,
                    protocol_minor: 0,
                    session_challenge: vec![0x7f; 32],
                })),
            },
            limits.frame_limit(),
        )
        .await
        .unwrap();
    });
    let error = driver.await.unwrap_err();
    drop(session);
    assert_eq!(error.code().as_str(), "PLUGIN_PROTOCOL_VIOLATION");
    assert_eq!(error.to_string(), "PLUGIN_PROTOCOL_VIOLATION");
    assert!(!format!("{error:?}").contains("7f"));
    peer.await.unwrap();
    assert_eq!(admission.available_permits(), 1);

    let admission = PluginHostAdmission::new(limits);
    let permit = admission.try_acquire().unwrap();
    let (host_write, _plugin_read) = tokio::io::duplex(70_000);
    let (_plugin_write, host_read) = tokio::io::duplex(70_000);
    let (_plugin_stderr, host_stderr) = tokio::io::duplex(8_192);
    let (driver, session) = permit
        .open(
            host_write,
            host_read,
            host_stderr,
            expected_session(),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
    drop(session);
    let error = driver.await.unwrap_err();
    assert_eq!(error.code().as_str(), "OPERATION_CANCELLED");
    assert_eq!(admission.available_permits(), 1);
}

#[tokio::test]
async fn silent_handshake_uses_one_absolute_deadline_and_releases_admission() {
    let limits = minimum_limits();
    let admission = PluginHostAdmission::new(limits);
    let permit = admission.try_acquire().unwrap();
    let (host_write, _plugin_read) = tokio::io::duplex(70_000);
    let (_plugin_write, host_read) = tokio::io::duplex(70_000);
    let (_plugin_stderr, host_stderr) = tokio::io::duplex(8_192);
    let (driver, session) = permit
        .open(
            host_write,
            host_read,
            host_stderr,
            expected_session(),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
    let started = Instant::now();
    let error = driver.await.unwrap_err();
    assert_eq!(error.code().as_str(), "DEADLINE_EXCEEDED");
    assert!(started.elapsed() >= Duration::from_millis(100));
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(session);
    assert_eq!(admission.available_permits(), 1);
}

#[derive(Clone, Copy)]
enum StallPoint {
    FirstWrite,
    PartialWrite,
    Flush,
}

struct StallingWriter {
    point: StallPoint,
    wrote_partial: bool,
    dropped: Arc<AtomicBool>,
}

impl AsyncWrite for StallingWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.point {
            StallPoint::FirstWrite => Poll::Pending,
            StallPoint::PartialWrite if !self.wrote_partial => {
                self.wrote_partial = true;
                Poll::Ready(Ok(bytes.len().min(2)))
            }
            StallPoint::PartialWrite => Poll::Pending,
            StallPoint::Flush => Poll::Ready(Ok(bytes.len())),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.point {
            StallPoint::Flush => Poll::Pending,
            StallPoint::FirstWrite | StallPoint::PartialWrite => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl Drop for StallingWriter {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

#[tokio::test]
async fn pending_and_partial_handshake_writes_and_flushes_are_deadline_bounded() {
    for point in [
        StallPoint::FirstWrite,
        StallPoint::PartialWrite,
        StallPoint::Flush,
    ] {
        let limits = minimum_limits();
        let admission = PluginHostAdmission::new(limits);
        let permit = admission.try_acquire().unwrap();
        let dropped = Arc::new(AtomicBool::new(false));
        let writer = StallingWriter {
            point,
            wrote_partial: false,
            dropped: Arc::clone(&dropped),
        };
        let (_plugin_write, host_read) = tokio::io::duplex(70_000);
        let (_plugin_stderr, host_stderr) = tokio::io::duplex(8_192);
        let (driver, session) = permit
            .open(
                writer,
                host_read,
                host_stderr,
                expected_session(),
                Instant::now() + Duration::from_secs(1),
            )
            .unwrap();

        let error = driver.await.unwrap_err();
        assert_eq!(error.code().as_str(), "DEADLINE_EXCEEDED");
        assert!(dropped.load(Ordering::Acquire));
        drop(session);
        assert_eq!(admission.available_permits(), 1);
    }
}

#[tokio::test]
async fn hostile_plugin_matrix_is_fail_closed_and_every_child_is_reaped() {
    use mengxia_plugin_host::{PluginRequestFailure, TerminationRequired};

    let valid = plugin_hostile::exercise("valid_ping").await;
    assert_eq!(
        valid.driver,
        Ok(SessionClose::CooperativeShutdown),
        "{valid:?}"
    );
    assert_eq!(
        valid.ping,
        Some(Ok(RequestOutcome::Completed(valid.ping_value)))
    );
    assert_eq!(valid.shutdown, Some(Ok(RequestOutcome::Completed(()))));
    assert!(valid.status.success());

    for (action, code) in [
        ("zero_frame", "PLUGIN_PROTOCOL_VIOLATION"),
        ("oversized_frame", "PLUGIN_PROTOCOL_VIOLATION"),
        ("stdout_flood", "PLUGIN_PROTOCOL_VIOLATION"),
        ("truncated_header", "IPC_TRANSPORT_ERROR"),
        ("truncated_payload", "IPC_TRANSPORT_ERROR"),
        ("unknown_field", "PLUGIN_PROTOCOL_VIOLATION"),
        ("duplicate_field", "PLUGIN_PROTOCOL_VIOLATION"),
        ("nonminimal_varint", "PLUGIN_PROTOCOL_VIOLATION"),
        ("wrong_wire_type", "PLUGIN_PROTOCOL_VIOLATION"),
        ("wrong_challenge", "PLUGIN_PROTOCOL_VIOLATION"),
        ("wrong_version", "PROTOCOL_VERSION_UNSUPPORTED"),
        ("stdout_text", "PLUGIN_PROTOCOL_VIOLATION"),
        ("core_protocol_frame", "PLUGIN_PROTOCOL_VIOLATION"),
        ("close_before_hello", "IPC_TRANSPORT_ERROR"),
        ("hang_before_hello", "DEADLINE_EXCEEDED"),
        ("failure_during_hello", "PLUGIN_PROTOCOL_VIOLATION"),
    ] {
        let result = plugin_hostile::exercise(action).await;
        let error = result.driver.expect_err(action);
        assert_eq!(error.code().as_str(), code, "{action}");
        assert!(!format!("{error:?}").contains(action));
    }

    for (action, code) in [
        ("close_during_response", "IPC_TRANSPORT_ERROR"),
        ("panic_after_hello", "IPC_TRANSPORT_ERROR"),
        ("wrong_sequence", "PLUGIN_PROTOCOL_VIOLATION"),
        ("failure_unspecified", "PLUGIN_PROTOCOL_VIOLATION"),
        ("failure_unknown", "PLUGIN_PROTOCOL_VIOLATION"),
    ] {
        let result = plugin_hostile::exercise(action).await;
        let error = result.driver.expect_err(action);
        assert_eq!(error.code().as_str(), code, "{action}");
    }

    let hung = plugin_hostile::exercise("hang_during_response").await;
    assert_eq!(
        hung.ping.unwrap().unwrap_err().code().as_str(),
        "DEADLINE_EXCEEDED"
    );
    assert_eq!(
        hung.driver.unwrap_err().code().as_str(),
        "OPERATION_CANCELLED"
    );

    for (action, failure) in [
        (
            "ping_failure_unsupported",
            PluginRequestFailure::UnsupportedRequest,
        ),
        ("ping_failure_invalid", PluginRequestFailure::InvalidRequest),
        ("ping_failure_internal", PluginRequestFailure::Internal),
    ] {
        let result = plugin_hostile::exercise(action).await;
        plugin_hostile::assert_rejected(result.ping.as_ref().unwrap(), failure);
        assert_eq!(result.driver, Ok(SessionClose::CooperativeShutdown));
    }

    let at_cap = plugin_hostile::exercise("stderr_at_cap").await;
    assert_eq!(
        at_cap.ping,
        Some(Ok(RequestOutcome::Completed(at_cap.ping_value)))
    );
    assert_eq!(at_cap.driver, Ok(SessionClose::CooperativeShutdown));

    let ignored = plugin_hostile::exercise("ignore_shutdown").await;
    assert_eq!(
        ignored.shutdown.unwrap().unwrap_err().code().as_str(),
        "DEADLINE_EXCEEDED"
    );
    assert_eq!(
        ignored.driver.unwrap_err().code().as_str(),
        "OPERATION_CANCELLED"
    );

    let duplicate = plugin_hostile::exercise("duplicate_failure").await;
    plugin_hostile::assert_rejected(
        duplicate.ping.as_ref().unwrap(),
        PluginRequestFailure::UnsupportedRequest,
    );
    assert_eq!(
        duplicate.driver.unwrap_err().code().as_str(),
        "PLUGIN_PROTOCOL_VIOLATION"
    );

    for action in [
        "shutdown_failure_unsupported",
        "shutdown_failure_invalid",
        "shutdown_failure_internal",
    ] {
        let result = plugin_hostile::exercise(action).await;
        let error = result.driver.expect_err(action);
        plugin_hostile::assert_termination(error, TerminationRequired::ShutdownRejected);
        assert!(matches!(
            result.shutdown,
            Some(Ok(RequestOutcome::Rejected(_)))
        ));
    }

    for action in ["stderr_cap_plus_one", "stderr_flood"] {
        let result = plugin_hostile::exercise(action).await;
        let error = result.driver.expect_err(action);
        plugin_hostile::assert_termination(error, TerminationRequired::ResourceLimit);
    }
}
