//! MengXia command-line composition root.

#![forbid(unsafe_code)]

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use mengxia_core_proto::{DecodeDepth, HandshakeLimits, NegotiatedHandshake, request_handshake};
use mengxia_framing::FrameLimit;
use mengxia_platform_fs::{
    AuthorityError, effective_user_id, validate_client_endpoint, validate_runtime_endpoint_path,
};
use mengxia_types::{ErrorCode, Id};

const HELP: &str = "mengxia handshake [--client-endpoint PATH] [--max-frame-bytes ASCII_U64]\n  [--max-decode-depth ASCII_U32]\n  [--client-handshake-timeout-ms ASCII_U64]\n";

struct RequestIdentity;

fn main() -> ExitCode {
    match parse_command(env::args_os().skip(1).collect()) {
        Ok(Command::Help) => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(Command::Handshake(cli)) => match resolve(cli) {
            Ok(config) => run(config),
            Err(code) => fail(code, 2),
        },
        Err(code) => fail(code, 2),
    }
}

fn run(config: ClientConfig) -> ExitCode {
    let owner_uid = effective_user_id();
    let endpoint = match validate_client_endpoint(&config.endpoint, owner_uid) {
        Ok(endpoint) => endpoint,
        Err(error) => return fail(authority_code(error), 1),
    };
    let request_id = match Id::<RequestIdentity>::try_new() {
        Ok(id) => id.to_string(),
        Err(_) => return fail(ErrorCode::IdGenerationUnavailable, 1),
    };
    let std_stream = match endpoint.connect() {
        Ok(stream) => stream,
        Err(_) => return fail(ErrorCode::IpcTransportError, 1),
    };
    if std_stream.set_nonblocking(true).is_err() {
        return fail(ErrorCode::IpcTransportError, 1);
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return fail(ErrorCode::InternalError, 1),
    };
    match runtime.block_on(handshake(std_stream, owner_uid, &request_id, config.limits)) {
        Ok(negotiated) => success(&negotiated),
        Err(code) => fail(code, 1),
    }
}

async fn handshake(
    stream: std::os::unix::net::UnixStream,
    owner_uid: u32,
    request_id: &str,
    limits: HandshakeLimits,
) -> Result<NegotiatedHandshake, ErrorCode> {
    let mut stream =
        tokio::net::UnixStream::from_std(stream).map_err(|_| ErrorCode::IpcTransportError)?;
    let peer = stream
        .peer_cred()
        .map_err(|_| ErrorCode::AuthenticationError)?;
    if peer.uid() != owner_uid {
        return Err(ErrorCode::AuthenticationError);
    }
    request_handshake(&mut stream, request_id, limits)
        .await
        .map_err(|error| error.code())
}

#[derive(Default)]
struct HandshakeCli {
    endpoint: Option<OsString>,
    frame: Option<OsString>,
    depth: Option<OsString>,
    timeout: Option<OsString>,
}

enum Command {
    Help,
    Handshake(HandshakeCli),
}

fn parse_command(args: Vec<OsString>) -> Result<Command, ErrorCode> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(Command::Help);
    }
    if args.first().is_none_or(|arg| arg != "handshake") {
        return Err(ErrorCode::ValidationError);
    }
    let mut cli = HandshakeCli::default();
    let mut index = 1;
    while index < args.len() {
        let option = args[index].to_str().ok_or(ErrorCode::ValidationError)?;
        let value = args.get(index + 1).ok_or(ErrorCode::ValidationError)?;
        let slot = match option {
            "--client-endpoint" => &mut cli.endpoint,
            "--max-frame-bytes" => &mut cli.frame,
            "--max-decode-depth" => &mut cli.depth,
            "--client-handshake-timeout-ms" => &mut cli.timeout,
            _ => return Err(ErrorCode::ValidationError),
        };
        if slot.is_some() {
            return Err(ErrorCode::ValidationError);
        }
        *slot = Some(value.clone());
        index += 2;
    }
    Ok(Command::Handshake(cli))
}

