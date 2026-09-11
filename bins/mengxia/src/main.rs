//! MengXia command-line composition root.

#![forbid(unsafe_code)]

use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::time::Duration;

use mengxia_app::{
    LibraryConfigDocument, LibraryConfigKey, opaque_cursor_checksum_is_valid, parse_project_policy,
    parse_work_specification,
};
use mengxia_core_proto::{
    CoreRequest, DecodeDepth, HandshakeLimits, IngestAssetCopyRequest, IngestMode,
    NegotiatedHandshake, OperationLimits, RetryAction, core_request, core_response,
    operation_safe_message, request_handshake, request_single_command, request_task_008_command,
    request_task_009_command, valid_operation_retry_pair,
};
use mengxia_framing::FrameLimit;
use mengxia_platform_fs::{
    AuthorityError, effective_user_id, read_library_config, validate_client_endpoint,
    validate_runtime_endpoint_path,
};
use mengxia_types::{ErrorCode, Id, Timestamp};

const HELP: &str = "mengxia handshake [--client-endpoint PATH]\n\
mengxia asset ingest-copy SOURCE --command-id UUIDV7 --asset-kind TOKEN ...\n\
mengxia library status [client transport options]\n\
mengxia library verify --mode normal|deep [operation/client transport options]\n\
mengxia library issues --verification-id UUIDV7 [page/cursor/operation/client options]\n\
mengxia asset list [page/cursor/operation/client options]\n\
mengxia asset inspect --asset-id UUIDV7 [revision/page/cursor/operation/client options]\n\
mengxia asset materialize --command-id UUIDV7 --asset-id UUIDV7 --asset-revision-id UUIDV7\n  --representation-id UUIDV7 --resource-id UUIDV7 --member-ordinal ASCII_U32\n  --destination ABSOLUTE_PATH [operation/client transport options]\n\
mengxia asset create-revision --command-id UUID --asset-id UUID --expected-revision U64\n  --parent-revision-id UUID{1..64} --content-kind TOKEN\n  (--representation TOKEN (--resource TOKEN --member LOGICAL_NAME_HEX:BLOB_SHA256_HEX{1..4096}){1..64}){1..64}\n  [operation/client transport options]\n\
mengxia asset retire --command-id UUID --asset-id UUID --expected-revision U64 [operation/client options]\n\
mengxia asset restore --command-id UUID --asset-id UUID --expected-revision U64 [operation/client options]\n\
mengxia project create --command-id UUID --name-hex HEX\n  [--resolution U32xU32] [--frame-rate U32/U32] [--aspect-ratio U32/U32]\n  --color-policy-json-hex HEX --audio-policy-json-hex HEX\n  --quality-policy-json-hex HEX --privacy-policy-json-hex HEX [operation/client options]\n\
mengxia project revise-spec --command-id UUID --project-id UUID --expected-revision U64\n  [complete ProjectSpec options] [operation/client options]\n\
mengxia project list [page/cursor/operation/client options]\n\
mengxia subject create --command-id UUID --kind TOKEN --canonical-name-hex HEX [operation/client options]\n\
mengxia subject list [page/cursor/operation/client options]\n\
mengxia work create --command-id UUID --project-id UUID --kind scene|shot --code-hex HEX\n  --specification-json-hex HEX [--subject-id UUID]{0..64} [--asset-id UUID]{0..64}\n  [operation/client options]\n\
mengxia work revise --command-id UUID --project-id UUID --work-item-id UUID\n  --expected-revision U64 --specification-json-hex HEX\n  [--subject-id UUID]{0..64} [--asset-id UUID]{0..64} [operation/client options]\n\
mengxia work list --project-id UUID [page/cursor/operation/client options]\n\
mengxia take create --command-id UUID --project-id UUID --work-item-id UUID\n  --work-revision-id UUID --primary-asset-id UUID [operation/client options]\n\
mengxia take transition --command-id UUID --project-id UUID --work-item-id UUID\n  --work-revision-id UUID --take-id UUID --expected-revision U64\n  --transition shortlist|select|approve|reject|supersede\n  [--reason-hex HEX] [--related-take-id UUID --related-take-expected-revision U64]\n  [operation/client options]\n\
mengxia take reopen --command-id UUID --project-id UUID --work-item-id UUID\n  --work-revision-id UUID --terminal-take-id UUID --expected-revision U64\n  --new-primary-asset-id UUID [operation/client options]\n\
mengxia take list --project-id UUID --work-item-id UUID --work-revision-id UUID\n  [page/cursor/operation/client options]\n";

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
        Ok(Command::Task008(cli)) => match resolve_task_008(*cli) {
            Ok(config) => run_task_008(config),
            Err(code) => fail_with_retry(code, RetryAction::None, 2),
        },
        Ok(Command::Task009(cli)) => match resolve_task_009(*cli) {
            Ok(config) => run_task_009(config),
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
    Task008(Box<Task008Cli>),
    Task009(Box<Task009Cli>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Task009Kind {
    CreateAssetRevision,
    RetireAsset,
    RestoreAsset,
    CreateProject,
    ReviseProject,
    ListProjects,
    CreateSubject,
    ListSubjects,
    CreateWork,
    ReviseWork,
    ListWork,
    CreateTake,
    TransitionTake,
    ReopenTake,
    ListTakes,
}

struct Task009Cli {
    kind: Task009Kind,
    semantic: Vec<(String, OsString)>,
    operation_timeout: Option<OsString>,
    page_size: Option<OsString>,
    cursor: Option<OsString>,
    transport: HandshakeCli,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Task008Kind {
    Status,
    Verify,
    Issues,
    ListAssets,
    InspectAsset,
    Materialize,
}

struct Task008Cli {
    kind: Task008Kind,
    mode: Option<OsString>,
    verification_id: Option<OsString>,
    page_size: Option<OsString>,
    cursor: Option<OsString>,
    asset_id: Option<OsString>,
    asset_revision_id: Option<OsString>,
    command_id: Option<OsString>,
    representation_id: Option<OsString>,
    resource_id: Option<OsString>,
    member_ordinal: Option<OsString>,
    destination: Option<OsString>,
    operation_timeout: Option<OsString>,
    transport: HandshakeCli,
}

impl Task008Cli {
    fn new(kind: Task008Kind) -> Self {
        Self {
            kind,
            mode: None,
            verification_id: None,
            page_size: None,
            cursor: None,
            asset_id: None,
            asset_revision_id: None,
            command_id: None,
            representation_id: None,
            resource_id: None,
            member_ordinal: None,
            destination: None,
            operation_timeout: None,
            transport: HandshakeCli::default(),
        }
    }
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
        if matches!(
            args.get(1).and_then(|value| value.to_str()),
            Some("create-revision" | "retire" | "restore")
        ) {
            return parse_task_009_command(args)
                .map(Box::new)
                .map(Command::Task009);
        }
        if args.get(1).is_some_and(|arg| arg != "ingest-copy") {
            return parse_task_008_command(args)
                .map(Box::new)
                .map(Command::Task008);
        }
        return parse_ingest_command(args)
            .map(Box::new)
            .map(Command::Ingest);
    }
    if args.first().is_some_and(|arg| arg == "library") {
        return parse_task_008_command(args)
            .map(Box::new)
            .map(Command::Task008);
    }
    if matches!(
        args.first().and_then(|value| value.to_str()),
        Some("project" | "subject" | "work" | "take")
    ) {
        return parse_task_009_command(args)
            .map(Box::new)
            .map(Command::Task009);
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

fn parse_task_008_command(args: Vec<OsString>) -> Result<Task008Cli, ErrorCode> {
    let kind = match (
        args.first().and_then(|value| value.to_str()),
        args.get(1).and_then(|value| value.to_str()),
    ) {
        (Some("library"), Some("status")) => Task008Kind::Status,
        (Some("library"), Some("verify")) => Task008Kind::Verify,
        (Some("library"), Some("issues")) => Task008Kind::Issues,
        (Some("asset"), Some("list")) => Task008Kind::ListAssets,
        (Some("asset"), Some("inspect")) => Task008Kind::InspectAsset,
        (Some("asset"), Some("materialize")) => Task008Kind::Materialize,
        _ => return Err(ErrorCode::ValidationError),
    };
    let mut cli = Task008Cli::new(kind);
    let mut index = 2;
    while index < args.len() {
        let option = args[index].to_str().ok_or(ErrorCode::ValidationError)?;
        let value = args.get(index + 1).ok_or(ErrorCode::ValidationError)?;
        let slot = match option {
            "--client-endpoint" => &mut cli.transport.endpoint,
            "--library-config" => &mut cli.transport.library_config,
            "--max-frame-bytes" => &mut cli.transport.frame,
            "--max-decode-depth" => &mut cli.transport.depth,
            "--client-handshake-timeout-ms" => &mut cli.transport.timeout,
            "--operation-timeout-ms" if kind != Task008Kind::Status => &mut cli.operation_timeout,
            "--mode" if kind == Task008Kind::Verify => &mut cli.mode,
            "--verification-id" if kind == Task008Kind::Issues => &mut cli.verification_id,
            "--page-size"
                if matches!(
                    kind,
                    Task008Kind::Issues | Task008Kind::ListAssets | Task008Kind::InspectAsset
                ) =>
            {
                &mut cli.page_size
            }
            "--cursor"
                if matches!(
                    kind,
                    Task008Kind::Issues | Task008Kind::ListAssets | Task008Kind::InspectAsset
                ) =>
            {
                &mut cli.cursor
            }
            "--asset-id"
                if matches!(kind, Task008Kind::InspectAsset | Task008Kind::Materialize) =>
            {
                &mut cli.asset_id
            }
            "--asset-revision-id"
                if matches!(kind, Task008Kind::InspectAsset | Task008Kind::Materialize) =>
            {
                &mut cli.asset_revision_id
            }
            "--command-id" if kind == Task008Kind::Materialize => &mut cli.command_id,
            "--representation-id" if kind == Task008Kind::Materialize => &mut cli.representation_id,
            "--resource-id" if kind == Task008Kind::Materialize => &mut cli.resource_id,
            "--member-ordinal" if kind == Task008Kind::Materialize => &mut cli.member_ordinal,
            "--destination" if kind == Task008Kind::Materialize => &mut cli.destination,
            _ => return Err(ErrorCode::ValidationError),
        };
        if slot.replace(value.clone()).is_some() {
            return Err(ErrorCode::ValidationError);
        }
        index += 2;
    }
    let complete = match kind {
        Task008Kind::Status | Task008Kind::ListAssets => true,
        Task008Kind::Verify => cli.mode.is_some(),
        Task008Kind::Issues => cli.verification_id.is_some(),
        Task008Kind::InspectAsset => cli.asset_id.is_some(),
        Task008Kind::Materialize => {
            cli.command_id.is_some()
                && cli.asset_id.is_some()
                && cli.asset_revision_id.is_some()
                && cli.representation_id.is_some()
                && cli.resource_id.is_some()
                && cli.member_ordinal.is_some()
                && cli.destination.is_some()
        }
    };
    complete.then_some(cli).ok_or(ErrorCode::ValidationError)
}

fn parse_task_009_command(args: Vec<OsString>) -> Result<Task009Cli, ErrorCode> {
    let kind = match (
        args.first().and_then(|value| value.to_str()),
        args.get(1).and_then(|value| value.to_str()),
    ) {
        (Some("asset"), Some("create-revision")) => Task009Kind::CreateAssetRevision,
        (Some("asset"), Some("retire")) => Task009Kind::RetireAsset,
        (Some("asset"), Some("restore")) => Task009Kind::RestoreAsset,
        (Some("project"), Some("create")) => Task009Kind::CreateProject,
        (Some("project"), Some("revise-spec")) => Task009Kind::ReviseProject,
        (Some("project"), Some("list")) => Task009Kind::ListProjects,
        (Some("subject"), Some("create")) => Task009Kind::CreateSubject,
        (Some("subject"), Some("list")) => Task009Kind::ListSubjects,
        (Some("work"), Some("create")) => Task009Kind::CreateWork,
        (Some("work"), Some("revise")) => Task009Kind::ReviseWork,
        (Some("work"), Some("list")) => Task009Kind::ListWork,
        (Some("take"), Some("create")) => Task009Kind::CreateTake,
        (Some("take"), Some("transition")) => Task009Kind::TransitionTake,
        (Some("take"), Some("reopen")) => Task009Kind::ReopenTake,
        (Some("take"), Some("list")) => Task009Kind::ListTakes,
        _ => return Err(ErrorCode::ValidationError),
    };
    let mut cli = Task009Cli {
        kind,
        semantic: Vec::new(),
        operation_timeout: None,
        page_size: None,
        cursor: None,
        transport: HandshakeCli::default(),
    };
    let is_list = matches!(
        kind,
        Task009Kind::ListProjects
            | Task009Kind::ListSubjects
            | Task009Kind::ListWork
            | Task009Kind::ListTakes
    );
    let mut index = 2;
    let mut graph_has_representation = false;
    let mut graph_has_resource = false;
    while index < args.len() {
        let option = args[index].to_str().ok_or(ErrorCode::ValidationError)?;
        let value = args.get(index + 1).ok_or(ErrorCode::ValidationError)?;
        let slot = match option {
            "--client-endpoint" => Some(&mut cli.transport.endpoint),
            "--library-config" => Some(&mut cli.transport.library_config),
            "--max-frame-bytes" => Some(&mut cli.transport.frame),
            "--max-decode-depth" => Some(&mut cli.transport.depth),
            "--client-handshake-timeout-ms" => Some(&mut cli.transport.timeout),
            "--operation-timeout-ms" => Some(&mut cli.operation_timeout),
            "--page-size" if is_list => Some(&mut cli.page_size),
            "--cursor" if is_list => Some(&mut cli.cursor),
            _ => None,
        };
        if let Some(slot) = slot {
            if slot.replace(value.clone()).is_some() {
                return Err(ErrorCode::ValidationError);
            }
        } else {
            if !task_009_option_allowed(kind, option) {
                return Err(ErrorCode::ValidationError);
            }
            match option {
                "--representation" => {
                    graph_has_representation = true;
                    graph_has_resource = false;
                }
                "--resource" if !graph_has_representation => {
                    return Err(ErrorCode::ValidationError);
                }
                "--resource" => graph_has_resource = true,
                "--member" if !graph_has_resource => return Err(ErrorCode::ValidationError),
                _ => {}
            }
            let repeated = matches!(
                (kind, option),
                (Task009Kind::CreateAssetRevision, "--parent-revision-id")
                    | (Task009Kind::CreateAssetRevision, "--representation")
                    | (Task009Kind::CreateAssetRevision, "--resource")
                    | (Task009Kind::CreateAssetRevision, "--member")
                    | (
                        Task009Kind::CreateWork | Task009Kind::ReviseWork,
                        "--subject-id"
                    )
                    | (
                        Task009Kind::CreateWork | Task009Kind::ReviseWork,
                        "--asset-id"
                    )
            );
            if !repeated && cli.semantic.iter().any(|(seen, _)| seen == option) {
                return Err(ErrorCode::ValidationError);
            }
            cli.semantic.push((option.to_owned(), value.clone()));
        }
        index += 2;
    }
    Ok(cli)
}

fn task_009_option_allowed(kind: Task009Kind, option: &str) -> bool {
    match kind {
        Task009Kind::CreateAssetRevision => matches!(
            option,
            "--command-id"
                | "--asset-id"
                | "--expected-revision"
                | "--parent-revision-id"
                | "--content-kind"
                | "--representation"
                | "--resource"
                | "--member"
        ),
        Task009Kind::RetireAsset | Task009Kind::RestoreAsset => {
            matches!(
                option,
                "--command-id" | "--asset-id" | "--expected-revision"
            )
        }
        Task009Kind::CreateProject => matches!(
            option,
            "--command-id"
                | "--name-hex"
                | "--resolution"
                | "--frame-rate"
                | "--aspect-ratio"
                | "--color-policy-json-hex"
                | "--audio-policy-json-hex"
                | "--quality-policy-json-hex"
                | "--privacy-policy-json-hex"
        ),
        Task009Kind::ReviseProject => matches!(
            option,
            "--command-id"
                | "--project-id"
                | "--expected-revision"
                | "--resolution"
                | "--frame-rate"
                | "--aspect-ratio"
                | "--color-policy-json-hex"
                | "--audio-policy-json-hex"
                | "--quality-policy-json-hex"
                | "--privacy-policy-json-hex"
        ),
        Task009Kind::ListProjects | Task009Kind::ListSubjects => false,
        Task009Kind::CreateSubject => {
            matches!(option, "--command-id" | "--kind" | "--canonical-name-hex")
        }
        Task009Kind::CreateWork => matches!(
            option,
            "--command-id"
                | "--project-id"
                | "--kind"
                | "--code-hex"
                | "--specification-json-hex"
                | "--subject-id"
                | "--asset-id"
        ),
        Task009Kind::ReviseWork => matches!(
            option,
            "--command-id"
                | "--project-id"
                | "--work-item-id"
                | "--expected-revision"
                | "--specification-json-hex"
                | "--subject-id"
                | "--asset-id"
        ),
        Task009Kind::ListWork => option == "--project-id",
        Task009Kind::CreateTake => matches!(
            option,
            "--command-id"
                | "--project-id"
                | "--work-item-id"
                | "--work-revision-id"
                | "--primary-asset-id"
        ),
        Task009Kind::TransitionTake => matches!(
            option,
            "--command-id"
                | "--project-id"
                | "--work-item-id"
                | "--work-revision-id"
                | "--take-id"
                | "--expected-revision"
                | "--transition"
                | "--reason-hex"
                | "--related-take-id"
                | "--related-take-expected-revision"
        ),
        Task009Kind::ReopenTake => matches!(
            option,
            "--command-id"
                | "--project-id"
                | "--work-item-id"
                | "--work-revision-id"
                | "--terminal-take-id"
                | "--expected-revision"
                | "--new-primary-asset-id"
        ),
        Task009Kind::ListTakes => {
            matches!(
                option,
                "--project-id" | "--work-item-id" | "--work-revision-id"
            )
        }
    }
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

struct Task008Config {
    client: ClientConfig,
    request: CoreRequest,
    kind: Task008Kind,
}

struct Task009Config {
    client: ClientConfig,
    request: CoreRequest,
    kind: Task009Kind,
}

struct CommandIdentity;
struct ResultIdentity;

fn resolve_task_008(mut cli: Task008Cli) -> Result<Task008Config, ErrorCode> {
    let explicit_timeout = cli.operation_timeout.take();
    let client = resolve_with_operation(std::mem::take(&mut cli.transport), explicit_timeout)?;
    let timeout_ms = u64::try_from(client.operation_timeout.as_millis())
        .map_err(|_| ErrorCode::ValidationError)?;
    let page_size = || -> Result<u32, ErrorCode> {
        let value = cli
            .page_size
            .as_deref()
            .map(parse_ascii_u64)
            .transpose()?
            .unwrap_or(32);
        u32::try_from(value)
            .ok()
            .filter(|value| (1..=64).contains(value))
            .ok_or(ErrorCode::ValidationError)
    };
    let cursor = |length: usize| -> Result<Vec<u8>, ErrorCode> {
        let cursor = cli
            .cursor
            .as_deref()
            .map(|value| parse_hex_exact(value, length))
            .transpose()
            .map(|value| value.unwrap_or_default())?;
        if !cursor.is_empty() && !opaque_cursor_checksum_is_valid(&cursor) {
            return Err(ErrorCode::ValidationError);
        }
        Ok(cursor)
    };
    let operation = match cli.kind {
        Task008Kind::Status => core_request::Operation::GetLibraryStatus(
            mengxia_core_proto::GetLibraryStatusRequest {},
        ),
        Task008Kind::Verify => {
            let mode = match selected_utf8(cli.mode.take())?.as_str() {
                "normal" => mengxia_core_proto::VerificationMode::Normal,
                "deep" => mengxia_core_proto::VerificationMode::Deep,
                _ => return Err(ErrorCode::ValidationError),
            };
            core_request::Operation::VerifyLibrary(mengxia_core_proto::VerifyLibraryRequest {
                mode: mode as i32,
                operation_timeout_ms: timeout_ms,
            })
        }
        Task008Kind::Issues => core_request::Operation::ListIntegrityIssues(
            mengxia_core_proto::ListIntegrityIssuesRequest {
                verification_id: parse_id_text(cli.verification_id.take())?,
                page_size: page_size()?,
                cursor: cursor(96)?,
                operation_timeout_ms: timeout_ms,
            },
        ),
        Task008Kind::ListAssets => {
            core_request::Operation::ListAssets(mengxia_core_proto::ListAssetsRequest {
                page_size: page_size()?,
                cursor: cursor(80)?,
                operation_timeout_ms: timeout_ms,
            })
        }
        Task008Kind::InspectAsset => {
            core_request::Operation::InspectAsset(mengxia_core_proto::InspectAssetRequest {
                asset_id: parse_id_text(cli.asset_id.take())?,
                asset_revision_id: cli
                    .asset_revision_id
                    .take()
                    .map(|value| parse_id_text(Some(value)))
                    .transpose()?,
                page_size: page_size()?,
                cursor: cursor(208)?,
                operation_timeout_ms: timeout_ms,
            })
        }
        Task008Kind::Materialize => {
            let destination = cli.destination.take().ok_or(ErrorCode::ValidationError)?;
            let destination = destination.as_os_str().as_bytes();
            let final_length = destination
                .rsplit(|byte| *byte == b'/')
                .next()
                .map_or(0, <[u8]>::len);
            if !(1..=1023).contains(&destination.len())
                || destination.contains(&0)
                || !normalized_absolute_bytes(destination)
                || !(1..=255).contains(&final_length)
            {
                return Err(ErrorCode::ValidationError);
            }
            let ordinal = parse_ascii_u64(
                cli.member_ordinal
                    .as_deref()
                    .ok_or(ErrorCode::ValidationError)?,
            )?;
            let ordinal = u32::try_from(ordinal)
                .ok()
                .filter(|value| *value <= 4095)
                .ok_or(ErrorCode::ValidationError)?;
            core_request::Operation::MaterializeAsset(mengxia_core_proto::MaterializeAssetRequest {
                command_id: parse_id_text(cli.command_id.take())?,
                asset_id: parse_id_text(cli.asset_id.take())?,
                asset_revision_id: parse_id_text(cli.asset_revision_id.take())?,
                representation_id: parse_id_text(cli.representation_id.take())?,
                resource_id: parse_id_text(cli.resource_id.take())?,
                member_ordinal: ordinal,
                destination_path: destination.to_vec(),
                operation_timeout_ms: timeout_ms,
            })
        }
    };
    Ok(Task008Config {
        client,
        request: CoreRequest {
            operation: Some(operation),
        },
        kind: cli.kind,
    })
}

fn semantic_one<'a>(
    cli: &'a Task009Cli,
    name: &str,
    required: bool,
) -> Result<Option<&'a OsStr>, ErrorCode> {
    let mut values = cli
        .semantic
        .iter()
        .filter(|(option, _)| option == name)
        .map(|(_, value)| value.as_os_str());
    let value = values.next();
    if values.next().is_some() || (required && value.is_none()) {
        return Err(ErrorCode::ValidationError);
    }
    Ok(value)
}

fn semantic_many<'a>(cli: &'a Task009Cli, name: &str) -> Vec<&'a OsStr> {
    cli.semantic
        .iter()
        .filter(|(option, _)| option == name)
        .map(|(_, value)| value.as_os_str())
        .collect()
}

fn parse_hex(value: &OsStr, maximum_bytes: usize) -> Result<Vec<u8>, ErrorCode> {
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(2)
        || bytes.len() > maximum_bytes.saturating_mul(2)
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(ErrorCode::ValidationError);
    }
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| Ok((hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?))
        .collect()
}

