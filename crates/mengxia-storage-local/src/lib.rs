//! Bounded local content-addressed blob custody adapter.

#![forbid(unsafe_code)]

mod config;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

use mengxia_platform_fs::{
    BlobFileError, OpenedBlobRootAuthority, OpenedBlobSource, OpenedBlobStaging,
};
use mengxia_ports::{
    BlobSourceError, BlobStorage, BlobStorageError, DurableBlob, IngestControl, IngestDirective,
    IngestOutcome, IngestStop,
};
use mengxia_types::Sha256Digest;
use sha2::{Digest as _, Sha256};

pub use config::{BlobConfigSource, BlobStorageConfig, ResolvedBlobStorageConfig};

const MAX_STAGING_NAME_ATTEMPTS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlobIngestState {
    Ready,
    OrphanReconciliationRequired,
}

pub struct BlobStartupReport {
    backend_id: String,
    orphan_count: u16,
    orphan_bytes: u64,
    state: BlobIngestState,
}

impl BlobStartupReport {
    #[must_use]
    pub fn backend_id(&self) -> &str {
        &self.backend_id
    }
    #[must_use]
    pub const fn staging_orphan_count(&self) -> u16 {
        self.orphan_count
    }
    #[must_use]
    pub const fn staging_orphan_bytes(&self) -> u64 {
        self.orphan_bytes
    }
    #[must_use]
    pub const fn ingest_state(&self) -> BlobIngestState {
        self.state
    }
}

/// Opaque local source capability; it cannot expose a path or descriptor.
pub struct OpenedLocalSource {
    inner: OpenedBlobSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeGate {
    Running,
    Closing,
    Failed,
}

struct AdmissionState {
    gate: RuntimeGate,
    active_ingests: usize,
    io_idle: Vec<bool>,
    hash_idle: Vec<bool>,
    observed_orphan_bytes: u128,
    active_written_bytes: u128,
    active_remaining_bytes: u128,
}

struct Shared {
    authority: Arc<OpenedBlobRootAuthority>,
    config: BlobStorageConfig,
    state: Mutex<AdmissionState>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> Result<MutexGuard<'_, AdmissionState>, BlobStorageError> {
        self.state.lock().map_err(|_| BlobStorageError::Internal)
    }

    fn fail(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.gate = RuntimeGate::Failed;
            self.wake.notify_all();
        }
    }
}

struct Admission {
    io_index: usize,
    hash_index: usize,
    declared_length: u64,
}

struct AdmissionPlan {
    io_index: usize,
    hash_index: usize,
    next_remaining_bytes: u128,
}

struct IoJob {
    source: OpenedLocalSource,
    expected_digest: Option<Sha256Digest>,
    control: Arc<dyn IngestControl>,
    admission: Admission,
    reply: SyncSender<Result<IngestOutcome, BlobStorageError>>,
}

enum IoCommand {
    Ingest(IoJob),
    Shutdown,
}

enum HashCommand {
    Begin,
    Chunk {
        buffer: Vec<u8>,
        used: usize,
        reply: SyncSender<Result<Vec<u8>, ()>>,
    },
    Finish {
        reply: SyncSender<Result<[u8; 32], ()>>,
    },
    Shutdown,
}

pub struct LocalBlobStorage {
    shared: Arc<Shared>,
    io_senders: Vec<SyncSender<IoCommand>>,
    hash_senders: Vec<SyncSender<HashCommand>>,
    workers: Mutex<Vec<JoinHandle<()>>>,
}

impl LocalBlobStorage {
    pub fn start(
        config: BlobStorageConfig,
        authority: OpenedBlobRootAuthority,
    ) -> Result<(Self, BlobStartupReport), BlobStorageError> {
        if !authority.authorizes(config.blob_root_request()) {
            return Err(BlobStorageError::Configuration);
        }
        authority
            .revalidate()
            .map_err(|_| BlobStorageError::Configuration)?;
        let orphans = authority
            .observe_staging_orphans()
            .map_err(map_file_error)?;
        if orphans.bytes > config.max_staging_bytes() {
            return Err(BlobStorageError::Configuration);
        }
        let report = BlobStartupReport {
            backend_id: backend_id(authority.backend_instance_digest()),
            orphan_count: orphans.count,
            orphan_bytes: orphans.bytes,
            state: if orphans.count == 0 {
                BlobIngestState::Ready
            } else {
                BlobIngestState::OrphanReconciliationRequired
            },
        };
        let io_count = config.storage_io_concurrency();
        let hash_count = config.hash_concurrency();
        let shared = Arc::new(Shared {
            authority: Arc::new(authority),
            config,
            state: Mutex::new(AdmissionState {
                gate: RuntimeGate::Running,
                active_ingests: 0,
                io_idle: vec![true; io_count],
                hash_idle: vec![true; hash_count],
                observed_orphan_bytes: u128::from(orphans.bytes),
                active_written_bytes: 0,
                active_remaining_bytes: 0,
            }),
            wake: Condvar::new(),
        });

        let mut hash_senders = Vec::with_capacity(hash_count);
        let mut workers = Vec::with_capacity(io_count + hash_count);
        for index in 0..hash_count {
            let (sender, receiver) = mpsc::sync_channel(1);
            let worker = match thread::Builder::new()
                .name(format!("mengxia-blob-hash-{index}"))
                .spawn(move || hash_worker(receiver))
            {
                Ok(worker) => worker,
                Err(_) => {
                    stop_started_workers(&[], &hash_senders, workers);
                    return Err(BlobStorageError::Io);
                }
            };
            hash_senders.push(sender);
            workers.push(worker);
        }
        let mut io_senders = Vec::with_capacity(io_count);
        for index in 0..io_count {
            let (sender, receiver) = mpsc::sync_channel(1);
            let io_shared = Arc::clone(&shared);
            let worker_hash_senders = hash_senders.clone();
            let worker = match thread::Builder::new()
                .name(format!("mengxia-blob-io-{index}"))
                .spawn(move || io_worker(io_shared, receiver, worker_hash_senders))
            {
                Ok(worker) => worker,
                Err(_) => {
                    stop_started_workers(&io_senders, &hash_senders, workers);
                    return Err(BlobStorageError::Io);
                }
            };
            io_senders.push(sender);
            workers.push(worker);
        }
        Ok((
            Self {
                shared,
                io_senders,
                hash_senders,
                workers: Mutex::new(workers),
            },
            report,
        ))
    }

