//! MengXia daemon composition root.

#![forbid(unsafe_code)]

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use mengxia_core_proto::{DecodeDepth, HandshakeLimits, serve_handshake};
use mengxia_framing::FrameLimit;
use mengxia_platform_fs::{AuthorityError, bind_runtime_endpoint, validate_runtime_endpoint_path};
use mengxia_store_sqlite::{
    ConfigSource, OpenedLibrary, ResolvedStoreConfig, StoreConfig, StoreError,
};
use mengxia_types::ErrorCode;
use tokio::net::UnixListener;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const HELP: &str = "mengxiad serve [--library-root PATH] [--client-endpoint PATH]\n  [--max-frame-bytes ASCII_U64] [--max-decode-depth ASCII_U32]\n  [--client-handshake-timeout-ms ASCII_U64]\n  [--max-pending-handshakes ASCII_U32]\n";

fn main() -> ExitCode {
    match parse_command(env::args_os().skip(1).collect()) {
        Ok(Command::Help) => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(Command::Serve(cli)) => match resolve(cli) {
            Ok(config) => run(config),
            Err(code) => fail(code, 2),
        },
        Err(code) => fail(code, 2),
    }
}

fn run(config: DaemonConfig) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return fail(ErrorCode::InternalError, 1),
    };
    match runtime.block_on(serve(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => fail(code, 1),
    }
}

async fn serve(config: DaemonConfig) -> Result<(), ErrorCode> {
    let opened = OpenedLibrary::open_or_bootstrap(&config.store).map_err(StoreError::code)?;
    let identity = opened.identity();
    let endpoint = match bind_runtime_endpoint(
        &config.endpoint,
        identity.library_id_bytes(),
        identity.owner_uid(),
    ) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            let primary = authority_code(error);
            let _ = opened.shutdown();
            return Err(primary);
        }
    };
    let std_listener = match endpoint.try_clone_listener() {
        Ok(listener) => listener,
        Err(error) => {
            let primary = authority_code(error);
            let _ = endpoint.cleanup();
            let _ = opened.shutdown();
            return Err(primary);
        }
    };
    let listener = match UnixListener::from_std(std_listener) {
        Ok(listener) => listener,
        Err(_) => {
            let _ = endpoint.cleanup();
            let _ = opened.shutdown();
            return Err(ErrorCode::StorageIoError);
        }
    };

    let admission = std::sync::Arc::new(Semaphore::new(config.max_pending));
    let mut tasks = JoinSet::new();
    let mut primary = None;
    let signal = shutdown_signal();
    tokio::pin!(signal);
    loop {
        tokio::select! {
            signal_result = &mut signal => {
                if signal_result.is_err() {
                    primary = Some(ErrorCode::InternalError);
                }
                break;
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((mut stream, _)) => {
                        let Ok(permit) = admission.clone().try_acquire_owned() else {
                            drop(stream);
                            continue;
                        };
                        let limits = config.limits;
                        let owner_uid = identity.owner_uid();
                        tasks.spawn(async move {
                            let _permit = permit;
                            serve_handshake(&mut stream, owner_uid, limits).await
                        });
                    }
                    Err(_) => {
                        primary = Some(ErrorCode::IpcTransportError);
                        break;
                    }
                }
            }
            completed = tasks.join_next(), if !tasks.is_empty() => {
                if completed.is_some_and(|result| result.is_err()) {
                    primary.get_or_insert(ErrorCode::InternalError);
                    break;
                }
            }
        }
    }
    drop(listener);

    tasks.abort_all();
    let join_deadline = tokio::time::Instant::now() + config.limits.timeout();
    while !tasks.is_empty() {
        match tokio::time::timeout_at(join_deadline, tasks.join_next()).await {
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(error))) if error.is_cancelled() => {}
            Ok(Some(Err(_))) => {
                primary.get_or_insert(ErrorCode::InternalError);
            }
            Ok(None) => break,
            Err(_) => {
                tasks.abort_all();
                while let Some(result) = tasks.join_next().await {
                    if result.is_err_and(|error| !error.is_cancelled()) {
                        primary.get_or_insert(ErrorCode::InternalError);
                    }
                }
                primary.get_or_insert(ErrorCode::DeadlineExceeded);
                break;
            }
        }
    }

    if let Err(error) = endpoint.cleanup() {
        primary.get_or_insert(authority_code(error));
    }
    if let Err(error) = opened.shutdown() {
        primary.get_or_insert(error.code());
    }
    primary.map_or(Ok(()), Err)
}