fn semantic_utf8_hex(
    cli: &Task009Cli,
    name: &str,
    maximum_bytes: usize,
) -> Result<String, ErrorCode> {
    String::from_utf8(parse_hex(
        semantic_one(cli, name, true)?.ok_or(ErrorCode::ValidationError)?,
        maximum_bytes,
    )?)
    .map_err(|_| ErrorCode::ValidationError)
}

fn semantic_id(cli: &Task009Cli, name: &str) -> Result<String, ErrorCode> {
    parse_id_text(Some(
        semantic_one(cli, name, true)?
            .ok_or(ErrorCode::ValidationError)?
            .to_os_string(),
    ))
}

fn semantic_revision(cli: &Task009Cli, name: &str) -> Result<u64, ErrorCode> {
    parse_ascii_u64(semantic_one(cli, name, true)?.ok_or(ErrorCode::ValidationError)?).and_then(
        |value| {
            (value != 0)
                .then_some(value)
                .ok_or(ErrorCode::ValidationError)
        },
    )
}

fn parse_pair(value: &OsStr, separator: u8) -> Result<(u32, u32), ErrorCode> {
    let bytes = value.as_bytes();
    let mut pieces = bytes.split(|byte| *byte == separator);
    let left = pieces.next().ok_or(ErrorCode::ValidationError)?;
    let right = pieces.next().ok_or(ErrorCode::ValidationError)?;
    if pieces.next().is_some() || left.is_empty() || right.is_empty() {
        return Err(ErrorCode::ValidationError);
    }
    let left = parse_ascii_u64(OsStr::from_bytes(left))?;
    let right = parse_ascii_u64(OsStr::from_bytes(right))?;
    let left = u32::try_from(left).map_err(|_| ErrorCode::ValidationError)?;
    let right = u32::try_from(right).map_err(|_| ErrorCode::ValidationError)?;
    if left == 0 || right == 0 {
        return Err(ErrorCode::ValidationError);
    }
    Ok((left, right))
}