    pub fn open_source(&self, path: &Path) -> Result<OpenedLocalSource, BlobSourceError> {
        if self
            .shared
            .state
            .lock()
            .map_err(|_| BlobSourceError::Io)?
            .gate
            != RuntimeGate::Running
        {
            return Err(BlobSourceError::Io);
        }
        self.shared
            .authority
            .open_source(path)
            .map(|inner| OpenedLocalSource { inner })
            .map_err(map_source_error)
    }

    pub fn shutdown(mut self) -> Result<(), BlobStorageError> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&mut self) -> Result<(), BlobStorageError> {
        {
            let mut state = self.shared.lock()?;
            if state.gate == RuntimeGate::Running {
                state.gate = RuntimeGate::Closing;
            }
            while state.active_ingests != 0 {
                state = self
                    .shared
                    .wake
                    .wait(state)
                    .map_err(|_| BlobStorageError::Internal)?;
            }
        }
        for sender in &self.io_senders {
            let _ = sender.send(IoCommand::Shutdown);
        }
        for sender in &self.hash_senders {
            let _ = sender.send(HashCommand::Shutdown);
        }
        let mut workers = self
            .workers
            .lock()
            .map_err(|_| BlobStorageError::Internal)?;
        let join_failed = join_workers(&mut workers);
        let revalidation = self
            .shared
            .authority
            .sync_for_shutdown()
            .map_err(|error| match error {
                mengxia_platform_fs::AuthorityError::Io => BlobStorageError::Io,
                _ => BlobStorageError::Configuration,
            });
        let failed = self
            .shared
            .state
            .lock()
            .map_or(true, |state| state.gate == RuntimeGate::Failed);
        if failed || join_failed {
            Err(BlobStorageError::Internal)
        } else {
            revalidation
        }
    }

    fn admit(&self, declared_length: u64) -> Result<Admission, BlobStorageError> {
        let mut state = self.shared.lock()?;
        let plan = plan_admission(&state, &self.shared.config, declared_length, || {
            self.shared.authority.capacity().map_err(map_file_error)
        })?;
        state.active_ingests += 1;
        state.io_idle[plan.io_index] = false;
        state.hash_idle[plan.hash_index] = false;
        state.active_remaining_bytes = plan.next_remaining_bytes;
        Ok(Admission {
            io_index: plan.io_index,
            hash_index: plan.hash_index,
            declared_length,
        })
    }
}

fn plan_admission(
    state: &AdmissionState,
    config: &BlobStorageConfig,
    declared_length: u64,
    capacity: impl FnOnce() -> Result<mengxia_platform_fs::BlobCapacity, BlobStorageError>,
) -> Result<AdmissionPlan, BlobStorageError> {
    match state.gate {
        RuntimeGate::Running => {}
        RuntimeGate::Closing => return Err(BlobStorageError::ShuttingDown),
        RuntimeGate::Failed => return Err(BlobStorageError::Internal),
    }
    if declared_length > config.max_ingest_bytes() {
        return Err(BlobStorageError::Validation);
    }
    let orphan_request = state
        .observed_orphan_bytes
        .checked_add(u128::from(declared_length))
        .ok_or(BlobStorageError::Internal)?;
    if orphan_request > u128::from(config.max_staging_bytes()) {
        return Err(BlobStorageError::RecoveryRequired);
    }
    let Some(io_index) = state.io_idle.iter().position(|idle| *idle) else {
        return Err(BlobStorageError::Backpressure);
    };
    let Some(hash_index) = state.hash_idle.iter().position(|idle| *idle) else {
        return Err(BlobStorageError::Backpressure);
    };
    if state.active_ingests == config.max_concurrent_ingests() {
        return Err(BlobStorageError::Backpressure);
    }
    let logical = state
        .observed_orphan_bytes
        .checked_add(state.active_written_bytes)
        .and_then(|value| value.checked_add(state.active_remaining_bytes))
        .and_then(|value| value.checked_add(u128::from(declared_length)))
        .ok_or(BlobStorageError::Internal)?;
    if logical > u128::from(config.max_staging_bytes()) {
        return Err(BlobStorageError::Backpressure);
    }
    let capacity = capacity()?;
    let required = reserve_bytes(config, capacity.total_bytes)?
        .checked_add(state.active_remaining_bytes)
        .and_then(|value| value.checked_add(u128::from(declared_length)))
        .ok_or(BlobStorageError::Internal)?;
    if capacity.available_bytes < required {
        return Err(BlobStorageError::Backpressure);
    }
    let next_remaining_bytes = state
        .active_remaining_bytes
        .checked_add(u128::from(declared_length))
        .ok_or(BlobStorageError::Internal)?;
    Ok(AdmissionPlan {
        io_index,
        hash_index,
        next_remaining_bytes,
    })
}

