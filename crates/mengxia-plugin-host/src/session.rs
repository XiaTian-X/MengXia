use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use mengxia_plugin_package::PackageDigest;
use mengxia_plugin_proto::{
    HostEnvelope, HostHello, MAX_IN_FLIGHT_REQUESTS, PROTOCOL_MAJOR, PROTOCOL_MINOR, PingRequest,
    PluginEnvelope, PluginFailureCode, ProtocolCodecError, ShutdownRequest, host_envelope,
    plugin_envelope, read_plugin_envelope, write_host_envelope,
};
use mengxia_types::ErrorCode;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use tokio::time::{Instant, sleep_until};

use crate::{
    PluginHostError, PluginHostLimits, PluginRequestFailure, RequestOutcome, STDERR_BUFFER_BYTES,
    SessionClose, TerminationRequired,
};

/// Host-side context bound before any Plugin-supplied bytes are accepted.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ExpectedPluginSession {
    package_digest: PackageDigest,
    challenge: [u8; 32],
}

impl fmt::Debug for ExpectedPluginSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ExpectedPluginSession(REDACTED)")
    }
}

impl ExpectedPluginSession {
    pub fn new(
        package_digest: PackageDigest,
        challenge: [u8; 32],
    ) -> Result<Self, PluginHostError> {
        if challenge.iter().all(|byte| *byte == 0) {
            return Err(PluginHostError::new(ErrorCode::ValidationError));
        }
        Ok(Self {
            package_digest,
            challenge,
        })
    }

    #[must_use]
    pub const fn package_digest(self) -> PackageDigest {
        self.package_digest
    }
}

/// Bounded admission controller. It does not own or open transport streams.
#[derive(Clone, Debug)]
pub struct PluginHostAdmission {
    limits: PluginHostLimits,
    permits: Arc<Semaphore>,
}

impl PluginHostAdmission {
    #[must_use]
    pub fn new(limits: PluginHostLimits) -> Self {
        Self {
            limits,
            permits: Arc::new(Semaphore::new(limits.active_sessions())),
        }
    }

    pub fn try_acquire(&self) -> Result<SessionPermit, PluginHostError> {
        let permit = Arc::clone(&self.permits)
            .try_acquire_owned()
            .map_err(|_| PluginHostError::new(ErrorCode::Backpressure))?;
        Ok(SessionPermit {
            limits: self.limits,
            permit,
        })
    }

    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.permits.available_permits()
    }
}

/// Single-use admission ownership, acquired before streams are moved.
pub struct SessionPermit {
    limits: PluginHostLimits,
    permit: OwnedSemaphorePermit,
}

impl SessionPermit {
    pub fn open<W, R, E>(
        self,
        writer: W,
        reader: R,
        stderr: E,
        expected: ExpectedPluginSession,
        caller_deadline: Instant,
    ) -> Result<(SessionDriver, PluginSession), PluginHostError>
    where
        W: AsyncWrite + Unpin + Send + 'static,
        R: AsyncRead + Unpin + Send + 'static,
        E: AsyncRead + Unpin + Send + 'static,
    {
        let opened_at = Instant::now();
        if caller_deadline <= opened_at {
            return Err(PluginHostError::new(ErrorCode::DeadlineExceeded));
        }
        let (commands, receiver) = mpsc::channel(1);
        let shared = Arc::new(Shared {
            cancelled: AtomicBool::new(false),
            closing: AtomicBool::new(false),
            in_flight: AtomicBool::new(false),
            handles: AtomicUsize::new(1),
            notify: Notify::new(),
        });
        let handle = PluginSession {
            commands,
            shared: Arc::clone(&shared),
            request_timeout: self.limits.request_timeout(),
            shutdown_timeout: self.limits.shutdown_timeout(),
        };
        let limits = self.limits;
        let permit = self.permit;
        let future = async move {
            let _permit = permit;
            run_session(
                writer,
                reader,
                stderr,
                receiver,
                shared,
                expected,
                limits,
                opened_at,
                caller_deadline,
            )
            .await
        };
        Ok((SessionDriver::new(future), handle))
    }
}

struct Shared {
    cancelled: AtomicBool,
    closing: AtomicBool,
    in_flight: AtomicBool,
    handles: AtomicUsize,
    notify: Notify,
}

/// Cloneable bounded request handle. Only one request may be admitted at a time.
pub struct PluginSession {
    commands: mpsc::Sender<Command>,
    shared: Arc<Shared>,
    request_timeout: Duration,
    shutdown_timeout: Duration,
}