fn project_spec_input(cli: &Task009Cli) -> Result<mengxia_core_proto::ProjectSpecInput, ErrorCode> {
    let resolution = semantic_one(cli, "--resolution", false)?
        .map(|value| parse_pair(value, b'x'))
        .transpose()?;
    let frame = semantic_one(cli, "--frame-rate", false)?
        .map(|value| parse_pair(value, b'/'))
        .transpose()?;
    let aspect = semantic_one(cli, "--aspect-ratio", false)?
        .map(|value| parse_pair(value, b'/'))
        .transpose()?;
    Ok(mengxia_core_proto::ProjectSpecInput {
        resolution_width: resolution.map(|pair| pair.0),
        resolution_height: resolution.map(|pair| pair.1),
        frame_rate_numerator: frame.map(|pair| pair.0),
        frame_rate_denominator: frame.map(|pair| pair.1),
        aspect_ratio_numerator: aspect.map(|pair| pair.0),
        aspect_ratio_denominator: aspect.map(|pair| pair.1),
        color_policy_json: parse_hex(
            semantic_one(cli, "--color-policy-json-hex", true)?
                .ok_or(ErrorCode::ValidationError)?,
            65_536,
        )?,
        audio_policy_json: parse_hex(
            semantic_one(cli, "--audio-policy-json-hex", true)?
                .ok_or(ErrorCode::ValidationError)?,
            65_536,
        )?,
        quality_policy_json: parse_hex(
            semantic_one(cli, "--quality-policy-json-hex", true)?
                .ok_or(ErrorCode::ValidationError)?,
            65_536,
        )?,
        privacy_policy_json: parse_hex(
            semantic_one(cli, "--privacy-policy-json-hex", true)?
                .ok_or(ErrorCode::ValidationError)?,
            65_536,
        )?,
    })
}

fn asset_revision_graph(
    cli: &Task009Cli,
) -> Result<Vec<mengxia_core_proto::AssetRevisionRepresentationInput>, ErrorCode> {
    let mut representations = Vec::new();
    for (option, raw) in &cli.semantic {
        match option.as_str() {
            "--representation" => {
                representations.push(mengxia_core_proto::AssetRevisionRepresentationInput {
                    representation_purpose: raw
                        .clone()
                        .into_string()
                        .map_err(|_| ErrorCode::ValidationError)?,
                    resources: Vec::new(),
                });
            }
            "--resource" => {
                let representation = representations
                    .last_mut()
                    .ok_or(ErrorCode::ValidationError)?;
                representation
                    .resources
                    .push(mengxia_core_proto::AssetRevisionResourceInput {
                        resource_kind: raw
                            .clone()
                            .into_string()
                            .map_err(|_| ErrorCode::ValidationError)?,
                        members: Vec::new(),
                    });
            }
            "--member" => {
                let bytes = raw.as_bytes();
                let split = bytes
                    .iter()
                    .position(|byte| *byte == b':')
                    .ok_or(ErrorCode::ValidationError)?;
                if bytes[split + 1..].contains(&b':') {
                    return Err(ErrorCode::ValidationError);
                }
                let logical =
                    String::from_utf8(parse_hex(OsStr::from_bytes(&bytes[..split]), 255)?)
                        .map_err(|_| ErrorCode::ValidationError)?;
                let digest = parse_hex_exact(OsStr::from_bytes(&bytes[split + 1..]), 32)?;
                representations
                    .last_mut()
                    .and_then(|representation| representation.resources.last_mut())
                    .ok_or(ErrorCode::ValidationError)?
                    .members
                    .push(mengxia_core_proto::AssetRevisionMemberInput {
                        logical_name: logical,
                        blob_sha256: digest,
                    });
            }
            _ => {}
        }
    }
    if representations.is_empty()
        || representations.len() > 64
        || representations.iter().any(|representation| {
            representation.resources.is_empty()
                || representation.resources.len() > 64
                || representation
                    .resources
                    .iter()
                    .any(|resource| resource.members.is_empty() || resource.members.len() > 4_096)
        })
    {
        return Err(ErrorCode::ValidationError);
    }
    Ok(representations)
}

fn sorted_semantic_ids(cli: &Task009Cli, name: &str) -> Result<Vec<String>, ErrorCode> {
    let mut ids = semantic_many(cli, name)
        .into_iter()
        .map(|value| parse_id_text(Some(value.to_os_string())))
        .collect::<Result<Vec<_>, _>>()?;
    ids.sort_by_key(|value| {
        Id::<ResultIdentity>::from_str(value)
            .expect("validated ID")
            .to_bytes()
    });
    if ids.windows(2).any(|pair| pair[0] == pair[1]) || ids.len() > 64 {
        return Err(ErrorCode::ValidationError);
    }
    Ok(ids)
}

fn task_009_page(cli: &Task009Cli) -> Result<(u32, Vec<u8>), ErrorCode> {
    let page = cli
        .page_size
        .as_deref()
        .map(parse_ascii_u64)
        .transpose()?
        .unwrap_or(32);
    let page = u32::try_from(page)
        .ok()
        .filter(|value| (1..=64).contains(value))
        .ok_or(ErrorCode::ValidationError)?;
    let cursor = cli
        .cursor
        .as_deref()
        .map(|value| parse_hex_exact(value, 160))
        .transpose()?
        .unwrap_or_default();
    if !cursor.is_empty() && !opaque_cursor_checksum_is_valid(&cursor) {
        return Err(ErrorCode::ValidationError);
    }
    Ok((page, cursor))
}

fn resolve_task_009(mut cli: Task009Cli) -> Result<Task009Config, ErrorCode> {
    let explicit_timeout = cli.operation_timeout.take();
    let client = resolve_with_operation(std::mem::take(&mut cli.transport), explicit_timeout)?;
    let timeout = u64::try_from(client.operation_timeout.as_millis())
        .map_err(|_| ErrorCode::ValidationError)?;
    let command = || semantic_id(&cli, "--command-id");
    let operation = match cli.kind {
        Task009Kind::CreateAssetRevision => {
            let parents = semantic_many(&cli, "--parent-revision-id")
                .into_iter()
                .map(|value| parse_id_text(Some(value.to_os_string())))
                .collect::<Result<Vec<_>, _>>()?;
            if parents.is_empty() || parents.len() > 64 {
                return Err(ErrorCode::ValidationError);
            }
            core_request::Operation::CreateAssetRevision(
                mengxia_core_proto::CreateAssetRevisionRequest {
                    command_id: command()?,
                    asset_id: semantic_id(&cli, "--asset-id")?,
                    expected_revision: semantic_revision(&cli, "--expected-revision")?,
                    parent_revision_ids: parents,
                    content_kind: semantic_one(&cli, "--content-kind", true)?
                        .ok_or(ErrorCode::ValidationError)?
                        .to_str()
                        .ok_or(ErrorCode::ValidationError)?
                        .to_owned(),
                    representations: asset_revision_graph(&cli)?,
                    operation_timeout_ms: timeout,
                },
            )
        }
        Task009Kind::RetireAsset | Task009Kind::RestoreAsset => {
            let request = mengxia_core_proto::AssetLifecycleRequest {
                command_id: command()?,
                asset_id: semantic_id(&cli, "--asset-id")?,
                expected_revision: semantic_revision(&cli, "--expected-revision")?,
                operation_timeout_ms: timeout,
            };
            if cli.kind == Task009Kind::RetireAsset {
                core_request::Operation::RetireAsset(request)
            } else {
                core_request::Operation::RestoreAsset(request)
            }
        }
        Task009Kind::CreateProject | Task009Kind::ReviseProject => {
            let specification = Some(project_spec_input(&cli)?);
            if cli.kind == Task009Kind::CreateProject {
                core_request::Operation::CreateProject(mengxia_core_proto::CreateProjectRequest {
                    command_id: command()?,
                    name: semantic_utf8_hex(&cli, "--name-hex", 255)?,
                    specification,
                    operation_timeout_ms: timeout,
                })
            } else {
                core_request::Operation::ReviseProjectSpec(
                    mengxia_core_proto::ReviseProjectSpecRequest {
                        command_id: command()?,
                        project_id: semantic_id(&cli, "--project-id")?,
                        expected_revision: semantic_revision(&cli, "--expected-revision")?,
                        specification,
                        operation_timeout_ms: timeout,
                    },
                )
            }
        }
        Task009Kind::ListProjects | Task009Kind::ListSubjects => {
            let (page_size, cursor) = task_009_page(&cli)?;
            if cli.kind == Task009Kind::ListProjects {
                core_request::Operation::ListProjects(mengxia_core_proto::ListProjectsRequest {
                    page_size,
                    cursor,
                    operation_timeout_ms: timeout,
                })
            } else {
                core_request::Operation::ListSubjects(mengxia_core_proto::ListSubjectsRequest {
                    page_size,
                    cursor,
                    operation_timeout_ms: timeout,
                })
            }
        }
        Task009Kind::CreateSubject => {
            core_request::Operation::CreateSubject(mengxia_core_proto::CreateSubjectRequest {
                command_id: command()?,
                kind: semantic_one(&cli, "--kind", true)?
                    .and_then(OsStr::to_str)
                    .ok_or(ErrorCode::ValidationError)?
                    .to_owned(),
                canonical_name: semantic_utf8_hex(&cli, "--canonical-name-hex", 255)?,
                operation_timeout_ms: timeout,
            })
        }
        Task009Kind::CreateWork | Task009Kind::ReviseWork => {
            let json = parse_hex(
                semantic_one(&cli, "--specification-json-hex", true)?
                    .ok_or(ErrorCode::ValidationError)?,
                262_144,
            )?;
            let subjects = sorted_semantic_ids(&cli, "--subject-id")?;
            let assets = sorted_semantic_ids(&cli, "--asset-id")?;
            if cli.kind == Task009Kind::CreateWork {
                let kind = match semantic_one(&cli, "--kind", true)?.and_then(OsStr::to_str) {
                    Some("scene") => mengxia_core_proto::WorkKindValue::Scene,
                    Some("shot") => mengxia_core_proto::WorkKindValue::Shot,
                    _ => return Err(ErrorCode::ValidationError),
                };
                core_request::Operation::CreateWorkItem(mengxia_core_proto::CreateWorkItemRequest {
                    command_id: command()?,
                    project_id: semantic_id(&cli, "--project-id")?,
                    kind: kind as i32,
                    code: semantic_utf8_hex(&cli, "--code-hex", 255)?,
                    specification_json: json,
                    subject_ids: subjects,
                    asset_ids: assets,
                    operation_timeout_ms: timeout,
                })
            } else {
                core_request::Operation::ReviseWork(mengxia_core_proto::ReviseWorkRequest {
                    command_id: command()?,
                    project_id: semantic_id(&cli, "--project-id")?,
                    work_item_id: semantic_id(&cli, "--work-item-id")?,
                    expected_revision: semantic_revision(&cli, "--expected-revision")?,
                    specification_json: json,
                    subject_ids: subjects,
                    asset_ids: assets,
                    operation_timeout_ms: timeout,
                })
            }
        }
        Task009Kind::ListWork => {
            let (page_size, cursor) = task_009_page(&cli)?;
            core_request::Operation::ListWork(mengxia_core_proto::ListWorkRequest {
                project_id: semantic_id(&cli, "--project-id")?,
                page_size,
                cursor,
                operation_timeout_ms: timeout,
            })
        }
        Task009Kind::CreateTake => {
            core_request::Operation::CreateTake(mengxia_core_proto::CreateTakeRequest {
                command_id: command()?,
                project_id: semantic_id(&cli, "--project-id")?,
                work_item_id: semantic_id(&cli, "--work-item-id")?,
                work_revision_id: semantic_id(&cli, "--work-revision-id")?,
                primary_asset_id: semantic_id(&cli, "--primary-asset-id")?,
                operation_timeout_ms: timeout,
            })
        }
        Task009Kind::TransitionTake => {
            let transition = match semantic_one(&cli, "--transition", true)?.and_then(OsStr::to_str)
            {
                Some("shortlist") => mengxia_core_proto::TakeTransitionValue::Shortlist,
                Some("select") => mengxia_core_proto::TakeTransitionValue::Select,
                Some("approve") => mengxia_core_proto::TakeTransitionValue::Approve,
                Some("reject") => mengxia_core_proto::TakeTransitionValue::Reject,
                Some("supersede") => mengxia_core_proto::TakeTransitionValue::Supersede,
                _ => return Err(ErrorCode::ValidationError),
            };
            let reason = semantic_one(&cli, "--reason-hex", false)?
                .map(|value| {
                    String::from_utf8(parse_hex(value, 1_024)?)
                        .map_err(|_| ErrorCode::ValidationError)
                })
                .transpose()?;
            let related_id = semantic_one(&cli, "--related-take-id", false)?
                .map(|value| parse_id_text(Some(value.to_os_string())))
                .transpose()?;
            let related_revision = semantic_one(&cli, "--related-take-expected-revision", false)?
                .map(parse_ascii_u64)
                .transpose()?;
            let option_shape = match transition {
                mengxia_core_proto::TakeTransitionValue::Reject => {
                    reason.is_some() && related_id.is_none() && related_revision.is_none()
                }
                mengxia_core_proto::TakeTransitionValue::Select => {
                    reason.is_none() && (related_id.is_some() == related_revision.is_some())
                }
                mengxia_core_proto::TakeTransitionValue::Supersede => {
                    reason.is_none() && related_id.is_some() && related_revision.is_some()
                }
                _ => reason.is_none() && related_id.is_none() && related_revision.is_none(),
            };
            if !option_shape || related_revision == Some(0) {
                return Err(ErrorCode::ValidationError);
            }
            core_request::Operation::TransitionTake(mengxia_core_proto::TransitionTakeRequest {
                command_id: command()?,
                project_id: semantic_id(&cli, "--project-id")?,
                work_item_id: semantic_id(&cli, "--work-item-id")?,
                work_revision_id: semantic_id(&cli, "--work-revision-id")?,
                take_id: semantic_id(&cli, "--take-id")?,
                expected_revision: semantic_revision(&cli, "--expected-revision")?,
                transition: transition as i32,
                reason,
                related_take_id: related_id,
                related_take_expected_revision: related_revision,
                operation_timeout_ms: timeout,
            })
        }
        Task009Kind::ReopenTake => {
            core_request::Operation::ReopenTake(mengxia_core_proto::ReopenTakeRequest {
                command_id: command()?,
                project_id: semantic_id(&cli, "--project-id")?,
                work_item_id: semantic_id(&cli, "--work-item-id")?,
                work_revision_id: semantic_id(&cli, "--work-revision-id")?,
                terminal_take_id: semantic_id(&cli, "--terminal-take-id")?,
                terminal_take_expected_revision: semantic_revision(&cli, "--expected-revision")?,
                new_primary_asset_id: semantic_id(&cli, "--new-primary-asset-id")?,
                operation_timeout_ms: timeout,
            })
        }
        Task009Kind::ListTakes => {
            let (page_size, cursor) = task_009_page(&cli)?;
            core_request::Operation::ListTakes(mengxia_core_proto::ListTakesRequest {
                project_id: semantic_id(&cli, "--project-id")?,
                work_item_id: semantic_id(&cli, "--work-item-id")?,
                work_revision_id: semantic_id(&cli, "--work-revision-id")?,
                page_size,
                cursor,
                operation_timeout_ms: timeout,
            })
        }
    };
    Ok(Task009Config {
        client,
        request: CoreRequest {
            operation: Some(operation),
        },
        kind: cli.kind,
    })
}

