# TASK-005 Local BlobStorage / CAS start-gate proposal

- Status: **ACCEPTED / ACTIVE TASK-005**
- Proposed task: `TASK-005`
- Date: 2026-08-26
- Canonical state: `TASK-005 IN_PROGRESS`
- Implementation authority granted by this document: **TASK-005 ONLY**

This document is the accepted normative TASK-005 supplement incorporated by
Specification v1.1.18 and ADR-0007. On 2026-08-27 the user explicitly activated
§16.1 in the canonical Plan. Production changes are authorized only for TASK-005's
exact §3.1 scope; TASK-006 and later remain unauthorized.

## 1. Gate conclusion and repository reality

The repository has an accepted and active TASK-005 contract. TASK-001, TASK-002,
TASK-004 and TASK-003 are complete. The exact-scope TASK-005 implementation and its
local formal completion candidate now exist; reviewed `macos-26` CI evidence remains
required before `DONE`. The table below records the pre-start repository gaps and
their accepted resolutions rather than pretending they remain current gaps.

| Finding | Evidence | Classification | Proposed resolution |
|---|---|---|---|
| `mengxia-ports` and `mengxia-storage-local` contain no CAS behavior | their current `src/lib.rs` files contain only crate boundaries | `EXPECTED_GAP` | implement only the contract below after authorization |
| canonical `MENGXIA_BLOB_ROOT` defaults to `<library>/storage`, while TASK-004 accepts only lock plus SQLite names and rechecks that exact set during shutdown | Specification §16; `OpenedLibraryAuthority::acquire_bootstrap_state`; `sync_closed_canonical_database` | `CONFLICT` | permit one exact, verified `storage` directory only in the complete canonical Library state; §5 defines the non-weakening rule |
| canonical TASK-005 file/dependency row lists only ports/local storage and TASK-002, but the required default-root authority and compatibility change live in `mengxia-platform-fs` and consume the completed Library foundation | Specification §18 TASK-005; Plan TASK-005 row; repository crate graph | `SPEC_STALE` | add TASK-004 plus ADR-0004 as prerequisites and the narrow platform/test files to the authorized scope |
| production composition currently resolves IPC/DB settings but none of the storage settings | `bins/mengxiad/src/main.rs` | `EXPECTED_GAP` | TASK-005 owns a source-free immutable DTO and validator only; TASK-007 owns the production four-layer resolver when ingest is enabled |
| orphan retention/deletion policy remains open | `OQ-008` | `LATER_PHASE` | TASK-005 preserves and reports prior-process orphans and never performs GC; TASK-008 verifies/reconciles and TASK-022 owns deletion |
| true power-loss behavior cannot be proven by a normal CI process-kill test | Apple `fsync(2)` and `fcntl(2)` semantics | `UNVERIFIABLE` release evidence | require ordered `F_FULLFSYNC`/directory synchronization and exact syscall-fault/SIGKILL tests; make no claim that CI simulated hardware power loss |
| Intake said TASK-003 was unauthorized although TASK-003 is complete | prior `PROJECT_INTAKE_REPORT.md` final action paragraph versus AGENTS/Spec/Plan | `SPEC_STALE / RESOLVED` | corrected by the Specification v1.1.18 TASK-005 gate synchronization; it was not a runtime blocker |

No open Provider, Plugin, Admin, Credential, Rights or retention decision blocks this
primitive. `OQ-006` is closed for TASK-005 by ADR-0005. `OQ-008` is not a false global
blocker because this task has no delete/GC authority.

## 2. Governing requirements and decisions

The implementation and review must read these exact inputs:

- feature: `FUNC-002` (storage primitive only, not the product ingest operation);
- data: `DATA-002`, `DATA-003`, `DATA-004`, `DATA-013`;
- performance/reliability/security/config: `PERF-001`, `REL-001`, `REL-004`,
  `REL-006`, `SEC-017`, `SEC-020`, `SEC-021`, `CFG-001`, `CFG-003`;
- baselines: `BASE-009`, `BASE-011`, `BASE-013`, `BASE-014`, `BASE-015`, `BASE-016`,
  `BASE-017`, `BASE-018`;
- decisions: ADR-0002, ADR-0003, ADR-0004, ADR-0005 and the existing
  platform/FFI isolation in ADR-0006; acceptance must create
  `ADR-0007-local-cas-custody-boundary` for the exact capability, stream, admission,
  namespace, cleanup/error and Location-instance decisions in this supplement;
- prerequisites: `TASK-002 DONE` and `TASK-004 DONE`.

TASK-005 proves the durable byte-custody primitive needed by DATA-002/DATA-003. It
cannot by itself prove that a Managed Asset has a Location because TASK-006 owns the
model and TASK-007 owns registration. Completion must therefore say “storage
precondition PASS”, not falsely mark the whole FUNC-002 or DATA-013 flow complete.

## 3. Exact authorized and forbidden scope

### 3.1 Proposed implementation file scope

After acceptance, TASK-005 may modify only:

```text
Cargo.toml
Cargo.lock                                  # only if metadata changes; no version drift
.github/workflows/ci.yml
scripts/verify-task-005.sh
crates/mengxia-ports/Cargo.toml
crates/mengxia-ports/src/**
crates/mengxia-ports/tests/**
crates/mengxia-storage-local/Cargo.toml
crates/mengxia-storage-local/src/**
crates/mengxia-storage-local/tests/**
crates/mengxia-platform-fs/src/lib.rs       # private Library-lock lease + namespace dispatch only
crates/mengxia-platform-fs/src/blob_storage.rs  # new Blob/source authority module only
crates/mengxia-store-sqlite/src/lib.rs      # narrow authorize_blob_root seam only
crates/mengxia-store-sqlite/src/lifecycle.rs # lease minting/lifecycle compatibility only
crates/mengxia-testkit/Cargo.toml
crates/mengxia-testkit/tests/task_005_foundation.rs
crates/mengxia-testkit/tests/document_traceability.rs
docs/proposals/TASK-005-GATE-PROPOSAL.md
docs/spec/IMPLEMENTATION_SPEC.md
docs/spec/DECISIONS.md
docs/spec/IMPLEMENTATION_REVIEW.md
docs/spec/IMPLEMENTATION_PLAN.md
docs/spec/PROJECT_INTAKE_REPORT.md
docs/spec/adr/ADR-0005-foundation-safety-caps.md # atomic reservation/EINTR constants clarification
docs/spec/adr/ADR-0007-local-cas-custody-boundary.md # new accepted TASK-005 architecture decision
AGENTS.md                                    # TASK-005 IN_PROGRESS start markers and DONE completion markers only
```

The `mengxia-store-sqlite` exception is limited to treating the exact verified
default `storage` directory as an orthogonal completed-Library child. It must not
change migrations, SQLite options, SQL, queues, recovery ownership or public raw
store authority. No new package is authorized.

Within `mengxia-platform-fs/src/lib.rs`, the only permitted symbols/regions are
`OpenedLibraryAuthority`'s private lock ownership representation,
`BootstrapFilesystemState`/canonical-only namespace classification, and exports of
the new private-by-construction Blob authority module. `runtime_endpoint`, ACL shim/
build evidence, bootstrap intent bytes and SQLite fixed-child consumers are frozen.
Within store, only `OpenedLibrary`, `OpenedLibraryOwner` and their tests may change;
`bootstrap.rs`, `config.rs`, `error.rs`, `intent.rs`, `migration.rs`, `runtime.rs`,
`stock_sqlite_open.rs`, `wal.rs`, SQL and migrations are explicitly out of scope.

### 3.2 Explicitly forbidden

TASK-005 must not implement or modify:

- `IngestAsset`, any product Command/Query, Protobuf message, daemon route or CLI;
- Asset/Revision/Blob/Location domain records or migration `0001` and later;
- canonical DB registration, idempotency records, audit/events or transactions;
- Admin, Plugin, Provider, Credential, Broker, HTTP/TCP or sandbox behavior;
- source move, unlink, chmod, ACL rewrite, ownership rewrite or content mutation;
- startup deep verification, orphan deletion, GC, Location removal or Purge;
- arbitrary path access by CLI/Plugin, a public CAS absolute path, raw file
  descriptor, Library lock or SQLite handle;
- background work that can outlive the synchronous operation, unbounded queues,
  unbounded retry or a new unsafe/FFI block.

## 4. Public contract and configuration boundary

### 4.1 Source-free immutable configuration

`mengxia-storage-local` owns `ResolvedBlobStorageConfig::validate()` and an immutable
`BlobStorageConfig`. Neither type reads environment variables, CLI arguments or a
Library config file. The composition root must pass already selected values and
their source. TASK-007 must add and test the production precedence
`CLI > environment > Library config > compiled default` exactly once before it
enables ingest.

The DTO contains:

```text
library_root: absolute platform Path bytes
blob_root: absolute platform Path bytes
storage_io_concurrency: 1..=8, default 2
hash_concurrency: 1..=8, default 2
max_concurrent_ingests: 1..=8, default 2
stream_buffer_bytes: 1 MiB..=32 MiB, default 8 MiB
max_ingest_bytes: 1..=1 TiB, default 1 TiB; tightening only
max_staging_bytes: 1..=2 TiB, default 2 TiB; tightening only
min_free_bytes: at least 10 GiB; default 10 GiB; may only increase
min_free_percent: 5..=100; default 5; may only increase
```

