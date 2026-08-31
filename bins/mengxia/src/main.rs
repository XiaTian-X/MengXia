//! MengXia command-line composition root.

#![forbid(unsafe_code)]

use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::time::Duration;

use mengxia_app::{LibraryConfigDocument, LibraryConfigKey};
use mengxia_core_proto::{
    CoreRequest, DecodeDepth, HandshakeLimits, IngestAssetCopyRequest, IngestMode,
    NegotiatedHandshake, OperationLimits, RetryAction, core_request, core_response,
    operation_safe_message, request_handshake, request_single_command, valid_operation_retry_pair,
};
use mengxia_framing::FrameLimit;
use mengxia_platform_fs::{
    AuthorityError, effective_user_id, read_library_config, validate_client_endpoint,
    validate_runtime_endpoint_path,
};
use mengxia_types::{ErrorCode, Id};

const HELP: &str = "mengxia handshake [--client-endpoint PATH] [--max-frame-bytes ASCII_U64]\n  [--max-decode-depth ASCII_U32] [--client-handshake-timeout-ms ASCII_U64]\n\
mengxia asset ingest-copy SOURCE --command-id UUIDV7 --asset-kind TOKEN\n  --content-kind TOKEN --representation-purpose TOKEN --resource-kind TOKEN\n  --logical-name UTF8 [--expected-sha256 LOWERCASE_HEX_64]\n  [--operation-timeout-ms ASCII_U64] [existing client transport options]\n";

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
        Ok(Command::Ingest(cli)) => match resolve_ingest(*cli) {
            Ok(config) => run_ingest(config),
            Err(code) => fail_with_retry(code, RetryAction::None, 2),
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
    library_config: Option<OsString>,
    endpoint: Option<OsString>,
    frame: Option<OsString>,
    depth: Option<OsString>,
    timeout: Option<OsString>,
}

enum Command {
    Help,
    Handshake(HandshakeCli),
    Ingest(Box<IngestCli>),
}

#[derive(Default)]
struct IngestCli {
    source: Option<OsString>,
    command_id: Option<OsString>,
    asset_kind: Option<OsString>,
    content_kind: Option<OsString>,
    representation_purpose: Option<OsString>,
    resource_kind: Option<OsString>,
    logical_name: Option<OsString>,
    expected_sha256: Option<OsString>,
    operation_timeout: Option<OsString>,
    transport: HandshakeCli,
}