impl BlobStorage for LocalBlobStorage {
    type Source = OpenedLocalSource;

    fn open_source(&self, path: &Path) -> Result<Self::Source, BlobSourceError> {
        Self::open_source(self, path)
    }

    fn ingest(
        &self,
        source: Self::Source,
        expected_digest: Option<Sha256Digest>,
        control: Arc<dyn IngestControl>,
    ) -> Result<IngestOutcome, BlobStorageError> {
        if let Some(stop) = checkpoint(&control)? {
            return Ok(IngestOutcome::Stopped(stop));
        }
        source.inner.revalidate().map_err(map_file_error)?;
        let admission = self.admit(source.inner.declared_length())?;
        match checkpoint(&control) {
            Ok(Some(stop)) => {
                release_admission(&self.shared, &admission, 0, false);
                return Ok(IngestOutcome::Stopped(stop));
            }
            Ok(None) => {}
            Err(error) => {
                release_admission(&self.shared, &admission, 0, false);
                return Err(error);
            }
        }
        let io_index = admission.io_index;
        let (reply, response) = mpsc::sync_channel(1);
        let job = IoJob {
            source,
            expected_digest,
            control,
            admission,
            reply,
        };
        if let Err((command, error)) =
            dispatch_admitted(&self.io_senders[io_index], IoCommand::Ingest(job))
        {
            if let IoCommand::Ingest(failed) = command {
                release_admission(&self.shared, &failed.admission, 0, true);
            }
            self.shared.fail();
            return Err(error);
        }
        response.recv().unwrap_or_else(|_| {
            self.shared.fail();
            Err(BlobStorageError::Internal)
        })
    }
}