Every selected field also carries its storage-local typed `BlobConfigSource`; the
validator does not infer a source and does not import the SQLite config module. The
field mapping is exact:
`MENGXIA_BLOB_ROOT → blob_root`,
`MENGXIA_STORAGE_IO_CONCURRENCY → storage_io_concurrency`,
`MENGXIA_HASH_CONCURRENCY → hash_concurrency`,
`MENGXIA_MAX_CONCURRENT_INGESTS → max_concurrent_ingests`,
`MENGXIA_STREAM_BUFFER_BYTES → stream_buffer_bytes`,
`MENGXIA_MAX_INGEST_BYTES → max_ingest_bytes`,
`MENGXIA_MAX_STAGING_BYTES → max_staging_bytes`,
`MENGXIA_MIN_FREE_BYTES → min_free_bytes` and
`MENGXIA_MIN_FREE_PERCENT → min_free_percent`.

All arithmetic is checked using a representation wide enough for multiplication.
Missing values, zero, sign text, whitespace, non-decimal text, non-Unicode path
loss, NUL, overflow, widening a tightening-only boundary or impossible reserve
combinations fail before a Blob root or source is opened. `stream_buffer_bytes`
uses the literal ADR-0005 range; a future change from 1 MiB to a smaller value needs
a recorded ADR change, not an implementation guess.

On the accepted macOS platform, a source absolute path is bounded to 1023 bytes
excluding the terminating NUL and every normal component to 255 bytes, matching the
checked SDK `PATH_MAX=1024` and `NAME_MAX=255`. A Blob root has the stricter maximum
of 937 bytes: the longest fixed relative CAS name is the exact 85-byte locator in
§4.2, and `937 + 1 separator + 85 == 1023`. Consequently the default
`<library_root>/storage` is available to TASK-005 only when the Library-root bytes
are at most 929; a longer already-valid TASK-004 Library remains openable but storage
startup fails configuration before creating `storage`. Cap+1 is rejected before
walking or mutation. Blob-root configuration remains Unicode as required by the
existing composition/config boundary; a local source may contain non-Unicode bytes
because it is never serialized or logged.

The default Blob root is computed by the future composition resolver as the exact
fixed child `<library_root>/storage`. A custom root must be absolute and lexically
canonical. It must be disjoint from the Library tree: equal to, an ancestor of, or a
descendant of the Library root is rejected, except for that one exact default child.
The validator owns the resulting opaque `BlobRootRequest` inside `BlobStorageConfig`;
`blob_root_request()` exposes only `&BlobRootRequest`, never path bytes.

### 4.2 Port and result shape

The provider-neutral port is synchronous and blocking. `LocalBlobStorage` owns the
fixed-size I/O and hash worker pools required by ADR-0005, but one caller remains
joined to its admitted job until it completes, stops before promote or rolls back;
work is never detached. The later application layer calls it from a bounded blocking
context rather than a Tokio core thread.

There is **no public `BlobSource` trait**. Production source authority is the concrete
opaque `OpenedLocalSource`: public type name, private fields, no public constructor,
no `Clone`, serialization, `Read`, `AsFd`, raw-fd, `Path`, `AsRef<Path>`, `Debug` or
path/string accessor. Only `LocalBlobStorage::open_source` can construct it after
§6.2 validation. `BlobStorage` uses an associated source type, so application tests
may implement a fake storage with their own test source without gaining the ability
to forge `OpenedLocalSource`. No `pub Fake*`, source constructor or production
feature flag is allowed; byte/fault fakes live only under `#[cfg(test)]` inside the
owning crates.

The exact port/control surface is:

```rust
pub trait IngestControl: Send + Sync + 'static {
    fn checkpoint(&self) -> IngestDirective;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngestDirective { Continue, Stop(IngestStop) }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngestStop { Cancelled, DeadlineReached }

pub enum IngestOutcome { Stored(DurableBlob), Stopped(IngestStop) }

pub trait BlobStorage: Send + Sync {
    type Source: Send + 'static;

    fn open_source(&self, path: &Path) -> Result<Self::Source, BlobSourceError>;

    fn ingest(
        &self,
        source: Self::Source,
        expected_digest: Option<Sha256Digest>,
        control: Arc<dyn IngestControl>,
    ) -> Result<IngestOutcome, BlobStorageError>;
}
```

`mengxia-ports` owns `BlobStorage`, the control/outcome/result/location/error types
and uses the existing `mengxia-types::{Sha256Digest, ErrorCode}`. It exposes no
general string/path constructor for `DurableBlob` or `DurableLocationDescriptor`.
Rust has no cross-crate “friend” visibility, so the implementable narrow seam is one
`#[doc(hidden)] pub fn DurableBlob::__from_verified_local_adapter(digest,
byte_length, backend_instance_digest: [u8; 32]) -> Self`; it constructs both exact
85-byte strings internally. Repository architecture lint permits this symbol only at
the local adapter's verified-return site. It is an honest trusted-code boundary, not
an unforgeability claim. `mengxia-storage-local` owns
`LocalBlobStorage`, `OpenedLocalSource`, `BlobStorageConfig` and startup reporting.
No domain or application crate depends on the local adapter; TASK-007 can invoke
`open_source` and `ingest` through its generic ports-only dependency.

The exact concrete lifecycle surface is:

```rust
impl LocalBlobStorage {
    pub fn start(
        config: BlobStorageConfig,
        authority: OpenedBlobRootAuthority,
    ) -> Result<(Self, BlobStartupReport), BlobStorageError>;

    pub fn open_source(
        &self,
        path: &Path,
    ) -> Result<OpenedLocalSource, BlobSourceError>;

    pub fn shutdown(self) -> Result<(), BlobStorageError>;
}

impl BlobStorage for LocalBlobStorage {
    type Source = OpenedLocalSource;
    // open_source and ingest have exactly the trait signatures above
}
```

The composition sequence is exact: validate config; call
`OpenedLibrary::authorize_blob_root(config.blob_root_request())`; then pass that same
config and returned authority to `start`. The authority retains an opaque request
identity and `start` compares it with the config's request before spawning workers or
mutating the root. A config/authority mismatch is `Configuration`. Neither crate
exposes the identity bytes or a caller-supplied constructor.

`IngestControl::checkpoint` must be non-blocking, allocation-free and perform no I/O;
a panic is caught at the storage boundary and returns `Internal` for that call. It
does not fail the whole runtime when no internal worker panicked and any staging
cleanup completed with a known result; cleanup uncertainty still follows §9 and
fails the runtime closed.
`IngestControl` is a cooperative caller signal, not an authority or product deadline
policy. TASK-007 may supply its authenticated command cancellation/deadline state
without changing the port and may check it before calling `open_source`. The storage
operation checks it before initial admission, after admission/before dispatch, before staging creation, before every
read, before every write, after every hash result and immediately before promote.
If stopped before promote, the call performs the exact §9 cleanup and returns
`IngestOutcome::Stopped`; cleanup uncertainty returns an error instead. Once the
no-replace promote syscall begins there is no cancellation checkpoint: the worker
must finish synchronization and return the stored/error result. A currently running
APFS syscall is not claimed to be preemptible.

The result types have private fields and only these read-only accessors:

```rust
pub struct DurableBlob { /* digest, byte_length, location */ }
impl DurableBlob {
    pub const fn digest(&self) -> Sha256Digest;
    pub const fn byte_length(&self) -> u64;
    pub const fn location(&self) -> &DurableLocationDescriptor;
}

pub struct DurableLocationDescriptor { /* backend_id, locator */ }
impl DurableLocationDescriptor {
    pub fn backend_id(&self) -> &str;
    pub fn locator(&self) -> &str;
}

pub struct BlobStartupReport { /* backend_id, orphan_count, orphan_bytes, state */ }
impl BlobStartupReport {
    pub fn backend_id(&self) -> &str;
    pub const fn staging_orphan_count(&self) -> u16;
    pub const fn staging_orphan_bytes(&self) -> u64;
    pub const fn ingest_state(&self) -> BlobIngestState;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlobIngestState { Ready, OrphanReconciliationRequired }
```

The count is bounded `0..=4096`; bytes are checked `u64`. Reports contain no path,
filename or digest. An invalid namespace fails `start` rather than producing a
partial report.

The locator is exactly `sha256-v1/<aa>/<bb>/<64-lowercase-hex>.blob` and remains
unchanged when a storage root moves because it is relative to a verified backend
instance. The backend ID and move semantics are fixed in §8.1. Neither value exposes
the root. `Debug`/`Display` for result types are omitted or redacted.

This contract is not a product API and has no pagination, authentication or command
idempotency claim. Byte idempotency is content-addressed: repeating identical bytes
returns the same digest/locator after re-verifying the existing object. TASK-007
owns command idempotency and the product deadline duration, but no longer needs to
change the storage interface to propagate deadline/cancellation.

## 5. TASK-004 Library namespace compatibility

The current TASK-004 namespace would reject the canonical default as soon as the
storage directory exists. The implementation must centralize, not duplicate, one
filtered SQLite-state enumerator with these rules:

1. Without `storage`, every existing TASK-004 state and result remains byte-for-byte
   unchanged.
2. `storage` is accepted only beside the exact complete state
   `.mengxia.lock + library.sqlite3`; it is not accepted in empty, lock-only,
   intent, staging, sidecar or lock-missing states.
3. The entry must be a no-follow directory on the same local APFS volume, owner equal
   to the durable Library owner/eUID, mode `0700`, empty ACL and ownership checking
   enabled. A file, symlink, mount change, wrong UID/mode/ACL or changed inode fails
   `STORAGE_CONFIGURATION_ERROR` without mutation.