fn parse_id_text(value: Option<OsString>) -> Result<String, ErrorCode> {
    let value = selected_utf8(value)?;
    let id = Id::<ResultIdentity>::from_str(&value).map_err(|_| ErrorCode::ValidationError)?;
    (id.to_string() == value)
        .then_some(value)
        .ok_or(ErrorCode::ValidationError)
}

fn parse_hex_exact(value: &OsStr, output_length: usize) -> Result<Vec<u8>, ErrorCode> {
    let bytes = value.as_bytes();
    if bytes.len() != output_length.saturating_mul(2)
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(ErrorCode::ValidationError);
    }
    let mut output = Vec::with_capacity(output_length);
    for pair in bytes.as_chunks::<2>().0 {
        output.push((hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?);
    }
    Ok(output)
}

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

fn run_task_008(config: Task008Config) -> ExitCode {
    let fallback_retry = if config.kind == Task008Kind::Materialize {
        RetryAction::SameCommand
    } else {
        RetryAction::FreshCommand
    };
    let owner_uid = effective_user_id();
    let endpoint = match validate_client_endpoint(&config.client.endpoint, owner_uid) {
        Ok(endpoint) => endpoint,
        Err(error) => return fail_with_retry(authority_code(error), fallback_retry, 1),
    };
    let request_id = match Id::<RequestIdentity>::try_new() {
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
        Err(_) => return fail_with_retry(ErrorCode::IpcTransportError, fallback_retry, 1),
    };
    if std_stream.set_nonblocking(true).is_err() {
        return fail_with_retry(ErrorCode::IpcTransportError, fallback_retry, 1);
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return fail_with_retry(ErrorCode::InternalError, fallback_retry, 1),
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
        request_task_008_command(
            &mut stream,
            &request_id,
            &config.request,
            config.client.limits,
            config.client.operation_limits,
            config.client.operation_timeout,
        )
        .await
        .map_err(|error| error.code())
    });
    match result {
        Ok((session, response)) => handle_task_008_response(
            config.kind,
            &config.request,
            session.correlation_id(),
            response,
        ),
        Err(code @ (ErrorCode::AuthenticationError | ErrorCode::ProtocolVersionUnsupported)) => {
            fail_with_retry(code, RetryAction::OperatorOrRuntimeAction, 1)
        }
        Err(code) => fail_with_retry(code, fallback_retry, 1),
    }
}

fn run_task_009(config: Task009Config) -> ExitCode {
    let fallback_retry = if config.kind.is_query() {
        RetryAction::FreshCommand
    } else {
        RetryAction::SameCommand
    };
    let owner_uid = effective_user_id();
    let endpoint = match validate_client_endpoint(&config.client.endpoint, owner_uid) {
        Ok(endpoint) => endpoint,
        Err(error) => return fail_with_retry(authority_code(error), fallback_retry, 1),
    };
    let request_id = match Id::<RequestIdentity>::try_new() {
        Ok(id) => id.to_string(),
        Err(_) => {
            return fail_with_retry(
                ErrorCode::IdGenerationUnavailable,
                RetryAction::OperatorOrRuntimeAction,
                1,
            );
        }
    };
    let stream = match endpoint.connect() {
        Ok(stream) => stream,
        Err(_) => return fail_with_retry(ErrorCode::IpcTransportError, fallback_retry, 1),
    };
    if stream.set_nonblocking(true).is_err() {
        return fail_with_retry(ErrorCode::IpcTransportError, fallback_retry, 1);
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return fail_with_retry(ErrorCode::InternalError, fallback_retry, 1),
    };
    let result = runtime.block_on(async {
        let mut stream =
            tokio::net::UnixStream::from_std(stream).map_err(|_| ErrorCode::IpcTransportError)?;
        if stream
            .peer_cred()
            .map_err(|_| ErrorCode::AuthenticationError)?
            .uid()
            != owner_uid
        {
            return Err(ErrorCode::AuthenticationError);
        }
        request_task_009_command(
            &mut stream,
            &request_id,
            &config.request,
            config.client.limits,
            config.client.operation_limits,
            config.client.operation_timeout,
        )
        .await
        .map_err(|error| error.code())
    });
    match result {
        Ok((session, response)) => handle_task_009_response(
            config.kind,
            &config.request,
            session.correlation_id(),
            response,
        ),
        Err(code @ (ErrorCode::AuthenticationError | ErrorCode::ProtocolVersionUnsupported)) => {
            fail_with_retry(code, RetryAction::OperatorOrRuntimeAction, 1)
        }
        Err(code) => fail_with_retry(code, fallback_retry, 1),
    }
}

impl Task009Kind {
    const fn operation_id(self) -> &'static str {
        match self {
            Self::CreateAssetRevision => "asset.revision.create.v1",
            Self::RetireAsset => "asset.retire.v1",
            Self::RestoreAsset => "asset.restore.v1",
            Self::CreateProject => "project.create.v1",
            Self::ReviseProject => "project.spec.revise.v1",
            Self::ListProjects => "project.list.v1",
            Self::CreateSubject => "subject.create.v1",
            Self::ListSubjects => "subject.list.v1",
            Self::CreateWork => "work.create.v1",
            Self::ReviseWork => "work.revise.v1",
            Self::ListWork => "work.list.v1",
            Self::CreateTake => "take.create.v1",
            Self::TransitionTake => "take.transition.v1",
            Self::ReopenTake => "take.reopen.v1",
            Self::ListTakes => "take.list.v1",
        }
    }

    const fn is_query(self) -> bool {
        matches!(
            self,
            Self::ListProjects | Self::ListSubjects | Self::ListWork | Self::ListTakes
        )
    }
}

fn handle_task_009_response(
    kind: Task009Kind,
    request: &CoreRequest,
    correlation_id: &str,
    response: mengxia_core_proto::CoreResponse,
) -> ExitCode {
    if let Some(core_response::Response::Error(error)) = response.response.as_ref() {
        return match validate_error_response(correlation_id, error) {
            Some((code, retry)) => fail_with_retry(code, retry, 1),
            None => fail_with_retry(ErrorCode::IpcTransportError, RetryAction::FreshCommand, 1),
        };
    }
    match render_task_009(kind, request, response.response) {
        Some(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        None => fail_with_retry(
            ErrorCode::IpcTransportError,
            if kind.is_query() {
                RetryAction::FreshCommand
            } else {
                RetryAction::SameCommand
            },
            1,
        ),
    }
}

fn task_009_mutation_header(output: &mut String, kind: Task009Kind, replayed: bool) {
    use std::fmt::Write as _;
    writeln!(
        output,
        "MENGXIA_RESULT operation={} replayed={}",
        kind.operation_id(),
        u8::from(replayed)
    )
    .expect("String write");
}

fn valid_timestamp(seconds: i64, nanos: u32) -> bool {
    Timestamp::from_unix_seconds_nanos(seconds, nanos).is_ok()
}

fn render_task_009(
    kind: Task009Kind,
    request: &CoreRequest,
    response: Option<core_response::Response>,
) -> Option<String> {
    use std::fmt::Write as _;
    let mut output = String::new();
    match (kind, request.operation.as_ref()?, response?) {
        (
            Task009Kind::CreateAssetRevision,
            core_request::Operation::CreateAssetRevision(request),
            core_response::Response::CreateAssetRevision(value),
        ) => {
            if value.command_id != request.command_id
                || value.asset_id != request.asset_id
                || !valid_id(&value.asset_revision_id)
                || request.expected_revision.checked_add(1) != Some(value.resulting_revision)
                || !valid_timestamp(value.created_at_seconds, value.created_at_nanos)
            {
                return None;
            }
            task_009_mutation_header(&mut output, kind, value.replayed);
            writeln!(output, "command_id={}", value.command_id).ok()?;
            writeln!(output, "asset_id={}", value.asset_id).ok()?;
            writeln!(output, "asset_revision_id={}", value.asset_revision_id).ok()?;
            writeln!(output, "resulting_revision={}", value.resulting_revision).ok()?;
            writeln!(output, "created_at_seconds={}", value.created_at_seconds).ok()?;
            writeln!(output, "created_at_nanos={}", value.created_at_nanos).ok()?;
        }
        (
            Task009Kind::RetireAsset,
            core_request::Operation::RetireAsset(request),
            core_response::Response::RetireAsset(value),
        ) => render_asset_lifecycle_mutation(
            &mut output,
            kind,
            &request.command_id,
            &request.asset_id,
            request.expected_revision,
            "retired",
            value,
        )?,
        (
            Task009Kind::RestoreAsset,
            core_request::Operation::RestoreAsset(request),
            core_response::Response::RestoreAsset(value),
        ) => render_asset_lifecycle_mutation(
            &mut output,
            kind,
            &request.command_id,
            &request.asset_id,
            request.expected_revision,
            "active",
            value,
        )?,
        (
            Task009Kind::CreateProject,
            core_request::Operation::CreateProject(request),
            core_response::Response::CreateProject(value),
        ) => render_project_mutation(&mut output, kind, &request.command_id, None, 1, value)?,
        (
            Task009Kind::ReviseProject,
            core_request::Operation::ReviseProjectSpec(request),
            core_response::Response::ReviseProjectSpec(value),
        ) => render_project_mutation(
            &mut output,
            kind,
            &request.command_id,
            Some(&request.project_id),
            request.expected_revision.checked_add(1)?,
            value,
        )?,
        (
            Task009Kind::CreateSubject,
            core_request::Operation::CreateSubject(request),
            core_response::Response::CreateSubject(value),
        ) => {
            if value.command_id != request.command_id
                || !valid_id(&value.subject_id)
                || value.subject_revision == 0
                || !valid_timestamp(value.created_at_seconds, value.created_at_nanos)
            {
                return None;
            }
            task_009_mutation_header(&mut output, kind, value.replayed);
            writeln!(output, "command_id={}", value.command_id).ok()?;
            writeln!(output, "subject_id={}", value.subject_id).ok()?;
            writeln!(output, "subject_revision={}", value.subject_revision).ok()?;
            writeln!(output, "created_at_seconds={}", value.created_at_seconds).ok()?;
            writeln!(output, "created_at_nanos={}", value.created_at_nanos).ok()?;
        }
        (
            Task009Kind::CreateWork,
            core_request::Operation::CreateWorkItem(request),
            core_response::Response::CreateWorkItem(value),
        ) => render_work_mutation(&mut output, kind, &request.command_id, None, 1, value)?,
        (
            Task009Kind::ReviseWork,
            core_request::Operation::ReviseWork(request),
            core_response::Response::ReviseWork(value),
        ) => render_work_mutation(
            &mut output,
            kind,
            &request.command_id,
            Some(&request.work_item_id),
            request.expected_revision.checked_add(1)?,
            value,
        )?,
        (
            Task009Kind::CreateTake,
            core_request::Operation::CreateTake(request),
            core_response::Response::CreateTake(value),
        ) => render_take_mutation(
            &mut output,
            kind,
            &request.command_id,
            TakeMutationExpectation {
                take_id: None,
                primary_asset_id: Some(&request.primary_asset_id),
                revision: 1,
                state: "candidate",
                related_take_id: None,
            },
            value,
        )?,
        (
            Task009Kind::TransitionTake,
            core_request::Operation::TransitionTake(request),
            core_response::Response::TransitionTake(value),
        ) => {
            let state =
                match mengxia_core_proto::TakeTransitionValue::try_from(request.transition).ok()? {
                    mengxia_core_proto::TakeTransitionValue::Shortlist => "shortlisted",
                    mengxia_core_proto::TakeTransitionValue::Select => "selected",
                    mengxia_core_proto::TakeTransitionValue::Approve => "approved",
                    mengxia_core_proto::TakeTransitionValue::Reject => "rejected",
                    mengxia_core_proto::TakeTransitionValue::Supersede => "superseded",
                    mengxia_core_proto::TakeTransitionValue::Unspecified => return None,
                };
            render_take_mutation(
                &mut output,
                kind,
                &request.command_id,
                TakeMutationExpectation {
                    take_id: Some(&request.take_id),
                    primary_asset_id: None,
                    revision: request.expected_revision.checked_add(1)?,
                    state,
                    related_take_id: request.related_take_id.as_deref(),
                },
                value,
            )?
        }
        (
            Task009Kind::ReopenTake,
            core_request::Operation::ReopenTake(request),
            core_response::Response::ReopenTake(value),
        ) => render_take_mutation(
            &mut output,
            kind,
            &request.command_id,
            TakeMutationExpectation {
                take_id: None,
                primary_asset_id: Some(&request.new_primary_asset_id),
                revision: 1,
                state: "candidate",
                related_take_id: Some(&request.terminal_take_id),
            },
            value,
        )?,
        (
            Task009Kind::ListProjects,
            core_request::Operation::ListProjects(request),
            core_response::Response::ListProjects(value),
        ) => render_projects(&mut output, kind, request.page_size, value)?,
        (
            Task009Kind::ListSubjects,
            core_request::Operation::ListSubjects(request),
            core_response::Response::ListSubjects(value),
        ) => render_subjects(&mut output, kind, request.page_size, value)?,
        (
            Task009Kind::ListWork,
            core_request::Operation::ListWork(request),
            core_response::Response::ListWork(value),
        ) => render_work(
            &mut output,
            kind,
            request.page_size,
            &request.project_id,
            value,
        )?,
        (
            Task009Kind::ListTakes,
            core_request::Operation::ListTakes(request),
            core_response::Response::ListTakes(value),
        ) => render_takes(
            &mut output,
            kind,
            request.page_size,
            &request.work_revision_id,
            value,
        )?,
        _ => return None,
    }
    Some(output)
}

fn task_009_page_header(
    output: &mut String,
    kind: Task009Kind,
    snapshot: u64,
    count: usize,
    requested_page_size: u32,
    cursor: Option<Vec<u8>>,
) -> Option<()> {
    use std::fmt::Write as _;
    if count > usize::try_from(requested_page_size).ok()?
        || count > 64
        || (snapshot == 0 && count != 0)
    {
        return None;
    }
    let cursor = match cursor {
        None => "NONE".to_owned(),
        Some(bytes) if bytes.len() == 160 && opaque_cursor_checksum_is_valid(&bytes) => {
            lowercase_hex(&bytes)
        }
        Some(_) => return None,
    };
    writeln!(
        output,
        "MENGXIA_PAGE operation={} snapshot={} count={} next_cursor={}",
        kind.operation_id(),
        snapshot,
        count,
        cursor
    )
    .ok()
}

fn render_asset_lifecycle_mutation(
    output: &mut String,
    kind: Task009Kind,
    expected_command: &str,
    expected_asset: &str,
    expected_revision: u64,
    expected_lifecycle: &str,
    value: mengxia_core_proto::AssetLifecycleMutationResult,
) -> Option<()> {
    use std::fmt::Write as _;
    let lifecycle = enum_name::<mengxia_core_proto::AssetLifecycleValue>(value.lifecycle)?;
    if value.command_id != expected_command
        || value.asset_id != expected_asset
        || expected_revision.checked_add(1) != Some(value.resulting_revision)
        || lifecycle != expected_lifecycle
        || !valid_timestamp(value.updated_at_seconds, value.updated_at_nanos)
    {
        return None;
    }
    task_009_mutation_header(output, kind, value.replayed);
    writeln!(output, "command_id={}", value.command_id).ok()?;
    writeln!(output, "asset_id={}", value.asset_id).ok()?;
    writeln!(output, "resulting_revision={}", value.resulting_revision).ok()?;
    writeln!(output, "lifecycle={}", lifecycle.to_ascii_uppercase()).ok()?;
    writeln!(output, "updated_at_seconds={}", value.updated_at_seconds).ok()?;
    writeln!(output, "updated_at_nanos={}", value.updated_at_nanos).ok()
}

struct TakeMutationExpectation<'a> {
    take_id: Option<&'a str>,
    primary_asset_id: Option<&'a str>,
    revision: u64,
    state: &'a str,
    related_take_id: Option<&'a str>,
}