fn parse_command(args: Vec<OsString>) -> Result<Command, ErrorCode> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(Command::Help);
    }
    if args.first().is_some_and(|arg| arg == "asset") {
        return parse_ingest_command(args)
            .map(Box::new)
            .map(Command::Ingest);
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
            "--library-config" => &mut cli.library_config,
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

fn parse_ingest_command(args: Vec<OsString>) -> Result<IngestCli, ErrorCode> {
    if args.len() < 3 || args[1] != "ingest-copy" || args[2].as_os_str().is_empty() {
        return Err(ErrorCode::ValidationError);
    }
    let mut cli = IngestCli {
        source: Some(args[2].clone()),
        ..IngestCli::default()
    };
    let mut index = 3;
    while index < args.len() {
        let option = args[index].to_str().ok_or(ErrorCode::ValidationError)?;
        let value = args.get(index + 1).ok_or(ErrorCode::ValidationError)?;
        let slot = match option {
            "--command-id" => &mut cli.command_id,
            "--asset-kind" => &mut cli.asset_kind,
            "--content-kind" => &mut cli.content_kind,
            "--representation-purpose" => &mut cli.representation_purpose,
            "--resource-kind" => &mut cli.resource_kind,
            "--logical-name" => &mut cli.logical_name,
            "--expected-sha256" => &mut cli.expected_sha256,
            "--operation-timeout-ms" => &mut cli.operation_timeout,
            "--client-endpoint" => &mut cli.transport.endpoint,
            "--library-config" => &mut cli.transport.library_config,
            "--max-frame-bytes" => &mut cli.transport.frame,
            "--max-decode-depth" => &mut cli.transport.depth,
            "--client-handshake-timeout-ms" => &mut cli.transport.timeout,
            _ => return Err(ErrorCode::ValidationError),
        };
        if slot.is_some() {
            return Err(ErrorCode::ValidationError);
        }
        *slot = Some(value.clone());
        index += 2;
    }
    if cli.command_id.is_none()
        || cli.asset_kind.is_none()
        || cli.content_kind.is_none()
        || cli.representation_purpose.is_none()
        || cli.resource_kind.is_none()
        || cli.logical_name.is_none()
    {
        return Err(ErrorCode::ValidationError);
    }
    Ok(cli)
}

struct ClientConfig {
    endpoint: PathBuf,
    limits: HandshakeLimits,
    operation_limits: OperationLimits,
    operation_timeout: Duration,
}

struct IngestConfig {
    client: ClientConfig,
    request: CoreRequest,
    operation_timeout: Duration,
}

struct CommandIdentity;
struct ResultIdentity;

fn resolve_ingest(mut cli: IngestCli) -> Result<IngestConfig, ErrorCode> {
    let source = cli.source.take().ok_or(ErrorCode::ValidationError)?;
    let source_path = source.as_os_str().as_bytes();
    if !(1..=1023).contains(&source_path.len())
        || source_path.contains(&0)
        || !normalized_absolute_bytes(source_path)
    {
        return Err(ErrorCode::ValidationError);
    }
    let command_text = selected_utf8(cli.command_id.take())?;
    let command =
        Id::<CommandIdentity>::from_str(&command_text).map_err(|_| ErrorCode::ValidationError)?;
    if command.to_string() != command_text {
        return Err(ErrorCode::ValidationError);
    }
    let asset_kind = selected_utf8(cli.asset_kind.take())?;
    let content_kind = selected_utf8(cli.content_kind.take())?;
    let representation_purpose = selected_utf8(cli.representation_purpose.take())?;
    let resource_kind = selected_utf8(cli.resource_kind.take())?;
    let logical_name = selected_utf8(cli.logical_name.take())?;
    let expected_sha256 = cli
        .expected_sha256
        .take()
        .map(|value| parse_sha256(&value).map(Vec::from))
        .transpose()?;
    let explicit_operation_timeout = cli.operation_timeout.take();
    let client = resolve_with_operation(cli.transport, explicit_operation_timeout)?;
    let operation_timeout_ms = u64::try_from(client.operation_timeout.as_millis())
        .map_err(|_| ErrorCode::ValidationError)?;
    if !(100..=86_400_000).contains(&operation_timeout_ms) {
        return Err(ErrorCode::ValidationError);
    }
    let request = CoreRequest {
        operation: Some(core_request::Operation::IngestAssetCopy(
            IngestAssetCopyRequest {
                command_id: command.to_string(),
                source_path: source_path.to_vec(),
                mode: IngestMode::Copy as i32,
                asset_kind,
                content_kind,
                representation_purpose,
                resource_kind,
                logical_name,
                expected_sha256,
                operation_timeout_ms,
            },
        )),
    };
    Ok(IngestConfig {
        client,
        request,
        operation_timeout: Duration::from_millis(operation_timeout_ms),
    })
}

fn selected_utf8(value: Option<OsString>) -> Result<String, ErrorCode> {
    value
        .ok_or(ErrorCode::ValidationError)?
        .into_string()
        .map_err(|_| ErrorCode::ValidationError)
}

fn parse_sha256(value: &OsStr) -> Result<[u8; 32], ErrorCode> {
    let bytes = value.as_bytes();
    if bytes.len() != 64
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(ErrorCode::ValidationError);
    }
    let mut output = [0_u8; 32];
    for (index, pair) in bytes.as_chunks::<2>().0.iter().enumerate() {
        output[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> Result<u8, ErrorCode> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(ErrorCode::ValidationError),
    }
}

fn normalized_absolute_bytes(path: &[u8]) -> bool {
    path.first() == Some(&b'/')
        && path.len() > 1
        && !path.ends_with(b"/")
        && path[1..]
            .split(|byte| *byte == b'/')
            .all(|component| !component.is_empty() && component != b"." && component != b"..")
}

fn run_ingest(config: IngestConfig) -> ExitCode {
    let owner_uid = effective_user_id();
    let endpoint = match validate_client_endpoint(&config.client.endpoint, owner_uid) {
        Ok(endpoint) => endpoint,
        Err(error) => return fail_with_retry(authority_code(error), RetryAction::SameCommand, 1),
    };
    let request_id = match Id::<CommandIdentity>::try_new() {
        Ok(id) => id.to_string(),
        Err(_) => {
            return fail_with_retry(
                ErrorCode::IdGenerationUnavailable,
                RetryAction::OperatorOrRuntimeAction,
                1,
            );
        }
    };
    let std_stream = match endpoint.connect() {
        Ok(stream) => stream,
        Err(_) => {
            return fail_with_retry(ErrorCode::IpcTransportError, RetryAction::SameCommand, 1);
        }
    };
    if std_stream.set_nonblocking(true).is_err() {
        return fail_with_retry(ErrorCode::IpcTransportError, RetryAction::SameCommand, 1);
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            return fail_with_retry(
                ErrorCode::InternalError,
                RetryAction::OperatorOrRuntimeAction,
                1,
            );
        }
    };
    let result = runtime.block_on(async {
        let mut stream = tokio::net::UnixStream::from_std(std_stream)
            .map_err(|_| ErrorCode::IpcTransportError)?;
        let peer = stream
            .peer_cred()
            .map_err(|_| ErrorCode::AuthenticationError)?;
        if peer.uid() != owner_uid {
            return Err(ErrorCode::AuthenticationError);
        }
        request_single_command(
            &mut stream,
            &request_id,
            &config.request,
            config.client.limits,
            config.client.operation_limits,
            config.operation_timeout,
        )
        .await
        .map_err(|error| error.code())
    });
    match result {
        Ok((session, response)) => handle_ingest_response(session.correlation_id(), response),
        Err(code @ (ErrorCode::AuthenticationError | ErrorCode::ProtocolVersionUnsupported)) => {
            fail_with_retry(code, RetryAction::OperatorOrRuntimeAction, 1)
        }
        Err(code) => fail_with_retry(code, RetryAction::SameCommand, 1),
    }
}