4. Store reopen and `sync_closed_canonical_database` validate the directory identity
   before and after SQLite finalization but do not inspect or mutate its children.
   Blob-root code owns its internal namespace.
5. No arbitrary additional Library-root name is accepted. The rule does not make
   root enumeration permissive and does not authorize storage initialization before
   canonical SQLite bootstrap is complete.

`TEST-NAMESPACE-005` proves the complete state with and without safe `storage` passes,
all negative combinations fail, and Library-root enumeration rejects on the first
entry beyond the largest accepted TASK-004/TASK-005 state without accumulating an
attacker-sized vector. The formal aggregate runs the retained TASK-004 gate once via
`verify-task-003.sh`; this test must not invoke that full gate again. This
compatibility change is normative only after the canonical documents explicitly
incorporate this supplement.

## 6. Blob-root and source authority

### 6.1 Blob root

`mengxia-platform-fs` may expose opaque `BlobRootRequest` and
`OpenedBlobRootAuthority` types. `BlobRootRequest` is fallibly constructed from the
already validated absolute Blob-root bytes and has no raw-path accessor.
`OpenedLibrary::authorize_blob_root(request)` is the only production minting seam:
while the Library lock is live it supplies the retained Library-root authority,
durable UUID and durable owner directly to platform code. Copyable
`OpenedLibraryIdentity`, caller UID text or configuration cannot mint the grant.

The returned authority contains retained Library/Blob directory identities, the
Blob-root lock, a private strong Library-lock lease and fixed-child operations. The
platform crate may refactor the existing non-public lock owner into a private shared
lease, but neither that lease nor a clone operation is public. The Library lock is
released only after both SQLite ownership and every minted Blob authority are gone,
so accidental store-first shutdown cannot admit a second daemon while CAS work is
alive.

The authority exposes no `Path`, `PathBuf`, string, raw fd, Library lock or general
`openat` method. Only `mengxia-storage-local` may consume it, enforced by architecture
tests. This narrow method is the only allowed TASK-004 public-interface addition; it
cannot open SQLite, read DB bytes or authorize an arbitrary child.

The root and every retained prefix use the accepted TASK-004 descriptor-first walk:
open from `/`, reject symlinks at every component, re-open every name edge, compare
device/inode, require local APFS with ownership checking enabled, and apply the
accepted ancestor/final-parent ACL policy. The Blob root is owner UID, mode `0700`,
empty ACL. Internal directories are `0700`; locks/blobs/staging files are regular,
non-symlink, owner UID, mode `0600`, empty ACL and link count exactly one. Every
internal directory must remain on the root device.

An absent root is created only through the retained final-parent descriptor with
mode `0700`, then opened no-follow, inspected, `F_FULLFSYNC`ed and followed by
`F_FULLFSYNC(final parent)` before lock creation. An existing safe empty root is
accepted. A returned root `mkdir` followed by SIGKILL is therefore the exact empty
recoverable state in §11; failures never authorize a different basename.

The root is exclusively bound to one Library by this exact lock basename, derived
from the already durable UUID bytes without entropy or clock access:

```text
.mengxia-cas-v1-<32 lowercase hex UUID bytes>.lock
```

The file is created only in an otherwise empty safe root, opened `O_EXCL|O_NOFOLLOW`,
locked non-blockingly and retained for the full storage lifetime. Creation order is:
create; lock; inspect; `F_FULLFSYNC(lock)`; `F_FULLFSYNC(root)`; revalidate; enumerate
under lock. An existing exact lock is re-used after validation. A different Library
lock name, missing lock beside data or any unknown top-level entry fails closed as
configuration; contention returns `CONFLICT` without waiting or stealing. The lock
is never replaced or deleted.

The only other top-level entries are:

```text
.staging-v1/
sha256-v1/
```

The lock and both directory names are exact byte names even on case-insensitive
APFS: after create/open, the retained-root plus bounded descriptor-derived
`F_GETPATH` proof used in §8 must show the requested component bytes exactly. A case
alias is an unknown namespace entry, not a recoverable prefix.

Lock-only, lock plus an **empty** `.staging-v1`, and the complete three-entry state
are the only recoverable ordered initialization prefixes. If `sha256-v1` is absent,
the staging directory must be empty: its first entry other than the literal `.` or
`..` marker fails closed without
creating CAS state. Missing directories are created one at a time, inspected and
followed by `F_FULLFSYNC` of the new directory and parent. Any other partial/unknown
state is preserved and rejected. A valid complete root has at most three entries
excluding only the literal `.` and `..` markers; dot-prefixed lock/staging names are
ordinary counted entries. Enumeration reads one entry at a time and rejects immediately upon reading
a fourth; “at most four” is not an accepted state. It never allocates a list
proportional to an attacker-controlled directory. Shard directories are opened by
fixed digest components on demand and are not recursively enumerated at startup.

### 6.2 Stable local source

`LocalBlobStorage::open_source` uses its retained authority and treats a source path
as untrusted bytes. It applies the §4.1 byte/component caps, rejects NUL and requires an
absolute lexically canonical path, walks from an opened `/` descriptor, opens every
directory and the final component with no-follow/close-on-exec flags and retains the
final read-only regular-file descriptor. Final symlinks, prefix symlinks, devices,
directories, sockets, FIFOs and non-local mounts are rejected. Source paths inside
the Library root or Blob root are rejected by retained component identity, not only
string prefix. A standalone copyable Library identity is insufficient to perform
this check or open a source.

The opaque capability records the initial device, inode, type, length, modification
time and change time at the successful end of `open_source`; none is publicly
accessible. At the beginning of `ingest`, after its first control checkpoint and
before admission, the same descriptor and retained name edge are revalidated against
that baseline. A change is `BlobStorageError::SourceModified` and creates no
reservation or staging name. That baseline length is the sole `declared_length` used
for admission and streaming.

Reads use a descriptor-relative offset operation so a shared file offset cannot
redirect the stream. The initial descriptor stat freezes `declared_length`. The
stream requests exactly `min(buffer_length, declared_length - offset)` and never
reads or writes beyond that length. A positive short read is progress; a zero read
while bytes remain is premature EOF and `SOURCE_MODIFIED_DURING_INGEST`. After the
declared bytes, the implementation performs exactly one logical one-byte read at
offset `declared_length`: zero is the required EOF and one byte is late data and
`SOURCE_MODIFIED_DURING_INGEST`. Interrupted calls may retry at most
`MAX_INTERRUPTED_SYSCALL_RETRIES = 8` times per logical read, write or EOF probe;
the counter resets only after positive progress. Exhaustion maps to storage I/O.
Writes loop until the bytes already read are complete; a zero write is storage I/O,
and a chunk enters the hash state only after its complete staging write succeeds.

Before and after streaming the same descriptor must have equal device, inode,
regular-file type, byte length, modification time and change time; both lengths must
equal `declared_length`. After streaming, the retained component chain is revalidated
and the final name is reopened no-follow through the retained parent; it must still
resolve to the original device/inode and metadata. Thus rename/unlink/replacement is
detected even when the original open descriptor's bytes did not change. Any mismatch,
missing name or premature/late EOF is `SOURCE_MODIFIED_DURING_INGEST`. A successful
or failed ingest never writes, renames, chmods, changes ACL/owner or unlinks the
source.

A stable external source is not required to have link count one: a pre-existing hard
link does not weaken copy-mode custody when the selected retained name edge and
metadata remain stable. Adding/removing a link changes `ctime` and must fail the
operation; replacing the selected name edge fails even if another hard link still
keeps the inode alive. The one-link requirement remains mandatory for staging,
canonical CAS and lock files.

The storage primitive accepts a zero-byte regular source. It produces the standard
SHA-256 empty digest and byte length zero, still executes the EOF probe, stability,
durability and publish checks, and reserves zero payload bytes while retaining all
fixed admission/free-reserve checks. Product-level “empty import” policy belongs to
TASK-007; changing this primitive decision requires a recorded decision update.

This detects ordinary concurrent mutation. As in ADR-0004, root, administrator and
same-eUID malicious processes are inside the V1 host trust boundary; the contract
does not claim it can defeat a same-eUID attacker that deliberately modifies bytes
and forges/restores metadata.

## 7. Bounded streaming, admission and capacity

`LocalBlobStorage` creates exactly the configured number of joined I/O workers and
hash workers. It has no admitted-input queue. Under one admission mutex, a call must
atomically acquire: one of `max_concurrent_ingests` slots, one currently idle I/O
worker, one currently idle hash worker, its logical staging reservation and its
physical remaining-byte reservation. If any is unavailable, the call returns
`BACKPRESSURE` **before** staging mutation and transfers no ownership to a worker.
This is the sole queue/permit backpressure point.

After that atomic point the job is admitted and dispatch targets the already
reserved workers; a full/closed internal channel, worker refusal or lost reply is an
`INTERNAL_ERROR`, never a second `BACKPRESSURE`. Every admitted caller remains joined
to one terminal result. An I/O worker owns source reads, staging writes and filesystem
transitions. Its reserved hash worker processes at most one outstanding chunk for
that ingest and returns the incremental SHA-256 state and same buffer before reuse.
No worker is shared with another admitted ingest until the current operation reaches
a terminal result. This makes effective concurrency the minimum of the three
configured bounds without creating an implicit queue.

All slots, reservations and channel ownership are released exactly once on every
normal/error/stopped result. A worker/channel panic closes new admission, completes
best-effort cleanup for affected work, joins the remaining workers and maps the
runtime to `INTERNAL_ERROR`; it is never silently replaced and no caller is abandoned.