fn render_take_mutation(
    output: &mut String,
    kind: Task009Kind,
    expected_command: &str,
    expected: TakeMutationExpectation<'_>,
    value: mengxia_core_proto::TakeMutationResult,
) -> Option<()> {
    use std::fmt::Write as _;
    let state = enum_name::<mengxia_core_proto::TakeStateValue>(value.state)?;
    if value.command_id != expected_command
        || !valid_id(&value.take_id)
        || !valid_id(&value.primary_asset_id)
        || expected.take_id.is_some_and(|id| value.take_id != id)
        || expected
            .primary_asset_id
            .is_some_and(|id| value.primary_asset_id != id)
        || value.ordinal > 1_048_575
        || value.take_revision != expected.revision
        || state != expected.state
        || value.related_take_id.as_deref() != expected.related_take_id
        || value
            .related_take_id
            .as_deref()
            .is_some_and(|id| !valid_id(id))
        || !valid_timestamp(value.updated_at_seconds, value.updated_at_nanos)
    {
        return None;
    }
    task_009_mutation_header(output, kind, value.replayed);
    writeln!(output, "command_id={}", value.command_id).ok()?;
    writeln!(output, "take_id={}", value.take_id).ok()?;
    writeln!(output, "ordinal={}", value.ordinal).ok()?;
    writeln!(output, "state={}", state.to_ascii_uppercase()).ok()?;
    writeln!(output, "primary_asset_id={}", value.primary_asset_id).ok()?;
    writeln!(output, "take_revision={}", value.take_revision).ok()?;
    if let Some(id) = value.related_take_id {
        writeln!(output, "related_take_id={id}").ok()?;
    }
    writeln!(output, "updated_at_seconds={}", value.updated_at_seconds).ok()?;
    writeln!(output, "updated_at_nanos={}", value.updated_at_nanos).ok()
}

fn render_project_mutation(
    output: &mut String,
    kind: Task009Kind,
    expected_command: &str,
    expected_project: Option<&str>,
    expected_revision: u64,
    value: mengxia_core_proto::ProjectMutationResult,
) -> Option<()> {
    use std::fmt::Write as _;
    if value.command_id != expected_command
        || !valid_id(&value.project_id)
        || !valid_id(&value.project_spec_revision_id)
        || expected_project.is_some_and(|id| value.project_id != id)
        || value.project_revision != expected_revision
        || value.specification_sequence != u32::try_from(expected_revision).ok()?
        || !valid_timestamp(value.updated_at_seconds, value.updated_at_nanos)
    {
        return None;
    }
    task_009_mutation_header(output, kind, value.replayed);
    writeln!(output, "command_id={}", value.command_id).ok()?;
    writeln!(output, "project_id={}", value.project_id).ok()?;
    writeln!(
        output,
        "project_spec_revision_id={}",
        value.project_spec_revision_id
    )
    .ok()?;
    writeln!(output, "project_revision={}", value.project_revision).ok()?;
    writeln!(
        output,
        "specification_sequence={}",
        value.specification_sequence
    )
    .ok()?;
    writeln!(output, "updated_at_seconds={}", value.updated_at_seconds).ok()?;
    writeln!(output, "updated_at_nanos={}", value.updated_at_nanos).ok()
}

fn render_work_mutation(
    output: &mut String,
    kind: Task009Kind,
    expected_command: &str,
    expected_work: Option<&str>,
    expected_revision: u64,
    value: mengxia_core_proto::WorkMutationResult,
) -> Option<()> {
    use std::fmt::Write as _;
    if value.command_id != expected_command
        || !valid_id(&value.work_item_id)
        || !valid_id(&value.work_revision_id)
        || expected_work.is_some_and(|id| value.work_item_id != id)
        || value.work_item_revision != expected_revision
        || value.work_revision_sequence != u32::try_from(expected_revision).ok()?
        || !valid_timestamp(value.updated_at_seconds, value.updated_at_nanos)
    {
        return None;
    }
    task_009_mutation_header(output, kind, value.replayed);
    writeln!(output, "command_id={}", value.command_id).ok()?;
    writeln!(output, "work_item_id={}", value.work_item_id).ok()?;
    writeln!(output, "work_revision_id={}", value.work_revision_id).ok()?;
    writeln!(output, "work_item_revision={}", value.work_item_revision).ok()?;
    writeln!(
        output,
        "work_revision_sequence={}",
        value.work_revision_sequence
    )
    .ok()?;
    writeln!(output, "updated_at_seconds={}", value.updated_at_seconds).ok()?;
    writeln!(output, "updated_at_nanos={}", value.updated_at_nanos).ok()
}

fn render_projects(
    output: &mut String,
    kind: Task009Kind,
    requested_page_size: u32,
    value: mengxia_core_proto::ListProjectsResult,
) -> Option<()> {
    use std::fmt::Write as _;
    task_009_page_header(
        output,
        kind,
        value.snapshot_commit_sequence,
        value.projects.len(),
        requested_page_size,
        value.next_cursor,
    )?;
    let mut previous_key = 0;
    for (index, project) in value.projects.into_iter().enumerate() {
        let spec = project.current_specification?;
        let scalar_pairs = [
            (spec.resolution_width, spec.resolution_height),
            (spec.frame_rate_numerator, spec.frame_rate_denominator),
            (spec.aspect_ratio_numerator, spec.aspect_ratio_denominator),
        ];
        if !valid_id(&project.project_id)
            || !valid_id(&spec.project_spec_revision_id)
            || project.revision == 0
            || project.creation_commit_sequence == 0
            || project.creation_commit_sequence <= previous_key
            || project.creation_commit_sequence > value.snapshot_commit_sequence
            || spec.sequence == 0
            || spec.policy_schema_version != 1
            || spec.policy_sha256.len() != 32
            || scalar_pairs
                .iter()
                .any(|(left, right)| left.is_some() != right.is_some())
            || !valid_timestamp(project.created_at_seconds, project.created_at_nanos)
            || !valid_timestamp(project.updated_at_seconds, project.updated_at_nanos)
            || enum_name::<mengxia_core_proto::ProjectTrustValue>(project.effective_trust)
                != Some("untrusted".to_owned())
        {
            return None;
        }
        previous_key = project.creation_commit_sequence;
        for policy in [
            &spec.color_policy_json,
            &spec.audio_policy_json,
            &spec.quality_policy_json,
            &spec.privacy_policy_json,
        ] {
            let parsed = parse_project_policy(policy).ok()?;
            if parsed.bytes() != policy {
                return None;
            }
        }
        writeln!(output, "ROW index={index}").ok()?;
        writeln!(output, "project_id={}", project.project_id).ok()?;
        writeln!(
            output,
            "name_hex={}",
            lowercase_hex(project.name.as_bytes())
        )
        .ok()?;
        writeln!(output, "revision={}", project.revision).ok()?;
        writeln!(output, "created_at_seconds={}", project.created_at_seconds).ok()?;
        writeln!(output, "created_at_nanos={}", project.created_at_nanos).ok()?;
        writeln!(output, "updated_at_seconds={}", project.updated_at_seconds).ok()?;
        writeln!(output, "updated_at_nanos={}", project.updated_at_nanos).ok()?;
        writeln!(
            output,
            "creation_commit_sequence={}",
            project.creation_commit_sequence
        )
        .ok()?;
        writeln!(
            output,
            "current_specification.project_spec_revision_id={}",
            spec.project_spec_revision_id
        )
        .ok()?;
        writeln!(output, "current_specification.sequence={}", spec.sequence).ok()?;
        for (name, scalar) in [
            ("resolution_width", spec.resolution_width),
            ("resolution_height", spec.resolution_height),
            ("frame_rate_numerator", spec.frame_rate_numerator),
            ("frame_rate_denominator", spec.frame_rate_denominator),
            ("aspect_ratio_numerator", spec.aspect_ratio_numerator),
            ("aspect_ratio_denominator", spec.aspect_ratio_denominator),
        ] {
            writeln!(
                output,
                "current_specification.{name}={}",
                scalar.map_or_else(|| "NONE".to_owned(), |item| item.to_string())
            )
            .ok()?;
        }
        writeln!(
            output,
            "current_specification.policy_schema_version={}",
            spec.policy_schema_version
        )
        .ok()?;
        for (name, bytes) in [
            ("color_policy_json_hex", spec.color_policy_json),
            ("audio_policy_json_hex", spec.audio_policy_json),
            ("quality_policy_json_hex", spec.quality_policy_json),
            ("privacy_policy_json_hex", spec.privacy_policy_json),
        ] {
            writeln!(
                output,
                "current_specification.{name}={}",
                lowercase_hex(&bytes)
            )
            .ok()?;
        }
        writeln!(
            output,
            "current_specification.policy_sha256={}",
            lowercase_hex(&spec.policy_sha256)
        )
        .ok()?;
        writeln!(output, "effective_trust=UNTRUSTED").ok()?;
    }
    Some(())
}