impl Drop for LocalBlobStorage {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

fn io_worker(
    shared: Arc<Shared>,
    receiver: Receiver<IoCommand>,
    hash_senders: Vec<SyncSender<HashCommand>>,
) {
    while let Ok(command) = receiver.recv() {
        match command {
            IoCommand::Ingest(job) => {
                let admission = Admission {
                    io_index: job.admission.io_index,
                    hash_index: job.admission.hash_index,
                    declared_length: job.admission.declared_length,
                };
                match catch_worker_process(|| {
                    process_ingest(&shared, job, &hash_senders[admission.hash_index])
                }) {
                    Ok((reply, result, written, fatal)) => {
                        release_admission(&shared, &admission, written, fatal);
                        let _ = reply.send(result);
                    }
                    Err(_) => {
                        shared.fail();
                        release_admission(&shared, &admission, 0, true);
                    }
                }
            }
            IoCommand::Shutdown => return,
        }
    }
    shared.fail();
}

fn dispatch_admitted<T>(sender: &SyncSender<T>, command: T) -> Result<(), (T, BlobStorageError)> {
    sender
        .send(command)
        .map_err(|error| (error.0, BlobStorageError::Internal))
}

fn catch_worker_process(operation: impl FnOnce() -> ProcessResult) -> Result<ProcessResult, ()> {
    catch_unwind(AssertUnwindSafe(operation)).map_err(|_| ())
}

type ProcessResult = (
    SyncSender<Result<IngestOutcome, BlobStorageError>>,
    Result<IngestOutcome, BlobStorageError>,
    u64,
    bool,
);

enum WriteAbort {
    Stopped(IngestStop),
    Error(BlobStorageError, bool),
}

struct ChunkWriteFailure {
    abort: WriteAbort,
    written: u64,
}

fn process_ingest(
    shared: &Shared,
    job: IoJob,
    hash_sender: &SyncSender<HashCommand>,
) -> ProcessResult {
    let IoJob {
        source,
        expected_digest,
        control,
        admission: _,
        reply,
    } = job;
    let declared_length = source.inner.declared_length();
    if let Some(result) = checkpoint_result(&control) {
        return match result {
            Ok(stop) => (reply, Ok(IngestOutcome::Stopped(stop)), 0, false),
            Err(error) => (reply, Err(error), 0, false),
        };
    }
    let mut staging = match create_staging(shared) {
        Ok(staging) => Some(staging),
        Err(error) => return (reply, Err(error), 0, false),
    };
    if let Some(result) = checkpoint_result(&control) {
        return match result {
            Ok(stop) => abort_stopped(shared, reply, staging.take(), stop, 0),
            Err(error) => abort(shared, reply, staging.take(), error, 0, false),
        };
    }
    if hash_sender.send(HashCommand::Begin).is_err() {
        return abort(
            shared,
            reply,
            staging.take(),
            BlobStorageError::Internal,
            0,
            true,
        );
    }
    let mut buffer = vec![0_u8; shared.config.stream_buffer_bytes()];
    let mut offset = 0_u64;
    while offset < declared_length {
        if let Some(result) = checkpoint_result(&control) {
            return match result {
                Ok(stop) => abort_stopped(shared, reply, staging.take(), stop, offset),
                Err(error) => abort(shared, reply, staging.take(), error, offset, false),
            };
        }
        let requested = usize::try_from((declared_length - offset).min(buffer.len() as u64))
            .expect("bounded buffer");
        let read = match read_declared_chunk_with(&mut buffer[..requested], offset, |buffer, at| {
            source.inner.read_at(buffer, at).map_err(map_file_error)
        }) {
            Ok(read) => read,
            Err(error) => {
                return abort(shared, reply, staging.take(), error, offset, false);
            }
        };
        let (hash_reply, hash_result) = mpsc::sync_channel(1);
        if hash_sender
            .send(HashCommand::Chunk {
                buffer,
                used: read,
                reply: hash_reply,
            })
            .is_err()
        {
            return abort(
                shared,
                reply,
                staging.take(),
                BlobStorageError::Internal,
                offset,
                true,
            );
        }
        buffer = match hash_result.recv() {
            Ok(Ok(buffer)) => buffer,
            _ => {
                return abort(
                    shared,
                    reply,
                    staging.take(),
                    BlobStorageError::Internal,
                    offset,
                    true,
                );
            }
        };
        if let Some(result) = checkpoint_result(&control) {
            return match result {
                Ok(stop) => abort_stopped(shared, reply, staging.take(), stop, offset),
                Err(error) => abort(shared, reply, staging.take(), error, offset, false),
            };
        }
        let staging_ref = staging.as_ref().expect("live staging");
        match write_chunk_with(
            &buffer[..read],
            offset,
            || {
                if let Some(result) = checkpoint_result(&control) {
                    return Err(match result {
                        Ok(stop) => WriteAbort::Stopped(stop),
                        Err(error) => WriteAbort::Error(error, false),
                    });
                }
                check_write_capacity(shared).map_err(|error| WriteAbort::Error(error, false))
            },
            |bytes, position| {
                staging_ref
                    .write_at(bytes, position)
                    .map_err(map_file_error)
            },
            |count| update_written(shared, count),
        ) {
            Ok(written) => offset += written,
            Err(failure) => {
                let written = offset + failure.written;
                return match failure.abort {
                    WriteAbort::Stopped(stop) => {
                        abort_stopped(shared, reply, staging.take(), stop, written)
                    }
                    WriteAbort::Error(error, fatal) => {
                        abort(shared, reply, staging.take(), error, written, fatal)
                    }
                };
            }
        }
        if let Some(result) = checkpoint_result(&control) {
            return match result {
                Ok(stop) => abort_stopped(shared, reply, staging.take(), stop, offset),
                Err(error) => abort(shared, reply, staging.take(), error, offset, false),
            };
        }
    }
    if let Err(error) = eof_probe_with(declared_length, |buffer, at| {
        source.inner.read_at(buffer, at).map_err(map_file_error)
    }) {
        return abort(shared, reply, staging.take(), error, offset, false);
    }
    if let Some(result) = checkpoint_result(&control) {
        return match result {
            Ok(stop) => abort_stopped(shared, reply, staging.take(), stop, offset),
            Err(error) => abort(shared, reply, staging.take(), error, offset, false),
        };
    }
    if source.inner.revalidate().is_err() {
        return abort(
            shared,
            reply,
            staging.take(),
            BlobStorageError::SourceModified,
            offset,
            false,
        );
    }
    if let Some(result) = checkpoint_result(&control) {
        return match result {
            Ok(stop) => abort_stopped(shared, reply, staging.take(), stop, offset),
            Err(error) => abort(shared, reply, staging.take(), error, offset, false),
        };
    }
    let (finish_reply, finish_result) = mpsc::sync_channel(1);
    if hash_sender
        .send(HashCommand::Finish {
            reply: finish_reply,
        })
        .is_err()
    {
        return abort(
            shared,
            reply,
            staging.take(),
            BlobStorageError::Internal,
            offset,
            true,
        );
    }
    let digest = match finish_result.recv() {
        Ok(Ok(digest)) => digest,
        _ => {
            return abort(
                shared,
                reply,
                staging.take(),
                BlobStorageError::Internal,
                offset,
                true,
            );
        }
    };
    if expected_digest.is_some_and(|expected| expected.to_bytes() != digest) {
        return abort(
            shared,
            reply,
            staging.take(),
            BlobStorageError::Corruption,
            offset,
            false,
        );
    }
    if let Some(result) = checkpoint_result(&control) {
        return match result {
            Ok(stop) => abort_stopped(shared, reply, staging.take(), stop, offset),
            Err(error) => abort(shared, reply, staging.take(), error, offset, false),
        };
    }
    let retained_staging = staging.as_ref().expect("live staging");
    match shared.authority.commit_staging(
        retained_staging,
        digest,
        declared_length,
        shared.config.stream_buffer_bytes(),
    ) {
        Ok(_) => (
            reply,
            Ok(IngestOutcome::Stored(
                DurableBlob::__from_verified_local_adapter(
                    Sha256Digest::from_bytes(digest),
                    declared_length,
                    shared.authority.backend_instance_digest(),
                ),
            )),
            offset,
            false,
        ),
        Err(BlobFileError::Corruption) => {
            let accounting = shared.lock().and_then(|mut state| {
                state.observed_orphan_bytes = state
                    .observed_orphan_bytes
                    .checked_add(u128::from(declared_length))
                    .ok_or(BlobStorageError::Internal)?;
                Ok(())
            });
            match accounting {
                Ok(()) => (reply, Err(BlobStorageError::Corruption), offset, false),
                Err(error) => (reply, Err(error), offset, true),
            }
        }
        Err(error) => abort(
            shared,
            reply,
            staging.take(),
            map_file_error(error),
            offset,
            false,
        ),
    }
}

fn hash_worker(receiver: Receiver<HashCommand>) {
    let mut hasher: Option<Sha256> = None;
    while let Ok(command) = receiver.recv() {
        match command {
            HashCommand::Begin => hasher = Some(Sha256::new()),
            HashCommand::Chunk {
                buffer,
                used,
                reply,
            } => {
                let result = hasher.as_mut().map(|hasher| {
                    hasher.update(&buffer[..used]);
                    buffer
                });
                let _ = reply.send(result.ok_or(()));
            }
            HashCommand::Finish { reply } => {
                let _ = reply.send(hasher.take().map(|value| value.finalize().into()).ok_or(()));
            }
            HashCommand::Shutdown => return,
        }
    }
}

fn read_declared_chunk_with(
    buffer: &mut [u8],
    offset: u64,
    mut read: impl FnMut(&mut [u8], u64) -> Result<usize, BlobStorageError>,
) -> Result<usize, BlobStorageError> {
    let count = read(buffer, offset)?;
    if count == 0 {
        return Err(BlobStorageError::SourceModified);
    }
    if count > buffer.len() {
        return Err(BlobStorageError::Internal);
    }
    Ok(count)
}

fn eof_probe_with(
    declared_length: u64,
    mut read: impl FnMut(&mut [u8], u64) -> Result<usize, BlobStorageError>,
) -> Result<(), BlobStorageError> {
    let mut probe = [0_u8; 1];
    match read(&mut probe, declared_length)? {
        0 => Ok(()),
        1 => Err(BlobStorageError::SourceModified),
        _ => Err(BlobStorageError::Internal),
    }
}

fn write_chunk_with(
    bytes: &[u8],
    base_offset: u64,
    mut before_attempt: impl FnMut() -> Result<(), WriteAbort>,
    mut write: impl FnMut(&[u8], u64) -> Result<usize, BlobStorageError>,
    mut account: impl FnMut(u64) -> Result<(), BlobStorageError>,
) -> Result<u64, ChunkWriteFailure> {
    let mut written = 0_usize;
    while written < bytes.len() {
        before_attempt().map_err(|abort| ChunkWriteFailure {
            abort,
            written: written as u64,
        })?;
        let position = base_offset
            .checked_add(written as u64)
            .ok_or(ChunkWriteFailure {
                abort: WriteAbort::Error(BlobStorageError::Internal, true),
                written: written as u64,
            })?;
        let count = write(&bytes[written..], position).map_err(|error| ChunkWriteFailure {
            abort: WriteAbort::Error(error, false),
            written: written as u64,
        })?;
        if count == 0 || count > bytes.len() - written {
            return Err(ChunkWriteFailure {
                abort: WriteAbort::Error(BlobStorageError::Io, count > bytes.len() - written),
                written: written as u64,
            });
        }
        written += count;
        account(count as u64).map_err(|_| ChunkWriteFailure {
            abort: WriteAbort::Error(BlobStorageError::Internal, true),
            written: written as u64,
        })?;
    }
    Ok(written as u64)
}

fn stop_started_workers(
    io_senders: &[SyncSender<IoCommand>],
    hash_senders: &[SyncSender<HashCommand>],
    workers: Vec<JoinHandle<()>>,
) {
    for sender in io_senders {
        let _ = sender.send(IoCommand::Shutdown);
    }
    for sender in hash_senders {
        let _ = sender.send(HashCommand::Shutdown);
    }
    for worker in workers {
        let _ = worker.join();
    }
}

fn join_workers(workers: &mut Vec<JoinHandle<()>>) -> bool {
    let mut failed = false;
    for worker in workers.drain(..) {
        if worker.join().is_err() {
            failed = true;
        }
    }
    failed
}

fn create_staging(shared: &Shared) -> Result<OpenedBlobStaging, BlobStorageError> {
    create_staging_with(
        |random| getrandom::fill(random).map_err(|_| BlobStorageError::EntropyUnavailable),
        |random| shared.authority.create_staging(random),
    )
}

fn create_staging_with<T>(
    mut entropy: impl FnMut(&mut [u8; 16]) -> Result<(), BlobStorageError>,
    mut create: impl FnMut([u8; 16]) -> Result<T, BlobFileError>,
) -> Result<T, BlobStorageError> {
    for _ in 0..MAX_STAGING_NAME_ATTEMPTS {
        let mut random = [0_u8; 16];
        entropy(&mut random)?;
        match create(random) {
            Ok(staging) => return Ok(staging),
            Err(BlobFileError::Collision) => {}
            Err(error) => return Err(map_file_error(error)),
        }
    }
    Err(BlobStorageError::StagingNamespaceUnavailable)
}

fn checkpoint(control: &Arc<dyn IngestControl>) -> Result<Option<IngestStop>, BlobStorageError> {
    catch_unwind(AssertUnwindSafe(|| control.checkpoint()))
        .map(|directive| match directive {
            IngestDirective::Continue => None,
            IngestDirective::Stop(stop) => Some(stop),
        })
        .map_err(|_| BlobStorageError::Internal)
}

fn checkpoint_result(
    control: &Arc<dyn IngestControl>,
) -> Option<Result<IngestStop, BlobStorageError>> {
    match checkpoint(control) {
        Ok(None) => None,
        Ok(Some(stop)) => Some(Ok(stop)),
        Err(error) => Some(Err(error)),
    }
}

fn abort(
    shared: &Shared,
    reply: SyncSender<Result<IngestOutcome, BlobStorageError>>,
    staging: Option<OpenedBlobStaging>,
    error: BlobStorageError,
    written: u64,
    fatal: bool,
) -> ProcessResult {
    match staging
        .as_ref()
        .map(|value| shared.authority.cleanup_staging(value))
    {
        Some(Err(_)) => (reply, Err(BlobStorageError::CleanupFailed), written, true),
        _ => (reply, Err(error), written, fatal),
    }
}

fn abort_stopped(
    shared: &Shared,
    reply: SyncSender<Result<IngestOutcome, BlobStorageError>>,
    staging: Option<OpenedBlobStaging>,
    stop: IngestStop,
    written: u64,
) -> ProcessResult {
    match staging
        .as_ref()
        .map(|value| shared.authority.cleanup_staging(value))
    {
        Some(Err(_)) => (reply, Err(BlobStorageError::CleanupFailed), written, true),
        _ => (reply, Ok(IngestOutcome::Stopped(stop)), written, false),
    }
}

fn check_write_capacity(shared: &Shared) -> Result<(), BlobStorageError> {
    let state = shared.lock()?;
    if state.gate != RuntimeGate::Running {
        return Err(if state.gate == RuntimeGate::Closing {
            BlobStorageError::ShuttingDown
        } else {
            BlobStorageError::Internal
        });
    }
    let capacity = shared.authority.capacity().map_err(map_file_error)?;
    let required = reserve_bytes(&shared.config, capacity.total_bytes)?
        .checked_add(state.active_remaining_bytes)
        .ok_or(BlobStorageError::Internal)?;
    if capacity.available_bytes < required {
        Err(BlobStorageError::Io)
    } else {
        Ok(())
    }
}

fn update_written(shared: &Shared, bytes: u64) -> Result<(), BlobStorageError> {
    let mut state = shared.lock()?;
    state.active_remaining_bytes = state
        .active_remaining_bytes
        .checked_sub(u128::from(bytes))
        .ok_or(BlobStorageError::Internal)?;
    state.active_written_bytes = state
        .active_written_bytes
        .checked_add(u128::from(bytes))
        .ok_or(BlobStorageError::Internal)?;
    Ok(())
}

fn release_admission(shared: &Shared, admission: &Admission, written: u64, fatal: bool) {
    let Ok(mut state) = shared.state.lock() else {
        return;
    };
    release_admission_state(&mut state, admission, written, fatal);
    shared.wake.notify_all();
}

fn release_admission_state(
    state: &mut AdmissionState,
    admission: &Admission,
    written: u64,
    fatal: bool,
) {
    let remaining = admission.declared_length.checked_sub(written);
    let next_remaining =
        remaining.and_then(|value| state.active_remaining_bytes.checked_sub(u128::from(value)));
    let next_written = state.active_written_bytes.checked_sub(u128::from(written));
    let next_active = state.active_ingests.checked_sub(1);
    let (Some(next_remaining), Some(next_written), Some(next_active)) =
        (next_remaining, next_written, next_active)
    else {
        // A caught worker panic may make its exact partial-write split
        // unknowable. Poison the runtime, but still retire this admission so
        // joined shutdown cannot wait forever. No later admission can consume
        // the deliberately conservative aggregate counters after `Failed`.
        state.active_ingests = state.active_ingests.saturating_sub(1);
        state.io_idle[admission.io_index] = true;
        state.hash_idle[admission.hash_index] = true;
        state.gate = RuntimeGate::Failed;
        return;
    };
    state.active_remaining_bytes = next_remaining;
    state.active_written_bytes = next_written;
    state.active_ingests = next_active;
    state.io_idle[admission.io_index] = true;
    state.hash_idle[admission.hash_index] = true;
    if fatal {
        state.gate = RuntimeGate::Failed;
    }
}

fn reserve_bytes(config: &BlobStorageConfig, total: u128) -> Result<u128, BlobStorageError> {
    let percent = total
        .checked_mul(u128::from(config.min_free_percent()))
        .and_then(|value| value.checked_add(99))
        .map(|value| value / 100)
        .ok_or(BlobStorageError::Internal)?;
    Ok(percent.max(u128::from(config.min_free_bytes())))
}

fn map_source_error(error: BlobFileError) -> BlobSourceError {
    match error {
        BlobFileError::InvalidPath | BlobFileError::Configuration => BlobSourceError::InvalidPath,
        BlobFileError::UnsupportedType => BlobSourceError::UnsupportedType,
        BlobFileError::Modified => BlobSourceError::Modified,
        _ => BlobSourceError::Io,
    }
}

fn map_file_error(error: BlobFileError) -> BlobStorageError {
    match error {
        BlobFileError::InvalidPath | BlobFileError::UnsupportedType => BlobStorageError::Validation,
        BlobFileError::Io => BlobStorageError::Io,
        BlobFileError::Modified => BlobStorageError::SourceModified,
        BlobFileError::Configuration => BlobStorageError::Configuration,
        BlobFileError::Corruption => BlobStorageError::Corruption,
        BlobFileError::Collision => BlobStorageError::StagingNamespaceUnavailable,
        BlobFileError::CleanupFailed => BlobStorageError::CleanupFailed,
        _ => BlobStorageError::Internal,
    }
}

fn backend_id(digest: [u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut result = String::with_capacity(85);
    result.push_str("mengxia.local-cas.v1/");
    for byte in digest {
        write!(&mut result, "{byte:02x}").expect("writing to String cannot fail");
    }
    result
}

#[cfg(test)]
mod scaling_tests {
    use std::cell::Cell;

    use mengxia_platform_fs::{BlobCapacity, BlobFileError};
    use mengxia_ports::BlobStorageError;
    use sha2::{Digest as _, Sha256};

    use super::{
        AdmissionState, BlobConfigSource, HashCommand, MAX_STAGING_NAME_ATTEMPTS,
        ResolvedBlobStorageConfig, RuntimeGate, WriteAbort, catch_worker_process,
        create_staging_with, dispatch_admitted, eof_probe_with, hash_worker, join_workers,
        plan_admission, read_declared_chunk_with, release_admission_state, write_chunk_with,
    };

    const GIB: u64 = 1024 * 1024 * 1024;
    const BUFFER_BYTES: usize = 8 * 1024 * 1024;

    fn test_config() -> super::BlobStorageConfig {
        let source = BlobConfigSource::CompiledDefault;
        let raw = |value: u64| Some(value.to_string());
        ResolvedBlobStorageConfig::from_selected(
            Some(std::path::PathBuf::from("/Users/example/Library")),
            source,
            Some(std::path::PathBuf::from("/Users/example/Library/storage")),
            source,
            raw(1),
            source,
            raw(1),
            source,
            raw(1),
            source,
            raw(1024 * 1024),
            source,
            raw(GIB),
            source,
            raw(2 * GIB),
            source,
            raw(10 * GIB),
            source,
            raw(5),
            source,
        )
        .validate()
        .unwrap()
    }

    #[test]
    #[ignore = "formal TASK-005 gate processes the full 1/10/100 GiB generated streams"]
    fn task_005_generated_scaling_evidence() {
        let mut buffer = vec![0_u8; BUFFER_BYTES];
        let allocation = buffer.capacity();
        for logical_length in [GIB, 10 * GIB, 100 * GIB] {
            let mut remaining = logical_length;
            let mut generated_offset = 0_u64;
            let mut discarded = 0_u64;
            let mut hasher = Sha256::new();
            while remaining != 0 {
                let used = usize::try_from(remaining.min(BUFFER_BYTES as u64)).unwrap();
                for (index, byte) in buffer[..used].iter_mut().enumerate() {
                    *byte = (generated_offset.wrapping_add(index as u64) & 0xff) as u8;
                }
                hasher.update(&buffer[..used]);
                discarded += used as u64;
                generated_offset += used as u64;
                remaining -= used as u64;
            }
            assert_eq!(discarded, logical_length);
            assert_eq!(buffer.capacity(), allocation);
            assert_ne!(<[u8; 32]>::from(hasher.finalize()), [0_u8; 32]);
        }
    }

    #[test]
    fn staging_entropy_and_eight_collision_failures_are_distinct_and_bounded() {
        let create_calls = Cell::new(0);
        let entropy_failure = create_staging_with::<()>(
            |_| Err(BlobStorageError::EntropyUnavailable),
            |_| {
                create_calls.set(create_calls.get() + 1);
                Ok(())
            },
        );
        assert_eq!(
            entropy_failure,
            Err(BlobStorageError::EntropyUnavailable),
            "FAULT-005-030"
        );
        assert_eq!(create_calls.get(), 0);

        let entropy_calls = Cell::new(0);
        let create_calls = Cell::new(0);
        let collisions = create_staging_with::<()>(
            |random| {
                entropy_calls.set(entropy_calls.get() + 1);
                random.fill(entropy_calls.get() as u8);
                Ok(())
            },
            |_| {
                create_calls.set(create_calls.get() + 1);
                Err(BlobFileError::Collision)
            },
        );
        assert_eq!(
            collisions,
            Err(BlobStorageError::StagingNamespaceUnavailable),
            "FAULT-005-031"
        );
        assert_eq!(entropy_calls.get(), MAX_STAGING_NAME_ATTEMPTS);
        assert_eq!(create_calls.get(), MAX_STAGING_NAME_ATTEMPTS);
    }

    #[test]
    fn short_and_zero_writes_have_exact_offsets_accounting_and_failure_semantics() {
        let responses = Cell::new(0_usize);
        let observed = std::cell::RefCell::new(Vec::new());
        let accounted = std::cell::RefCell::new(Vec::new());
        let sizes = [2_usize, 3, 5];
        let result = write_chunk_with(
            &[0_u8; 10],
            100,
            || Ok(()),
            |remaining, offset| {
                let index = responses.get();
                responses.set(index + 1);
                observed.borrow_mut().push((remaining.len(), offset));
                Ok(sizes[index])
            },
            |count| {
                accounted.borrow_mut().push(count);
                Ok(())
            },
        );
        assert_eq!(result.ok(), Some(10), "FAULT-005-032/033/034");
        assert_eq!(&*observed.borrow(), &[(10, 100), (8, 102), (5, 105)]);
        assert_eq!(&*accounted.borrow(), &[2, 3, 5]);

        let attempts = Cell::new(0);
        let zero = write_chunk_with(
            &[1_u8; 4],
            0,
            || Ok(()),
            |_, _| {
                attempts.set(attempts.get() + 1);
                Ok(0)
            },
            |_| Ok(()),
        )
        .expect_err("zero write must fail");
        assert_eq!(attempts.get(), 1);
        assert_eq!(zero.written, 0);
        assert!(matches!(
            zero.abort,
            WriteAbort::Error(BlobStorageError::Io, false)
        ));
    }

    #[test]
    fn short_premature_and_eof_reads_have_exact_bounded_mapping() {
        let mut buffer = [0_u8; 8];
        let calls = Cell::new(0);
        let short = read_declared_chunk_with(&mut buffer, 41, |requested, offset| {
            calls.set(calls.get() + 1);
            assert_eq!(requested.len(), 8);
            assert_eq!(offset, 41);
            requested[..3].copy_from_slice(b"abc");
            Ok(3)
        });
        assert_eq!(short, Ok(3), "FAULT-005-028");
        assert_eq!(calls.get(), 1);

        let premature = read_declared_chunk_with(&mut buffer, 44, |_, _| Ok(0));
        assert_eq!(
            premature,
            Err(BlobStorageError::SourceModified),
            "FAULT-005-029"
        );

        let eof_calls = Cell::new(0);
        assert_eq!(
            eof_probe_with(99, |probe, offset| {
                eof_calls.set(eof_calls.get() + 1);
                assert_eq!(probe.len(), 1);
                assert_eq!(offset, 99);
                Ok(0)
            }),
            Ok(()),
            "FAULT-005-027 empty probe"
        );
        assert_eq!(eof_calls.get(), 1);
        assert_eq!(
            eof_probe_with(99, |_, _| Ok(1)),
            Err(BlobStorageError::SourceModified),
            "FAULT-005-027 late byte"
        );
        assert_eq!(
            eof_probe_with(99, |_, _| Err(BlobStorageError::Io)),
            Err(BlobStorageError::Io),
            "FAULT-005-027 bounded I/O error"
        );
    }

    #[test]
    fn worker_reply_loss_panic_and_join_failure_are_detected_without_detaching() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || hash_worker(receiver));
        sender.send(HashCommand::Begin).unwrap();
        let (reply, result) = std::sync::mpsc::sync_channel(1);
        sender
            .send(HashCommand::Chunk {
                buffer: vec![0],
                used: 2,
                reply,
            })
            .unwrap();
        assert!(result.recv().is_err(), "FAULT-005-066 reply loss");
        let mut workers = vec![worker];
        assert!(join_workers(&mut workers), "FAULT-005-068 join failure");
        assert!(workers.is_empty());
    }

