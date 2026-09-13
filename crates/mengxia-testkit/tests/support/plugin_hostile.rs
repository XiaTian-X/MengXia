use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream as StdUnixStream;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mengxia_plugin_host::{
    ExpectedPluginSession, PluginHostAdmission, PluginHostError, PluginHostLimits,
    PluginRequestFailure, RequestOutcome, SessionClose, TerminationRequired,
};
use mengxia_plugin_package::inspect_manifest;
use tokio::net::UnixStream;
use tokio::time::{Instant, sleep, timeout};

static LIVE_CHILD: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub struct HostileResult {
    pub ping_value: u64,
    pub ping: Option<Result<RequestOutcome<u64>, PluginHostError>>,
    pub shutdown: Option<Result<RequestOutcome<()>, PluginHostError>>,
    pub driver: Result<SessionClose, PluginHostError>,
    pub status: ExitStatus,
}

pub async fn exercise(action: &str) -> HostileResult {
    let guard = LiveChildGuard::acquire();
    let limits = PluginHostLimits::new(
        65_536,
        2,
        1,
        1,
        1,
        65_536,
        Duration::from_millis(2_000),
        Duration::from_millis(2_000),
        Duration::from_millis(2_000),
    )
    .unwrap();
    let admission = PluginHostAdmission::new(limits);
    let permit = admission.try_acquire().unwrap();
    let manifest = include_bytes!("../fixtures/task_010/manifest-v1.golden.json");
    let digest = inspect_manifest(manifest).unwrap().digest();
    let expected = ExpectedPluginSession::new(digest, [0x5a; 32]).unwrap();
    let (host_write, child_stdin) = pair();
    let (host_read, child_stdout) = pair();
    let (host_stderr, child_stderr) = pair();
    let mut child = Command::new(env!("CARGO_BIN_EXE_task_011_hostile_plugin"))
        .arg(action)
        .stdin(Stdio::from(OwnedFd::from(child_stdin)))
        .stdout(Stdio::from(OwnedFd::from(child_stdout)))
        .stderr(Stdio::from(OwnedFd::from(child_stderr)))
        .spawn()
        .expect("hostile fixture must spawn");
    let deadline = Instant::now() + Duration::from_secs(10);
    let opened = permit.open(host_write, host_read, host_stderr, expected, deadline);
    let (driver, session) = match opened {
        Ok(opened) => opened,
        Err(_) => {
            let status = reap(&mut child).await;
            status.expect("hostile child must be reaped after host-open failure");
            guard.release_after_reap();
            panic!("validated hostile host session failed to open");
        }
    };
    let mut driver = tokio::spawn(driver);

    let mut ping = None;
    let mut shutdown = None;
    let ping_value = runtime_ping_value();
    if should_ping(action) {
        ping = Some(session.ping(ping_value, deadline).await);
    }
    if should_shutdown(action, ping.as_ref()) {
        shutdown = Some(session.shutdown(deadline).await);
    }
    if action == "duplicate_failure" && matches!(ping, Some(Ok(RequestOutcome::Rejected(_)))) {
        let _ = session.ping(ping_value.wrapping_add(1), deadline).await;
    }
    let joined = match timeout(Duration::from_secs(10), &mut driver).await {
        Ok(joined) => Some(joined),
        Err(_) => {
            session.cancel();
            driver.abort();
            let _ = driver.await;
            None
        }
    };
    drop(session);
    let status = reap(&mut child).await;
    let status = status.expect("hostile child must be boundedly killed and reaped");
    guard.release_after_reap();
    assert_eq!(admission.available_permits(), 1);
    let driver = joined
        .expect("host driver exercise timeout")
        .expect("outer host task must join");
    HostileResult {
        ping_value,
        ping,
        shutdown,
        driver,
        status,
    }
}

fn runtime_ping_value() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock must be after the Unix epoch");
    (elapsed.as_nanos() as u64) ^ u64::from(std::process::id())
}

fn pair() -> (UnixStream, StdUnixStream) {
    let (parent, child) = StdUnixStream::pair().expect("UnixStream pair");
    parent.set_nonblocking(true).expect("parent nonblocking");
    (
        UnixStream::from_std(parent).expect("Tokio parent stream"),
        child,
    )
}

fn should_ping(action: &str) -> bool {
    matches!(
        action,
        "valid_ping"
            | "close_during_response"
            | "hang_during_response"
            | "panic_after_hello"
            | "wrong_sequence"
            | "stderr_at_cap"
            | "stderr_cap_plus_one"
            | "stderr_flood"
            | "ping_failure_unsupported"
            | "ping_failure_invalid"
            | "ping_failure_internal"
            | "shutdown_failure_unsupported"
            | "shutdown_failure_invalid"
            | "shutdown_failure_internal"
            | "failure_unspecified"
            | "failure_unknown"
            | "duplicate_failure"
    )
}

fn should_shutdown(
    action: &str,
    ping: Option<&Result<RequestOutcome<u64>, PluginHostError>>,
) -> bool {
    matches!(
        action,
        "valid_ping"
            | "stderr_at_cap"
            | "ping_failure_unsupported"
            | "ping_failure_invalid"
            | "ping_failure_internal"
            | "shutdown_failure_unsupported"
            | "shutdown_failure_invalid"
            | "shutdown_failure_internal"
            | "ignore_shutdown"
    ) && !matches!(ping, Some(Err(_)))
}

async fn reap(child: &mut Child) -> Result<ExitStatus, &'static str> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let natural_exit_deadline = Instant::now() + Duration::from_millis(100);
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| "natural-exit try_wait failed")?
        {
            return Ok(status);
        }
        if Instant::now() >= natural_exit_deadline {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    if child.kill().is_err()
        && child
            .try_wait()
            .map_err(|_| "post-kill-race try_wait failed")?
            .is_none()
    {
        return Err("hostile child kill failed");
    }
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| "bounded reap try_wait failed")?
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err("hostile child reap timeout");
        }
        sleep(Duration::from_millis(10)).await;
    }
}

struct LiveChildGuard(bool);

impl LiveChildGuard {
    fn acquire() -> Self {
        assert!(
            LIVE_CHILD
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            "only one hostile fixture child may be live"
        );
        Self(true)
    }

    fn release_after_reap(mut self) {
        self.0 = false;
        assert!(LIVE_CHILD.swap(false, Ordering::AcqRel));
    }
}

impl Drop for LiveChildGuard {
    fn drop(&mut self) {
        if self.0 {
            LIVE_CHILD.store(false, Ordering::Release);
        }
    }
}

pub fn assert_rejected(
    outcome: &Result<RequestOutcome<u64>, PluginHostError>,
    expected: PluginRequestFailure,
) {
    assert_eq!(*outcome, Ok(RequestOutcome::Rejected(expected)));
}

pub fn assert_termination(error: PluginHostError, expected: TerminationRequired) {
    assert_eq!(error.termination_required(), Some(expected));
}
