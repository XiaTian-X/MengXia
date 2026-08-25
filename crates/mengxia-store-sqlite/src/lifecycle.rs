use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

use mengxia_platform_fs::{OpenedLibraryAuthority, SqliteChild};
use rusqlite::Connection;
use tokio::sync::oneshot;

use super::bootstrap::finalize_opened_canonical;
use super::migration::{OpenedLibraryMetadata, verify_bootstrap_schema_matches};
use super::runtime::verify_and_harden;
use super::stock_sqlite_open::{self, ConnectionAccess};
use super::{StoreConfig, StoreError};

type CommandResult = Result<(), StoreError>;
pub(crate) type CommandReceipt = oneshot::Receiver<CommandResult>;

trait WriterJob: Send {
    fn execute(self: Box<Self>, connection: &mut Connection) -> CommandResult;
}

trait ReadJob: Send {
    fn execute(self: Box<Self>, connection: &Connection) -> CommandResult;
}

struct WriterEnvelope {
    job: Box<dyn WriterJob>,
    result: oneshot::Sender<CommandResult>,
}

struct ReadEnvelope {
    job: Box<dyn ReadJob>,
    result: oneshot::Sender<CommandResult>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmissionGate {
    Open,
    ShuttingDown,
    Failed,
}

struct ReadSlot {
    assigned: Option<ReadEnvelope>,
    running: bool,
}

struct LifecycleState {
    gate: AdmissionGate,
    writer_capacity: usize,
    writer_queue: VecDeque<WriterEnvelope>,
    writer_running: bool,
    read_slots: Vec<ReadSlot>,
}

struct SharedLifecycle {
    state: Mutex<LifecycleState>,
    wake: Condvar,
}

impl SharedLifecycle {
    fn lock(&self) -> Result<MutexGuard<'_, LifecycleState>, StoreError> {
        self.state.lock().map_err(|_| StoreError::Internal)
    }