struct ClientConfig {
    endpoint: PathBuf,
    limits: HandshakeLimits,
}

fn resolve(cli: HandshakeCli) -> Result<ClientConfig, ErrorCode> {
    resolve_from_layers(
        cli,
        ClientEnvironment::capture(),
        ClientLibraryConfig::default(),
    )
}

#[derive(Default)]
struct ClientLibraryConfig {
    endpoint: Option<PathBuf>,
    frame_bytes: Option<u64>,
    decode_depth: Option<u64>,
    handshake_timeout_ms: Option<u64>,
}

struct ClientEnvironment {
    endpoint: Option<OsString>,
    frame_bytes: Option<OsString>,
    decode_depth: Option<OsString>,
    handshake_timeout_ms: Option<OsString>,
    platform_temp_root: PathBuf,
}

impl ClientEnvironment {
    fn capture() -> Self {
        Self {
            endpoint: env::var_os("MENGXIA_CLIENT_ENDPOINT"),
            frame_bytes: env::var_os("MENGXIA_MAX_FRAME_BYTES"),
            decode_depth: env::var_os("MENGXIA_MAX_DECODE_DEPTH"),
            handshake_timeout_ms: env::var_os("MENGXIA_CLIENT_HANDSHAKE_TIMEOUT_MS"),
            platform_temp_root: env::temp_dir(),
        }
    }
}

fn resolve_from_layers(
    cli: HandshakeCli,
    environment: ClientEnvironment,
    library: ClientLibraryConfig,
) -> Result<ClientConfig, ErrorCode> {
    let endpoint = cli
        .endpoint
        .map(PathBuf::from)
        .or_else(|| environment.endpoint.map(PathBuf::from))
        .or(library.endpoint)
        .map_or_else(
            || {
                std::fs::canonicalize(environment.platform_temp_root)
                    .map(|root| root.join("mengxia-runtime-v1/client.sock"))
            },
            Ok,
        )
        .map_err(|_| ErrorCode::ValidationError)?;
    validate_runtime_endpoint_path(&endpoint).map_err(|_| ErrorCode::ValidationError)?;
    let frame = select_u64(
        cli.frame,
        environment.frame_bytes,
        library.frame_bytes,
        4 * 1024 * 1024,
    )?;
    let frame = u32::try_from(frame)
        .ok()
        .and_then(|value| FrameLimit::new(value).ok())
        .ok_or(ErrorCode::ValidationError)?;
    let depth = select_u64(
        cli.depth,
        environment.decode_depth,
        library.decode_depth,
        64,
    )?;
    let depth = u8::try_from(depth)
        .ok()
        .and_then(|value| DecodeDepth::new(value).ok())
        .ok_or(ErrorCode::ValidationError)?;
    let timeout = select_u64(
        cli.timeout,
        environment.handshake_timeout_ms,
        library.handshake_timeout_ms,
        5_000,
    )?;
    let limits = HandshakeLimits::new(frame, depth, Duration::from_millis(timeout))
        .map_err(|error| error.code())?;
    Ok(ClientConfig { endpoint, limits })
}

fn select_u64(
    cli: Option<OsString>,
    environment: Option<OsString>,
    library: Option<u64>,
    default: u64,
) -> Result<u64, ErrorCode> {
    if let Some(value) = cli {
        parse_ascii_u64(&value)
    } else if let Some(value) = environment {
        parse_ascii_u64(&value)
    } else {
        Ok(library.unwrap_or(default))
    }
}

fn parse_ascii_u64(value: &OsStr) -> Result<u64, ErrorCode> {
    let text = value.to_str().ok_or(ErrorCode::ValidationError)?;
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ErrorCode::ValidationError);
    }
    text.parse().map_err(|_| ErrorCode::ValidationError)
}

fn success(negotiated: &NegotiatedHandshake) -> ExitCode {
    println!(
        "MENGXIA_HANDSHAKE_OK protocol=1.0 request_id={} correlation_id={}",
        negotiated.request_id(),
        negotiated.correlation_id()
    );
    ExitCode::SUCCESS
}