Hashing uses `sha2` 0.11.0 incrementally. The same chunk read from the source is
written to staging and fed to one SHA-256 state before reuse. Memory per call is no
more than one configured byte buffer plus fixed metadata/hash state; total media
buffer memory is bounded by `max_concurrent_ingests * stream_buffer_bytes`. No media
byte enters SQLite, Protobuf, logs or an error.

Admission uses `fstatvfs` on the retained staging directory. One mutex owns these
checked `u128` counters:

```text
observed_orphan_bytes
active_written_bytes
active_remaining_bytes
```

`observed_orphan_bytes` is the size of valid prior-process staging entries.
`active_written_bytes + active_remaining_bytes` is the original declared length of
all admitted calls. A successful write of `n` atomically subtracts `n` from remaining
and adds `n` to written; neither total nor ownership changes. With checked `u128`
arithmetic:

```text
available = u128(f_bavail) * u128(f_frsize)
total = u128(f_blocks) * u128(f_frsize)
reserve = max(min_free_bytes, ceil(total * min_free_percent / 100))
```

`f_bavail`, not root-only `f_bfree`, is authoritative. While holding the same mutex,
a new length is admitted only if all of these hold:

```text
declared_length <= max_ingest_bytes
observed_orphan_bytes + active_written_bytes + active_remaining_bytes
    + declared_length <= max_staging_bytes
available >= reserve + active_remaining_bytes + declared_length
```

The physical expression deliberately does not add observed orphans or active written
bytes because current `available` already excludes their allocated blocks; it does
include every other active call's remaining reservation. The logical expression
includes both. The `fstatvfs` sample, comparisons and counter update occur inside the
same admission critical section, so concurrent admissions cannot oversubscribe one
snapshot. Exact cap is allowed; cap+1 or any conversion/add/multiply/ceil overflow
fails closed before mutation.

Before every write, another `fstatvfs` sample must satisfy
`available >= reserve + active_remaining_bytes` while the counters are locked. This
preserves already admitted reservations if unrelated processes consume space.
ENOSPC/EDQUOT, short/zero-write exhaustion or reserve loss cannot yield a durable
descriptor. A failed call removes its active reservation only after §9 cleanup has a
known result: a revalidated retained staging file becomes observed orphan bytes;
confirmed unlink removes it from active accounting. Any cleanup operation failure or
identity uncertainty changes `Running` to `Failed` before releasing the admission
mutex, so an estimate can never authorize another ingest. Restart performs the only
subsequent authoritative enumeration.

Startup enumerates at most 4096 staging entries. Entry 4097, arithmetic overflow,
wrong filename/type/owner/mode/ACL/link count or unreadable metadata fails closed.
The fixed safety ceiling prevents a hostile/corrupt directory from causing
unbounded startup work; changing it requires gate review. Valid prior-process
staging files count at their logical length against the aggregate ceiling. Any
nonzero orphan count is reported as `OrphanReconciliationRequired`, but ingest may
use safely remaining capacity. If prior-process bytes alone make a requested length
permanently inadmissible, the result is `RecoveryRequired`, not retryable
`BACKPRESSURE`; blind retry cannot make TASK-005-owned state disappear.

TASK-005 introduces only three fixed, finite safety constants beyond ADR-0005:

| Constant | Value | Purpose |
|---|---:|---|
| `MAX_STAGING_NAME_ATTEMPTS` | 8 | bound entropy-name collision work before any copy |
| `MAX_OBSERVED_STAGING_ENTRIES` | 4096 | bound restart enumeration CPU/file-descriptor work |
| `MAX_INTERRUPTED_SYSCALL_RETRIES` | 8 | bound per-logical-operation EINTR work |

Acceptance must record these in the ADR-0005 TASK-005 clarification and its negative
tests. They are correctness/abuse bounds, not throughput SLOs. Widening any value
requires a recorded decision and cap-boundary regression evidence.

The 1/10/100 GiB O(buffer) evidence uses a deterministic generated reader, the real
incremental SHA-256 implementation and a bounded counting/discard filesystem seam,
so CI processes every byte but does not allocate or persist 100 GiB.
Separate real-APFS tests exercise actual files, full sync, capacity faults and
publish at practical sizes. Sparse-file metadata alone is not accepted as memory
evidence, and this task makes no throughput/latency SLO claim.

## 8. Staging, hashing and no-clobber durable promote

Staging names are `.ingest-<32 lowercase random hex>.part`. Exactly 16 bytes come
from fallible OS entropy. Creation is `O_EXCL|O_NOFOLLOW`; at most eight collisions
are attempted. Entropy failure is `EntropyUnavailable`; eight successful entropy
draws that each collide are the distinct `StagingNamespaceUnavailable` condition.
Both happen before source copying, but they have the separate stable mappings in
§10. The returned staging descriptor, name and inode are retained and revalidated
before any unlink or promote.

For a new digest, the exact ordered success path is:

1. acquire all permits and reserve declared logical bytes;
2. create/inspect staging, then `F_FULLFSYNC(.staging-v1)` so its name is observable;
3. stream once, enforcing size/free-space bounds and incrementally hashing;
4. require exact EOF and post-read source stability; compare an optional expected
   digest before any publish;
5. require `F_FULLFSYNC(staging file)` and revalidate its name/inode/length;
6. create/open the exact two lowercase digest shard directories descriptor-relative;
   prove each exact lowercase basename as specified below, inspect
   device/owner/mode/ACL and `F_FULLFSYNC` each new directory and parent;
7. use `rustix::fs::renameat_with(..., RenameFlags::NOREPLACE)` from staging to the
   absent canonical filename; ordinary replacing `rename` is forbidden;
8. re-open canonical no-follow, require the same inode and metadata, then
   `F_FULLFSYNC(destination directory)` and `F_FULLFSYNC(.staging-v1)`;
9. only after every step returns success construct `DurableBlob`.

The source and destination directories are retained descendants of one Blob root and
must share the root device, so `EXDEV` is a failed invariant/configuration, never a
copy fallback.

APFS may be case-insensitive, so a successful descriptor-relative lookup alone is
not exact-name evidence. For both shard levels and the final blob, every newly
created and `EEXIST` path is opened no-follow and then checked with rustix's safe,
bounded macOS `F_GETPATH` support: the descriptor-derived final component bytes and
retained-parent prefix must equal the requested lowercase bytes exactly. The code
then reopens the name from the retained parent and matches device/inode. A requested
`aa` resolving to pre-existing `AA`, at either shard or blob level, fails
`STORAGE_CONFIGURATION_ERROR` without rename, deduplication or cleanup of foreign
state. The proof is required on both mkdir-success and mkdir-`EEXIST` paths and on
newly promoted/existing final files; string-only prechecks are forbidden.

The per-prefix state machine is exact: absent first shard may be created/synced;
exact existing first shard may be validated; alias/wrong-type/symlink/device mismatch
fails without touching the second shard. Only then is the identical rule applied to
the second shard. Only an exact validated two-level prefix may receive promote.

If no-replace returns `EEXIST`, the adapter first proves the exact lowercase final
basename, then opens the canonical file no-follow and
re-hashes it with the same bounded algorithm while checking before/after metadata.
It then reopens the fixed digest name through the retained shard descriptor and
requires the same inode. Only an exact digest, length and name binding match is dedup
success. Then the adapter may unlink
its own staging name only after proving that name still identifies the exclusively
created inode, followed by `F_FULLFSYNC(.staging-v1)`. A mismatch is
`STORAGE_CORRUPTION`; canonical and staging evidence are preserved. No path ever
overwrites a canonical digest name.

### 8.1 Stable Location identity and root moves

The locator is permanently the relative value defined in §4.2. It does not change
when the configured root path changes. A backend ID distinguishes storage instances
without exposing a path and is derived once at `start` as:

```text
"mengxia.local-cas.v1/" + lowercase_hex(
  SHA256("mengxia.local-cas-instance-v1\0" ||
         library_uuid_16_bytes ||
         root_st_dev_as_u64_be || root_st_ino_as_u64_be))
)
```

The accepted macOS `st_dev` and `st_ino` values must convert losslessly to `u64`;
conversion failure is configuration error. Both the ASCII backend ID and locator
are exactly 85 bytes; their literal prefixes, separators, lowercase-hex alphabet and
component lengths have parser/golden tests. Renaming the same retained root inode on the same
volume keeps both backend ID and locator stable; only runtime configuration changes.
Copying/recreating the directory or moving to another volume creates a different
backend ID even if bytes are identical.

TASK-005 never rewrites persisted Locations. TASK-006 must store the exact opaque
backend ID and locator with bounded format checks. TASK-007's future root-move flow
must open and validate the new root, verify every referenced digest at its unchanged
locator, and transactionally update only affected `Location.backend_id` values. A
missing/corrupt object aborts without a partial rebind. Until that explicit flow,
Locations bound to a previous backend ID are unavailable rather than silently
resolved under the current path. Digest, Asset identity and locator never change as
a side effect of moving a root.

The software durability contract requires all listed synchronization calls to
return successfully. Apple documents that ordinary `fsync` can leave drive writes
reordered and that `F_FULLFSYNC` requests a permanent-media flush; therefore this
task uses `rustix::fs::fcntl_fullfsync` on macOS rather than silently weakening to
`fsync`. Initialization probes the actual Blob-root file and directory support and
fails before ingest if unsupported. The accepted local APFS host probe on 2026-08-26
returned success for a directory descriptor.