fn render_subjects(
    output: &mut String,
    kind: Task009Kind,
    requested_page_size: u32,
    value: mengxia_core_proto::ListSubjectsResult,
) -> Option<()> {
    use std::fmt::Write as _;
    task_009_page_header(
        output,
        kind,
        value.snapshot_commit_sequence,
        value.subjects.len(),
        requested_page_size,
        value.next_cursor,
    )?;
    let mut previous_key = 0;
    for (index, subject) in value.subjects.into_iter().enumerate() {
        if !valid_id(&subject.subject_id)
            || subject.revision == 0
            || subject.creation_commit_sequence == 0
            || subject.creation_commit_sequence <= previous_key
            || subject.creation_commit_sequence > value.snapshot_commit_sequence
            || !safe_token(&subject.kind, 64)
            || !valid_timestamp(subject.created_at_seconds, subject.created_at_nanos)
        {
            return None;
        }
        previous_key = subject.creation_commit_sequence;
        writeln!(output, "ROW index={index}").ok()?;
        writeln!(output, "subject_id={}", subject.subject_id).ok()?;
        writeln!(output, "kind={}", subject.kind).ok()?;
        writeln!(
            output,
            "canonical_name_hex={}",
            lowercase_hex(subject.canonical_name.as_bytes())
        )
        .ok()?;
        writeln!(output, "revision={}", subject.revision).ok()?;
        writeln!(output, "created_at_seconds={}", subject.created_at_seconds).ok()?;
        writeln!(output, "created_at_nanos={}", subject.created_at_nanos).ok()?;
        writeln!(
            output,
            "creation_commit_sequence={}",
            subject.creation_commit_sequence
        )
        .ok()?;
    }
    Some(())
}

fn render_work(
    output: &mut String,
    kind: Task009Kind,
    requested_page_size: u32,
    expected_project_id: &str,
    value: mengxia_core_proto::ListWorkResult,
) -> Option<()> {
    use std::fmt::Write as _;
    task_009_page_header(
        output,
        kind,
        value.snapshot_commit_sequence,
        value.work_items.len(),
        requested_page_size,
        value.next_cursor,
    )?;
    let mut previous_key = 0;
    for (index, work) in value.work_items.into_iter().enumerate() {
        let work_kind = enum_name::<mengxia_core_proto::WorkKindValue>(work.kind)?;
        if !valid_id(&work.work_item_id)
            || !valid_id(&work.project_id)
            || work.project_id != expected_project_id
            || !valid_id(&work.current_work_revision_id)
            || work.revision == 0
            || work.current_work_revision_sequence == 0
            || work.creation_commit_sequence == 0
            || work.creation_commit_sequence <= previous_key
            || work.creation_commit_sequence > value.snapshot_commit_sequence
            || work.specification_schema_version != 1
            || work.specification_sha256.len() != 32
            || !valid_timestamp(work.created_at_seconds, work.created_at_nanos)
            || !valid_timestamp(work.updated_at_seconds, work.updated_at_nanos)
            || work.subject_ids.iter().any(|id| !valid_id(id))
            || work.asset_ids.iter().any(|id| !valid_id(id))
            || !ids_are_strictly_sorted(&work.subject_ids)
            || !ids_are_strictly_sorted(&work.asset_ids)
        {
            return None;
        }
        previous_key = work.creation_commit_sequence;
        let specification = parse_work_specification(&work.specification_json).ok()?;
        if specification.bytes() != work.specification_json
            || specification.digest().to_bytes().as_slice() != work.specification_sha256
        {
            return None;
        }
        writeln!(output, "ROW index={index}").ok()?;
        writeln!(output, "work_item_id={}", work.work_item_id).ok()?;
        writeln!(output, "project_id={}", work.project_id).ok()?;
        writeln!(output, "kind={}", work_kind.to_ascii_uppercase()).ok()?;
        writeln!(output, "code_hex={}", lowercase_hex(work.code.as_bytes())).ok()?;
        writeln!(output, "revision={}", work.revision).ok()?;
        writeln!(output, "created_at_seconds={}", work.created_at_seconds).ok()?;
        writeln!(output, "created_at_nanos={}", work.created_at_nanos).ok()?;
        writeln!(output, "updated_at_seconds={}", work.updated_at_seconds).ok()?;
        writeln!(output, "updated_at_nanos={}", work.updated_at_nanos).ok()?;
        writeln!(
            output,
            "creation_commit_sequence={}",
            work.creation_commit_sequence
        )
        .ok()?;
        writeln!(
            output,
            "current_work_revision_id={}",
            work.current_work_revision_id
        )
        .ok()?;
        writeln!(
            output,
            "current_work_revision_sequence={}",
            work.current_work_revision_sequence
        )
        .ok()?;
        writeln!(
            output,
            "specification_schema_version={}",
            work.specification_schema_version
        )
        .ok()?;
        writeln!(
            output,
            "specification_json_hex={}",
            lowercase_hex(&work.specification_json)
        )
        .ok()?;
        writeln!(
            output,
            "specification_sha256={}",
            lowercase_hex(&work.specification_sha256)
        )
        .ok()?;
        for (item, id) in work.subject_ids.iter().enumerate() {
            writeln!(output, "subject_id.{item}={id}").ok()?;
        }
        for (item, id) in work.asset_ids.iter().enumerate() {
            writeln!(output, "asset_id.{item}={id}").ok()?;
        }
    }
    Some(())
}

fn render_takes(
    output: &mut String,
    kind: Task009Kind,
    requested_page_size: u32,
    expected_work_revision_id: &str,
    value: mengxia_core_proto::ListTakesResult,
) -> Option<()> {
    use std::fmt::Write as _;
    task_009_page_header(
        output,
        kind,
        value.snapshot_ordinal,
        value.takes.len(),
        requested_page_size,
        value.next_cursor,
    )?;
    let mut previous_key = None;
    for (index, take) in value.takes.into_iter().enumerate() {
        let state = enum_name::<mengxia_core_proto::TakeStateValue>(take.state)?;
        if !valid_id(&take.take_id)
            || !valid_id(&take.work_revision_id)
            || take.work_revision_id != expected_work_revision_id
            || !valid_id(&take.primary_asset_id)
            || take.revision == 0
            || take.ordinal > 1_048_575
            || take.ordinal == 0
            || previous_key.is_some_and(|key| take.ordinal <= key)
            || u64::from(take.ordinal) > value.snapshot_ordinal
            || take.outgoing_relationships.len() > 2
            || !valid_timestamp(take.created_at_seconds, take.created_at_nanos)
            || !valid_timestamp(take.updated_at_seconds, take.updated_at_nanos)
        {
            return None;
        }
        previous_key = Some(take.ordinal);
        let mut previous_relationship: Option<(i32, &str)> = None;
        writeln!(output, "ROW index={index}").ok()?;
        writeln!(output, "take_id={}", take.take_id).ok()?;
        writeln!(output, "work_revision_id={}", take.work_revision_id).ok()?;
        writeln!(output, "ordinal={}", take.ordinal).ok()?;
        writeln!(output, "state={}", state.to_ascii_uppercase()).ok()?;
        writeln!(output, "primary_asset_id={}", take.primary_asset_id).ok()?;
        writeln!(output, "revision={}", take.revision).ok()?;
        writeln!(output, "created_at_seconds={}", take.created_at_seconds).ok()?;
        writeln!(output, "created_at_nanos={}", take.created_at_nanos).ok()?;
        writeln!(output, "updated_at_seconds={}", take.updated_at_seconds).ok()?;
        writeln!(output, "updated_at_nanos={}", take.updated_at_nanos).ok()?;
        for (item, relationship) in take.outgoing_relationships.iter().enumerate() {
            let relationship_kind =
                enum_name::<mengxia_core_proto::TakeRelationshipKindValue>(relationship.kind)?;
            if !valid_id(&relationship.relationship_id) || !valid_id(&relationship.target_take_id) {
                return None;
            }
            let current_relationship = (relationship.kind, relationship.target_take_id.as_str());
            if relationship.target_take_id == take.take_id
                || previous_relationship.is_some_and(|previous| previous >= current_relationship)
            {
                return None;
            }
            previous_relationship = Some(current_relationship);
            writeln!(
                output,
                "relationship.{item}={}:{}:{}",
                relationship_kind.to_ascii_uppercase(),
                relationship.relationship_id,
                relationship.target_take_id
            )
            .ok()?;
        }
    }
    Some(())
}

fn handle_task_008_response(
    kind: Task008Kind,
    request: &CoreRequest,
    correlation_id: &str,
    response: mengxia_core_proto::CoreResponse,
) -> ExitCode {
    if let Some(core_response::Response::Error(error)) = response.response.as_ref() {
        return match validate_error_response(correlation_id, error) {
            Some((code, retry)) => fail_with_retry(code, retry, 1),
            None => fail_with_retry(ErrorCode::IpcTransportError, RetryAction::FreshCommand, 1),
        };
    }
    let valid = match (kind, request.operation.as_ref(), response.response) {
        (
            Task008Kind::Status,
            Some(core_request::Operation::GetLibraryStatus(_)),
            Some(core_response::Response::GetLibraryStatus(value)),
        ) => render_status(value),
        (
            Task008Kind::Verify,
            Some(core_request::Operation::VerifyLibrary(request)),
            Some(core_response::Response::VerifyLibrary(value)),
        ) => render_verification(request, value),
        (
            Task008Kind::Issues,
            Some(core_request::Operation::ListIntegrityIssues(request)),
            Some(core_response::Response::ListIntegrityIssues(value)),
        ) => render_issues(request, value),
        (
            Task008Kind::ListAssets,
            Some(core_request::Operation::ListAssets(_)),
            Some(core_response::Response::ListAssets(value)),
        ) => render_asset_list(value),
        (
            Task008Kind::InspectAsset,
            Some(core_request::Operation::InspectAsset(request)),
            Some(core_response::Response::InspectAsset(value)),
        ) => render_asset_inspection(request, value),
        (
            Task008Kind::Materialize,
            Some(core_request::Operation::MaterializeAsset(request)),
            Some(core_response::Response::MaterializeAsset(value)),
        ) => render_materialization(request, value),
        _ => false,
    };
    if valid {
        ExitCode::SUCCESS
    } else {
        fail_with_retry(
            ErrorCode::IpcTransportError,
            if kind == Task008Kind::Materialize {
                RetryAction::SameCommand
            } else {
                RetryAction::FreshCommand
            },
            1,
        )
    }
}

fn validate_error_response(
    correlation_id: &str,
    error: &mengxia_core_proto::ErrorEnvelope,
) -> Option<(ErrorCode, RetryAction)> {
    let code = ErrorCode::from_str(&error.code).ok()?;
    let retry = error
        .retry_action
        .and_then(|value| RetryAction::try_from(value).ok())?;
    (retry != RetryAction::Unspecified
        && error.correlation_id.as_deref() == Some(correlation_id)
        && error.safe_details.is_empty()
        && operation_safe_message(code) == Some(error.safe_message.as_str())
        && valid_operation_retry_pair(code, retry)
        && error.retryable
            == !matches!(
                retry,
                RetryAction::None | RetryAction::OperatorOrRuntimeAction
            ))
    .then_some((code, retry))
}