fn authority_code(error: AuthorityError) -> ErrorCode {
    match error {
        AuthorityError::UnsafeConfiguration | AuthorityError::Contended => {
            ErrorCode::StorageConfigurationError
        }
        AuthorityError::Io => ErrorCode::StorageIoError,
        AuthorityError::ConflictingData => ErrorCode::StorageCorruption,
        _ => ErrorCode::InternalError,
    }
}

fn fail(code: ErrorCode, status: u8) -> ExitCode {
    eprintln!("MENGXIA_ERROR code={}", code.as_str());
    ExitCode::from(status)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use mengxia_types::ErrorCode;

    use super::{
        ClientEnvironment, ClientLibraryConfig, Command, HandshakeCli, parse_ascii_u64,
        parse_command, resolve_from_layers,
    };

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn exact_client_grammar_accepts_only_help_or_handshake() {
        assert!(matches!(
            parse_command(args(&["--help"])),
            Ok(Command::Help)
        ));
        assert!(matches!(
            parse_command(args(&[
                "handshake",
                "--client-endpoint",
                "/private/tmp/runtime/client.sock",
            ])),
            Ok(Command::Handshake(_))
        ));
        for invalid in [
            args(&[]),
            args(&["handshake", "--help"]),
            args(&["handshake", "--max-frame-bytes=65536"]),
            args(&["handshake", "--client-endpoint"]),
            args(&[
                "handshake",
                "--max-decode-depth",
                "3",
                "--max-decode-depth",
                "4",
            ]),
            args(&["unknown"]),
        ] {
            assert_eq!(
                parse_command(invalid).err(),
                Some(ErrorCode::ValidationError)
            );
        }
    }

    #[test]
    fn numeric_values_are_unsigned_ascii_decimal_only() {
        assert_eq!(parse_ascii_u64(&OsString::from("65536")), Ok(65536));
        for invalid in ["", " 1", "+1", "-1", "1_0", "18446744073709551616"] {
            assert_eq!(
                parse_ascii_u64(&OsString::from(invalid)),
                Err(ErrorCode::ValidationError)
            );
        }
    }

    #[test]
    fn typed_layers_obey_cli_environment_library_default_precedence() {
        let endpoint = PathBuf::from("/private/tmp/task003-client-resolver/client.sock");
        let config = resolve_from_layers(
            HandshakeCli {
                endpoint: Some(endpoint.clone().into_os_string()),
                frame: Some(OsString::from("65536")),
                depth: Some(OsString::from("3")),
                timeout: Some(OsString::from("100")),
            },
            ClientEnvironment {
                endpoint: Some(OsString::from("invalid-relative-endpoint")),
                frame_bytes: Some(OsString::from("invalid")),
                decode_depth: Some(OsString::from("invalid")),
                handshake_timeout_ms: Some(OsString::from("invalid")),
                platform_temp_root: PathBuf::from("/private/tmp"),
            },
            ClientLibraryConfig {
                endpoint: Some(PathBuf::from("/private/tmp/lower/client.sock")),
                frame_bytes: Some(131_072),
                decode_depth: Some(4),
                handshake_timeout_ms: Some(200),
            },
        )
        .unwrap();
        assert_eq!(config.endpoint, endpoint);
        assert_eq!(
            config.limits.timeout(),
            std::time::Duration::from_millis(100)
        );

        let invalid_higher_layer = resolve_from_layers(
            HandshakeCli {
                endpoint: Some(OsString::from(
                    "/private/tmp/task003-client-resolver/client.sock",
                )),
                ..HandshakeCli::default()
            },
            ClientEnvironment {
                endpoint: None,
                frame_bytes: Some(OsString::from("invalid")),
                decode_depth: None,
                handshake_timeout_ms: None,
                platform_temp_root: PathBuf::from("/private/tmp"),
            },
            ClientLibraryConfig {
                frame_bytes: Some(65_536),
                ..ClientLibraryConfig::default()
            },
        );
        assert!(matches!(
            invalid_higher_layer,
            Err(ErrorCode::ValidationError)
        ));
    }
}