    fn fail_locked(state: &mut LifecycleState) {
        state.gate = AdmissionGate::Failed;
        for envelope in state.writer_queue.drain(..) {
            let _ = envelope.result.send(Err(StoreError::Internal));
        }
        for slot in &mut state.read_slots {
            if let Some(envelope) = slot.assigned.take() {
                let _ = envelope.result.send(Err(StoreError::Internal));
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct StoreHandle {
    shared: Arc<SharedLifecycle>,
    metadata: OpenedLibraryMetadata,
}

impl StoreHandle {
    pub(crate) fn verify_on_writer(&self) -> Result<CommandReceipt, StoreError> {
        self.submit_writer(VerifyWriter {
            metadata: self.metadata,
        })
    }

    pub(crate) fn verify_on_reader(&self) -> Result<CommandReceipt, StoreError> {
        self.submit_read(VerifyRead {
            metadata: self.metadata,
        })
    }

    fn submit_writer<Job>(&self, job: Job) -> Result<CommandReceipt, StoreError>
    where
        Job: WriterJob + 'static,
    {
        let (result, receipt) = oneshot::channel();
        let mut state = self.shared.lock()?;
        match state.gate {
            AdmissionGate::Open => {}
            AdmissionGate::ShuttingDown => return Err(StoreError::ShuttingDown),
            AdmissionGate::Failed => return Err(StoreError::Internal),
        }
        if state.writer_queue.len() == state.writer_capacity {
            return Err(StoreError::Backpressure);
        }
        state.writer_queue.push_back(WriterEnvelope {
            job: Box::new(job),
            result,
        });
        self.shared.wake.notify_all();
        Ok(receipt)
    }

    fn submit_read<Job>(&self, job: Job) -> Result<CommandReceipt, StoreError>
    where
        Job: ReadJob + 'static,
    {
        let (result, receipt) = oneshot::channel();
        let mut state = self.shared.lock()?;
        match state.gate {
            AdmissionGate::Open => {}
            AdmissionGate::ShuttingDown => return Err(StoreError::ShuttingDown),
            AdmissionGate::Failed => return Err(StoreError::Internal),
        }
        let Some(slot) = state
            .read_slots
            .iter_mut()
            .find(|slot| slot.assigned.is_none() && !slot.running)
        else {
            return Err(StoreError::Backpressure);
        };
        slot.assigned = Some(ReadEnvelope {
            job: Box::new(job),
            result,
        });
        self.shared.wake.notify_all();
        Ok(receipt)
    }

    #[cfg(test)]
    fn snapshot(&self) -> (AdmissionGate, usize, usize) {
        let state = self.shared.state.lock().expect("lifecycle test state");
        (
            state.gate,
            state.writer_queue.len(),
            state
                .read_slots
                .iter()
                .filter(|slot| slot.assigned.is_some() || slot.running)
                .count(),
        )
    }
}

pub(crate) struct OpenedLibraryOwner {
    handle: StoreHandle,
    config: StoreConfig,
    metadata: OpenedLibraryMetadata,
    authority: Option<OpenedLibraryAuthority>,
    workers: Vec<JoinHandle<()>>,
}

impl OpenedLibraryOwner {
    pub(crate) fn start(
        config: &StoreConfig,
        authority: OpenedLibraryAuthority,
        metadata: OpenedLibraryMetadata,
    ) -> Result<Self, StoreError> {
        let mut writer =
            open_verified_connection(config, &authority, metadata, ConnectionAccess::ReadWrite)?;
        let mut readers = Vec::with_capacity(config.read_connection_count());
        for _ in 0..config.read_connection_count() {
            readers.push(open_verified_connection(
                config,
                &authority,
                metadata,
                ConnectionAccess::ReadOnly,
            )?);
        }

        let shared = Arc::new(SharedLifecycle {
            state: Mutex::new(LifecycleState {
                gate: AdmissionGate::Open,
                writer_capacity: config.write_queue_capacity(),
                writer_queue: VecDeque::with_capacity(config.write_queue_capacity()),
                writer_running: false,
                read_slots: (0..config.read_connection_count())
                    .map(|_| ReadSlot {
                        assigned: None,
                        running: false,
                    })
                    .collect(),
            }),
            wake: Condvar::new(),
        });

        let mut workers = Vec::with_capacity(config.read_connection_count() + 1);
        let writer_shared = Arc::clone(&shared);
        match thread::Builder::new()
            .name("mengxia-db-writer".to_owned())
            .spawn(move || writer_worker(writer_shared, &mut writer))
        {
            Ok(worker) => workers.push(worker),
            Err(_) => return Err(StoreError::Io),
        }

        for (index, reader) in readers.into_iter().enumerate() {
            let read_shared = Arc::clone(&shared);
            match thread::Builder::new()
                .name(format!("mengxia-db-reader-{index}"))
                .spawn(move || read_worker(read_shared, index, reader))
            {
                Ok(worker) => workers.push(worker),
                Err(_) => {
                    close_admission_for_start_failure(&shared);
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(StoreError::Io);
                }
            }
        }

        Ok(Self {
            handle: StoreHandle { shared, metadata },
            config: config.clone(),
            metadata,
            authority: Some(authority),
            workers,
        })
    }

    #[must_use]
    pub(crate) fn handle(&self) -> StoreHandle {
        self.handle.clone()
    }

    #[must_use]
    pub(crate) const fn metadata(&self) -> OpenedLibraryMetadata {
        self.metadata
    }

    pub(crate) fn shutdown(mut self) -> Result<(), StoreError> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&mut self) -> Result<(), StoreError> {
        if self.authority.is_none() {
            return Ok(());
        }

        {
            let mut state = self.handle.shared.lock()?;
            if state.gate == AdmissionGate::Open {
                state.gate = AdmissionGate::ShuttingDown;
                for envelope in state.writer_queue.drain(..) {
                    let _ = envelope.result.send(Err(StoreError::ShuttingDown));
                }
            }
            self.handle.shared.wake.notify_all();
        }

        let mut join_failed = false;
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                join_failed = true;
            }
        }
        let lifecycle_failed = self
            .handle
            .shared
            .state
            .lock()
            .map_or(true, |state| state.gate == AdmissionGate::Failed);
        let finalization = self.authority.as_ref().map_or(Ok(()), |authority| {
            finalize_opened_canonical(&self.config, authority, self.metadata)
        });
        self.authority.take();
        if lifecycle_failed || join_failed {
            Err(StoreError::Internal)
        } else {
            finalization
        }
    }
}

struct VerifyWriter {
    metadata: OpenedLibraryMetadata,
}

impl WriterJob for VerifyWriter {
    fn execute(self: Box<Self>, connection: &mut Connection) -> CommandResult {
        verify_bootstrap_schema_matches(
            connection,
            self.metadata.library_id,
            self.metadata.owner_uid,
            self.metadata.created_at,
        )
        .map(|_| ())
    }
}

struct VerifyRead {
    metadata: OpenedLibraryMetadata,
}

impl ReadJob for VerifyRead {
    fn execute(self: Box<Self>, connection: &Connection) -> CommandResult {
        verify_bootstrap_schema_matches(
            connection,
            self.metadata.library_id,
            self.metadata.owner_uid,
            self.metadata.created_at,
        )
        .map(|_| ())
    }
}

impl Drop for OpenedLibraryOwner {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

fn open_verified_connection(
    config: &StoreConfig,
    authority: &OpenedLibraryAuthority,
    metadata: OpenedLibraryMetadata,
    access: ConnectionAccess,
) -> Result<Connection, StoreError> {
    let connection =
        stock_sqlite_open::open(authority.path_authority(), SqliteChild::Canonical, access)?;
    verify_and_harden(&connection, config.busy_timeout())?;
    verify_bootstrap_schema_matches(
        &connection,
        metadata.library_id,
        metadata.owner_uid,
        metadata.created_at,
    )?;
    Ok(connection)
}

fn close_admission_for_start_failure(shared: &SharedLifecycle) {
    if let Ok(mut state) = shared.state.lock() {
        state.gate = AdmissionGate::ShuttingDown;
        for envelope in state.writer_queue.drain(..) {
            let _ = envelope.result.send(Err(StoreError::ShuttingDown));
        }
        shared.wake.notify_all();
    }
}

fn writer_worker(shared: Arc<SharedLifecycle>, connection: &mut Connection) {
    loop {
        let envelope = {
            let mut state = match shared.state.lock() {
                Ok(state) => state,
                Err(_) => return,
            };
            while state.writer_queue.is_empty() && state.gate == AdmissionGate::Open {
                state = match shared.wake.wait(state) {
                    Ok(state) => state,
                    Err(_) => return,
                };
            }
            let Some(envelope) = state.writer_queue.pop_front() else {
                return;
            };
            state.writer_running = true;
            envelope
        };

        let execution = catch_unwind(AssertUnwindSafe(|| envelope.job.execute(connection)));
        let (result, fatal) = match execution {
            Ok(result) => {
                let fatal = result == Err(StoreError::Internal);
                (result, fatal)
            }
            Err(_) => (Err(StoreError::Internal), true),
        };
        let _ = envelope.result.send(result);

        let mut state = match shared.state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        state.writer_running = false;
        if fatal {
            SharedLifecycle::fail_locked(&mut state);
            shared.wake.notify_all();
            return;
        }
    }
}

fn read_worker(shared: Arc<SharedLifecycle>, index: usize, connection: Connection) {
    loop {
        let envelope = {
            let mut state = match shared.state.lock() {
                Ok(state) => state,
                Err(_) => return,
            };
            while state.read_slots[index].assigned.is_none() && state.gate == AdmissionGate::Open {
                state = match shared.wake.wait(state) {
                    Ok(state) => state,
                    Err(_) => return,
                };
            }
            let Some(envelope) = state.read_slots[index].assigned.take() else {
                return;
            };
            state.read_slots[index].running = true;
            envelope
        };

        let execution = catch_unwind(AssertUnwindSafe(|| envelope.job.execute(&connection)));
        let (result, fatal) = match execution {
            Ok(result) => {
                let fatal = result == Err(StoreError::Internal);
                (result, fatal)
            }
            Err(_) => (Err(StoreError::Internal), true),
        };
        let _ = envelope.result.send(result);

        let mut state = match shared.state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        state.read_slots[index].running = false;
        if fatal {
            SharedLifecycle::fail_locked(&mut state);
            shared.wake.notify_all();
            return;
        }
        shared.wake.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc::{self, SyncSender};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;

    use mengxia_platform_fs::{BootstrapFilesystemState, OpenedLibraryAuthority};
    use mengxia_types::{Id, Timestamp};

    use super::{AdmissionGate, OpenedLibraryOwner, ReadJob, StoreHandle, WriterJob};
    use crate::bootstrap::{bootstrap_staging_database, publish_bootstrapped_staging};
    use crate::intent::BootstrapIntent;
    use crate::migration::LibraryIdentity;
    use crate::path_authority::acquire_bootstrap_authority;
    use crate::{ConfigSource, ResolvedStoreConfig, StoreConfig, StoreError};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        parent: PathBuf,
        library: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|path| path.parent())
                .expect("crate is inside workspace")
                .to_path_buf();
            let parent = repository.join(format!(
                "target/task-004-lifecycle-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&parent)
                .expect("create lifecycle fixture parent");
            fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
                .expect("secure lifecycle fixture parent");
            let library = parent.join("Library");
            Self { parent, library }
        }

        fn config(&self, writer_capacity: usize, read_connections: usize) -> StoreConfig {
            ResolvedStoreConfig::from_selected(
                Some(self.library.clone()),
                ConfigSource::Cli,
                writer_capacity,
                ConfigSource::CompiledDefault,
                read_connections,
                ConfigSource::CompiledDefault,
                37,
                ConfigSource::CompiledDefault,
            )
            .validate()
            .expect("valid lifecycle config")
        }

        fn opened(&self, writer_capacity: usize, read_connections: usize) -> OpenedLibraryOwner {
            let config = self.config(writer_capacity, read_connections);
            let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
            let intent =
                BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                    .expect("durable lifecycle intent");
            bootstrap_staging_database(&config, &authority, intent)
                .expect("bootstrap lifecycle staging");
            let metadata = publish_bootstrapped_staging(&config, &authority, intent)
                .expect("publish lifecycle canonical");
            OpenedLibraryOwner::start(&config, authority, metadata)
                .expect("start opened Library lifecycle")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.parent);
        }
    }