fn handle_ingest_response(
    correlation_id: &str,
    response: mengxia_core_proto::CoreResponse,
) -> ExitCode {
    match response.response {
        Some(core_response::Response::IngestAssetCopy(result)) => {
            let ids = [
                &result.asset_id,
                &result.asset_revision_id,
                &result.representation_id,
                &result.resource_id,
                &result.location_id,
            ];
            if ids.iter().any(|value| {
                !Id::<ResultIdentity>::from_str(value)
                    .is_ok_and(|parsed| parsed.to_string() == **value)
            }) || result.blob_sha256.len() != 32
            {
                return fail_with_retry(ErrorCode::IpcTransportError, RetryAction::SameCommand, 1);
            }
            println!(
                "MENGXIA_ASSET_INGEST_OK operation=asset.ingest.v1 asset_id={} asset_revision_id={} representation_id={} resource_id={} location_id={} blob_sha256={}",
                result.asset_id,
                result.asset_revision_id,
                result.representation_id,
                result.resource_id,
                result.location_id,
                lowercase_hex(&result.blob_sha256),
            );
            ExitCode::SUCCESS
        }
        Some(core_response::Response::Error(error)) => {
            let code = match ErrorCode::from_str(&error.code) {
                Ok(code) => code,
                Err(_) => {
                    return fail_with_retry(
                        ErrorCode::IpcTransportError,
                        RetryAction::SameCommand,
                        1,
                    );
                }
            };
            let retry = error
                .retry_action
                .and_then(|value| RetryAction::try_from(value).ok());
            let valid = retry.is_some_and(|retry| retry != RetryAction::Unspecified)
                && error.correlation_id.as_deref() == Some(correlation_id)
                && error.safe_details.is_empty()
                && operation_safe_message(code) == Some(error.safe_message.as_str())
                && retry.is_some_and(|retry| valid_operation_retry_pair(code, retry))
                && error.retryable
                    == !matches!(
                        retry,
                        Some(RetryAction::None | RetryAction::OperatorOrRuntimeAction)
                    );
            if !valid {
                fail_with_retry(ErrorCode::IpcTransportError, RetryAction::SameCommand, 1)
            } else {
                fail_with_retry(code, retry.unwrap(), 1)
            }
        }
        None => fail_with_retry(ErrorCode::IpcTransportError, RetryAction::SameCommand, 1),
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn fail_with_retry(code: ErrorCode, retry: RetryAction, status: u8) -> ExitCode {
    eprintln!(
        "MENGXIA_ERROR code={} retry={}",
        code.as_str(),
        retry_name(retry)
    );
    ExitCode::from(status)
}

const fn retry_name(retry: RetryAction) -> &'static str {
    match retry {
        RetryAction::None => "NONE",
        RetryAction::SameCommand => "SAME_COMMAND",
        RetryAction::FreshCommand => "FRESH_COMMAND",
        RetryAction::SourceStableSameCommand => "SOURCE_STABLE_SAME_COMMAND",
        RetryAction::SourceStableFreshCommand => "SOURCE_STABLE_FRESH_COMMAND",
        RetryAction::OperatorOrRuntimeAction => "OPERATOR_OR_RUNTIME_ACTION",
        RetryAction::Unspecified => "NONE",
    }
}

fn resolve(cli: HandshakeCli) -> Result<ClientConfig, ErrorCode> {
    resolve_with_operation(cli, None)
}

fn resolve_with_operation(
    cli: HandshakeCli,
    operation_timeout: Option<OsString>,
) -> Result<ClientConfig, ErrorCode> {
    let mut cli = cli;
    let selector = cli
        .library_config
        .take()
        .or_else(|| env::var_os("MENGXIA_LIBRARY_CONFIG"));
    let library = match selector {
        Some(path) => {
            let bytes = read_library_config(&PathBuf::from(path))
                .map_err(|_| ErrorCode::ValidationError)?;
            let document = LibraryConfigDocument::parse(&bytes)?;
            ClientLibraryConfig::from_document(&document)?
        }
        None => ClientLibraryConfig::default(),
    };
    resolve_from_layers(
        cli,
        ClientEnvironment::capture(),
        library,
        operation_timeout,
    )
}

#[derive(Default)]
struct ClientLibraryConfig {
    endpoint: Option<PathBuf>,
    frame_bytes: Option<OsString>,
    decode_depth: Option<OsString>,
    handshake_timeout_ms: Option<OsString>,
    operation_timeout_ms: Option<OsString>,
}

impl ClientLibraryConfig {
    fn from_document(document: &LibraryConfigDocument) -> Result<Self, ErrorCode> {
        Ok(Self {
            endpoint: document
                .value(LibraryConfigKey::ClientEndpoint)
                .map(|value| PathBuf::from(OsString::from_vec(value.to_vec()))),
            frame_bytes: client_library_raw(document, LibraryConfigKey::MaxFrameBytes),
            decode_depth: client_library_raw(document, LibraryConfigKey::MaxDecodeDepth),
            handshake_timeout_ms: client_library_raw(
                document,
                LibraryConfigKey::ClientHandshakeTimeoutMs,
            ),
            operation_timeout_ms: client_library_raw(
                document,
                LibraryConfigKey::ClientOperationTimeoutMs,
            ),
        })
    }
}

fn client_library_raw(document: &LibraryConfigDocument, key: LibraryConfigKey) -> Option<OsString> {
    document
        .value(key)
        .map(|value| OsString::from_vec(value.to_vec()))
}

#[derive(Default)]
struct ClientEnvironment {
    endpoint: Option<OsString>,
    frame_bytes: Option<OsString>,
    decode_depth: Option<OsString>,
    handshake_timeout_ms: Option<OsString>,
    operation_timeout_ms: Option<OsString>,
    platform_temp_root: PathBuf,
}

impl ClientEnvironment {
    fn capture() -> Self {
        Self {
            endpoint: env::var_os("MENGXIA_CLIENT_ENDPOINT"),
            frame_bytes: env::var_os("MENGXIA_MAX_FRAME_BYTES"),
            decode_depth: env::var_os("MENGXIA_MAX_DECODE_DEPTH"),
            handshake_timeout_ms: env::var_os("MENGXIA_CLIENT_HANDSHAKE_TIMEOUT_MS"),
            operation_timeout_ms: env::var_os("MENGXIA_CLIENT_OPERATION_TIMEOUT_MS"),
            platform_temp_root: env::temp_dir(),
        }
    }
}

fn resolve_from_layers(
    cli: HandshakeCli,
    environment: ClientEnvironment,
    library: ClientLibraryConfig,
    operation_timeout: Option<OsString>,
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
    let operation_limits = OperationLimits::new(frame, depth).map_err(|error| error.code())?;
    let operation_timeout_ms = select_u64(
        operation_timeout,
        environment.operation_timeout_ms,
        library.operation_timeout_ms,
        3_600_000,
    )?;
    if !(100..=86_400_000).contains(&operation_timeout_ms) {
        return Err(ErrorCode::ValidationError);
    }
    Ok(ClientConfig {
        endpoint,
        limits,
        operation_limits,
        operation_timeout: Duration::from_millis(operation_timeout_ms),
    })
}

fn select_u64(
    cli: Option<OsString>,
    environment: Option<OsString>,
    library: Option<OsString>,
    default: u64,
) -> Result<u64, ErrorCode> {
    if let Some(value) = cli {
        parse_ascii_u64(&value)
    } else if let Some(value) = environment {
        parse_ascii_u64(&value)
    } else if let Some(value) = library {
        parse_ascii_u64(&value)
    } else {
        Ok(default)
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
    use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
    use std::path::PathBuf;

    use mengxia_types::ErrorCode;

    use super::{
        ClientEnvironment, ClientLibraryConfig, Command, HandshakeCli, RetryAction,
        normalized_absolute_bytes, parse_ascii_u64, parse_command, parse_ingest_command,
        parse_sha256, resolve_from_layers, retry_name, valid_operation_retry_pair,
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

    fn ingest_args(source: OsString) -> Vec<OsString> {
        let mut values = vec![
            OsString::from("asset"),
            OsString::from("ingest-copy"),
            source,
        ];
        values.extend(args(&[
            "--command-id",
            "018d442f-c000-7a11-8022-334455667788",
            "--asset-kind",
            "file",
            "--content-kind",
            "binary",
            "--representation-purpose",
            "original",
            "--resource-kind",
            "blob",
            "--logical-name",
            "source.bin",
        ]));
        values
    }

    #[test]
    fn ingest_grammar_is_exact_and_preserves_raw_source_bytes() {
        let source = OsString::from_vec(b"/private/tmp/source-\xff".to_vec());
        let parsed = parse_ingest_command(ingest_args(source.clone())).unwrap();
        assert_eq!(
            parsed.source.expect("parsed source").as_os_str().as_bytes(),
            source.as_os_str().as_bytes()
        );
        assert!(normalized_absolute_bytes(source.as_os_str().as_bytes()));

        for mut invalid in [
            args(&["asset", "ingest-copy", "/private/tmp/source"]),
            {
                let mut values = ingest_args(OsString::from("relative/source"));
                values.push(OsString::from("--unknown"));
                values.push(OsString::from("value"));
                values
            },
            {
                let mut values = ingest_args(OsString::from("/private/tmp/source"));
                values.push(OsString::from("--asset-kind=file"));
                values.push(OsString::from("ignored"));
                values
            },
        ] {
            assert_eq!(
                parse_command(std::mem::take(&mut invalid)).err(),
                Some(ErrorCode::ValidationError)
            );
        }
        assert!(!normalized_absolute_bytes(b"relative/source"));
        assert!(!normalized_absolute_bytes(b"/private/tmp/../source"));
        assert_eq!(
            parse_sha256(std::ffi::OsStr::new(
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
            )),
            Err(ErrorCode::ValidationError)
        );
    }

    #[test]
    fn operation_retry_matrix_and_rendered_names_are_closed() {
        for (code, retry) in [
            (ErrorCode::Conflict, RetryAction::None),
            (ErrorCode::StorageBusy, RetryAction::SameCommand),
            (ErrorCode::ValidationError, RetryAction::FreshCommand),
            (
                ErrorCode::SourceModifiedDuringIngest,
                RetryAction::SourceStableFreshCommand,
            ),
            (
                ErrorCode::StorageConfigurationError,
                RetryAction::OperatorOrRuntimeAction,
            ),
        ] {
            assert!(valid_operation_retry_pair(code, retry));
            assert!(!retry_name(retry).is_empty());
        }
        assert_eq!(retry_name(RetryAction::None), "NONE");
        assert!(!valid_operation_retry_pair(
            ErrorCode::Conflict,
            RetryAction::SameCommand
        ));
        assert!(!valid_operation_retry_pair(
            ErrorCode::InternalError,
            RetryAction::OperatorOrRuntimeAction
        ));
        assert!(!valid_operation_retry_pair(
            ErrorCode::StorageBusy,
            RetryAction::Unspecified
        ));
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
                ..HandshakeCli::default()
            },
            ClientEnvironment {
                endpoint: Some(OsString::from("invalid-relative-endpoint")),
                frame_bytes: Some(OsString::from("invalid")),
                decode_depth: Some(OsString::from("invalid")),
                handshake_timeout_ms: Some(OsString::from("invalid")),
                operation_timeout_ms: Some(OsString::from("invalid")),
                platform_temp_root: PathBuf::from("/private/tmp"),
            },
            ClientLibraryConfig {
                endpoint: Some(PathBuf::from("/private/tmp/lower/client.sock")),
                frame_bytes: Some(OsString::from("invalid-lower-frame")),
                decode_depth: Some(OsString::from("4")),
                handshake_timeout_ms: Some(OsString::from("200")),
                ..ClientLibraryConfig::default()
            },
            Some(OsString::from("1000")),
        )
        .unwrap();
        assert_eq!(config.endpoint, endpoint);
        assert_eq!(
            config.limits.timeout(),
            std::time::Duration::from_millis(100)
        );
        assert_eq!(
            config.operation_timeout,
            std::time::Duration::from_millis(1000)
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
                ..ClientEnvironment::default()
            },
            ClientLibraryConfig {
                frame_bytes: Some(OsString::from("65536")),
                ..ClientLibraryConfig::default()
            },
            None,
        );
        assert!(matches!(
            invalid_higher_layer,
            Err(ErrorCode::ValidationError)
        ));
    }
}