impl Clone for PluginSession {
    fn clone(&self) -> Self {
        self.shared.handles.fetch_add(1, Ordering::Relaxed);
        Self {
            commands: self.commands.clone(),
            shared: Arc::clone(&self.shared),
            request_timeout: self.request_timeout,
            shutdown_timeout: self.shutdown_timeout,
        }
    }
}

impl Drop for PluginSession {
    fn drop(&mut self) {
        if self.shared.handles.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.shared.cancelled.store(true, Ordering::Release);
            self.shared.notify.notify_waiters();
        }
    }
}

impl PluginSession {
    pub async fn ping(
        &self,
        nonce: u64,
        caller_deadline: Instant,
    ) -> Result<RequestOutcome<u64>, PluginHostError> {
        let _guard = self.admit(false)?;
        let deadline = operation_deadline(caller_deadline, self.request_timeout)?;
        let (respond, receive) = oneshot::channel();
        self.commands
            .try_send(Command::Ping {
                nonce,
                deadline,
                respond,
            })
            .map_err(map_send_error)?;
        await_response(receive, deadline, &self.shared).await
    }

    pub async fn shutdown(
        &self,
        caller_deadline: Instant,
    ) -> Result<RequestOutcome<()>, PluginHostError> {
        self.shared.closing.store(true, Ordering::Release);
        let deadline = match operation_deadline(caller_deadline, self.shutdown_timeout) {
            Ok(deadline) => deadline,
            Err(error) => {
                self.cancel();
                return Err(error);
            }
        };
        while self.shared.in_flight.load(Ordering::Acquire) {
            tokio::select! {
                () = self.shared.notify.notified() => {}
                () = sleep_until(deadline) => {
                    self.cancel();
                    return Err(PluginHostError::terminal(
                        ErrorCode::DeadlineExceeded,
                        TerminationRequired::Cancellation,
                    ));
                }
            }
        }
        let _guard = self.admit(true)?;
        let (respond, receive) = oneshot::channel();
        self.commands
            .try_send(Command::Shutdown { deadline, respond })
            .map_err(map_send_error)?;
        await_response(receive, deadline, &self.shared).await
    }

    pub fn cancel(&self) {
        self.shared.cancelled.store(true, Ordering::Release);
        self.shared.notify.notify_waiters();
    }

    fn admit(&self, shutdown: bool) -> Result<InFlightGuard, PluginHostError> {
        if self.shared.cancelled.load(Ordering::Acquire)
            || (!shutdown && self.shared.closing.load(Ordering::Acquire))
        {
            return Err(PluginHostError::new(ErrorCode::OperationCancelled));
        }
        self.shared
            .in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| PluginHostError::new(ErrorCode::Backpressure))?;
        Ok(InFlightGuard(Arc::clone(&self.shared)))
    }
}

struct InFlightGuard(Arc<Shared>);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.in_flight.store(false, Ordering::Release);
        self.0.notify.notify_waiters();
    }
}

enum Command {
    Ping {
        nonce: u64,
        deadline: Instant,
        respond: oneshot::Sender<Result<RequestOutcome<u64>, PluginHostError>>,
    },
    Shutdown {
        deadline: Instant,
        respond: oneshot::Sender<Result<RequestOutcome<()>, PluginHostError>>,
    },
}

type DriverResult = Result<SessionClose, PluginHostError>;
type DriverFuture = Pin<Box<dyn Future<Output = DriverResult> + Send>>;

/// Owning session future. Dropping it synchronously drops streams, queues and permit.
pub struct SessionDriver {
    inner: Option<DriverFuture>,
}

impl SessionDriver {
    fn new(future: impl Future<Output = DriverResult> + Send + 'static) -> Self {
        Self {
            inner: Some(Box::pin(future)),
        }
    }
}