    struct ReleaseGate {
        released: std::sync::Mutex<bool>,
        wake: std::sync::Condvar,
    }

    impl ReleaseGate {
        fn new() -> Self {
            Self {
                released: std::sync::Mutex::new(false),
                wake: std::sync::Condvar::new(),
            }
        }

        fn wait(&self) {
            let mut released = self.released.lock().expect("release gate");
            while !*released {
                released = self.wake.wait(released).expect("release wait");
            }
        }

        fn release(&self) {
            *self.released.lock().expect("release gate") = true;
            self.wake.notify_all();
        }
    }

    struct BlockingWriter {
        entered: SyncSender<()>,
        release: Arc<ReleaseGate>,
    }

    impl WriterJob for BlockingWriter {
        fn execute(
            self: Box<Self>,
            connection: &mut rusqlite::Connection,
        ) -> Result<(), StoreError> {
            let transaction = connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(crate::error::map_sqlite_error)?;
            let count: i64 = transaction
                .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                    row.get(0)
                })
                .map_err(crate::error::map_sqlite_error)?;
            if count != 1 {
                return Err(StoreError::Corruption);
            }
            transaction
                .execute("UPDATE library_meta SET owner_uid = owner_uid", [])
                .map_err(crate::error::map_sqlite_error)?;
            self.entered.send(()).expect("signal writer entry");
            self.release.wait();
            transaction.commit().map_err(crate::error::map_sqlite_error)
        }
    }

    struct OrderedWriter {
        sequence: usize,
        observed: Arc<std::sync::Mutex<Vec<usize>>>,
    }

    impl WriterJob for OrderedWriter {
        fn execute(
            self: Box<Self>,
            connection: &mut rusqlite::Connection,
        ) -> Result<(), StoreError> {
            connection
                .query_row("SELECT singleton FROM library_meta", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(crate::error::map_sqlite_error)?;
            self.observed
                .lock()
                .expect("ordered writer record")
                .push(self.sequence);
            Ok(())
        }
    }

    struct BlockingRead {
        entered: SyncSender<()>,
        release: Arc<ReleaseGate>,
    }

    impl ReadJob for BlockingRead {
        fn execute(self: Box<Self>, connection: &rusqlite::Connection) -> Result<(), StoreError> {
            connection
                .query_row("SELECT singleton FROM library_meta", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(crate::error::map_sqlite_error)?;
            self.entered.send(()).expect("signal read entry");
            self.release.wait();
            Ok(())
        }
    }

    struct PanicWriter;

    impl WriterJob for PanicWriter {
        fn execute(
            self: Box<Self>,
            _connection: &mut rusqlite::Connection,
        ) -> Result<(), StoreError> {
            panic!("injected writer invariant failure")
        }
    }

    #[test]
    fn typed_bootstrap_commands_complete_and_shutdown_releases_a_closed_canonical_lock() {
        let fixture = Fixture::new();
        let owner = fixture.opened(16, 2);
        let handle = owner.handle();
        let writer = handle
            .verify_on_writer()
            .expect("admit typed writer verification");
        let reader = handle
            .verify_on_reader()
            .expect("admit typed reader verification");
        assert_eq!(writer.blocking_recv(), Ok(Ok(())));
        assert_eq!(reader.blocking_recv(), Ok(Ok(())));
        owner.shutdown().expect("close and join opened Library");

        let (reopened, state) = OpenedLibraryAuthority::acquire_bootstrap_state(&fixture.library)
            .expect("reacquire lock after joined shutdown");
        assert_eq!(state, BootstrapFilesystemState::CanonicalOnly);
        drop(reopened);
    }

    #[test]
    fn writer_queue_is_exactly_bounded_fifo_and_cancellation_does_not_revoke() {
        const CAPACITY: usize = 16;
        let fixture = Fixture::new();
        let owner = fixture.opened(CAPACITY, 1);
        let handle = owner.handle();
        let release = Arc::new(ReleaseGate::new());
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let current = handle
            .submit_writer(BlockingWriter {
                entered: entered_tx,
                release: Arc::clone(&release),
            })
            .expect("admit current writer");
        entered_rx.recv().expect("writer reaches in-flight state");

        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut receipts = Vec::new();
        for sequence in 0..CAPACITY - 1 {
            receipts.push(
                handle
                    .submit_writer(OrderedWriter {
                        sequence,
                        observed: Arc::clone(&observed),
                    })
                    .expect("admit cap-minus-one writer"),
            );
        }
        assert_eq!(handle.snapshot(), (AdmissionGate::Open, CAPACITY - 1, 0));
        receipts.push(
            handle
                .submit_writer(OrderedWriter {
                    sequence: CAPACITY - 1,
                    observed: Arc::clone(&observed),
                })
                .expect("admit exact-capacity writer"),
        );
        assert_eq!(handle.snapshot(), (AdmissionGate::Open, CAPACITY, 0));
        assert!(matches!(
            handle.submit_writer(OrderedWriter {
                sequence: CAPACITY,
                observed: Arc::clone(&observed),
            }),
            Err(StoreError::Backpressure)
        ));

        drop(receipts.remove(7));
        release.release();
        assert_eq!(current.blocking_recv(), Ok(Ok(())));
        for receipt in receipts {
            assert_eq!(receipt.blocking_recv(), Ok(Ok(())));
        }
        for _ in 0..500 {
            if observed.lock().expect("ordered result").len() == CAPACITY {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            *observed.lock().expect("ordered result"),
            (0..CAPACITY).collect::<Vec<_>>()
        );
        owner.shutdown().expect("joined writer shutdown");
        assert!(matches!(
            handle.submit_writer(OrderedWriter {
                sequence: CAPACITY,
                observed,
            }),
            Err(StoreError::ShuttingDown)
        ));
    }

    #[test]
    fn shutdown_revokes_only_queued_writers_and_joins_the_current_command() {
        let fixture = Fixture::new();
        let owner = fixture.opened(16, 1);
        let handle = owner.handle();
        let release = Arc::new(ReleaseGate::new());
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let current = handle
            .submit_writer(BlockingWriter {
                entered: entered_tx,
                release: Arc::clone(&release),
            })
            .expect("admit current writer");
        entered_rx.recv().expect("writer reaches in-flight state");
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let queued_one = handle
            .submit_writer(OrderedWriter {
                sequence: 1,
                observed: Arc::clone(&observed),
            })
            .expect("admit first queued writer");
        let queued_two = handle
            .submit_writer(OrderedWriter {
                sequence: 2,
                observed: Arc::clone(&observed),
            })
            .expect("admit second queued writer");

        let (shutdown_tx, shutdown_rx) = mpsc::sync_channel(1);
        let shutdown = thread::spawn(move || {
            shutdown_tx
                .send(owner.shutdown())
                .expect("report shutdown result");
        });
        wait_for_gate(&handle, AdmissionGate::ShuttingDown);
        assert_eq!(
            queued_one.blocking_recv(),
            Ok(Err(StoreError::ShuttingDown))
        );
        assert_eq!(
            queued_two.blocking_recv(),
            Ok(Err(StoreError::ShuttingDown))
        );
        assert!(shutdown_rx.recv_timeout(Duration::from_millis(50)).is_err());
        assert!(observed.lock().expect("revoked writer record").is_empty());

        release.release();
        assert_eq!(current.blocking_recv(), Ok(Ok(())));
        assert_eq!(
            shutdown_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("joined shutdown result"),
            Ok(())
        );
        shutdown.join().expect("join shutdown observer");
        assert!(matches!(
            handle.submit_writer(OrderedWriter {
                sequence: 3,
                observed,
            }),
            Err(StoreError::ShuttingDown)
        ));
    }

    #[test]
    fn concurrent_submission_and_shutdown_have_one_serialized_admission_outcome() {
        const SUBMITTERS: usize = 32;
        let fixture = Fixture::new();
        let owner = fixture.opened(16, 1);
        let handle = owner.handle();
        let release = Arc::new(ReleaseGate::new());
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let current = handle
            .submit_writer(BlockingWriter {
                entered: entered_tx,
                release: Arc::clone(&release),
            })
            .expect("admit race blocker");
        entered_rx.recv().expect("race blocker is in flight");

        let barrier = Arc::new(Barrier::new(SUBMITTERS + 1));
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let submitters: Vec<_> = (0..SUBMITTERS)
            .map(|sequence| {
                let handle = handle.clone();
                let barrier = Arc::clone(&barrier);
                let observed = Arc::clone(&observed);
                thread::spawn(move || {
                    barrier.wait();
                    match handle.submit_writer(OrderedWriter { sequence, observed }) {
                        Ok(receipt) => Ok(receipt
                            .blocking_recv()
                            .expect("admitted race command gets a disposition")),
                        Err(error) => Err(error),
                    }
                })
            })
            .collect();
        let shutdown_barrier = Arc::clone(&barrier);
        let shutdown = thread::spawn(move || {
            shutdown_barrier.wait();
            owner.shutdown()
        });

        wait_for_gate(&handle, AdmissionGate::ShuttingDown);
        let mut admitted = 0;
        for submitter in submitters {
            match submitter.join().expect("join race submitter") {
                Ok(Err(StoreError::ShuttingDown)) => admitted += 1,
                Err(StoreError::Backpressure | StoreError::ShuttingDown) => {}
                outcome => panic!("unexpected admission/shutdown race outcome: {outcome:?}"),
            }
        }
        assert!(admitted <= 16);
        assert!(observed.lock().expect("race execution record").is_empty());
        release.release();
        assert_eq!(current.blocking_recv(), Ok(Ok(())));
        assert_eq!(shutdown.join().expect("join race shutdown"), Ok(()));
    }

    #[test]
    fn read_admission_has_no_waiter_queue_and_shutdown_joins_running_read() {
        let fixture = Fixture::new();
        let owner = fixture.opened(16, 2);
        let handle = owner.handle();
        let release = Arc::new(ReleaseGate::new());
        let (entered_tx, entered_rx) = mpsc::sync_channel(2);
        let first = handle
            .submit_read(BlockingRead {
                entered: entered_tx.clone(),
                release: Arc::clone(&release),
            })
            .expect("admit first read lease");
        let second = handle
            .submit_read(BlockingRead {
                entered: entered_tx,
                release: Arc::clone(&release),
            })
            .expect("admit second read lease");
        entered_rx.recv().expect("first read reaches running state");
        entered_rx
            .recv()
            .expect("second read reaches running state");
        assert_eq!(handle.snapshot(), (AdmissionGate::Open, 0, 2));
        let (extra_tx, _extra_rx) = mpsc::sync_channel(1);
        assert!(matches!(
            handle.submit_read(BlockingRead {
                entered: extra_tx,
                release: Arc::clone(&release),
            }),
            Err(StoreError::Backpressure)
        ));

        let (shutdown_tx, shutdown_rx) = mpsc::sync_channel(1);
        let shutdown = thread::spawn(move || {
            shutdown_tx
                .send(owner.shutdown())
                .expect("report read shutdown result");
        });
        wait_for_gate(&handle, AdmissionGate::ShuttingDown);
        assert!(shutdown_rx.recv_timeout(Duration::from_millis(50)).is_err());
        release.release();
        assert_eq!(first.blocking_recv(), Ok(Ok(())));
        assert_eq!(second.blocking_recv(), Ok(Ok(())));
        assert_eq!(
            shutdown_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("joined read shutdown result"),
            Ok(())
        );
        shutdown.join().expect("join read shutdown observer");
    }

    #[test]
    fn worker_panic_is_internal_and_fails_future_admission_closed() {
        let fixture = Fixture::new();
        let owner = fixture.opened(16, 1);
        let handle = owner.handle();
        let receipt = handle
            .submit_writer(PanicWriter)
            .expect("admit injected worker panic");
        assert_eq!(receipt.blocking_recv(), Ok(Err(StoreError::Internal)));
        wait_for_gate(&handle, AdmissionGate::Failed);
        assert!(matches!(
            handle.submit_writer(PanicWriter),
            Err(StoreError::Internal)
        ));
        assert_eq!(owner.shutdown(), Err(StoreError::Internal));
    }

    #[test]
    fn joined_worker_failure_maps_internal_and_still_releases_the_lock_last() {
        let fixture = Fixture::new();
        let mut owner = fixture.opened(16, 1);
        owner
            .workers
            .push(thread::spawn(|| panic!("injected joined-worker failure")));
        assert_eq!(owner.shutdown(), Err(StoreError::Internal));

        let (reopened, state) = OpenedLibraryAuthority::acquire_bootstrap_state(&fixture.library)
            .expect("join failure still closes connections before lock release");
        assert_eq!(state, BootstrapFilesystemState::CanonicalOnly);
        drop(reopened);
    }

    fn wait_for_gate(handle: &StoreHandle, expected: AdmissionGate) {
        for _ in 0..500 {
            if handle.snapshot().0 == expected {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("lifecycle gate did not reach {expected:?}");
    }

    fn fixed_library_id() -> Id<LibraryIdentity> {
        Id::from_bytes([
            0x01, 0x89, 0x0f, 0x1d, 0xe0, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01,
        ])
        .expect("fixed UUIDv7")
    }

    fn fixed_timestamp() -> Timestamp {
        Timestamp::from_unix_seconds_nanos(1_700_000_000, 123_456_789).expect("fixed timestamp")
    }
}