The gate does not claim that CI physically removed power or that every storage
controller honors flush. Actual hardware power-loss evidence remains
`UNVERIFIABLE`; same-running-OS SIGKILL visibility and exact syscall order are the
mandatory reproducible claims. Apple and POSIX sources:

- <https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fsync.2.html>
- <https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html>
- <https://pubs.opengroup.org/onlinepubs/9799919799/xrat/V4_xbd_chap01.html>
- <https://developer.apple.com/documentation/foundation/urlresourcevalues/volumesupportsexclusiverenaming>
- <https://pubs.opengroup.org/onlinepubs/9799919799/functions/rename.html>

## 9. Failure cleanup, orphans and shutdown

Within the still-running call, failure before publish may unlink only the staging
entry created exclusively by that call after name-to-inode, owner, mode, type and
link-count revalidation. Cleanup is exactly `revalidate → unlinkat(name) →
F_FULLFSYNC(.staging-v1)`. A failure before `unlinkat` returns preserves the entry
when identity was re-proved; a revalidation failure makes its length untrusted. Once `unlinkat`
returns success the name is absent in the same-OS model and accounting removes those
bytes even if the following full sync fails; that post-unlink sync failure still
returns `CleanupFailed`/`STORAGE_IO_ERROR` and must not claim the name was preserved.
Every revalidation, unlink or post-unlink sync failure atomically moves the runtime to
`Failed`; the current caller receives `CleanupFailed`, later calls receive `Internal`,
and only shutdown/restart may re-establish capacity truth. Cleanup never
broadens the target or scans by prefix.

After process death, TASK-005 never automatically deletes a staging file. A valid
prior-process staging entry is reported as an orphan and charged to capacity; an
invalid entry fails closed. A canonical file created before a lost response is a
safe unregistered orphan and is never deleted. TASK-008 may later verify/reconcile;
TASK-022 may later delete only after OQ-008 and its own authorization.

Shutdown is an explicit joined state machine: close admission; complete or roll back
every already admitted job; close bounded channels; join I/O workers; join hash
workers; revalidate/synchronize the root; release the Blob-root lock last. There is
no queued pre-admission job; every dispatched job is already admitted and may not be
discarded. Worker panic/join failure returns
`INTERNAL_ERROR` after best-effort joining and leaves the runtime failed closed.
Drop performs the same join and cannot release the root lock while work remains.

The admission mutex also owns `Running | Closing | Failed`. The admission
linearization point is the single successful counter/worker reservation update in
`Running`; the shutdown linearization point is `Running → Closing` under that same
mutex. A call that has not linearized before `Closing` gets `ShuttingDown`; a call
that has linearized owns its work and shutdown waits for its terminal reply. `Failed`
rejects every new call as `Internal`. Resource unavailability is examined only in
`Running`, so a shutdown race cannot be mislabeled `Backpressure`.

Composition must first stop product admission, join every caller waiting on storage,
shut storage, then shut the SQLite owner and release the Library lock. TASK-007 owns
that daemon integration. TASK-005 verifies no job, channel or thread survives
shutdown and no admitted caller loses ownership without a terminal result. A
negative lifecycle test deliberately shuts SQLite first while retaining the Blob
authority: reopening the Library must still return contention until storage drops,
then succeed. This proves lock lifetime rather than relying only on call order.

## 10. Exact error mapping and redaction