async fn shutdown_signal() -> Result<(), ()> {
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(|_| ())?;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|_| ())?;
    tokio::select! {
        _ = interrupt.recv() => Ok(()),
        _ = terminate.recv() => Ok(()),
    }
}

#[derive(Default)]
struct ServeCli {
    library_root: Option<OsString>,
    endpoint: Option<OsString>,
    frame: Option<OsString>,
    depth: Option<OsString>,
    timeout: Option<OsString>,
    pending: Option<OsString>,
}

enum Command {
    Help,
    Serve(ServeCli),
}

fn parse_command(args: Vec<OsString>) -> Result<Command, ErrorCode> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(Command::Help);
    }
    if args.first().is_none_or(|arg| arg != "serve") {
        return Err(ErrorCode::ValidationError);
    }
    let mut cli = ServeCli::default();
    let mut index = 1;
    while index < args.len() {
        let option = args[index].to_str().ok_or(ErrorCode::ValidationError)?;
        let value = args.get(index + 1).ok_or(ErrorCode::ValidationError)?;
        let slot = match option {
            "--library-root" => &mut cli.library_root,
            "--client-endpoint" => &mut cli.endpoint,
            "--max-frame-bytes" => &mut cli.frame,
            "--max-decode-depth" => &mut cli.depth,
            "--client-handshake-timeout-ms" => &mut cli.timeout,
            "--max-pending-handshakes" => &mut cli.pending,
            _ => return Err(ErrorCode::ValidationError),
        };
        if slot.is_some() {
            return Err(ErrorCode::ValidationError);
        }
        *slot = Some(value.clone());
        index += 2;
    }
    Ok(Command::Serve(cli))
}

struct DaemonConfig {
    store: StoreConfig,
    endpoint: PathBuf,
    limits: HandshakeLimits,
    max_pending: usize,
}

fn resolve(cli: ServeCli) -> Result<DaemonConfig, ErrorCode> {
    resolve_from_layers(
        cli,
        DaemonEnvironment::capture(),
        DaemonLibraryConfig::default(),
    )
}

#[derive(Default)]
struct DaemonLibraryConfig {
    endpoint: Option<PathBuf>,
    frame_bytes: Option<u64>,
    decode_depth: Option<u64>,
    handshake_timeout_ms: Option<u64>,
    max_pending_handshakes: Option<u64>,
    write_queue: Option<u64>,
    read_connections: Option<u64>,
    busy_timeout_ms: Option<u64>,
}

struct DaemonEnvironment {
    library_root: Option<OsString>,
    endpoint: Option<OsString>,
    frame_bytes: Option<OsString>,
    decode_depth: Option<OsString>,
    handshake_timeout_ms: Option<OsString>,
    max_pending_handshakes: Option<OsString>,
    write_queue: Option<OsString>,
    read_connections: Option<OsString>,
    busy_timeout_ms: Option<OsString>,
    platform_temp_root: PathBuf,
}

impl DaemonEnvironment {
    fn capture() -> Self {
        Self {
            library_root: env::var_os("MENGXIA_LIBRARY_ROOT"),
            endpoint: env::var_os("MENGXIA_CLIENT_ENDPOINT"),
            frame_bytes: env::var_os("MENGXIA_MAX_FRAME_BYTES"),
            decode_depth: env::var_os("MENGXIA_MAX_DECODE_DEPTH"),
            handshake_timeout_ms: env::var_os("MENGXIA_CLIENT_HANDSHAKE_TIMEOUT_MS"),
            max_pending_handshakes: env::var_os("MENGXIA_MAX_PENDING_HANDSHAKES"),
            write_queue: env::var_os("MENGXIA_DB_WRITE_QUEUE"),
            read_connections: env::var_os("MENGXIA_DB_READ_CONNECTIONS"),
            busy_timeout_ms: env::var_os("MENGXIA_DB_BUSY_TIMEOUT_MS"),
            platform_temp_root: env::temp_dir(),
        }
    }
}