impl Future for SessionDriver {
    type Output = DriverResult;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let Some(inner) = this.inner.as_mut() else {
            return Poll::Ready(Err(PluginHostError::terminal(
                ErrorCode::InternalError,
                TerminationRequired::InternalFailure,
            )));
        };
        match catch_unwind(AssertUnwindSafe(|| inner.as_mut().poll(context))) {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(result)) => {
                this.inner = None;
                Poll::Ready(result)
            }
            Err(_) => {
                this.inner = None;
                Poll::Ready(Err(PluginHostError::terminal(
                    ErrorCode::InternalError,
                    TerminationRequired::InternalFailure,
                )))
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_session<W, R, E>(
    mut writer: W,
    mut reader: R,
    mut stderr: E,
    mut commands: mpsc::Receiver<Command>,
    shared: Arc<Shared>,
    expected: ExpectedPluginSession,
    limits: PluginHostLimits,
    opened_at: Instant,
    caller_deadline: Instant,
) -> DriverResult
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
    E: AsyncRead + Unpin,
{
    let handshake_deadline = earlier_deadline(
        caller_deadline,
        opened_at
            .checked_add(limits.handshake_timeout())
            .ok_or_else(|| PluginHostError::new(ErrorCode::ValidationError))?,
    );
    let hello = HostEnvelope {
        sequence: 0,
        body: Some(host_envelope::Body::Hello(HostHello {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            session_challenge: expected.challenge.to_vec(),
            max_frame_bytes: limits.frame_limit().get(),
            max_in_flight_requests: MAX_IN_FLIGHT_REQUESTS,
        })),
    };
    let mut inbound = BoundedFrameQueue::new(limits.inbound_frames());
    let mut outbound = BoundedFrameQueue::new(limits.outbound_frames());
    let mut stderr_state = StderrState::new(limits.stderr_total_bytes());
    outbound.try_push(hello).map_err(|_| internal_failure())?;
    let hello = outbound.pop().ok_or_else(internal_failure)?;
    drive_with_stderr(
        write_host_envelope(&mut writer, &hello, limits.frame_limit()),
        &mut stderr,
        &mut stderr_state,
        &shared,
        handshake_deadline,
    )
    .await?;
    let response = drive_with_stderr(
        read_plugin_envelope(&mut reader, limits.frame_limit(), limits.decode_depth()),
        &mut stderr,
        &mut stderr_state,
        &shared,
        handshake_deadline,
    )
    .await?;
    inbound.try_push(response).map_err(|_| internal_failure())?;
    validate_hello(inbound.pop().ok_or_else(internal_failure)?, expected)?;

    let mut sequence = 1_u64;
    loop {
        let incoming =
            read_plugin_envelope(&mut reader, limits.frame_limit(), limits.decode_depth());
        tokio::pin!(incoming);
        let command = loop {
            tokio::select! {
                biased;
                () = cancellation_signal(&shared) => {
                    return Err(cancelled());
                }
                response = &mut incoming => {
                    inbound
                        .try_push(response.map_err(map_codec_error)?)
                        .map_err(|_| internal_failure())?;
                    let _unsolicited = inbound.pop().ok_or_else(internal_failure)?;
                    return Err(protocol_failure());
                }
                stderr_result = drain_stderr_once(&mut stderr, &mut stderr_state), if stderr_state.open => {
                    stderr_result?;
                }
                command = commands.recv() => break command,
            }
        };
        match command {
            Some(Command::Ping {
                nonce,
                deadline,
                respond,
            }) => {
                let envelope = HostEnvelope {
                    sequence,
                    body: Some(host_envelope::Body::Ping(PingRequest { nonce })),
                };
                let result = exchange(
                    &mut writer,
                    incoming.as_mut(),
                    &mut stderr,
                    &mut stderr_state,
                    &shared,
                    limits,
                    &mut inbound,
                    &mut outbound,
                    deadline,
                    envelope,
                    ExpectedResponse::Ping(nonce),
                )
                .await;
                sequence = sequence.checked_add(1).ok_or_else(|| {
                    PluginHostError::terminal(
                        ErrorCode::InternalError,
                        TerminationRequired::InternalFailure,
                    )
                })?;
                let terminal = result.as_ref().err().copied();
                if respond
                    .send(result.map(|outcome| outcome.map_ping()))
                    .is_err()
                {
                    return Err(PluginHostError::terminal(
                        ErrorCode::OperationCancelled,
                        TerminationRequired::Cancellation,
                    ));
                }
                if let Some(error) = terminal {
                    return Err(error);
                }
            }
            Some(Command::Shutdown { deadline, respond }) => {
                let envelope = HostEnvelope {
                    sequence,
                    body: Some(host_envelope::Body::Shutdown(ShutdownRequest {})),
                };
                let result = exchange(
                    &mut writer,
                    incoming.as_mut(),
                    &mut stderr,
                    &mut stderr_state,
                    &shared,
                    limits,
                    &mut inbound,
                    &mut outbound,
                    deadline,
                    envelope,
                    ExpectedResponse::Shutdown,
                )
                .await;
                let terminal = result.as_ref().err().copied();
                let shutdown_rejected = matches!(result, Ok(ExchangeOutcome::Rejected(_)));
                if respond
                    .send(result.map(|outcome| outcome.map_shutdown()))
                    .is_err()
                {
                    return Err(PluginHostError::terminal(
                        ErrorCode::OperationCancelled,
                        TerminationRequired::Cancellation,
                    ));
                }
                if let Some(error) = terminal {
                    return Err(error);
                }
                if shutdown_rejected {
                    return Err(PluginHostError::terminal(
                        ErrorCode::PluginProtocolViolation,
                        TerminationRequired::ShutdownRejected,
                    ));
                }
                return Ok(SessionClose::CooperativeShutdown);
            }
            None => return Err(cancelled()),
        }
    }
}

#[derive(Clone, Copy)]
enum ExpectedResponse {
    Ping(u64),
    Shutdown,
}

enum ExchangeOutcome {
    Ping(u64),
    Shutdown,
    Rejected(PluginRequestFailure),
}

impl ExchangeOutcome {
    fn map_ping(self) -> RequestOutcome<u64> {
        match self {
            Self::Ping(nonce) => RequestOutcome::Completed(nonce),
            Self::Rejected(failure) => RequestOutcome::Rejected(failure),
            Self::Shutdown => unreachable!("response shape is validated before mapping"),
        }
    }

    fn map_shutdown(self) -> RequestOutcome<()> {
        match self {
            Self::Shutdown => RequestOutcome::Completed(()),
            Self::Rejected(failure) => RequestOutcome::Rejected(failure),
            Self::Ping(_) => unreachable!("response shape is validated before mapping"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn exchange<W, F, E>(
    writer: &mut W,
    incoming: Pin<&mut F>,
    stderr: &mut E,
    stderr_state: &mut StderrState,
    shared: &Arc<Shared>,
    limits: PluginHostLimits,
    inbound: &mut BoundedFrameQueue<PluginEnvelope>,
    outbound: &mut BoundedFrameQueue<HostEnvelope>,
    deadline: Instant,
    request: HostEnvelope,
    expected: ExpectedResponse,
) -> Result<ExchangeOutcome, PluginHostError>
where
    W: AsyncWrite + Unpin,
    F: Future<Output = Result<PluginEnvelope, ProtocolCodecError>>,
    E: AsyncRead + Unpin,
{
    outbound.try_push(request).map_err(|_| internal_failure())?;
    let request = outbound.pop().ok_or_else(internal_failure)?;
    drive_with_stderr(
        write_host_envelope(writer, &request, limits.frame_limit()),
        stderr,
        stderr_state,
        shared,
        deadline,
    )
    .await?;
    let response =
        drive_pinned_with_stderr(incoming, stderr, stderr_state, shared, deadline).await?;
    inbound.try_push(response).map_err(|_| internal_failure())?;
    validate_response(
        inbound.pop().ok_or_else(internal_failure)?,
        request.sequence,
        expected,
    )
}

/// Lazy bounded queue: a full queue returns ownership to its producer, so the
/// producer can stop polling until the consumer restores capacity. No capacity
/// is allocated eagerly and no frame is cloned or dropped on backpressure.
struct BoundedFrameQueue<T> {
    frames: VecDeque<T>,
    capacity: usize,
}

impl<T> BoundedFrameQueue<T> {
    fn new(capacity: usize) -> Self {
        debug_assert!(capacity > 0);
        Self {
            frames: VecDeque::new(),
            capacity,
        }
    }

    fn try_push(&mut self, frame: T) -> Result<(), T> {
        if self.frames.len() >= self.capacity {
            return Err(frame);
        }
        self.frames.push_back(frame);
        Ok(())
    }

    fn pop(&mut self) -> Option<T> {
        self.frames.pop_front()
    }
}

fn validate_hello(
    envelope: PluginEnvelope,
    expected: ExpectedPluginSession,
) -> Result<(), PluginHostError> {
    if envelope.sequence != 0 {
        return Err(protocol_failure());
    }
    let Some(plugin_envelope::Body::Hello(hello)) = envelope.body else {
        return Err(protocol_failure());
    };
    if hello.protocol_major != PROTOCOL_MAJOR || hello.protocol_minor != PROTOCOL_MINOR {
        return Err(PluginHostError::terminal(
            ErrorCode::ProtocolVersionUnsupported,
            TerminationRequired::ProtocolFailure,
        ));
    }
    if hello.session_challenge.as_ref() != expected.challenge {
        return Err(protocol_failure());
    }
    Ok(())
}

fn validate_response(
    envelope: PluginEnvelope,
    sequence: u64,
    expected: ExpectedResponse,
) -> Result<ExchangeOutcome, PluginHostError> {
    if envelope.sequence != sequence {
        return Err(protocol_failure());
    }
    match (expected, envelope.body) {
        (ExpectedResponse::Ping(nonce), Some(plugin_envelope::Body::Ping(response)))
            if response.nonce == nonce =>
        {
            Ok(ExchangeOutcome::Ping(nonce))
        }
        (ExpectedResponse::Shutdown, Some(plugin_envelope::Body::Shutdown(_))) => {
            Ok(ExchangeOutcome::Shutdown)
        }
        (_, Some(plugin_envelope::Body::Failure(failure))) => {
            let failure = match PluginFailureCode::try_from(failure.code) {
                Ok(PluginFailureCode::UnsupportedRequest) => {
                    PluginRequestFailure::UnsupportedRequest
                }
                Ok(PluginFailureCode::InvalidRequest) => PluginRequestFailure::InvalidRequest,
                Ok(PluginFailureCode::Internal) => PluginRequestFailure::Internal,
                _ => return Err(protocol_failure()),
            };
            Ok(ExchangeOutcome::Rejected(failure))
        }
        _ => Err(protocol_failure()),
    }
}

struct StderrState {
    total: usize,
    limit: usize,
    open: bool,
}

impl StderrState {
    const fn new(limit: usize) -> Self {
        Self {
            total: 0,
            limit,
            open: true,
        }
    }
}

async fn drain_stderr_once<E: AsyncRead + Unpin>(
    stderr: &mut E,
    state: &mut StderrState,
) -> Result<(), PluginHostError> {
    let remaining_plus_one = state
        .limit
        .checked_sub(state.total)
        .and_then(|remaining| remaining.checked_add(1))
        .ok_or_else(resource_limit)?;
    let length = STDERR_BUFFER_BYTES.min(remaining_plus_one);
    let mut scratch = [0_u8; STDERR_BUFFER_BYTES];
    let read = stderr.read(&mut scratch[..length]).await.map_err(|_| {
        PluginHostError::terminal(
            ErrorCode::IpcTransportError,
            TerminationRequired::ProtocolFailure,
        )
    })?;
    if read == 0 {
        state.open = false;
        return Ok(());
    }
    state.total = state.total.checked_add(read).ok_or_else(resource_limit)?;
    if state.total > state.limit {
        return Err(resource_limit());
    }
    Ok(())
}

async fn drive_with_stderr<F, E, T>(
    operation: F,
    stderr: &mut E,
    stderr_state: &mut StderrState,
    shared: &Arc<Shared>,
    deadline: Instant,
) -> Result<T, PluginHostError>
where
    F: Future<Output = Result<T, ProtocolCodecError>>,
    E: AsyncRead + Unpin,
{
    tokio::pin!(operation);
    drive_pinned_with_stderr(operation.as_mut(), stderr, stderr_state, shared, deadline).await
}

async fn drive_pinned_with_stderr<F, E, T>(
    mut operation: Pin<&mut F>,
    stderr: &mut E,
    stderr_state: &mut StderrState,
    shared: &Arc<Shared>,
    deadline: Instant,
) -> Result<T, PluginHostError>
where
    F: Future<Output = Result<T, ProtocolCodecError>>,
    E: AsyncRead + Unpin,
{
    loop {
        tokio::select! {
            biased;
            () = cancellation_signal(shared) => {
                return Err(PluginHostError::terminal(
                    ErrorCode::OperationCancelled,
                    TerminationRequired::Cancellation,
                ));
            }
            () = sleep_until(deadline) => {
                return Err(PluginHostError::terminal(
                    ErrorCode::DeadlineExceeded,
                    TerminationRequired::Cancellation,
                ));
            }
            result = operation.as_mut() => return result.map_err(map_codec_error),
            stderr_result = drain_stderr_once(stderr, stderr_state), if stderr_state.open => {
                stderr_result?;
            }
        }
    }
}

async fn cancellation_signal(shared: &Shared) {
    loop {
        if shared.cancelled.load(Ordering::Acquire) {
            return;
        }
        shared.notify.notified().await;
    }
}

async fn await_response<T>(
    receive: oneshot::Receiver<Result<RequestOutcome<T>, PluginHostError>>,
    deadline: Instant,
    shared: &Shared,
) -> Result<RequestOutcome<T>, PluginHostError> {
    tokio::select! {
        biased;
        () = cancellation_signal(shared) => Err(PluginHostError::terminal(
            ErrorCode::OperationCancelled,
            TerminationRequired::Cancellation,
        )),
        () = sleep_until(deadline) => {
            shared.cancelled.store(true, Ordering::Release);
            shared.notify.notify_waiters();
            Err(PluginHostError::terminal(
                ErrorCode::DeadlineExceeded,
                TerminationRequired::Cancellation,
            ))
        }
        result = receive => result.unwrap_or_else(|_| Err(PluginHostError::terminal(
            ErrorCode::OperationCancelled,
            TerminationRequired::Cancellation,
        ))),
    }
}

fn operation_deadline(
    caller_deadline: Instant,
    budget: Duration,
) -> Result<Instant, PluginHostError> {
    let now = Instant::now();
    if caller_deadline <= now {
        return Err(PluginHostError::new(ErrorCode::DeadlineExceeded));
    }
    let budget_deadline = now
        .checked_add(budget)
        .ok_or_else(|| PluginHostError::new(ErrorCode::ValidationError))?;
    Ok(earlier_deadline(caller_deadline, budget_deadline))
}

fn earlier_deadline(left: Instant, right: Instant) -> Instant {
    if left <= right { left } else { right }
}

fn map_send_error<T>(error: mpsc::error::TrySendError<T>) -> PluginHostError {
    match error {
        mpsc::error::TrySendError::Full(_) => PluginHostError::new(ErrorCode::Backpressure),
        mpsc::error::TrySendError::Closed(_) => PluginHostError::new(ErrorCode::OperationCancelled),
    }
}

fn map_codec_error(error: ProtocolCodecError) -> PluginHostError {
    match error {
        ProtocolCodecError::Transport => PluginHostError::terminal(
            ErrorCode::IpcTransportError,
            TerminationRequired::ProtocolFailure,
        ),
        ProtocolCodecError::AllocationUnavailable => PluginHostError::terminal(
            ErrorCode::Backpressure,
            TerminationRequired::ProtocolFailure,
        ),
        ProtocolCodecError::InvalidLimit => PluginHostError::terminal(
            ErrorCode::InternalError,
            TerminationRequired::InternalFailure,
        ),
        _ => protocol_failure(),
    }
}

fn protocol_failure() -> PluginHostError {
    PluginHostError::terminal(
        ErrorCode::PluginProtocolViolation,
        TerminationRequired::ProtocolFailure,
    )
}

fn cancelled() -> PluginHostError {
    PluginHostError::terminal(
        ErrorCode::OperationCancelled,
        TerminationRequired::Cancellation,
    )
}

fn resource_limit() -> PluginHostError {
    PluginHostError::terminal(
        ErrorCode::PluginProtocolViolation,
        TerminationRequired::ResourceLimit,
    )
}

fn internal_failure() -> PluginHostError {
    PluginHostError::terminal(
        ErrorCode::InternalError,
        TerminationRequired::InternalFailure,
    )
}

#[cfg(test)]
mod tests {
    use super::BoundedFrameQueue;

    #[test]
    fn inbound_and_outbound_frame_queues_are_lazy_bounded_and_restore_capacity() {
        for capacity in [1, 16] {
            let mut inbound = BoundedFrameQueue::new(capacity);
            let mut outbound = BoundedFrameQueue::new(capacity);

            for value in 0..capacity {
                assert_eq!(inbound.try_push(value), Ok(()));
                assert_eq!(outbound.try_push(value), Ok(()));
            }
            assert_eq!(inbound.try_push(capacity), Err(capacity));
            assert_eq!(outbound.try_push(capacity), Err(capacity));

            assert_eq!(inbound.pop(), Some(0));
            assert_eq!(outbound.pop(), Some(0));
            assert_eq!(inbound.try_push(capacity), Ok(()));
            assert_eq!(outbound.try_push(capacity), Ok(()));

            let expected: Vec<_> = (1..=capacity).collect();
            let actual_inbound: Vec<_> = std::iter::from_fn(|| inbound.pop()).collect();
            let actual_outbound: Vec<_> = std::iter::from_fn(|| outbound.pop()).collect();
            assert_eq!(actual_inbound, expected);
            assert_eq!(actual_outbound, expected);
        }
    }
}