    #[test]
    fn capacity_sampling_and_arithmetic_fail_before_admission_mutation() {
        let config = test_config();
        let state = AdmissionState {
            gate: RuntimeGate::Running,
            active_ingests: 0,
            io_idle: vec![true],
            hash_idle: vec![true],
            observed_orphan_bytes: 0,
            active_written_bytes: 0,
            active_remaining_bytes: 0,
        };
        assert_eq!(
            plan_admission(&state, &config, 1, || Err(BlobStorageError::Io)).err(),
            Some(BlobStorageError::Io),
            "FAULT-005-064 capacity sample"
        );
        assert_eq!(
            plan_admission(&state, &config, 1, || {
                Ok(BlobCapacity {
                    available_bytes: u128::MAX,
                    total_bytes: u128::MAX,
                })
            })
            .err(),
            Some(BlobStorageError::Internal),
            "FAULT-005-064 arithmetic overflow"
        );
        assert_eq!(state.gate, RuntimeGate::Running);
        assert_eq!(state.active_ingests, 0);
        assert_eq!(state.io_idle, [true]);
        assert_eq!(state.hash_idle, [true]);
        assert_eq!(state.active_written_bytes, 0);
        assert_eq!(state.active_remaining_bytes, 0);
    }

    #[test]
    fn closed_admitted_dispatch_returns_ownership_and_internal_not_backpressure() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        drop(receiver);
        assert_eq!(
            dispatch_admitted(&sender, 0x5a_u8),
            Err((0x5a, BlobStorageError::Internal)),
            "FAULT-005-065"
        );
    }

    #[test]
    fn io_worker_panic_is_caught_at_the_joined_worker_boundary() {
        let caught = catch_worker_process(|| panic!("injected I/O worker panic"));
        assert!(caught.is_err(), "FAULT-005-067");

        let mut state = AdmissionState {
            gate: RuntimeGate::Running,
            active_ingests: 1,
            io_idle: vec![false],
            hash_idle: vec![false],
            observed_orphan_bytes: 0,
            active_written_bytes: 7,
            active_remaining_bytes: 9,
        };
        let admission = super::Admission {
            io_index: 0,
            hash_index: 0,
            declared_length: 16,
        };
        // A panic after seven bytes makes the caller-side fallback's `written`
        // value stale. It must fail closed and retire the admission instead of
        // leaving joined shutdown blocked forever.
        release_admission_state(&mut state, &admission, 0, true);
        assert_eq!(state.gate, RuntimeGate::Failed);
        assert_eq!(state.active_ingests, 0);
        assert_eq!(state.io_idle, [true]);
        assert_eq!(state.hash_idle, [true]);
    }
}