fn resolve_from_layers(
    cli: ServeCli,
    environment: DaemonEnvironment,
    library: DaemonLibraryConfig,
) -> Result<DaemonConfig, ErrorCode> {
    let (library_raw, library_source) =
        selected_required(cli.library_root, environment.library_root)?;
    let library_root = PathBuf::from(library_raw);
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

    let (frame, _) = select_u64(
        cli.frame,
        environment.frame_bytes,
        library.frame_bytes,
        4 * 1024 * 1024,
    )?;
    let frame = u32::try_from(frame)
        .ok()
        .and_then(|value| FrameLimit::new(value).ok())
        .ok_or(ErrorCode::ValidationError)?;
    let (depth, _) = select_u64(
        cli.depth,
        environment.decode_depth,
        library.decode_depth,
        64,
    )?;
    let depth = u8::try_from(depth)
        .ok()
        .and_then(|value| DecodeDepth::new(value).ok())
        .ok_or(ErrorCode::ValidationError)?;
    let (timeout_ms, _) = select_u64(
        cli.timeout,
        environment.handshake_timeout_ms,
        library.handshake_timeout_ms,
        5_000,
    )?;
    let limits = HandshakeLimits::new(frame, depth, Duration::from_millis(timeout_ms))
        .map_err(|error| error.code())?;
    let (max_pending, _) = select_u64(
        cli.pending,
        environment.max_pending_handshakes,
        library.max_pending_handshakes,
        32,
    )?;
    let max_pending = usize::try_from(max_pending).map_err(|_| ErrorCode::ValidationError)?;
    if !(1..=256).contains(&max_pending) {
        return Err(ErrorCode::ValidationError);
    }

    let (write_queue, write_queue_source) =
        select_u64(None, environment.write_queue, library.write_queue, 256)?;
    let (readers, readers_source) = select_u64(
        None,
        environment.read_connections,
        library.read_connections,
        4,
    )?;
    let (busy, busy_source) = select_u64(
        None,
        environment.busy_timeout_ms,
        library.busy_timeout_ms,
        5_000,
    )?;
    let store = ResolvedStoreConfig::from_selected(
        Some(library_root),
        library_source,
        usize::try_from(write_queue).map_err(|_| ErrorCode::ValidationError)?,
        write_queue_source,
        usize::try_from(readers).map_err(|_| ErrorCode::ValidationError)?,
        readers_source,
        busy,
        busy_source,
    )
    .validate()
    .map_err(|_| ErrorCode::ValidationError)?;
    Ok(DaemonConfig {
        store,
        endpoint,
        limits,
        max_pending,
    })
}

fn selected_required(
    cli: Option<OsString>,
    environment: Option<OsString>,
) -> Result<(OsString, ConfigSource), ErrorCode> {
    if let Some(value) = cli {
        Ok((value, ConfigSource::Cli))
    } else if let Some(value) = environment {
        Ok((value, ConfigSource::Environment))
    } else {
        Err(ErrorCode::ValidationError)
    }
}

fn select_u64(
    cli: Option<OsString>,
    environment: Option<OsString>,
    library: Option<u64>,
    default: u64,
) -> Result<(u64, ConfigSource), ErrorCode> {
    if let Some(value) = cli {
        Ok((parse_ascii_u64(&value)?, ConfigSource::Cli))
    } else if let Some(value) = environment {
        Ok((parse_ascii_u64(&value)?, ConfigSource::Environment))
    } else if let Some(value) = library {
        Ok((value, ConfigSource::Library))
    } else {
        Ok((default, ConfigSource::CompiledDefault))
    }
}