fn render_status(value: mengxia_core_proto::GetLibraryStatusResult) -> bool {
    use mengxia_core_proto::{CoreAvailability, CoreLiveness, CoreReadiness, ReadinessBlockReason};

    let Ok(liveness_value) = CoreLiveness::try_from(value.liveness) else {
        return false;
    };
    let Ok(readiness_value) = CoreReadiness::try_from(value.readiness) else {
        return false;
    };
    let Ok(availability_value) = CoreAvailability::try_from(value.availability) else {
        return false;
    };
    let Some(security) =
        enum_name::<mengxia_core_proto::LocalSecurityBaseline>(value.local_security_baseline)
    else {
        return false;
    };
    let Some(custody) =
        enum_name::<mengxia_core_proto::CustodyObservation>(value.custody_observation)
    else {
        return false;
    };
    let Ok(block_value) = ReadinessBlockReason::try_from(value.readiness_block_reason) else {
        return false;
    };
    if matches!(liveness_value, CoreLiveness::Unspecified)
        || matches!(readiness_value, CoreReadiness::Unspecified)
        || matches!(availability_value, CoreAvailability::Unspecified)
        || matches!(block_value, ReadinessBlockReason::Unspecified)
        || value.staging_orphan_count > u32::from(u16::MAX)
        || (!value.recovery_observation_available && value.recovery_required_command_count != 0)
        || (block_value == ReadinessBlockReason::None && !value.recovery_observation_available)
        || ((block_value == ReadinessBlockReason::None)
            != (readiness_value == CoreReadiness::Ready))
        || (readiness_value == CoreReadiness::Ready
            && (liveness_value != CoreLiveness::Live
                || value.local_security_baseline
                    != mengxia_core_proto::LocalSecurityBaseline::Verified as i32))
        || ((availability_value == CoreAvailability::ReadOnlyCustody)
            != !value.local_backend_matches)
    {
        return false;
    }
    let expected_capabilities =
        if readiness_value == CoreReadiness::NotReady || liveness_value != CoreLiveness::Live {
            (false, false, false, false)
        } else if availability_value == CoreAvailability::ReadOnlyCustody {
            (true, true, false, false)
        } else {
            (true, true, true, true)
        };
    if expected_capabilities
        != (
            value.can_read_metadata,
            value.can_verify,
            value.can_ingest,
            value.can_materialize,
        )
    {
        return false;
    }
    let liveness = enum_debug_name(liveness_value);
    let readiness = enum_debug_name(readiness_value);
    let availability = enum_debug_name(availability_value);
    let block = enum_debug_name(block_value);
    println!(
        "MENGXIA_LIBRARY_STATUS liveness={liveness} readiness={readiness} availability={availability} local_security_baseline={security} custody_observation={custody} readiness_block_reason={block} can_read_metadata={} can_verify={} can_ingest={} can_materialize={} staging_orphan_count={} staging_orphan_bytes={} local_backend_matches={} observability_degraded={} recovery_observation_available={} recovery_required_command_count={}",
        value.can_read_metadata,
        value.can_verify,
        value.can_ingest,
        value.can_materialize,
        value.staging_orphan_count,
        value.staging_orphan_bytes,
        value.local_backend_matches,
        value.observability_degraded,
        value.recovery_observation_available,
        value.recovery_required_command_count
    );
    true
}

fn render_verification(
    request: &mengxia_core_proto::VerifyLibraryRequest,
    value: mengxia_core_proto::VerifyLibraryResult,
) -> bool {
    if value.mode != request.mode
        || !valid_id(&value.verification_id)
        || value.snapshot_commit_sequence > i64::MAX as u64
        || value.stored_issue_count > 4_096
        || u64::from(value.stored_issue_count) > value.discovered_issue_count
        || value.dropped_issue_count
            != value.discovered_issue_count - u64::from(value.stored_issue_count)
        || value.has_fatal_local_issue != value.first_fatal_issue.is_some()
    {
        return false;
    }
    let Some(mode) = enum_name::<mengxia_core_proto::VerificationMode>(value.mode) else {
        return false;
    };
    println!(
        "MENGXIA_VERIFY_OK verification_id={} mode={mode} snapshot_commit_sequence={} discovered_issue_count={} stored_issue_count={} dropped_issue_count={} has_fatal_local_issue={} has_custody_degradation={} canonical_extra_classification_deferred={}",
        value.verification_id,
        value.snapshot_commit_sequence,
        value.discovered_issue_count,
        value.stored_issue_count,
        value.dropped_issue_count,
        value.has_fatal_local_issue,
        value.has_custody_degradation,
        value.canonical_extra_classification_deferred
    );
    value.first_fatal_issue.is_none_or(|issue| {
        render_issue(
            &value.verification_id,
            issue,
            value.discovered_issue_count,
            value.stored_issue_count,
            value.dropped_issue_count,
            None,
        )
    })
}

fn render_issues(
    request: &mengxia_core_proto::ListIntegrityIssuesRequest,
    value: mengxia_core_proto::ListIntegrityIssuesResult,
) -> bool {
    if value.verification_id != request.verification_id
        || !valid_id(&value.verification_id)
        || value.issues.len() > 64
        || (value.issues.is_empty() && value.next_cursor.is_some())
        || value.stored_issue_count > 4_096
        || value
            .issues
            .windows(2)
            .any(|pair| pair[0].ordinal >= pair[1].ordinal)
        || u64::from(value.stored_issue_count) > value.discovered_issue_count
        || value.dropped_issue_count
            != value.discovered_issue_count - u64::from(value.stored_issue_count)
        || value
            .next_cursor
            .as_ref()
            .is_some_and(|cursor| cursor.len() != 96 || !opaque_cursor_checksum_is_valid(cursor))
    {
        return false;
    }
    let cursor = value.next_cursor.as_deref();
    if value.issues.is_empty() {
        println!(
            "MENGXIA_INTEGRITY_ISSUE verification_id={} ordinal=- kind=- severity=- object_kind=- object_id=- remediation=- discovered_issue_count={} stored_issue_count={} dropped_issue_count={} next_cursor={}",
            value.verification_id,
            value.discovered_issue_count,
            value.stored_issue_count,
            value.dropped_issue_count,
            optional_hex(cursor)
        );
        return true;
    }
    for issue in value.issues {
        if !render_issue(
            &value.verification_id,
            issue,
            value.discovered_issue_count,
            value.stored_issue_count,
            value.dropped_issue_count,
            cursor,
        ) {
            return false;
        }
    }
    true
}

fn render_issue(
    verification_id: &str,
    issue: mengxia_core_proto::IntegrityIssue,
    discovered_issue_count: u64,
    stored_issue_count: u32,
    dropped_issue_count: u64,
    next_cursor: Option<&[u8]>,
) -> bool {
    use mengxia_core_proto::{IntegrityIssueKind, IntegrityObjectKind, IntegritySeverity};

    let Ok(kind_value) = IntegrityIssueKind::try_from(issue.kind) else {
        return false;
    };
    let Ok(severity_value) = IntegritySeverity::try_from(issue.severity) else {
        return false;
    };
    let Ok(object_kind_value) = IntegrityObjectKind::try_from(issue.object_kind) else {
        return false;
    };
    let Some(remediation) =
        enum_name::<mengxia_core_proto::IntegrityRemediation>(issue.remediation)
    else {
        return false;
    };
    let expected_severity = match kind_value {
        IntegrityIssueKind::DatabaseIntegrityFailure
        | IntegrityIssueKind::SchemaOrMigrationMismatch
        | IntegrityIssueKind::LibraryAuthorityMismatch
        | IntegrityIssueKind::EventOrGraphInconsistent => IntegritySeverity::FatalLocal,
        IntegrityIssueKind::CommandRecoveryRequired
        | IntegrityIssueKind::UnregisteredCanonicalBlob
        | IntegrityIssueKind::StagingOrphan
        | IntegrityIssueKind::MaterializationRecoveryRequired => IntegritySeverity::OperatorAction,
        IntegrityIssueKind::LocalBackendMismatch | IntegrityIssueKind::UnsafeCasNamespaceEntry => {
            IntegritySeverity::ReadOnlyCustody
        }
        IntegrityIssueKind::ManagedBlobMissing
        | IntegrityIssueKind::ManagedBlobUnsafe
        | IntegrityIssueKind::ManagedBlobLengthMismatch
        | IntegrityIssueKind::ManagedBlobDigestMismatch => IntegritySeverity::DegradedCustody,
        IntegrityIssueKind::Unspecified => return false,
    };
    let object_id_valid = match (object_kind_value, issue.object_id.as_deref()) {
        (IntegrityObjectKind::Unspecified, _) => false,
        (IntegrityObjectKind::Blob, Some(id)) => id.len() == 32,
        (IntegrityObjectKind::Blob, None) => true,
        (_, Some(id)) => id.len() == 16,
        (_, None) => true,
    };
    if issue.ordinal == 0
        || u64::from(issue.ordinal) > discovered_issue_count
        || issue.ordinal > stored_issue_count
        || severity_value != expected_severity
        || !object_id_valid
    {
        return false;
    }
    let kind = enum_debug_name(kind_value);
    let severity = enum_debug_name(severity_value);
    let object_kind = enum_debug_name(object_kind_value);
    println!(
        "MENGXIA_INTEGRITY_ISSUE verification_id={verification_id} ordinal={} kind={kind} severity={severity} object_kind={object_kind} object_id={} remediation={remediation} discovered_issue_count={discovered_issue_count} stored_issue_count={stored_issue_count} dropped_issue_count={dropped_issue_count} next_cursor={}",
        issue.ordinal,
        optional_hex(issue.object_id.as_deref()),
        optional_hex(next_cursor)
    );
    true
}

fn render_asset_list(value: mengxia_core_proto::ListAssetsResult) -> bool {
    if value.snapshot_commit_sequence > i64::MAX as u64
        || value.assets.len() > 64
        || (value.assets.is_empty() && value.next_cursor.is_some())
        || value
            .assets
            .windows(2)
            .any(|pair| pair[0].creation_commit_sequence >= pair[1].creation_commit_sequence)
        || value
            .next_cursor
            .as_ref()
            .is_some_and(|cursor| cursor.len() != 80 || !opaque_cursor_checksum_is_valid(cursor))
    {
        return false;
    }
    let cursor = optional_hex(value.next_cursor.as_deref());
    if value.assets.is_empty() {
        println!(
            "MENGXIA_ASSET snapshot_commit_sequence={} asset_id=- kind=- lifecycle=- revision=- created_at_seconds=- created_at_nanos=- creation_commit_sequence=- asset_revision_id=- revision_sequence=- content_kind=- custody=- parent_revision_ids=- next_cursor={cursor}",
            value.snapshot_commit_sequence
        );
        return true;
    }
    for asset in value.assets {
        if !render_asset_summary(value.snapshot_commit_sequence, &cursor, asset) {
            return false;
        }
    }
    true
}

fn render_asset_summary(
    snapshot: u64,
    cursor: &str,
    asset: mengxia_core_proto::AssetSummary,
) -> bool {
    if !valid_id(&asset.asset_id)
        || !safe_token(&asset.kind, 64)
        || asset.revision == 0
        || asset.creation_commit_sequence == 0
        || asset.creation_commit_sequence > i64::MAX as u64
        || asset.creation_commit_sequence > snapshot
        || Timestamp::from_unix_seconds_nanos(asset.created_at_seconds, asset.created_at_nanos)
            .is_err()
    {
        return false;
    }
    let Some(lifecycle) = enum_name::<mengxia_core_proto::AssetLifecycleValue>(asset.lifecycle)
    else {
        return false;
    };
    println!(
        "MENGXIA_ASSET snapshot_commit_sequence={snapshot} asset_id={} kind={} lifecycle={lifecycle} revision={} created_at_seconds={} created_at_nanos={} creation_commit_sequence={} asset_revision_id=- revision_sequence=- content_kind=- custody=- parent_revision_ids=- next_cursor={cursor}",
        asset.asset_id,
        asset.kind,
        asset.revision,
        asset.created_at_seconds,
        asset.created_at_nanos,
        asset.creation_commit_sequence
    );
    true
}

fn render_asset_inspection(
    request: &mengxia_core_proto::InspectAssetRequest,
    value: mengxia_core_proto::InspectAssetResult,
) -> bool {
    let Some(asset) = value.asset else {
        return false;
    };
    if asset.asset_id != request.asset_id
        || request
            .asset_revision_id
            .as_ref()
            .is_some_and(|expected| expected != &value.asset_revision_id)
        || !valid_id(&value.asset_revision_id)
        || value.revision_sequence == 0
        || !safe_token(&value.content_kind, 64)
        || value.parent_revision_ids.len() > 64
        || value.parent_revision_ids.iter().any(|id| !valid_id(id))
        || value.members.len() > 64
        || (value.members.is_empty() && value.next_cursor.is_some())
        || value
            .next_cursor
            .as_ref()
            .is_some_and(|cursor| cursor.len() != 208 || !opaque_cursor_checksum_is_valid(cursor))
        || !valid_id(&asset.asset_id)
        || !safe_token(&asset.kind, 64)
        || asset.revision == 0
        || enum_name::<mengxia_core_proto::AssetLifecycleValue>(asset.lifecycle).is_none()
        || asset.creation_commit_sequence == 0
        || asset.creation_commit_sequence > i64::MAX as u64
        || Timestamp::from_unix_seconds_nanos(asset.created_at_seconds, asset.created_at_nanos)
            .is_err()
    {
        return false;
    }
    let Some(custody) = enum_name::<mengxia_core_proto::RevisionCustodyValue>(value.custody) else {
        return false;
    };
    let Some(lifecycle) = enum_name::<mengxia_core_proto::AssetLifecycleValue>(asset.lifecycle)
    else {
        return false;
    };
    let parents = if value.parent_revision_ids.is_empty() {
        "-".to_owned()
    } else {
        value.parent_revision_ids.join(",")
    };
    let cursor = optional_hex(value.next_cursor.as_deref());
    println!(
        "MENGXIA_ASSET snapshot_commit_sequence=- asset_id={} kind={} lifecycle={lifecycle} revision={} created_at_seconds={} created_at_nanos={} creation_commit_sequence={} asset_revision_id={} revision_sequence={} content_kind={} custody={custody} parent_revision_ids={parents} next_cursor={cursor}",
        asset.asset_id,
        asset.kind,
        asset.revision,
        asset.created_at_seconds,
        asset.created_at_nanos,
        asset.creation_commit_sequence,
        value.asset_revision_id,
        value.revision_sequence,
        value.content_kind
    );
    for member in value.members {
        if !render_asset_member(&value.asset_revision_id, member) {
            return false;
        }
    }
    true
}