The public error shapes are exact and non-exhaustive only for downstream match
compatibility; implementation variants below may not be merged:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BlobSourceError { InvalidPath, UnsupportedType, Io, Modified }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BlobStorageError {
    Validation,
    SourceModified,
    Io,
    Corruption,
    Configuration,
    RecoveryRequired,
    Conflict,
    Backpressure,
    EntropyUnavailable,
    StagingNamespaceUnavailable,
    CleanupFailed,
    ShuttingDown,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlobRetryClass {
    AfterInputChange,
    AfterSourceStabilizes,
    AfterStorageConditionChanges,
    NeverAutomatically,
    AfterOperatorConfigurationChange,
    AfterOperatorReconciliation,
    AfterOwnerExit,
    FreshAdmissionWithBoundedDelay,
    AfterPlatformConditionChanges,
    SameRuntimeForbidden,
}

impl BlobSourceError {
    pub const fn code(&self) -> ErrorCode;
    pub const fn retry_class(&self) -> BlobRetryClass;
}
impl BlobStorageError {
    pub const fn code(&self) -> ErrorCode;
    pub const fn retry_class(&self) -> BlobRetryClass;
}
```

Each stores no path/source bytes, digest, arbitrary OS string or nested caller error.
Both implement `std::error::Error`; their derived `Debug` contains only the variant
name. They have no conversion from a boxed/arbitrary error and no source chain.
The only `Display` strings, in declaration order, are:

```text
invalid source path
unsupported source type
source access failed
source changed during ingest
blob input validation failed
source changed during ingest
blob storage operation failed
blob storage integrity verification failed
blob storage configuration is unsupported or unsafe
blob storage requires orphan reconciliation
blob storage is already open
blob storage admission is full
blob staging identifier generation is unavailable
blob staging namespace is unavailable
blob staging cleanup did not complete durably
blob storage is shutting down
blob storage internal invariant failed
```

Adapter code maps a source error by variant without formatting or retaining it. The
stable code and retry contract are:

| Variant / condition | Stable code | Retry class |
|---|---|---|
| source `InvalidPath`, `UnsupportedType`; storage `Validation` including max-ingest cap+1 | `VALIDATION_ERROR` | `AfterInputChange` |
| source `Modified`; storage `SourceModified` | `SOURCE_MODIFIED_DURING_INGEST` | `AfterSourceStabilizes` |
| source `Io`; storage `Io`, including read/write/sync/open permission, ENOSPC/EDQUOT and reserve loss before a known cleanup result | `STORAGE_IO_ERROR` | `AfterStorageConditionChanges` |
| storage `Corruption`, including expected-digest mismatch, canonical mismatch or unstable canonical | `STORAGE_CORRUPTION` | `NeverAutomatically` |
| storage `Configuration`, including invalid DTO, unsafe namespace/ACL/owner/mount/device/layout or unsupported primitive | `STORAGE_CONFIGURATION_ERROR` | `AfterOperatorConfigurationChange` |
| storage `RecoveryRequired`, when preserved orphans alone prevent the request | `STORAGE_CONFIGURATION_ERROR` | `AfterOperatorReconciliation` |
| storage `Conflict`, exact lock already owned | `CONFLICT` | `AfterOwnerExit` |
| storage `Backpressure`, solely failure of the atomic initial admission due to active slots/workers/reservations | `BACKPRESSURE` | `FreshAdmissionWithBoundedDelay` |
| storage `EntropyUnavailable`, OS entropy call fails | `ID_GENERATION_UNAVAILABLE` | `AfterPlatformConditionChanges` |
| storage `StagingNamespaceUnavailable`, eight independently generated names collide | `STORAGE_CONFIGURATION_ERROR` | `AfterOperatorReconciliation` |
| storage `CleanupFailed`, any staging revalidation/unlink/post-unlink-sync failure | `STORAGE_IO_ERROR` | `SameRuntimeForbidden` |
| storage `ShuttingDown`, call arrives after admission closes | `STORAGE_IO_ERROR` | `SameRuntimeForbidden` |
| storage `Internal`, poison/overflow after validation, worker/channel panic or admitted dispatch/reply failure | `INTERNAL_ERROR` | `SameRuntimeForbidden` |

`retry_class()` returns exactly those closed values; it is context guidance, not an
automatic retry loop. In particular, accepted internal work can never return
`BACKPRESSURE`, and preserved orphan capacity or eight collisions are not treated as
transient queue pressure.

No local storage condition is mapped to `STORAGE_BUSY` (SQLite-only),
`DEADLINE_EXCEEDED` (current local IPC contract), Provider or Plugin codes. Logs and
metrics contain only stable code, bounded counts/bytes/duration and safe operation
phase. Absolute/source/CAS paths, filenames, raw errno text and digest labels are
not metric labels or ordinary user messages.

Canonical synchronization adds no new stable `ErrorCode`, but must make four existing
taxonomy sources precise: `BACKPRESSURE` becomes “bounded queue or transient atomic
admission capacity full”; `CONFLICT` includes an already-owned validated Library/Blob
root lock; `STORAGE_CONFIGURATION_ERROR` includes required orphan reconciliation,
exhausted staging-name namespace and unsafe case alias; and `STORAGE_IO_ERROR`
includes cleanup uncertainty and local storage shutdown for which the same runtime
cannot be retried. The
existing API exposure/log/metric redaction remains unchanged. Physical ENOSPC,
EDQUOT and free-reserve loss remain storage I/O, not queue pressure.

## 11. Deterministic crash and fault matrix

SIGKILL tests kill a child on the same still-running macOS immediately after the
named syscall returned. They do not permit power-loss rollback of a returned call.

The following names and numbers are stable test API; implementation checkpoints use
the symbolic name, never an ordinal alone:

| ID / symbolic point | Exact state visible to restart | Required result |
|---|---|---|
| `KILL-005-001 BEFORE_ROOT_MKDIR` | absent safe target | no mutation; retry may initialize |
| `KILL-005-002 AFTER_ROOT_MKDIR` | empty safe root exists | initialize from empty root |
| `KILL-005-003 AFTER_ROOT_SYNC` | empty safe root exists | initialize from empty root |
| `KILL-005-004 AFTER_LOCK_CREATE_LOCK` | lock-only | validate/acquire/sync and continue |
| `KILL-005-005 AFTER_LOCK_FILE_SYNC` | lock-only | validate/acquire/sync and continue |
| `KILL-005-006 AFTER_LOCK_ROOT_SYNC` | lock-only | continue after locked re-enumeration |
| `KILL-005-007 AFTER_STAGING_MKDIR` | lock plus empty `.staging-v1` | validate/sync and continue |
| `KILL-005-008 AFTER_STAGING_SYNCS` | same ordered prefix | continue |
| `KILL-005-009 AFTER_CAS_MKDIR` | complete three-entry state | validate/sync and continue |
| `KILL-005-010 AFTER_CAS_SYNCS` | complete root | continue |
| `KILL-005-011 AFTER_ADMISSION` | complete root only | no orphan; reservation dies with process |
| `KILL-005-012 AFTER_STAGING_CREATE` | one zero-length exact staging entry | report/count one orphan; delete nothing |
| `KILL-005-013 AFTER_STAGING_NAME_SYNC` | same entry | report/count one orphan; delete nothing |
| `KILL-005-014 AFTER_FIRST_CHUNK_WRITE` | exact first positive written length | report/count orphan |
| `KILL-005-015 AFTER_MIDDLE_CHUNK_WRITE` | exact accumulated middle length | report/count orphan |
| `KILL-005-016 AFTER_FINAL_CHUNK_WRITE` | exact declared length | report/count orphan; no canonical |
| `KILL-005-017 AFTER_EOF_PROBE` | complete staging bytes | report/count orphan; no canonical |
| `KILL-005-018 AFTER_SOURCE_REVALIDATE` | complete staging bytes | report/count orphan; no canonical |
| `KILL-005-019 AFTER_STAGING_FILE_SYNC` | durable complete staging | report/count orphan; no canonical |
| `KILL-005-020 AFTER_FIRST_SHARD_MKDIR` | first exact shard plus staging | validate prefix; report orphan |
| `KILL-005-021 AFTER_FIRST_SHARD_SYNCS` | same prefix | validate prefix; report orphan |
| `KILL-005-022 AFTER_SECOND_SHARD_MKDIR` | exact two-level prefix plus staging | validate prefix; report orphan |
| `KILL-005-023 AFTER_SECOND_SHARD_SYNCS` | same prefix | validate prefix; report orphan |
| `KILL-005-024 AFTER_NOREPLACE_RENAME` | canonical exists; own staging name absent | validate canonical; safe unregistered orphan |
| `KILL-005-025 AFTER_DESTINATION_SYNC` | canonical only | validate; retry deduplicates |
| `KILL-005-026 AFTER_STAGING_DIR_POST_PROMOTE_SYNC` | canonical only | retry deduplicates |
| `KILL-005-027 AFTER_EEXIST_VERIFY` | canonical plus own staging | report staging orphan after restart |
| `KILL-005-028 AFTER_DEDUP_STAGING_UNLINK` | canonical; staging absent | retry deduplicates |
| `KILL-005-029 AFTER_DEDUP_STAGING_SYNC` | canonical; staging absent | retry deduplicates |
| `KILL-005-030 BEFORE_SUCCESS_REPLY` | canonical durable; no staging | retry deduplicates; no false rollback |

Returned namespace syscalls remain visible to the next process in this SIGKILL model;
power-loss alternatives are not accepted outcomes. The independent deterministic
fault seam has this equally stable registry:

| Fault ID | Injection / mandatory assertion |
|---|---|
| `FAULT-005-001 BEFORE_ROOT_MKDIR` | fail before root mkdir; no mutation |
| `FAULT-005-002 AFTER_ROOT_MKDIR` | fail after returned mkdir; exact empty recoverable root |
| `FAULT-005-003 BEFORE_ROOT_SYNC` | empty root remains; no lock |
| `FAULT-005-004 AFTER_ROOT_SYNC` | synced empty root remains; no lock |
| `FAULT-005-005 BEFORE_LOCK_CREATE` | empty root remains |
| `FAULT-005-006 AFTER_LOCK_CREATE_AND_LOCK` | exact lock-only recoverable state |
| `FAULT-005-007 BEFORE_LOCK_FILE_SYNC` | lock-only; no internal directory |
| `FAULT-005-008 AFTER_LOCK_FILE_SYNC` | lock-only; no internal directory |
| `FAULT-005-009 BEFORE_LOCK_ROOT_SYNC` | lock-only; no internal directory |
| `FAULT-005-010 AFTER_LOCK_ROOT_SYNC` | lock-only; no internal directory |
| `FAULT-005-011 BEFORE_LOCKED_REENUMERATE` | no continuation from pre-lock snapshot |
| `FAULT-005-012 AFTER_LOCKED_REENUMERATE` | exact state classification or fail closed |
| `FAULT-005-013 BEFORE_STAGING_MKDIR` | lock-only |
| `FAULT-005-014 AFTER_STAGING_MKDIR` | lock plus empty staging only |
| `FAULT-005-015 BEFORE_STAGING_CHILD_SYNC` | same ordered prefix |
| `FAULT-005-016 AFTER_STAGING_PARENT_SYNC` | fully synced ordered prefix |
| `FAULT-005-017 BEFORE_CAS_MKDIR` | require staging empty or refuse mutation |
| `FAULT-005-018 AFTER_CAS_MKDIR` | complete top-level state |
| `FAULT-005-019 BEFORE_CAS_CHILD_SYNC` | complete top-level state, initialization error |
| `FAULT-005-020 AFTER_CAS_PARENT_SYNC` | complete initialized state |
| `FAULT-005-021 BEFORE_SOURCE_WALK` | no source/root mutation |
| `FAULT-005-022 AFTER_SOURCE_OPEN` | retained source closes; no source mutation |
| `FAULT-005-023 BEFORE_SOURCE_REVALIDATE` | staging cleanup follows exact authority |
| `FAULT-005-024 AFTER_SOURCE_REVALIDATE` | stable or source-modified; no source mutation |
| `FAULT-005-025 FIRST_READ_EINTR_EXHAUSTED` | eight retries then bounded I/O error |
| `FAULT-005-026 MIDDLE_READ_EINTR_PROGRESS` | retry counter resets only after positive progress |
| `FAULT-005-027 EOF_PROBE_EINTR_RESULT` | one logical bounded probe; zero/one-byte mapping exact |
| `FAULT-005-028 POSITIVE_SHORT_READ` | exact remaining request and one-copy hash behavior |
| `FAULT-005-029 PREMATURE_ZERO_READ` | source-modified and authorized cleanup |
| `FAULT-005-030 ENTROPY_UNAVAILABLE` | no name/mutation; entropy-specific variant |
| `FAULT-005-031 EIGHT_NAME_COLLISIONS` | no copy; namespace-specific variant |
| `FAULT-005-032 FIRST_CHUNK_SHORT_WRITE` | loop/account/hash exactly once |
| `FAULT-005-033 MIDDLE_CHUNK_SHORT_WRITE` | loop/account/hash exactly once |
| `FAULT-005-034 FINAL_CHUNK_SHORT_WRITE` | loop/account/hash exactly once |
| `FAULT-005-035 ZERO_WRITE` | I/O error and exact cleanup/accounting |
| `FAULT-005-036 WRITE_EINTR_EXHAUSTED` | eight retries then bounded I/O error |
| `FAULT-005-037 BEFORE_STAGING_FILE_SYNC` | no canonical; cleanup exact |
| `FAULT-005-038 AFTER_STAGING_FILE_SYNC` | durable staging; no canonical |
| `FAULT-005-039 BEFORE_PREPROMOTE_REVALIDATE` | no canonical; cleanup exact |
| `FAULT-005-040 AFTER_PREPROMOTE_REVALIDATE` | exact inode/name or corruption; no premature promote |
| `FAULT-005-041 BEFORE_FIRST_SHARD_MKDIR_OPEN` | no shard mutation |
| `FAULT-005-042 AFTER_FIRST_SHARD_MKDIR_OPEN` | exact-case/type proof required; no second shard |
| `FAULT-005-043 BEFORE_FIRST_SHARD_SYNCS` | no second shard |
| `FAULT-005-044 AFTER_FIRST_SHARD_SYNCS` | first prefix durable; no second shard |
| `FAULT-005-045 BEFORE_SECOND_SHARD_MKDIR_OPEN` | first prefix only |
| `FAULT-005-046 AFTER_SECOND_SHARD_MKDIR_OPEN` | exact-case/type proof required; no final file |
| `FAULT-005-047 BEFORE_SECOND_SHARD_SYNCS` | no final file |
| `FAULT-005-048 AFTER_SECOND_SHARD_SYNCS` | full prefix durable; no final file |
| `FAULT-005-049 BEFORE_NOREPLACE_RENAME` | staging preserved; no canonical mutation |
| `FAULT-005-050 NOREPLACE_ERROR` | existing/absent state classified without overwrite |
| `FAULT-005-051 AFTER_NOREPLACE_RENAME` | canonical present; staging name absent |
| `FAULT-005-052 CANONICAL_REOPEN_OR_CASE_PROOF` | exact name/inode or fail closed |
| `FAULT-005-053 BEFORE_DESTINATION_SYNC` | canonical may exist; call returns error |
| `FAULT-005-054 AFTER_DESTINATION_SYNC` | canonical synced; call not yet successful |
| `FAULT-005-055 BEFORE_POSTPROMOTE_STAGING_SYNC` | canonical exists; staging absent |
| `FAULT-005-056 AFTER_POSTPROMOTE_STAGING_SYNC` | durable success state |
| `FAULT-005-057 BEFORE_EEXIST_VERIFY` | canonical and own staging preserved |
| `FAULT-005-058 AFTER_EEXIST_VERIFY` | exact match or corruption; no foreign mutation |
| `FAULT-005-059 BEFORE_DEDUP_STAGING_REVALIDATE` | both names preserved |
| `FAULT-005-060 AFTER_DEDUP_STAGING_REVALIDATE` | only exact own inode is unlink-authorized |
| `FAULT-005-061 CLEANUP_REVALIDATE_FAILURE` | staging preserved but identity/length untrusted; runtime failed closed |
| `FAULT-005-062 CLEANUP_UNLINK_FAILURE` | revalidated staging preserved/accounted; runtime failed closed |
| `FAULT-005-063 CLEANUP_POSTUNLINK_SYNC_FAILURE` | staging absent, `CleanupFailed`, counters removed, runtime failed closed |
| `FAULT-005-064 CAPACITY_SAMPLE_OR_ARITHMETIC_FAILURE` | no over-admission or mutation |
| `FAULT-005-065 ADMITTED_DISPATCH_CHANNEL_CLOSED` | `Internal`, never `Backpressure`; terminal reply |
| `FAULT-005-066 HASH_WORKER_PANIC_OR_REPLY_LOSS` | failed-closed runtime, joins and exact cleanup |
| `FAULT-005-067 IO_WORKER_PANIC_OR_REPLY_LOSS` | failed-closed runtime, joins and exact cleanup |
| `FAULT-005-068 SHUTDOWN_OR_JOIN_FAILURE` | lock retained until surviving work joins/drops |
| `FAULT-005-069 AFTER_ROOT_CHILD_SYNC_BEFORE_PARENT_SYNC` | exact empty root; parent-sync ordering remains observable |
| `FAULT-005-070 ROOT_PARENT_SYNC_FAILURE` | exact empty recoverable root; no lock creation |
| `FAULT-005-071 AFTER_STAGING_CHILD_SYNC_BEFORE_PARENT_SYNC` | lock plus exact empty staging prefix only |
| `FAULT-005-072 STAGING_PARENT_SYNC_FAILURE` | same recoverable prefix; CAS creation forbidden in the failed call |
| `FAULT-005-073 AFTER_CAS_CHILD_SYNC_BEFORE_PARENT_SYNC` | complete top-level names; initialization not reported successful |
| `FAULT-005-074 CAS_PARENT_SYNC_FAILURE` | complete recoverable top-level state; no ingest admission |
| `FAULT-005-075 AFTER_FIRST_SHARD_CHILD_SYNC_BEFORE_PARENT_SYNC` | first exact shard only; no second shard |
| `FAULT-005-076 FIRST_SHARD_PARENT_SYNC_FAILURE` | first exact shard prefix; promote forbidden |
| `FAULT-005-077 AFTER_SECOND_SHARD_CHILD_SYNC_BEFORE_PARENT_SYNC` | exact two-shard prefix; no final file |
| `FAULT-005-078 SECOND_SHARD_PARENT_SYNC_FAILURE` | exact two-shard prefix; promote forbidden |

Removing, renumbering or making a point unreachable is a proposal change. Tests
assert call ordering, cleanup authority, counters, terminal reply and static error
mapping, not merely the final return code.

## 12. Stable acceptance criteria

### `AC-074` — configuration and compatibility

All TASK-005 values are parsed into one source-free immutable DTO with exact ADR-0005
bounds before mutation. The canonical default/custom-root rules hold, and the narrow
TASK-004 `storage` compatibility matrix passes without weakening other states.

### `AC-075` — descriptor authority and root ownership

Blob root, fixed Library binding lock, internal directories/files and local source
are opened descriptor-first with the exact no-follow/APFS/UID/mode/ACL/device rules.
No raw path/fd/lock/general child opener escapes the infrastructure boundary.

### `AC-076` — stable, non-destructive source

A regular local source is streamed only through its retained handle; before/after
identity and the single bounded EOF probe detect deterministic mutation. Partial,
zero and interrupted I/O follow §6.2; zero-byte input has the recorded primitive
result. Source bytes and metadata are unchanged on success, failure, collision,
retry and injected crash.

### `AC-077` — bounded verified streaming

SHA-256 and exact length are computed in one bounded stream. Cooperative control is
checked at every §4.2 boundary without detached work. Formal 1/10/100 GiB generated
evidence proves O(configured buffer) memory, and no media bytes enter DB/protocol/logs.

### `AC-078` — finite admission and capacity

One atomic admission enforces ingest/I/O/hash concurrency, single-ingest size,
historical plus every active reservation, actual available space and reserve with
concurrent cap−1/cap/cap+1 and checked-`u128` evidence. Accepted work is never
internally rejected as backpressure. Rejection creates no canonical descriptor.

### `AC-079` — durable no-clobber CAS publish

The exact lowercase layout (including case-insensitive APFS proof), incremental
digest, `F_FULLFSYNC` ordering and no-replace atomic promote yield a canonical file
only after verification. Concurrent identical input deduplicates; collision with
different bytes fails corruption and overwrites nothing. Backend ID/locator and
root-move ownership follow §8.1.

### `AC-080` — crash/orphan safety

Every named §11 SIGKILL/fault point has the exact restart result. Cleanup distinguishes
pre-unlink preservation from post-unlink sync failure. Prior-process staging and
unregistered canonical orphans are preserved/reported, counted safely and never
auto-deleted by TASK-005.

### `AC-081` — error, lifecycle and architecture boundary

Exact variants, retry classes and redacted static messages follow §10. Work is
synchronous, non-detached and permit/lock lifetimes are bounded. No product operation, DB registration,
Asset/Location ownership, source deletion, GC, CLI/Plugin CAS access or later scope
exists.

### Requirement-to-acceptance closure

| Governing ID | TASK-005 claim | AC / mandatory evidence |
|---|---|---|
| `FUNC-002` | storage primitive only; product feature remains incomplete | AC-074, AC-075, AC-076, AC-077, AC-078, AC-079, AC-080, AC-081; TASK-007 deferred |
| `DATA-002` | digest-verified durable local Location descriptor exists before return | AC-077, AC-079; `TEST-STREAM-005`, `TEST-PROMOTE-005`, `TEST-LOCATION-005` |
| `DATA-003` | physical byte custody can precede registration; no registration is present here | AC-079, AC-080; `TEST-PROMOTE-005`, `TEST-RECOVERY-005` |
| `DATA-004` | media stays in source→buffer→file/hash path | AC-077, AC-081; `TEST-STREAM-005`, `TEST-ARCH-005` |
| `DATA-013` | primitive cannot assert Managed state; TASK-006/007 remain owners | AC-081; `TEST-ARCH-005`, `TEST-DOC-005` |
| `PERF-001` | memory O(buffer), including 1/10/100 GiB logical cases | AC-077; `TEST-STREAM-005` |
| `SEC-017` | every path, config, filename, size, metadata and descriptor edge is bounded/validated | AC-074, AC-075, AC-076, AC-077, AC-078, AC-079, AC-080; CONFIG/PATH/SOURCE/ERROR tests |
| `SEC-020` | only existing pinned/minimally featured dependencies and accepted macOS primitives are used | AC-081; `TEST-SUPPLY-005`, `TEST-ARCH-005` |
| `REL-001` | all internal channels and admissions are finite; overload rejects only at the initial atomic gate | AC-078, AC-081; `TEST-RESOURCE-005`, `TEST-CONCURRENCY-005`, `TEST-LIFECYCLE-005` |
| `REL-004` | restart classifies durable staging/canonical prefixes and never retries/deletes unknown effects blindly | AC-080; `TEST-RECOVERY-005`, `TEST-ORPHAN-005` |
| `REL-006` | bounded cancellation/deadline propagation seam; no detached job | AC-077, AC-081; `TEST-CONTROL-005`, `TEST-LIFECYCLE-005` |
| `SEC-021` | finite workers, queues, sizes, staging entries, free reserve and retries | AC-078, AC-081; RESOURCE/CONCURRENCY tests |
| `CFG-001`, `CFG-003` | immutable DTO validation only; production precedence remains TASK-007 | AC-074; `TEST-CONFIG-005`, `TEST-DOC-005` plus future TASK-007 gate |

## 13. Stable test registry

| Test ID | Mandatory evidence |
|---|---|
| `TEST-CONFIG-005` | every path/numeric source case, 1023-byte source and 937-byte Blob-root headroom, default/custom overlap, config/authority mismatch, cap−1/cap/cap+1, overflow and no-mutation proof |
| `TEST-NAMESPACE-005` | bounded Library enumeration plus exact safe/unsafe `storage` state matrix and shutdown/reopen; no nested invocation of a retained full-task gate |
| `TEST-PATH-005` | prefix/final symlink races, type/UID/mode/ACL/mount/device changes, internal-root source denial, stable fd; mixed-case aliases at both shard levels/final file on create and EEXIST |
| `TEST-SOURCE-005` | before/mid/after mutation, truncate/grow/replace, stable pre-existing hard link versus link/name-edge mutation, zero-byte policy and exact source non-mutation |
| `TEST-STREAM-005` | SHA-256 vectors, expected-digest match/mismatch, positive-short/zero/EINTR read/write, single EOF probe and buffer instrumentation |
| `TEST-CONTROL-005` | cancel/deadline at pre-admission, post-admission, pre-staging, every read/write/hash, pre-promote and post-promote non-interruption boundary |
| `TEST-RESOURCE-005` | all permit caps; concurrent historical/written/remaining/new reservations; logical/physical cap−1/cap/cap+1; ENOSPC/EDQUOT; checked-u128; 4096/4097 entries |
| `TEST-PROMOTE-005` | exact lowercase layout/locator, full-sync trace, NOREPLACE, identical dedup, hostile existing canonical, no EXDEV fallback |
| `TEST-LOCATION-005` | exact 85-byte backend/locator format; real-APFS same-inode rename/copy/recreate; checked metadata seam proves changed/cross-volume `st_dev` identity; TASK-006/007 ownership contract |
| `TEST-RECOVERY-005` | every named §11 child-process SIGKILL point and fixed fault ID on real APFS/fault seam |
| `TEST-ORPHAN-005` | valid/invalid staging observation, capacity accounting, canonical orphan retry, zero prior-process deletion |
| `TEST-CONCURRENCY-005` | at least two sources racing one/different digests and physical reserve; atomic one-time admission, one winner, no oversubscription/second backpressure/deadlock |
| `TEST-ERROR-005` | exhaustive condition→code/retry class/static display mapping, cleanup-failed runtime closure and path/errno/digest/media redaction |
| `TEST-LIFECYCLE-005` | every admitted caller gets one terminal result; control panic with known cleanup does not poison storage; worker panic/channel loss and cleanup uncertainty do; shutdown joins work and Library lock outlives storage authority |
| `TEST-ARCH-005` | dependency directions, port surface, no unsafe/new package/DB/proto/daemon/CLI/Plugin/GC/later symbols, CAS path hidden |
| `TEST-SUPPLY-005` | exact locked sha2 0.11.0, rustix 1.1.4, getrandom 0.4.3, offline build, cargo-deny, no ambient system hash/rename tool |
| `TEST-DOC-005` | stable IDs, governing requirement/decision mapping, TASK-004 compatibility, TASK-006/007/008/022 ownership and lifecycle markers |

`scripts/verify-task-005.sh` is the only named TASK-005 gate driver and requires one
mode argument:

- `developer` runs the fast deterministic subset of all seventeen IDs, formatting,
  targeted check/Clippy/tests, document/naming checks and `git diff --check`. It omits
  the generated 1/10/100 GiB trio, full child-process SIGKILL matrix and prior-task
  aggregate. It prints `FAST_PASS` per covered subset, never the canonical `PASS`
  token used by the start record; it is feedback, never completion evidence.
- `formal` runs all seventeen IDs including real-APFS/SIGKILL evidence and the complete
  generated 1/10/100 GiB byte streams in the optimized test profile with one
  preallocated configured buffer; then calls `scripts/verify-task-003.sh`
  exactly once (that script already aggregates TASK-001, TASK-002 and TASK-004), and
  runs any remaining workspace-wide all-target/all-feature offline baseline exactly
  once. Supply-chain, naming/traceability and diff checks must likewise have only one
  recorded invocation in this gate graph.

The existing main `macos-26` arm64 CI job replaces its three separate TASK-001/004/003
steps with `scripts/verify-task-005.sh formal`; the real-second-UID TASK-003 job remains
unchanged. The main job name advances through TASK-005 and its timeout becomes 180
minutes; individual formal subcommands retain explicit finite timeouts. A local
developer PASS cannot stand in for formal real-APFS CI evidence,
and the 100 GiB evidence is mandatory before completion but not on every edit.

## 14. Dependency and supply-chain contract

The implementation may add only existing workspace-pinned dependencies to
`mengxia-storage-local`: `mengxia-platform-fs`, `sha2 = 0.11.0`,
`rustix = 1.1.4` and `getrandom = 0.4.3`. `mengxia-ports` may use the existing
`mengxia-types` dependency. No system `shasum`, `openssl`, shell, ambient `PATH`,
custom C, new FFI, SQLite, Tokio, async-trait, Serde or new crate version is needed.

`rustix` already exposes safe macOS `renameat_with(NOREPLACE)`, descriptor
`getpath`/`F_GETPATH`, `fstatvfs`, `fsync` and `fcntl_fullfsync`; all ordinary TASK-005 Rust crates remain
`#![forbid(unsafe_code)]`. Any discovery that these safe APIs cannot implement an
accepted requirement is a new blocker and must return to gate review, not authorize
an ad-hoc unsafe block.

## 15. Downstream non-regression and deferrals

- TASK-006 still depends on TASK-004 and TASK-005 and owns Blob/Location domain and
  persistence. It consumes only the verified descriptor/opaque locator.
- TASK-007 still depends on TASK-003 and TASK-006 and owns the product command,
  production config resolution, authorization, command idempotency, deadline,
  cancellation, daemon/CLI integration and durability-before-DB transaction.
  Canonical synchronization must add `CFG-001`/`CFG-003` consumption to its Plan
  row; its future gate must assign a stable test to the real four-layer resolver and
  daemon startup, not reuse `TEST-CONFIG-005` as false production evidence.
- TASK-008 owns startup/deep verification and reconciliation; it receives no delete
  authority from this proposal.
- TASK-022 remains blocked on OQ-008 and exclusively owns removal/GC/Purge.
- CLI, Plugin and Provider code never imports `mengxia-storage-local` or receives a
  Blob-root/source-opening capability.

The graph remains acyclic. TASK-005's new dependency on the already completed
TASK-004 is a prerequisite correction, not an edge from TASK-004 back to storage.
`mengxia-platform-fs` remains a downward-only leaf and cannot depend on ports,
storage, store, application or binaries.

## 16. Authorization sequence

### 16.1 Copy-ready canonical start record

The following record was copied into the Implementation Plan by the reviewed
activation revision that changed every lifecycle marker together. ADR-0007 and the
pre-start canonical synchronization remain the governing contract; this block is
now active while the lifecycle is `IN_PROGRESS`:

```text
### TASK-005 start record — 2026-08-26

STATUS: IN_PROGRESS
SCOPE: TASK-005 ONLY — Local BlobStorage/CAS custody primitive; no product ingest,
       database registration, domain Location persistence, deletion or GC.

FEATURE: FUNC-002 (storage precondition only; feature remains incomplete)
REQUIREMENTS:
  DATA-002, DATA-003, DATA-004, DATA-013, PERF-001, REL-001, REL-004, REL-006,
  SEC-017, SEC-020, SEC-021,
  CFG-001, CFG-003, BASE-009, BASE-011, BASE-013, BASE-014, BASE-015,
  BASE-016, BASE-017, BASE-018
DECISIONS:
  ADR-0002, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0007
PREREQUISITES: TASK-002 DONE; TASK-004 DONE
ACCEPTANCE:
  AC-074, AC-075, AC-076, AC-077, AC-078, AC-079, AC-080, AC-081
TESTS:
  TEST-CONFIG-005, TEST-NAMESPACE-005, TEST-PATH-005, TEST-SOURCE-005,
  TEST-STREAM-005, TEST-CONTROL-005, TEST-RESOURCE-005, TEST-PROMOTE-005,
  TEST-LOCATION-005, TEST-RECOVERY-005, TEST-ORPHAN-005,
  TEST-CONCURRENCY-005, TEST-ERROR-005, TEST-LIFECYCLE-005, TEST-ARCH-005,
  TEST-SUPPLY-005, TEST-DOC-005
DEVELOPER_GATE: scripts/verify-task-005.sh developer (FAST_PASS only)
FORMAL_COMPLETION_GATE: scripts/verify-task-005.sh formal (all TEST IDs PASS)
AUTHORIZED_FILES: proposal §3.1 exact list and symbol restrictions
FORBIDDEN: proposal §3.2; TASK-006 and later remain unauthorized
```

That activation changes this proposal's top status to exactly
`ACCEPTED / ACTIVE TASK-005` and synchronizes these marker values in Specification,
Decisions, Review, Plan, Intake and `AGENTS.md`:

```text
TASK005_CANONICAL_GATE: ACCEPTED
TASK005_SPECIFICATION_VERSION: 1.1.18
TASK005_LIFECYCLE: IN_PROGRESS
TASK005_IMPLEMENTATION_AUTHORITY: TASK_005_ONLY
TASK005_PROPOSAL: docs/proposals/TASK-005-GATE-PROPOSAL.md
```

The historical contract version remains 1.1.18 unless activation discovers a real
contract defect; a later document version alone does not rewrite that marker.

### 16.2 Ordered activation

The pre-start review, canonical synchronization, exact start-record activation,
document gate and retained TASK-003 aggregate are complete. The remaining ordered
path is:

1. implement only TASK-005 under the active record;
2. pass all gates, review the complete diff, record evidence and mark TASK-005 DONE;
3. stop. Do not automatically start TASK-006.

The activation is complete; the current status is:

```text
TASK-005: IN_PROGRESS
IMPLEMENTATION AUTHORITY: TASK-005 ONLY
```

### 16.3 Local completion candidate

On 2026-08-27 the exact implementation passed `scripts/verify-task-005.sh developer`
and a complete local APFS `scripts/verify-task-005.sh formal` candidate run. The
latter executed every stable TEST mapping, all 30 KILL points, 78 named fault seams,
the generated 1/10/100 GiB O(buffer) evidence, supply-chain checks, retained prior
task gates and the workspace-wide offline baseline. Final diff review found no path
outside §3.1 after relocating the CAS/recovery integration tests into the authorized
`mengxia-storage-local/tests/**` boundary.

This is not a completion record. The accepted gate explicitly requires reviewed
`macos-26` formal CI evidence for the same committed candidate. Until that evidence
exists, TASK-005 remains `IN_PROGRESS`, implementation authority remains
`TASK_005_ONLY`, and TASK-006 remains unauthorized.