fn parse_ascii_u64(value: &OsStr) -> Result<u64, ErrorCode> {
    let text = value.to_str().ok_or(ErrorCode::ValidationError)?;
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ErrorCode::ValidationError);
    }
    text.parse().map_err(|_| ErrorCode::ValidationError)
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
#[test]
#[ignore = "requires the reviewed formal second-UID macOS runner"]
fn task_003_real_second_uid_peer_is_rejected_before_frame() {
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::os::unix::net::{UnixListener as StdUnixListener, UnixStream as StdUnixStream};
    use std::process::Command as ProcessCommand;
    use std::time::Instant as StdInstant;

    const ROLE: &str = "MENGXIA_TASK003_TEST_ROLE";
    const ENDPOINT: &str = "MENGXIA_TASK003_TEST_ENDPOINT";
    const ACCOUNT: &str = "mengxia-task003-ci";

    if env::var_os(ROLE).as_deref() == Some(OsStr::new("second_uid_client")) {
        let endpoint = PathBuf::from(env::var_os(ENDPOINT).expect("formal endpoint is present"));
        let production_case = endpoint
            .parent()
            .and_then(|path| path.file_name())
            .is_some_and(|name| name == "mengxia-runtime-v1");
        match StdUnixStream::connect(&endpoint) {
            Err(_) if production_case => return,
            Ok(mut stream) if !production_case => {
                stream
                    .set_write_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                stream.write_all(b"MENGXIA-TASK003-CANARY").unwrap();
                return;
            }
            _ => panic!("second-UID reachability did not match the expected branch"),
        }
    }

    let executable = env::current_exe().unwrap();
    let mut preflight = ProcessCommand::new("/usr/bin/sudo")
        .args([
            "-n",
            "-u",
            ACCOUNT,
            "--",
            "/usr/bin/env",
            "-i",
            "/usr/bin/test",
            "-x",
        ])
        .arg(&executable)
        .spawn()
        .unwrap();
    wait_formal_child(&mut preflight, Duration::from_secs(5));

    let owner_home = fs::canonicalize(PathBuf::from(env::var_os("HOME").unwrap())).unwrap();
    let owner_root = owner_home.join(format!(".mengxia-task003-owner-{}", std::process::id()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&owner_root)
        .unwrap();
    let production_endpoint = owner_root.join("mengxia-runtime-v1/client.sock");
    let mut library_id = [0x5a; 16];
    library_id[6] = 0x7a;
    library_id[8] = 0x9a;
    let published = bind_runtime_endpoint(
        &production_endpoint,
        library_id,
        mengxia_platform_fs::effective_user_id(),
    )
    .unwrap();
    run_formal_child(&executable, ACCOUNT, &production_endpoint);
    published.cleanup().unwrap();
    fs::remove_dir_all(&owner_root).unwrap();

    let fixture_root = PathBuf::from(format!(
        "/private/tmp/mengxia-task003-peer-{}",
        std::process::id()
    ));
    fs::DirBuilder::new()
        .mode(0o777)
        .create(&fixture_root)
        .unwrap();
    fs::set_permissions(&fixture_root, fs::Permissions::from_mode(0o777)).unwrap();
    let fixture_endpoint = fixture_root.join("client.sock");
    let listener = StdUnixListener::bind(&fixture_endpoint).unwrap();
    fs::set_permissions(&fixture_endpoint, fs::Permissions::from_mode(0o666)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut child = spawn_formal_child(&executable, ACCOUNT, &fixture_endpoint);
    let deadline = StdInstant::now() + Duration::from_secs(5);
    let accepted = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if let Some(status) = child.try_wait().unwrap() {
                    panic!("formal client exited before accept: {status}");
                }
                if StdInstant::now() >= deadline {
                    child.kill().unwrap();
                    let _ = child.wait();
                    panic!("formal accept exceeded its deadline");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => panic!("formal listener failed"),
        }
    };
    accepted.set_nonblocking(true).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut accepted = tokio::net::UnixStream::from_std(accepted).unwrap();
    let limits = HandshakeLimits::new(
        FrameLimit::default(),
        DecodeDepth::new(3).unwrap(),
        Duration::from_millis(500),
    )
    .unwrap();
    assert_eq!(
        runtime
            .block_on(serve_handshake(
                &mut accepted,
                mengxia_platform_fs::effective_user_id(),
                limits,
            ))
            .map_err(|error| error.code()),
        Err(ErrorCode::AuthenticationError)
    );
    wait_formal_child(&mut child, Duration::from_secs(5));
    drop(listener);
    fs::remove_file(&fixture_endpoint).unwrap();
    fs::remove_dir(&fixture_root).unwrap();
}

#[cfg(test)]
fn spawn_formal_child(
    executable: &std::path::Path,
    account: &str,
    endpoint: &std::path::Path,
) -> std::process::Child {
    let role = "MENGXIA_TASK003_TEST_ROLE=second_uid_client".to_owned();
    let endpoint = format!(
        "MENGXIA_TASK003_TEST_ENDPOINT={}",
        endpoint.to_str().unwrap()
    );
    std::process::Command::new("/usr/bin/sudo")
        .args(["-n", "-u", account, "--", "/usr/bin/env", "-i"])
        .arg(role)
        .arg(endpoint)
        .arg(executable)
        .args([
            "task_003_real_second_uid_peer_is_rejected_before_frame",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .spawn()
        .unwrap()
}

#[cfg(test)]
fn run_formal_child(executable: &std::path::Path, account: &str, endpoint: &std::path::Path) {
    let mut child = spawn_formal_child(executable, account, endpoint);
    wait_formal_child(&mut child, std::time::Duration::from_secs(5));
}

#[cfg(test)]
fn wait_formal_child(child: &mut std::process::Child, timeout: std::time::Duration) {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait().unwrap() {
            Some(status) => {
                assert!(status.success(), "formal second-UID child failed");
                return;
            }
            None if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            None => {
                child.kill().unwrap();
                let _ = child.wait();
                panic!("formal second-UID child exceeded its deadline");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use mengxia_store_sqlite::ConfigSource;
    use mengxia_types::ErrorCode;

    use super::{
        Command, DaemonEnvironment, DaemonLibraryConfig, ServeCli, parse_ascii_u64, parse_command,
        resolve_from_layers,
    };

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn exact_daemon_grammar_accepts_only_help_or_serve() {
        assert!(matches!(
            parse_command(args(&["--help"])),
            Ok(Command::Help)
        ));
        assert!(matches!(
            parse_command(args(&[
                "serve",
                "--library-root",
                "/private/tmp/Library",
                "--max-pending-handshakes",
                "32",
            ])),
            Ok(Command::Serve(_))
        ));
        for invalid in [
            args(&[]),
            args(&["serve", "--help"]),
            args(&["serve", "--library-root=/tmp/x"]),
            args(&["serve", "--library-root"]),
            args(&[
                "serve",
                "--library-root",
                "/tmp/a",
                "--library-root",
                "/tmp/b",
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
        assert_eq!(parse_ascii_u64(&OsString::from("5000")), Ok(5000));
        for invalid in ["", " 1", "+1", "-1", "1_0", "18446744073709551616"] {
            assert_eq!(
                parse_ascii_u64(&OsString::from(invalid)),
                Err(ErrorCode::ValidationError)
            );
        }
    }

    #[test]
    fn typed_layers_obey_cli_environment_library_default_precedence() {
        let endpoint = PathBuf::from("/private/tmp/task003-resolver/client.sock");
        let config = resolve_from_layers(
            ServeCli {
                library_root: Some(OsString::from("/private/tmp/Task003Library")),
                endpoint: Some(endpoint.clone().into_os_string()),
                frame: Some(OsString::from("65536")),
                depth: Some(OsString::from("3")),
                timeout: Some(OsString::from("100")),
                pending: Some(OsString::from("1")),
            },
            DaemonEnvironment {
                library_root: Some(OsString::from("invalid-relative-library")),
                endpoint: Some(OsString::from("invalid-relative-endpoint")),
                frame_bytes: Some(OsString::from("invalid")),
                decode_depth: Some(OsString::from("invalid")),
                handshake_timeout_ms: Some(OsString::from("invalid")),
                max_pending_handshakes: Some(OsString::from("invalid")),
                write_queue: Some(OsString::from("32")),
                read_connections: None,
                busy_timeout_ms: None,
                platform_temp_root: PathBuf::from("/private/tmp"),
            },
            DaemonLibraryConfig {
                endpoint: Some(PathBuf::from("/private/tmp/lower/client.sock")),
                frame_bytes: Some(131_072),
                decode_depth: Some(4),
                handshake_timeout_ms: Some(200),
                max_pending_handshakes: Some(2),
                write_queue: Some(64),
                read_connections: Some(2),
                busy_timeout_ms: None,
            },
        )
        .unwrap();

        assert_eq!(config.endpoint, endpoint);
        assert_eq!(
            config.limits.timeout(),
            std::time::Duration::from_millis(100)
        );
        assert_eq!(config.max_pending, 1);
        assert_eq!(config.store.library_root_source(), ConfigSource::Cli);
        assert_eq!(config.store.write_queue_capacity(), 32);
        assert_eq!(config.store.write_queue_source(), ConfigSource::Environment);
        assert_eq!(config.store.read_connection_count(), 2);
        assert_eq!(config.store.read_connection_source(), ConfigSource::Library);
        assert_eq!(
            config.store.busy_timeout_source(),
            ConfigSource::CompiledDefault
        );

        let invalid_higher_layer = resolve_from_layers(
            ServeCli {
                library_root: Some(OsString::from("/private/tmp/Task003Library")),
                endpoint: Some(OsString::from("/private/tmp/task003-resolver/client.sock")),
                ..ServeCli::default()
            },
            DaemonEnvironment {
                library_root: None,
                endpoint: None,
                frame_bytes: Some(OsString::from("invalid")),
                decode_depth: None,
                handshake_timeout_ms: None,
                max_pending_handshakes: None,
                write_queue: None,
                read_connections: None,
                busy_timeout_ms: None,
                platform_temp_root: PathBuf::from("/private/tmp"),
            },
            DaemonLibraryConfig {
                frame_bytes: Some(65_536),
                ..DaemonLibraryConfig::default()
            },
        );
        assert!(matches!(
            invalid_higher_layer,
            Err(ErrorCode::ValidationError)
        ));
    }
}