fn render_asset_member(
    asset_revision_id: &str,
    member: mengxia_core_proto::AssetMemberView,
) -> bool {
    if !valid_id(&member.representation_id)
        || !safe_token(&member.representation_purpose, 64)
        || !valid_id(&member.resource_id)
        || !safe_token(&member.resource_kind, 64)
        || member.member_ordinal > 4095
        || member.logical_name.len() > 255
        || member.logical_name.as_bytes().contains(&0)
        || member.blob_sha256.len() != 32
        || member.byte_length > i64::MAX as u64
        || member
            .media_type
            .as_ref()
            .is_some_and(|value| !safe_token(value, 255))
    {
        return false;
    }
    let location = match member.location_id.as_deref() {
        None if member.location_lifecycle == 0
            && member.location_custody == 0
            && member.location_durability == 0 =>
        {
            "-"
        }
        Some(id)
            if valid_id(id)
                && enum_name::<mengxia_core_proto::LocationLifecycleValue>(
                    member.location_lifecycle,
                )
                .is_some()
                && enum_name::<mengxia_core_proto::LocationCustodyValue>(
                    member.location_custody,
                )
                .is_some()
                && enum_name::<mengxia_core_proto::LocationDurabilityValue>(
                    member.location_durability,
                )
                .is_some() =>
        {
            id
        }
        _ => return false,
    };
    let lifecycle =
        optional_enum_name::<mengxia_core_proto::LocationLifecycleValue>(member.location_lifecycle);
    let custody =
        optional_enum_name::<mengxia_core_proto::LocationCustodyValue>(member.location_custody);
    let durability = optional_enum_name::<mengxia_core_proto::LocationDurabilityValue>(
        member.location_durability,
    );
    println!(
        "MENGXIA_ASSET_MEMBER asset_revision_id={asset_revision_id} representation_id={} representation_purpose={} resource_id={} resource_kind={} member_ordinal={} logical_name_hex={} blob_sha256={} byte_length={} media_type={} location_id={location} location_lifecycle={lifecycle} location_custody={custody} location_durability={durability}",
        member.representation_id,
        member.representation_purpose,
        member.resource_id,
        member.resource_kind,
        member.member_ordinal,
        lowercase_hex(member.logical_name.as_bytes()),
        lowercase_hex(&member.blob_sha256),
        member.byte_length,
        member.media_type.as_deref().unwrap_or("-")
    );
    true
}

fn render_materialization(
    request: &mengxia_core_proto::MaterializeAssetRequest,
    value: mengxia_core_proto::MaterializeAssetResult,
) -> bool {
    if value.command_id != request.command_id
        || value.asset_revision_id != request.asset_revision_id
        || value.representation_id != request.representation_id
        || value.resource_id != request.resource_id
        || value.member_ordinal != request.member_ordinal
        || !valid_id(&value.command_id)
        || !valid_id(&value.asset_revision_id)
        || !valid_id(&value.representation_id)
        || !valid_id(&value.resource_id)
        || value.member_ordinal > 4095
        || value.blob_sha256.len() != 32
        || value.byte_length > i64::MAX as u64
    {
        return false;
    }
    println!(
        "MENGXIA_MATERIALIZE_OK command_id={} asset_revision_id={} representation_id={} resource_id={} member_ordinal={} blob_sha256={} byte_length={} replayed={} cleanup_pending={}",
        value.command_id,
        value.asset_revision_id,
        value.representation_id,
        value.resource_id,
        value.member_ordinal,
        lowercase_hex(&value.blob_sha256),
        value.byte_length,
        value.replayed,
        value.cleanup_pending
    );
    true
}

fn enum_name<T>(value: i32) -> Option<String>
where
    T: TryFrom<i32> + std::fmt::Debug,
{
    if value == 0 {
        return None;
    }
    T::try_from(value).ok().map(enum_debug_name)
}

fn enum_debug_name<T: std::fmt::Debug>(value: T) -> String {
    let debug = format!("{value:?}");
    let mut canonical = String::with_capacity(debug.len());
    for (index, character) in debug.chars().enumerate() {
        if index != 0 && character.is_ascii_uppercase() {
            canonical.push('_');
        }
        canonical.push(character.to_ascii_lowercase());
    }
    canonical
}

fn optional_enum_name<T>(value: i32) -> String
where
    T: TryFrom<i32> + std::fmt::Debug,
{
    if value == 0 {
        "unspecified".to_owned()
    } else {
        enum_name::<T>(value).unwrap_or_else(|| "invalid".to_owned())
    }
}

fn valid_id(value: &str) -> bool {
    Id::<ResultIdentity>::from_str(value).is_ok_and(|id| id.to_string() == value)
}

fn ids_are_strictly_sorted(values: &[String]) -> bool {
    values.len() <= 64
        && values.iter().all(|value| valid_id(value))
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn safe_token(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-' | b'/')
        })
}

fn optional_hex(value: Option<&[u8]>) -> String {
    value.map_or_else(|| "-".to_owned(), lowercase_hex)
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
        Some(_) => fail_with_retry(ErrorCode::IpcTransportError, RetryAction::SameCommand, 1),
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
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
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
        ClientEnvironment, ClientLibraryConfig, Command, HELP, HandshakeCli, RetryAction,
        Task008Kind, Task009Kind, enum_name, normalized_absolute_bytes, parse_ascii_u64,
        parse_command, parse_ingest_command, parse_sha256, parse_task_008_command,
        parse_task_009_command, render_issue, render_materialization, resolve_from_layers,
        resolve_task_009, retry_name, valid_operation_retry_pair,
    };

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn exact_client_grammar_accepts_only_help_or_handshake() {
        assert!(HELP.starts_with("mengxia handshake [--client-endpoint PATH]\n"));
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
    fn task_008_command_grammar_is_exact_and_preserves_destination_bytes() {
        for (arguments, kind) in [
            (args(&["library", "status"]), Task008Kind::Status),
            (
                args(&["library", "verify", "--mode", "deep"]),
                Task008Kind::Verify,
            ),
            (
                args(&[
                    "library",
                    "issues",
                    "--verification-id",
                    "018d442f-c000-7a11-8022-334455667701",
                ]),
                Task008Kind::Issues,
            ),
            (args(&["asset", "list"]), Task008Kind::ListAssets),
            (
                args(&[
                    "asset",
                    "inspect",
                    "--asset-id",
                    "018d442f-c000-7a11-8022-334455667702",
                ]),
                Task008Kind::InspectAsset,
            ),
        ] {
            assert_eq!(parse_task_008_command(arguments).unwrap().kind, kind);
        }

        let destination = OsString::from_vec(b"/private/tmp/output-\xff".to_vec());
        let mut materialize = args(&[
            "asset",
            "materialize",
            "--command-id",
            "018d442f-c000-7a11-8022-334455667703",
            "--asset-id",
            "018d442f-c000-7a11-8022-334455667704",
            "--asset-revision-id",
            "018d442f-c000-7a11-8022-334455667705",
            "--representation-id",
            "018d442f-c000-7a11-8022-334455667706",
            "--resource-id",
            "018d442f-c000-7a11-8022-334455667707",
            "--member-ordinal",
            "0",
            "--destination",
            "placeholder",
        ]);
        *materialize.last_mut().unwrap() = destination.clone();
        let parsed = parse_task_008_command(materialize).unwrap();
        assert_eq!(parsed.kind, Task008Kind::Materialize);
        assert_eq!(
            parsed.destination.unwrap().as_os_str().as_bytes(),
            destination.as_os_str().as_bytes()
        );

        for invalid in [
            args(&["library", "verify"]),
            args(&["library", "status", "--operation-timeout-ms", "100"]),
            args(&["asset", "inspect"]),
            args(&["asset", "list", "--mode", "deep"]),
            args(&["library", "issues", "--verification-id", "id", "extra"]),
        ] {
            assert_eq!(
                parse_task_008_command(invalid).err(),
                Some(ErrorCode::ValidationError)
            );
        }
    }

    #[test]
    fn task_008_success_output_rejects_identity_and_typed_object_mismatch() {
        use mengxia_core_proto::{
            IntegrityIssue, IntegrityIssueKind, IntegrityObjectKind, IntegrityRemediation,
            IntegritySeverity, MaterializeAssetRequest, MaterializeAssetResult,
        };

        let request = MaterializeAssetRequest {
            command_id: "018d442f-c000-7a11-8022-334455667703".to_owned(),
            asset_id: "018d442f-c000-7a11-8022-334455667704".to_owned(),
            asset_revision_id: "018d442f-c000-7a11-8022-334455667705".to_owned(),
            representation_id: "018d442f-c000-7a11-8022-334455667706".to_owned(),
            resource_id: "018d442f-c000-7a11-8022-334455667707".to_owned(),
            member_ordinal: 7,
            destination_path: b"/private/tmp/output".to_vec(),
            operation_timeout_ms: 100,
        };
        let result = MaterializeAssetResult {
            command_id: request.command_id.clone(),
            asset_revision_id: request.asset_revision_id.clone(),
            representation_id: request.representation_id.clone(),
            resource_id: request.resource_id.clone(),
            member_ordinal: request.member_ordinal,
            blob_sha256: vec![0x55; 32],
            byte_length: 3,
            replayed: false,
            cleanup_pending: false,
        };
        assert!(render_materialization(&request, result.clone()));
        let mut mismatched = result;
        mismatched.command_id = "018d442f-c000-7a11-8022-334455667708".to_owned();
        assert!(!render_materialization(&request, mismatched));

        let mut issue = IntegrityIssue {
            ordinal: 1,
            kind: IntegrityIssueKind::ManagedBlobMissing as i32,
            severity: IntegritySeverity::DegradedCustody as i32,
            object_kind: IntegrityObjectKind::Blob as i32,
            object_id: Some(vec![0x77; 32]),
            remediation: IntegrityRemediation::FutureAdminAction as i32,
        };
        assert!(render_issue(
            "018d442f-c000-7a11-8022-334455667709",
            issue.clone(),
            1,
            1,
            0,
            None
        ));
        issue.object_id = Some(vec![0x77; 16]);
        assert!(!render_issue(
            "018d442f-c000-7a11-8022-334455667709",
            issue,
            1,
            1,
            0,
            None
        ));
    }

    #[test]
    fn operation_retry_matrix_and_rendered_names_are_closed() {
        assert_eq!(
            enum_name::<mengxia_core_proto::CoreAvailability>(
                mengxia_core_proto::CoreAvailability::ReadOnlyCustody as i32
            )
            .as_deref(),
            Some("read_only_custody")
        );
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
        for invalid in ["", " 1", "+1", "-1", "01", "1_0", "18446744073709551616"] {
            assert_eq!(
                parse_ascii_u64(&OsString::from(invalid)),
                Err(ErrorCode::ValidationError)
            );
        }
    }

    #[test]
    fn task_009_grammar_builds_exact_nested_graph_and_rejects_invalid_shapes() {
        let uuid = "018d442f-c000-7a11-8022-334455667788";
        let digest = "81".repeat(32);
        let member = format!("6672616d652e706e67:{digest}");
        let valid = vec![
            OsString::from("asset"),
            OsString::from("create-revision"),
            OsString::from("--resource"),
            OsString::from("file"),
        ];
        assert_eq!(
            parse_task_009_command(valid).err(),
            Some(ErrorCode::ValidationError)
        );

        let mut command = args(&[
            "asset",
            "create-revision",
            "--command-id",
            uuid,
            "--asset-id",
            uuid,
            "--expected-revision",
            "1",
            "--parent-revision-id",
            uuid,
            "--content-kind",
            "raster",
            "--representation",
            "original",
            "--resource",
            "file",
            "--member",
        ]);
        command.push(OsString::from(member));
        command.extend(args(&[
            "--client-endpoint",
            "/private/tmp/task009-client/client.sock",
            "--max-frame-bytes",
            "1048576",
            "--max-decode-depth",
            "5",
            "--client-handshake-timeout-ms",
            "100",
            "--operation-timeout-ms",
            "100",
        ]));
        let cli = parse_task_009_command(command).unwrap();
        assert_eq!(cli.kind, Task009Kind::CreateAssetRevision);
        let resolved = resolve_task_009(cli).unwrap();
        let Some(mengxia_core_proto::core_request::Operation::CreateAssetRevision(request)) =
            resolved.request.operation
        else {
            panic!("expected create-revision request");
        };
        assert_eq!(request.representations.len(), 1);
        assert_eq!(request.representations[0].resources.len(), 1);
        assert_eq!(request.representations[0].resources[0].members.len(), 1);
        assert_eq!(
            request.representations[0].resources[0].members[0].blob_sha256,
            vec![0x81; 32]
        );

        for invalid in [
            args(&["project", "list", "--project-id", uuid]),
            args(&[
                "take",
                "transition",
                "--command-id",
                uuid,
                "--command-id",
                uuid,
            ]),
        ] {
            assert_eq!(
                parse_task_009_command(invalid).err(),
                Some(ErrorCode::ValidationError)
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
                depth: Some(OsString::from("5")),
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
