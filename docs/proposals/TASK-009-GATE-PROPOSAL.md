---
title: "TASK-009 creative-intent and Asset lifecycle start-gate proposal"
project: "梦夏 / MengXia"
document_role: "Accepted TASK-009 implementation supplement"
status: "ACCEPTED_INCORPORATED_BY_CANONICAL_SPECIFICATION_1_1_35"
version: "0.1.5"
date: "2026-09-09"
canonical_specification_reviewed: "IMPLEMENTATION_SPEC.md v1.1.32"
repository_head_reviewed: "ba3af720d6e968778c97e9efcb4d5ea9b414db25"
---

# TASK-009 Gate Proposal

## 0. Gate verdict

TASK-009 is complete under this accepted implementation supplement and canonical
Specification v1.1.35. Independent review found no unresolved blocker, the complete
local developer/formal gates passed, and exact head
`fa7a0047c95c8b8eba12e859284223a1a78f51e2` passed reviewed `macos-26` CI.

```text
TASK009_CANONICAL_GATE: ACCEPTED
TASK009_LIFECYCLE: DONE
TASK009_IMPLEMENTATION_AUTHORITY: NONE
TASK009_PROPOSAL_VERSION: 0.1.5
TASK009_INDEPENDENT_REVIEW: PASS_2026_09_09
TASK009_UNRESOLVED_BLOCKING_FINDINGS: NONE
```

Version 0.1.4 records the narrow `REPO_STALE / SPEC_STALE` protocol-fixture
correction found during the full implementation baseline: v1.2 byte identity belongs
to the frozen TASK-008 fixture, while the current v1.3 source/descriptor/provenance
belong to TASK-009. It also adds the three mechanically required files omitted from
the v0.1.3 whitelist. This correction changes no product, protocol or migration
semantics and does not reopen completed TASK-008 implementation authority.

Version 0.1.5 records the additional retained-formal-gate correction found only
after the recursive developer gate passed: the TASK-003 CLI integration fixture
starts the current all-operation daemon and therefore must supply TASK-009's
production decode-depth floor 5, while its client requests continue to exercise
the retained protocol-1.0 depth-3 contract. This adds one test script to §3 and
changes no production, wire, migration or completed-task semantics.

This supplement does not authorize edits to migration 0000/0001 or any file outside
§3. The accepted SQL candidate may become migration 0002 only byte-for-byte and only
through the migration/recovery contract below. TASK-010+, Admin, root rebind,
Provider/Plugin, Credential, Rights and destructive behavior remain unauthorized.

## 1. Authority inputs and verified repository reality

Authority order read for this draft:

1. `docs/spec/IMPLEMENTATION_SPEC.md` v1.1.31;
2. `docs/spec/DECISIONS.md` v0.3.32 and ADR-0001 through ADR-0011;
3. `docs/spec/IMPLEMENTATION_REVIEW.md` v1.1.42;
4. `docs/spec/IMPLEMENTATION_PLAN.md` v0.3.42;
5. `docs/spec/PROJECT_INTAKE_REPORT.md` v1.3.37;
6. root `AGENTS.md` (the only applicable instruction file);
7. completed TASK-006/007/008 code, migrations and formal evidence.

Fresh pre-draft evidence:

- clean `main` at `ba3af720d6e968778c97e9efcb4d5ea9b414db25`;
- local `scripts/verify-repository.sh docs`: PASS;
- immutable `0001_library_assets.sql`: 12,733 bytes and the ADR-0008 digest;
- `commands.result_kind` accepts only `ASSET`, `ASSET_REVISION`, `LOCATION`;
- `domain_events.aggregate_kind` accepts only the four TASK-006 aggregate kinds;
- existing pure SQLite commands already provide atomic claim/mutation/event/outcome;
- no Project, Subject, WorkRevision, Take, relationship or TASK-009 wire type exists.

The missing TASK-009 implementation is `EXPECTED_GAP`. The restrictive completed
ledger is not stale; it was correct for TASK-006. Its forward extension is the
pre-start design obligation recorded by `REVIEW-GAP-005`.

## 2. Blocking gaps and proposed resolutions

### 2.1 Extensible CommandRecord and DomainEvent outcomes

Classification: `EXPECTED_GAP / DATA_MODEL / MIGRATION`.

Migration 0002 rebuilds `commands` and `domain_events` exactly once under the
exclusive Library owner/startup migration boundary. It preserves every old column
and byte, adds a bounded versioned result payload to `commands`, makes new result
kind tokens syntactically extensible, adds a bounded optional event payload, and
makes aggregate kind tokens syntactically extensible. Existing result/event rows
retain null payloads and their exact TASK-006/007/008 validation and replay paths.

New successful results use:

```text
result_kind               closed operation-owned token
result_id                 optional primary typed UUIDv7 (16 bytes)
result_schema_version     1
result_payload            exact fixed-layout bytes, 1..512
result_payload_sha256     SHA-256(result_payload)
```

For every non-legacy result, the versioned payload is the authoritative complete
result. `result_id` is a nullable convenience projection: TASK-009's UUID-primary
results require it and require it to equal the primary canonical row selected by the
closed operation codec/rehydration query, while a future accepted Blob-digest,
no-primary-object or composite result may
leave it null without another ledger-table rebuild. A future result may not place a
32-byte digest or arbitrary bytes in this UUID column; those bytes belong in its
versioned payload. The three legacy kinds `ASSET`, `ASSET_REVISION` and `LOCATION`
retain their exact non-null 16-byte `result_id` and payload-free constraints.

The schema permits future uppercase tokens without another table rebuild, but the
running binary keeps a closed operation/result/codec registry. Unknown tokens,
wrong operation-kind pairs, non-canonical payloads, hash mismatch, a TASK-009 result
with a missing/wrong primary UUID, missing canonical rows or missing/mismatched
events are `STORAGE_CORRUPTION`; they are never returned as an opaque success. Old
result kinds may not carry a new payload.

`domain_events` retains one Library-wide commit allocator and append-only table.
New event payloads are at most 2,048 bytes and require a matching SHA-256. Known
events have a closed event/aggregate/payload matrix. Ordinary mutation APIs cannot
update or delete either old or new events.

This resolves `REVIEW-GAP-005` without parallel ledgers, per-result-kind tables or
reuse of a semantically false legacy result kind. It requires proposed ADR-0012 and
must re-run all existing Asset/Location/materialize replay evidence.

### 2.2 Migration 0002 is a destructive schema rewrite

Classification: `MIGRATION / DATA_INTEGRITY`.

The candidate DDL is
`docs/proposals/TASK-009-0002-CANDIDATE.sql`, exactly 18,681 bytes with SHA-256
`dc95fcfee381d07834e14975a0fdacd0874de9c6512c72ff0ac04777e07522d1`.
It has been parsed against a 0000+0001 database and passed `foreign_key_check` and
`integrity_check`; this diagnostic does not authorize application.

On acceptance, the production filename is exactly `0002_projects_work.sql`, the
stored `schema_migrations.migration_name` is exactly `0002_projects_work` (without
the extension), and the stored checksum covers the complete accepted SQL file bytes
without normalization. Sequence is exactly 2.

Before running it, the daemon holds the TASK-004 Library lock, admits no worker,
checkpoints/truncates WAL, closes SQLite, and creates one durable owner-only
pre-0002 snapshot using the exact fixed names:

```text
.mengxia-migration-0002.intent
.library.sqlite3.pre-0002.snapshot.staging
.library.sqlite3.pre-0002.snapshot
```

Every artifact is a no-follow regular file on the already validated local APFS
Library device, owned by the durable owner UID, mode `0600`, with no extended ACL,
and the expected link count. The retained root remains
mode `0700` with its accepted empty-ACL/ownership protections. Security validation
is repeated after each create/write/link/unlink and before any cleanup; a mismatch
preserves evidence and fails closed.

The immutable intent is created first with no-follow/no-clobber semantics and
identifies the Library, canonical database inode, migration sequence/name, source
migration-prefix digest and a fresh typed migration UUID. It is both the pre-commit
intent and the post-commit immutable restore manifest: it is retained for exactly as
long as the final snapshot and is never deleted independently. After intent and root
full-sync, the migration performs the exact checked-u128 capacity calculation below.

Let `S` be the exact closed source database length, `P` the validated 4,096-byte page
size, `C` the exact legacy command count, `E` the exact legacy DomainEvent count and
`R=max(min_free_bytes, ceil(volume_bytes*min_free_percent/100))` using the already
resolved Library-wide TASK-005 reserve inputs. Define:

```text
G = P * (128 + 8*C + 10*E)
J = 2*S + 67_108_864
required_available = R + S + G + J
```

`G` is the deliberately conservative main-database growth allowance: two pages per
table/index B-tree entry, covering the four command B-trees and five DomainEvent
B-trees, plus 128 fixed schema/root pages. Every migrated legacy row is proven below
one page before this calculation. `J` bounds a rollback journal containing every
original database page plus framing and fixed slack. The exclusive migration
connection must use `journal_mode=DELETE`, `synchronous=FULL` and
`temp_store=MEMORY`; read-back mismatch fails before `BEGIN`. The accepted SQL drops
the old secondary indexes, creates their replacements on empty `_v2` tables and then
copies rows, so it performs no data-proportional external sort. On commit it restores
and reads back the accepted WAL mode/configuration before normal reopen. The
temporary `library.sqlite3-journal` is accepted only with a valid intent during this
closed migration phase and follows SQLite rollback recovery; it must be absent after
successful recovery/commit before endpoint publication.

A preflight arithmetic/conversion overflow is `STORAGE_CONFIGURATION_ERROR` before
snapshot bytes are written. `available < required_available`, ENOSPC/EDQUOT or
reserve loss after preflight is `STORAGE_IO_ERROR` with the existing
storage-condition retry contract; it rolls back, preserves the intent/final snapshot
evidence and never publishes the endpoint. Fault tests must measure actual high-water
database/journal bytes for maximum-size legacy rows and prove they do not exceed
these terms; the constants are accepted safety ceilings, not an SLO.

The composition root resolves `MENGXIA_MIN_FREE_BYTES` and
`MENGXIA_MIN_FREE_PERCENT` once and passes the same immutable typed values to both
Store migration and BlobStorage; neither subsystem rereads configuration. This adds
no new key, but it does require extending `StoreConfig` and ordering resolution
before `OpenedLibrary::open_or_bootstrap`.

The closed database is copied O(buffer) to staging while hashing;
copy stops at the retained source length, rejects premature EOF or an extra byte,
and proves source device/inode/length plus the exact pre-copy `st_mtime`,
`st_mtime_nsec`, `st_ctime` and `st_ctime_nsec` are unchanged before publication.
Access time and birth time are deliberately excluded because reading the source may
change access time and neither field is part of the existing source-change
fingerprint.
Staging is full-synced and re-read, then opened only through a dedicated snapshot
validator using `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_URI | SQLITE_OPEN_NOFOLLOW |
SQLITE_OPEN_NOMUTEX | SQLITE_OPEN_PRIVATECACHE | SQLITE_OPEN_EXRESCODE`. It
constructs an ASCII `file:` URI
from the already authorized absolute path by percent-encoding every path byte except
`/` and RFC-3986 unreserved bytes, appends exactly `?mode=ro&immutable=1`, accepts no
caller URI/query input, enables and reads back `query_only`, and never runs a write
pragma. Pre/post descriptor stat and directory enumeration must prove the same
inode/length/hash and absence of `-wal`, `-shm` or `-journal`. This narrowly scoped
URI open is forbidden for the mutable canonical database. Because stock SQLite
accepts a pathname rather than a caller-owned directory descriptor, this immutable
URI open is the sole non-descriptor-relative namespace open: it is enclosed by the
same whole-prefix edge revalidation used by the accepted stock SQLite opener, plus
fixed-child, owner/mode/ACL/device/inode/link-count, pre/post hash and sidecar proofs.
Implementing a custom VFS or adding FFI to make this open descriptor-relative is not
authorized.

After exact 0000+0001 validation, staging is published by a descriptor-relative
no-replace hard link. Before `linkat`, staging alone has link count 1. After returned
`linkat`, staging and final must be the same inode with link count 2; root full-sync
precedes revalidation and unlink of staging. After returned unlink and the next root
full-sync, final alone has link count 1. Restart accepts the final snapshot only when
a fresh hash of the still-exact 0001 canonical database matches a fresh hash of the
snapshot and both independently pass the immutable old-prefix validator. The intent
is never rewritten in place, so no partially updated phase can be mistaken for
authority.

Only then may SQLite reopen, set `foreign_keys=OFF` before `BEGIN IMMEDIATE`, execute
the exact candidate, copy old rows, run `foreign_key_check`, complete the migration
row, commit, restore `foreign_keys=ON`, and run the full schema/row/event/outcome
validator. Any failure rolls back and retains the intent/snapshot. A successful
migration removes only a proven staging name after durable verification; the
owner-only final snapshot and its immutable intent/manifest remain the inseparable
explicit restore source for the previous binary. No automatic restore RPC is added.
A future retention/Admin task owns a separately accepted durable retirement state
machine for both retained artifacts; one fixed pair is accepted here, not an
unbounded series.

The connection reads back `foreign_keys=OFF` before `BEGIN` and reads back `ON`
after commit; inability to prove either state closes the connection and publishes no
endpoint. Migration-row `applied_at` is one typed clock sample taken after snapshot
verification and immediately before the transaction. It is distinct from immutable
intent creation time; a rolled-back attempt stores no row and a later retry may
resample it. All migration-row identity/timestamp validation remains fail closed.

Rollback is an offline operator procedure, not a daemon fallback: stop every process,
retain the Library lock authority, verify the snapshot against the intent and old
prefix, atomically replace the canonical database from a separately verified copy,
full-sync it, then move the migration intent, staging name and retained snapshot out
of the Library root into an owner-only evidence directory before launching the old
binary. The old TASK-008 binary accepts only its exact root namespace and therefore
must never be started while any TASK-009 migration artifact remains in that root.

Startup recognizes only these exact states:

| Canonical prefix | Intent/manifest | snapshot namespace | Action |
|---|---|---|---|
| exact 0001 | absent | absent | begin migration |
| exact 0001 | valid | absent | recreate staging and restart copy |
| exact 0001 | valid | staging only, link count 1 | validate complete staging or truncate/restart an owned partial copy |
| exact 0001 | valid | staging + final, same inode/link count 2 | validate, full-sync root, unlink staging, full-sync root |
| exact 0001 | valid | final only, link count 1 | immutable-validate snapshot, then apply 0002 |
| exact 0002 | valid | final only, link count 1 | validate exact 0002 and retained pair; normal open |
| exact 0002 | valid | staging + final | impossible accepted prefix; preserve and fail closed |
| exact 0002 | absent | any | fail closed; restore authority is missing |
| any prefix | invalid/unknown | any artifact | fail closed without cleanup |
| any prefix | absent | staging or final present | fail closed; origin unproved |
| any prefix | valid | differing staging/final inode or wrong link count | fail closed without cleanup |
| any prefix | valid | snapshot hash/schema mismatch | `STORAGE_CORRUPTION`; preserve evidence |

The canonical SQLite sidecar sub-state is independently closed:

| Intent/manifest | WAL/SHM | rollback journal | Action |
|---|---|---|---|
| absent | accepted TASK-004 pair/absence | absent | ordinary exact-0001 pre-migration classification |
| valid | accepted TASK-004 pair/absence | absent | run the accepted bounded WAL classifier first; recovered exact 0001 is checkpointed/truncated and closed before the DELETE-mode migration, while recovered exact 0002 requires the retained-pair validator and restoration/read-back of WAL mode before normal open |
| valid, closed migration transaction | absent | owner-only canonical `library.sqlite3-journal` | bundled SQLite rollback recovery; require exact 0001 or exact committed 0002, remove/reject the journal under SQLite's result, then follow the preceding valid-intent row |
| any | any WAL name | any journal name simultaneously | fail closed; mixed journal authority |
| absent/invalid | any | journal present | fail closed without cleanup |
| valid | any | symlink/wrong owner/mode/ACL/inode class | fail closed without cleanup |

The existing canonical `library.sqlite3-wal/-shm` pair continues through the
accepted WAL classifier and is not migration-snapshot state. Snapshot and intent
names never have SQLite sidecars; any `*.snapshot-wal`, `*.snapshot-shm`,
`*.snapshot-journal`, unknown case variant, symlink or extra root entry fails
closed. Ordinary read-only SQLite opening is forbidden because it can create
WAL/SHM for a WAL-mode file; only the immutable URI contract above may inspect the
snapshot. Enumeration and every namespace create/write/link/unlink remain
descriptor-relative and exact-case; the fixed immutable SQLite URI open is only the
explicitly bounded exception described above. In particular, exact 0002 with a
valid retained intent/snapshot and an accepted canonical WAL/SHM pair is a normal
restart state, not mixed journal authority.

With a valid migration intent, startup first validates root/artifact authority. A
canonical WAL/SHM prefix from before the journal-mode switch uses the already
accepted bounded WAL recovery; a rollback journal from the closed migration
transaction uses bundled SQLite rollback recovery. WAL and rollback-journal state
may never coexist. Recovery runs with no worker admission and must end with no
journal before the canonical prefix is classified as exact 0001 or exact 0002. A
committed migration is never rolled back merely because the retained manifest still
exists; an uncommitted transaction must recover to exact 0001. Missing disposable
SHM and a WAL ending at the last valid committed frame follow SQLite's accepted
recovery semantics, while permission/configuration errors and definite corruption
stay distinct and fail closed.

The immutable migration intent is exactly 512 bytes. Integers are big-endian;
timestamp seconds are signed i64 and every other integer is unsigned. Reserved bytes
are zero.

| Offset | Width | Field |
|---:|---:|---|
| 0 | 16 | ASCII `MENGXIA_MIG2_V1\0` |
| 16 | 2 | version = 1 |
| 18 | 2 | record length = 512 |
| 20 | 4 | flags = 0 |
| 24 | 16 | migration attempt UUIDv7 |
| 40 | 16 | Library UUIDv7 |
| 56 | 8 | retained Library-root device |
| 64 | 8 | retained Library-root inode |
| 72 | 8 | source canonical database device |
| 80 | 8 | source canonical database inode |
| 88 | 8 | source database byte length |
| 96 | 32 | SHA-256 of the closed exact-0001 database file |
| 128 | 32 | SHA-256 of the ordered 0000+0001 migration-row identity tuple |
| 160 | 32 | accepted 0002 SQL SHA-256 |
| 192 | 8 | creation timestamp Unix seconds |
| 200 | 4 | creation timestamp nanoseconds |
| 204 | 2 | migration sequence = 2 |
| 206 | 2 | migration-name length = 18 |
| 208 | 32 | ASCII migration-name slot: `0002_projects_work` then 14 zero bytes |
| 240 | 240 | zero reserved |
| 480 | 32 | SHA-256 of bytes 0..480 |

The source file is hashed before intent creation while SQLite is closed and the
Library lock/root authority are retained. A golden fixture, offset/EOF/checksum
tests, typed UUID/timestamp checks and one-bit mutation of every field are mandatory.
Device/inode conversion overflow fails before any namespace mutation.

The migration-prefix digest at offset 128 is:

```text
SHA-256(
  "MENGXIA_MIGRATION_PREFIX_V1\0" || u16_be(2) ||
  for rows ordered by migration_sequence:
    u16_be(sequence) || u16_be(name_byte_length) || name_utf8 || sha256[32] ||
    i64_be(applied_at_seconds) || u32_be(applied_at_nanos)
)
```

Only the exact accepted 0000 and 0001 identities are valid input.

Same-OS SIGKILL tests cover every acknowledged namespace and SQLite transaction
boundary. Fault seams prove syscall/SQLite ordering. Power-loss recovery is not
inferred from SIGKILL; macOS full-sync follows the already accepted platform policy.

### 2.3 Canonical creative model is not executable as written

Classification: `SPECIFICATION / PARTIALLY_SPECIFIED`.

This proposal fixes the V1 meanings:

- Project is Library-global work/policy context, never a tenant or owner of Asset or
  Subject identity. New projects have effective trust `UNTRUSTED`; TASK-009 has no
  trust-escalation operation and migration 0002 deliberately stores no parallel
  trust column because migration 0004/TASK-013 owns the future authoritative
  `project_trust` records.
- ProjectSpecRevision and WorkRevision are immutable. Their parent Project/WorkItem
  has an optimistic `revision` and a current-revision pointer updated atomically.
- Subject is a Library-global semantic identity. TASK-009 supports create and
  bounded list only; no merge/delete/rename operation is invented.
- WorkItem belongs to exactly one Project. Each immutable WorkRevision owns its
  bounded sets of Library-global Subject/Asset typed relationships, so later Run
  binding does not depend on mutable “current” context. The same Subject/Asset may
  be referenced by WorkRevisions in different Projects.
- Take belongs to one immutable WorkRevision and has a store-assigned ordinal.
  TASK-009 product creation requires one existing Asset as its primary candidate;
  it never generates media or claims Provider execution.
- Relationship kinds are closed to `WORK_SUBJECT`, `WORK_ASSET`, `TAKE_REOPENS` and
  `TAKE_SUPERSEDES`. Cross-table endpoints are checked transactionally and again at
  reopen; generic caller-defined relationship kinds are forbidden. The SQL columns
  use bounded uppercase token/16-or-32-byte ID envelopes so a future accepted binary
  can add a typed relationship without rebuilding the table, but this binary's
  operation/kind/endpoint matrix is closed and any unknown stored token is
  corruption.

The candidate adds `current_work_revision_id`, creation commit sequences and command
ownership columns as a canonical refinement required for deterministic current
reads, pagination and replay. It does not make Project an identity/security owner.

### 2.4 AC-010 and REQ-003 cross the TASK-009/TASK-015 boundary

Classification: `CONFLICT / SPEC_STALE`.

TASK-009 cannot truthfully execute “regeneration” because Provider/Run execution and
creation of a new generated Asset are owned by TASK-015 and later Provider tasks.
TASK-009 proves only a prerequisite: CreateTake always allocates a new Take ID and
ordinal and never mutates an earlier Take/Asset. Ordinary CreateTake may reference an
Asset already referenced by another Take; TASK-009 adds no false global
Take-to-Asset uniqueness constraint. `take.reopen.v1` alone requires its replacement
Asset to differ from the reopened terminal Take's Asset. Final end-to-end
`AC-010`/`REQ-003` PASS remains with TASK-015 plus its selected Provider validation
task. TASK-009 records this prerequisite evidence without assigning either ID a
PASS or `CONTRIBUTOR_PASS` status.

Likewise, TASK-009 supplies immutable WorkRevision/ProjectSpecRevision bindings for
`REQ-006`; TASK-015 owns final Run binding evidence. Canonical synchronization must
split these ownerships before activation and must not mark either whole requirement
complete here.

### 2.5 ReopenTake is required but absent from the operation registry

Classification: `SPEC_STALE / API`.

The state machine requires a dedicated audited reopen command while the minimum
registry omits it. TASK-009 adds `take.reopen.v1`. It never mutates a terminal Take;
it creates a new `CANDIDATE` for the same WorkRevision, requires a distinct existing
primary Asset, allocates the next ordinal and writes `TAKE_REOPENS(new, old)`.

Subject otherwise has no reachable V1 path. TASK-009 therefore adds only
`subject.create.v1` and `subject.list.v1`. These operations close the named TASK-009
Subject scope without adding generic CRUD.

### 2.6 JSON target shapes lack executable trust and size rules

Classification: `SPECIFICATION / SECURITY / RESOURCE_BOUND`.

Project policies and Work specifications are untrusted UTF-8. The app layer parses
them once with exact pinned `serde = 1.0.229` and `serde_json = 1.0.150` (both
default features off, `std` only), using a duplicate-key-rejecting visitor. One Work
document accepts 2..262,144 input and canonical bytes. Each Project policy accepts
2..65,536 bytes and the four-policy request total is at most 262,144 bytes. Per
document limits are depth 32, 4,096 total nodes, 1,024 object members/array items per
container,
64-byte keys and 8,192-byte strings. Non-UTF-8, NUL/control keys, duplicate keys,
non-finite/out-of-envelope numbers, limit overflow or trailing data fail before
command claim. The pinned deserializer callback kind is authoritative: values
delivered through `visit_i64` or `visit_u64` are accepted; values delivered through
`visit_f64` are accepted only when finite and strictly greater than
`-9_223_372_036_854_775_808_f64` and strictly less than
`18_446_744_073_709_551_616_f64`. The strict floating endpoints are intentional:
serde_json converts an overflowing integer token to `f64`, so an input immediately
outside the signed/unsigned integer envelope rounds to one of those endpoints and
is rejected rather than silently accepted as a float. This rule also deliberately
rejects finite fractional/exponent values outside the same numeric envelope.
`arbitrary_precision`, `float_roundtrip`, `preserve_order` and `raw_value` remain
disabled. Accepted alternate in-envelope number spellings are reduced to the pinned
encoder's representation rather than retained as authoritative bytes.

Canonical JSON V1 is the compact `serde_json` encoding of the validated tree with
lexicographically ordered UTF-8 object keys and no `preserve_order` feature. The
encoder output is parsed and compared structurally before persistence. The four
Project policy values must each be an object; Work specification must be an object.
Every ProjectSpecRevision stores `policy_schema_version=1`; every WorkRevision
stores `specification_schema_version=1`. These are closed V1 opaque-object formats,
not caller-selected schema identifiers; an unknown stored version fails reopen.
They are classified as bounded opaque creative/policy metadata and grant no
authority. TASK-009 does not interpret arbitrary keys as trust, approval, rights,
egress, Plugin or Provider policy. Stored bytes and SHA-256 are revalidated on read.

The exact dependency, transitive graph, license, MSRV and advisory review is part of
`TEST-SUPPLY-009`. If that review fails, acceptance fails; a handwritten JSON parser
is not a fallback.

### 2.7 “Audited” Take actions precede the SecurityAuditEvent migration

Classification: `CONFLICT / SPEC_STALE`.

Canonical §9.1 calls approval “audit + DomainEvent” and ReopenTake “audited”, while
the separate `security_audit_events` authority is deliberately owned by migration
0004/TASK-013 and SEC-019 is not assigned to TASK-009. TASK-009 must not silently
create a second security-audit table or represent ordinary DomainEvent rows as
SecurityAuditEvent rows.

For V1's single authenticated owner, this proposal refines those two §9.1 uses of
“audit” to durable domain action history: the same transaction stores the immutable
CommandRecord binding and ordered DomainEvent containing the action/result, actor is
recoverable from the CommandRecord's server-derived principal, and the bounded
reason code or reopen relationship is recoverable from canonical state/event data.
This is not SEC-019 security-audit write/query/export evidence. TASK-013 may later
project or cross-link these immutable records into its separate audit capability but
must not rewrite them. This refinement must be accepted canonically before approve
or reopen becomes reachable; otherwise those two operations remain disabled and
TASK-009 cannot complete.

The same boundary applies to TASK-009's non-destructive `RetireAsset` contribution
to REQ-015: the explicit authenticated command, expected revision, CommandRecord and
DomainEvent satisfy its domain-history slice, while TASK-009 does not claim the
whole Rights/Location-removal/GC/Purge requirement or SEC-019. Those terminal
capabilities remain owned by TASK-013/TASK-021 and later destructive tasks.
Accordingly AC-041 receives only contributor evidence here (no byte/Location
mutation plus durable domain history); TASK-022 retains terminal ownership.

### 2.8 Metadata operation configuration has one composition owner

Classification: `EXPECTED_GAP / CONFIGURATION`.

The daemon composition root resolves the exact non-secret key through
`--max-metadata-operation-timeout-ms` > environment
`MENGXIA_MAX_METADATA_OPERATION_TIMEOUT_MS` > the same Library-config key > compiled
default `5000`. It parses once to an immutable typed value in 100..5,000 ms before
Library namespace mutation or endpoint publication. An explicitly supplied empty,
whitespace, signed, non-decimal, non-Unicode or overflowing value is
`STORAGE_CONFIGURATION_ERROR`; absence alone selects the default. Lower values may
tighten but no source may exceed 5,000.

Store/app handlers receive only the typed value and never read configuration
sources. Each TASK-009 request timeout must be 100..5,000 ms and no greater than the
resolved ceiling. JSON byte/depth/node/container/string/key limits, page size 64 and
response content 1,048,576 bytes are fixed ADR-0012 V1 safety maxima, not widening
configuration. `TEST-CONFIG-009` must cover every source/priority/error boundary and
prove invalid configuration fails before any root or endpoint change.

The existing `MENGXIA_MIN_FREE_BYTES` and `MENGXIA_MIN_FREE_PERCENT` four-layer
values are likewise captured once before Library open, validated under their
accepted tightening-only rules, and copied into immutable `StoreConfig` as well as
BlobStorage config. This is shared configuration data, not shared mutable ownership.
The migration uses it only through the §2.2 checked capacity contract. Missing,
conflicting or invalid reserve input fails before migration namespace mutation;
`TEST-CONFIG-009` proves the store and Blob values are identical for every source
precedence case.

### 2.9 Project policy is not an unimplemented authorization oracle

Classification: `SPECIFICATION / SECURITY / PARTIALLY_SPECIFIED`.

Canonical API language requires Project context and policy but defines no V1 policy
evaluator for ordinary-owner creative edits. The four Project policy JSON objects
are color/audio/quality/privacy metadata, not executable authorization. TASK-009's
closed authorization policy is therefore: the TASK-003 authenticated Library owner
Client may perform the listed non-Admin creative operations only after the same
transaction proves the exact Project ancestry and expected revisions. A new Project
has the derived effective value `UNTRUSTED` until TASK-013 creates its separate
authoritative trust record; that value prevents any inference of Plugin/Provider
trust but does not block its owner from defining creative intent directly.

No JSON key, Project ID or `ProjectTrust` value grants actor identity, Admin,
Provider, Plugin, egress or cross-Library authority. A later accepted policy task may
tighten dispatch before command claim, but cannot reinterpret already committed
CommandRecords. Canonical synchronization must record this V1 policy rather than
claiming an absent generic policy engine.

## 3. Exact implementation scope after acceptance

Only a later accepted start record may authorize these files:

- root `Cargo.toml`, `Cargo.lock` and `crates/mengxia-app/Cargo.toml` solely for the
  reviewed JSON dependency;
- `migrations/sqlite/0002_projects_work.sql`, byte-identical to the accepted
  candidate;
- `crates/mengxia-domain/src/{lib.rs,asset.rs,creative.rs}`;
- `crates/mengxia-events/src/lib.rs`;
- `crates/mengxia-ports/src/lib.rs`;
- `crates/mengxia-app/src/{lib.rs,asset_persistence.rs,asset_query.rs,creative.rs,config.rs,observability.rs}`;
- `crates/mengxia-app/src/ingest.rs` only to implement the additive TASK-009
  `AssetUnitOfWork` methods on its existing test fake; no ingest production behavior
  may change;
- `crates/mengxia-store-sqlite/src/{lib.rs,bootstrap.rs,config.rs,migration.rs,migration_intent.rs,migration_snapshot_sqlite.rs,asset_query.rs,asset_repository.rs,creative_query.rs,creative_repository.rs,lifecycle.rs,verification.rs,path_authority.rs}`;
- `crates/mengxia-store-sqlite/tests/{task_006_assets.rs,task_008_queries.rs,task_009_creative.rs}`;
- `crates/mengxia-platform-fs/src/{lib.rs,migration_snapshot.rs}` only for the fixed
  migration snapshot namespace and existing safe full-sync primitives; existing
  materialization authority is not repurposed and no new FFI is permitted;
- `proto/core/v1/{handshake.proto,handshake.pb,handshake.provenance}` and exact new
  immutable files
  `crates/mengxia-testkit/tests/fixtures/task_008/{handshake-v1.2.proto,handshake-v1.2.pb,handshake-v1.2.provenance}`;
- `crates/mengxia-core-proto/{build.rs,src/lib.rs,src/session.rs}`;
- `bins/mengxia/src/main.rs` and `bins/mengxiad/src/main.rs`;
- `crates/mengxia-testkit/tests/{architecture.rs,ci_orchestration.rs,document_traceability.rs,task_008_foundation.rs,task_009_foundation.rs}`; the TASK-008 file may change
  only to bind v1.2 byte identity to its frozen fixture while retaining TASK-008
  fields, tags, reservations and security assertions against the current protocol;
- `scripts/{verify-repository.sh,verify-task-009.sh,run-task-003-cli-tests.sh}`;
  the retained TASK-003 script may change only to start the current daemon at the
  accepted TASK-009 decode-depth floor 5 while continuing to issue its historical
  protocol-1.0 client assertions at depth 3; and
  `.github/workflows/ci.yml`; the workflow change is limited to renaming its stale
  “through TASK-008” title/comment through TASK-009 because its existing jobs already
  invoke the repository driver—runner, triggers, permissions and job topology do not
  change;
- `AGENTS.md`, `docs/spec/{IMPLEMENTATION_SPEC.md,DECISIONS.md,IMPLEMENTATION_REVIEW.md,IMPLEMENTATION_PLAN.md,PROJECT_INTAKE_REPORT.md}`,
  this proposal, `docs/proposals/TASK-009-0002-CANDIDATE.sql` and
  `docs/spec/adr/ADR-0012-task-009-creative-ledger-migration.md`.

The final accepted file list must be exact; a reviewer may remove files proved
unnecessary. Adding any package, unsafe block, FFI, network/service dependency or
filesystem authority outside the fixed migration snapshot is out of scope.

### 3.1 Explicitly forbidden

- editing migration 0000 or 0001 bytes/checksums;
- TASK-010+, Recipe/Run/Provider generation, Asset byte creation or CAS mutation;
- Subject merge/delete, Project delete, Work delete or Take delete;
- arbitrary field update, generic CRUD, direct lifecycle/state assignment;
- Admin, trust escalation, multi-tenant claims or caller-supplied principal;
- Rights/clearance, Plugin, Credential, egress, root rebind, Location removal, GC or
  Purge;
- automatic deletion of either retained pre-0002 snapshot/manifest artifact;
- logging JSON, names, reasons, paths, raw IDs or arbitrary error text;
- weakening TASK-008 readiness, fatal-store behavior, bounded queues or existing
  protocol 1.0/1.1/1.2 behavior.

## 4. Domain invariants

### 4.1 Project and ProjectSpecRevision

- Project IDs and spec-revision IDs are distinct typed UUIDv7 values and globally
  byte-unique against every ID created by the same command.
- name is valid UTF-8 and 1..255 bytes; it must equal its Unicode-whitespace-trimmed
  form, contain no Unicode control scalar and is stored byte-exactly without Unicode
  normalization or case folding. Invalid input is rejected rather than changed.
- create stores Project revision 1 and immutable spec sequence 1 in one transaction.
- resolution is absent or a width/height pair in 1..65,535. Frame rate and aspect
  ratio are absent or positive u32 numerator/denominator pairs with gcd 1;
  non-reduced or half-present pairs are rejected rather than normalized.
- revise requires exact Project and `expected_revision`, inserts sequence `max+1`,
  updates the current pointer and Project revision, and emits one event.
- in TASK-009, Project revision equals the current spec sequence widened to u64;
  pointer, sequence and revision mismatch is corruption.
- sequence or RevisionNo overflow is terminal `REVISION_EXHAUSTED` with no mutation.

### 4.2 Subject and relationships

- Subject kind uses the existing lowercase ASCII token grammar; canonical name is
  1..255 UTF-8 bytes, trim-stable and contains no Unicode control scalar. It is
  stored byte-exactly without normalization or case folding.
- duplicate names are not silently deduplicated because name equality does not prove
  semantic identity.
- relationship ID is Core-generated; `WORK_SUBJECT` and `WORK_ASSET` source exactly
  one immutable WorkRevision; subject and Asset targets are Library-global.
- a WorkRevision's WorkItem/Project chain is validated for command context, but the target Subject or
  Asset is not required to be “owned” by that Project.
- input relationship IDs are sorted by raw bytes for digest/event determinism;
  duplicates and source/target kind mismatch are validation failures. A WorkItem
  accepts at most 64 Subject IDs and 64 Asset IDs (128 relationship rows total).
- each Take has at most one outgoing `TAKE_REOPENS` and at most one outgoing
  `TAKE_SUPERSEDES` edge. The candidate SQL enforces those source-cardinality limits
  with partial unique indexes. A terminal Take may have multiple later Takes reopening
  it; those unbounded incoming edges are deliberately not embedded in `ListTakes`.
  TASK-009 returns only the at-most-two outgoing typed edges; any future incoming-edge
  query requires its own accepted pagination contract.

### 4.3 WorkItem and WorkRevision

- Work kind is exactly `SCENE|SHOT`; code is 1..64 UTF-8 bytes, trim-stable,
  Unicode-control-free, stored without normalization/case folding and unique by
  `(project_id, kind, exact code bytes)`.
- create inserts WorkItem revision 1, WorkRevision sequence 1 and that revision's
  complete bounded relationship set atomically.
- revise requires Project context, exact WorkItem membership and expected revision;
  it supplies a complete replacement Subject/Asset set for the new revision; old
  WorkRevision rows, JSON and relationships never change.
- WorkItem current pointer and revision advance with the new immutable revision and
  one DomainEvent.
- in TASK-009, WorkItem revision equals current WorkRevision sequence widened to
  u64; pointer, sequence and revision mismatch is corruption.

### 4.4 Asset revision and lifecycle

- `asset.revision.create.v1` exposes the completed TASK-006 pure command without
  changing its persisted operation ID, result kind, event type or replay mapper.
- every referenced Blob is `AVAILABLE` and has at least one `MANAGED`, `DURABLE`,
  `AVAILABLE` Location, preserving the completed TASK-006 custody precondition; this
  command performs no file/CAS operation. Asset must be `ACTIVE` and expected
  revision must match.
- retire is only `ACTIVE -> RETIRED`; restore is only `RETIRED -> ACTIVE`. Both
  require expected revision, increment exactly once, append exactly one DomainEvent
  and store an `ASSET_LIFECYCLE` result. Neither changes revisions, Blob/Location
  state, relationships or bytes.
- same-state/backward/unknown transition returns `INVALID_TRANSITION`; revision
  mismatch takes precedence as `CONFLICT` after authorized existence is established.
- migration 0002 appends nullable paired `assets.updated_at_seconds/nanos` columns
  without rewriting a legacy row. Null means the effective update time is the
  existing `created_at`; every post-0002 Asset revision/retire/restore mutation sets
  both columns to its event timestamp, while exact replay never changes them. Reads
  expose the effective timestamp and reopen rejects a half-null or invalid pair.

### 4.5 Take

- CreateTake validates Project -> WorkItem -> WorkRevision context and an existing
  Library-global Asset, allocates `max(ordinal)+1` transactionally and starts revision
  1 at `CANDIDATE`. It always creates a new Take but may reference an Asset already
  used by another Take; this operation is not a regeneration claim.
- shortlist: `CANDIDATE -> SHORTLISTED` and primary Asset must still exist.
- select: `CANDIDATE|SHORTLISTED -> SELECTED`. If another Take for the same
  WorkRevision is `SELECTED`, the request must name it with its expected revision;
  the transaction explicitly moves it to `SUPERSEDED`, creates
  `TAKE_SUPERSEDES(new, old)`, and emits both ordered events. Silent replacement is
  forbidden.
- approve: only `SELECTED -> APPROVED`; authenticated Library owner is the V1 human
  approval authority. This is not Admin authority or Project tenant ownership.
- reject: any non-terminal -> `REJECTED`, with a required 1..1,024-byte UTF-8,
  trim-stable, Unicode-control-free reason persisted only in the bounded event
  payload.
- supersede outside the explicit select pair requires a distinct replacement Take
  for the same WorkRevision and its expected revision.
- `APPROVED|REJECTED|SUPERSEDED` are terminal. Reopen creates a distinct candidate
  whose primary Asset differs from the terminal Take's primary Asset and writes one
  `TAKE_REOPENS` relationship; it never modifies the terminal row.
- every changed Take uses expected revision. One command, all state changes,
  relationships, events and stored result commit atomically.

Transition request codes are exactly `1 SHORTLIST`, `2 SELECT`, `3 APPROVE`,
`4 REJECT`, `5 SUPERSEDE`. SHORTLIST/APPROVE require reason and related-Take fields
absent; REJECT requires only reason; SELECT permits only an optional prior-selected
Take ID plus its expected revision and requires it when a different selected row
exists; SUPERSEDE requires only replacement Take ID plus expected revision. Any
half-present, extraneous or unknown variant is validation failure before claim.

## 5. Versioned result payload registry

All integers are unsigned big-endian; reserved bytes are zero. For the closed
TASK-009 registry, `result_id` is required and is the operation's newly created or
mutated primary UUID object; that UUID is not repeated in payload. At the SQL
extensibility layer it is optional for future non-legacy payload-authoritative result
shapes as defined in §2.1. Decoders must therefore enforce the closed operation codec,
not infer validity merely from column nullability.

| result_kind | result_id | bytes | Payload v1 |
|---|---|---:|---|
| `PROJECT` | Project ID | 32 | spec revision ID[16], Project revision u64, spec sequence u32, reserved[4] |
| `PROJECT_SPEC_REVISION` | new spec revision ID | 32 | Project ID[16], Project revision u64, spec sequence u32, reserved[4] |
| `SUBJECT` | Subject ID | 16 | Subject revision u64, reserved[8] |
| `WORK_ITEM` | WorkItem ID | 32 | WorkRevision ID[16], WorkItem revision u64, WorkRevision sequence u32, reserved[4] |
| `WORK_REVISION` | new WorkRevision ID | 32 | WorkItem ID[16], WorkItem revision u64, WorkRevision sequence u32, reserved[4] |
| `TAKE` | created or transitioned Take ID | 48 | Take revision u64, ordinal u32, state u8, flags u8, reserved[2], primary Asset ID[16], related Take ID-or-zero[16] |
| `ASSET_LIFECYCLE` | Asset ID | 16 | Asset revision u64, lifecycle u8, reserved[7] |

The exact `TAKE` offsets are 0..8 revision, 8..12 ordinal, 12 state, 13 flags,
14..16 reserved, 16..32 primary Asset and 32..48 optional related Take UUID.
`flags bit0` is one iff the related ID is present; all other bits are zero.

State codes are `1 CANDIDATE`, `2 SHORTLISTED`, `3 SELECTED`, `4 APPROVED`,
`5 REJECTED`, `6 SUPERSEDED`; lifecycle codes are `1 ACTIVE`, `2 RETIRED`.
Every codec has a checked-in golden vector, exact EOF test, bit/padding/hash mutation
matrix and a rehydration query proving canonical row/event equality.

The closed operation/result mapping is exact:

| operation | result kind |
|---|---|
| `asset.revision.create.v1` | legacy payload-free `ASSET_REVISION` |
| `asset.retire.v1`, `asset.restore.v1` | `ASSET_LIFECYCLE` |
| `project.create.v1` | `PROJECT` |
| `project.spec.revise.v1` | `PROJECT_SPEC_REVISION` |
| `subject.create.v1` | `SUBJECT` |
| `work.create.v1` | `WORK_ITEM` |
| `work.revise.v1` | `WORK_REVISION` |
| `take.create.v1`, `take.transition.v1`, `take.reopen.v1` | `TAKE` |

List operations never create CommandRecord rows. Any other TASK-009 pair, a missing
TASK-009 primary UUID, a new payload attached to a legacy result, or a payload-free
new result is corruption. Migration tests additionally insert a synthetic
future-codec fixture with null `result_id` and an authoritative valid payload, prove
the SQL accepts it without weakening the running binary's closed registry, and prove
that a legacy result with null `result_id` remains rejected.

## 6. Event registry and request digests

The closed event matrix is:

| event_type | aggregate kind / ID | aggregate revision | payload v1 |
|---|---|---|---|
| `asset.retired.v1` / `asset.restored.v1` | `ASSET` / Asset ID | resulting Asset revision | absent |
| `project.created.v1` | `PROJECT` / Project ID | 1 | absent |
| `project.spec.revised.v1` | `PROJECT_SPEC_REVISION` / new spec revision ID | resulting Project revision | absent |
| `subject.created.v1` | `SUBJECT` / Subject ID | 1 | absent |
| `work.created.v1` | `WORK_ITEM` / WorkItem ID | 1 | absent |
| `work.revised.v1` | `WORK_REVISION` / new WorkRevision ID | resulting WorkItem revision | absent |
| `take.created.v1` / `take.shortlisted.v1` / `take.approved.v1` | `TAKE` / Take ID | resulting Take revision | absent |
| `take.selected.v1` | `TAKE` / selected Take ID | resulting revision | optional tag 1 = superseded prior Take ID[16] |
| `take.rejected.v1` | `TAKE` / rejected Take ID | resulting revision | tag 1 = exact reason UTF-8 bytes |
| `take.superseded.v1` | `TAKE` / superseded Take ID | resulting revision | tag 1 = replacement Take ID[16] |
| `take.reopened.v1` | `TAKE` / new Take ID | 1 | tag 1 = terminal prior Take ID[16] |

Aggregate kinds are `ASSET`, `PROJECT`, `PROJECT_SPEC_REVISION`, `SUBJECT`,
`WORK_ITEM`, `WORK_REVISION`, `TAKE`; all IDs are 16-byte typed UUIDv7. Existing
`BLOB` aggregate semantics remain unchanged. Payload is absent unless the matrix
requires it. Present payloads use strictly increasing
`u8 tag || u16_be length || value` TLV and are at most 2,048 bytes; unknown,
repeated, missing or out-of-order tags fail closed. Payload hash is
`SHA-256(payload)` and must be absent iff payload is absent. User text is never a
log field.

When selection replaces an existing selected Take, the old Take's
`take.superseded.v1` is allocated first and the new Take's `take.selected.v1`
second, with contiguous commit sequences in the same transaction. The relationship
is exactly `TAKE_SUPERSEDES(new, old)`. Reopen uses exactly
`TAKE_REOPENS(new, terminal-old)`.

Stored JSON digests are exact. WorkRevision `specification_digest` is
`SHA-256(canonical_work_json)`. ProjectSpecRevision `policy_digest` is:

```text
SHA-256(
  "MENGXIA_PROJECT_POLICIES_V1\0" ||
  u32_be(len(color))   || color   ||
  u32_be(len(audio))   || audio   ||
  u32_be(len(quality)) || quality ||
  u32_be(len(privacy)) || privacy
)
```

where each value is its canonical JSON bytes in that fixed order.

Each new mutation digest is exactly
`SHA-256("MENGXIA_TASK009_REQUEST_V1\0" || u16_be(operation_id_byte_length) ||
operation_id_ascii || fields)`, where fields are strictly increasing
`u8 tag || u32_be length || canonical value`. IDs are raw 16 bytes, integers are
big-endian, enums are their fixed numeric code, JSON contributes SHA-256 of canonical
bytes, and Subject/Asset sets are raw-ID sorted and duplicate-free. `command_id`,
request/correlation ID, timeout, cursor and transport encoding are excluded.
Principal UID remains a separate CommandRecord binding field. The existing
`asset.revision.create.v1` digest remains byte-exact and is not re-versioned.

Every field row below is encoded at tags 1..N in listed order and every tag is
present. Optional fixed values encode as one presence byte followed by the value
when present; sets encode `u32 count || sorted raw IDs`; strings are exact validated
UTF-8 bytes. No other field contributes:

| operation | ordered semantic fields |
|---|---|
| `asset.revision.create.v1` | the already accepted TASK-006 canonical digest, unchanged |
| `asset.retire.v1` / `asset.restore.v1` | Asset ID; expected revision |
| `project.create.v1` | name; optional resolution pair; optional frame-rate pair; optional aspect-ratio pair; color/audio/quality/privacy policy digests |
| `project.spec.revise.v1` | Project ID; expected revision; the same scalar/policy fields as create |
| `subject.create.v1` | kind; canonical name |
| `work.create.v1` | Project ID; kind; code; specification digest; Subject ID set; Asset ID set |
| `work.revise.v1` | Project ID; WorkItem ID; expected revision; specification digest; Subject ID set; Asset ID set |
| `take.create.v1` | Project ID; WorkItem ID; WorkRevision ID; primary Asset ID |
| `take.transition.v1` | Project ID; WorkItem ID; WorkRevision ID; Take ID; expected revision; transition code; optional reason; optional replacement/prior-selected Take ID plus its expected revision |
| `take.reopen.v1` | Project ID; WorkItem ID; WorkRevision ID; terminal Take ID; expected terminal revision; new primary Asset ID |

Golden vectors must prove every semantic field changes the digest, transport-only
fields do not, JSON key order canonicalizes identically, and Project/relationship
scope cannot be swapped without a conflict.

## 7. Protocol 1.3 and semantic operation registry

TASK-009 proposes protocol minor 3 while retaining exact 1.0, 1.1 and 1.2 behavior.
Before modifying the current proto, checked-in immutable protocol-1.2 source,
descriptor and provenance fixtures are created and verified.

| tag request/response | Operation ID | Contract summary |
|---|---|---|
| 16/16 | `asset.revision.create.v1` | bounded revision graph; owner Client; pure atomic mutation/replay |
| 17/17 | `asset.retire.v1` | Asset + expected revision; non-destructive lifecycle event |
| 18/18 | `asset.restore.v1` | retired Asset + expected revision |
| 19/19 | `project.create.v1` | name + initial typed scalars/policies |
| 20/20 | `project.spec.revise.v1` | Project + expected revision + immutable spec |
| 21/21 | `project.list.v1` | bounded snapshot page |
| 22/22 | `subject.create.v1` | global kind/name |
| 23/23 | `subject.list.v1` | bounded global snapshot page |
| 24/24 | `work.create.v1` | Project, kind/code/spec, bounded Subject/Asset refs |
| 25/25 | `work.revise.v1` | Project/Work + expected revision + immutable spec |
| 26/26 | `work.list.v1` | Project-scoped bounded snapshot page |
| 27/27 | `take.create.v1` | Project/WorkRevision + existing primary Asset |
| 28/28 | `take.transition.v1` | exact event, expected revision, conditional reason/replacement |
| 29/29 | `take.reopen.v1` | terminal Take + distinct primary Asset; creates new Take |
| 30/30 | `take.list.v1` | WorkRevision-scoped bounded snapshot page |

CoreRequest retains permanent `reserved 8 to 15` and uses 16..30. CoreResponse
retains permanent `reserved 8 to 14`, preserves `error=15`, and uses 16..30. Both
reserve every otherwise unused tag through 31. Removed fields are reserved by name
and number. The descriptor preflight depth and frame proof are recomputed; no
recursive message or media bytes are introduced.

### 7.1 Exact protocol messages

The following is the normative field registry; implementation may reorder source
declarations but not names, types, presence or numbers. Every request reserves
`actor`, `actor_principal`, `principal`, `admin`, `credential`, `backend_id`,
`locator` and `cas_root` where not already globally reserved. Protobuf acceptance is
followed by the stricter semantic bounds in §§4, 6 and 8; zero/empty/default enum
values are invalid unless explicitly described as optional.

```proto
message AssetRevisionMemberInput {
  string logical_name = 1;
  bytes blob_sha256 = 2;
  reserved 3 to 15;
}
message AssetRevisionResourceInput {
  string resource_kind = 1;
  repeated AssetRevisionMemberInput members = 2;
  reserved 3 to 15;
}
message AssetRevisionRepresentationInput {
  string representation_purpose = 1;
  repeated AssetRevisionResourceInput resources = 2;
  reserved 3 to 15;
}
message ProjectSpecInput {
  optional uint32 resolution_width = 1;
  optional uint32 resolution_height = 2;
  optional uint32 frame_rate_numerator = 3;
  optional uint32 frame_rate_denominator = 4;
  optional uint32 aspect_ratio_numerator = 5;
  optional uint32 aspect_ratio_denominator = 6;
  bytes color_policy_json = 7;
  bytes audio_policy_json = 8;
  bytes quality_policy_json = 9;
  bytes privacy_policy_json = 10;
  reserved 11 to 15;
}
enum WorkKindValue {
  WORK_KIND_VALUE_UNSPECIFIED = 0;
  WORK_KIND_VALUE_SCENE = 1;
  WORK_KIND_VALUE_SHOT = 2;
}
enum ProjectTrustValue {
  PROJECT_TRUST_VALUE_UNSPECIFIED = 0;
  PROJECT_TRUST_VALUE_UNTRUSTED = 1;
}
enum TakeStateValue {
  TAKE_STATE_VALUE_UNSPECIFIED = 0;
  TAKE_STATE_VALUE_CANDIDATE = 1;
  TAKE_STATE_VALUE_SHORTLISTED = 2;
  TAKE_STATE_VALUE_SELECTED = 3;
  TAKE_STATE_VALUE_APPROVED = 4;
  TAKE_STATE_VALUE_REJECTED = 5;
  TAKE_STATE_VALUE_SUPERSEDED = 6;
}
enum TakeTransitionValue {
  TAKE_TRANSITION_VALUE_UNSPECIFIED = 0;
  TAKE_TRANSITION_VALUE_SHORTLIST = 1;
  TAKE_TRANSITION_VALUE_SELECT = 2;
  TAKE_TRANSITION_VALUE_APPROVE = 3;
  TAKE_TRANSITION_VALUE_REJECT = 4;
  TAKE_TRANSITION_VALUE_SUPERSEDE = 5;
}
enum TakeRelationshipKindValue {
  TAKE_RELATIONSHIP_KIND_VALUE_UNSPECIFIED = 0;
  TAKE_RELATIONSHIP_KIND_VALUE_REOPENS = 1;
  TAKE_RELATIONSHIP_KIND_VALUE_SUPERSEDES = 2;
}

message CreateAssetRevisionRequest {
  string command_id = 1;
  string asset_id = 2;
  uint64 expected_revision = 3;
  repeated string parent_revision_ids = 4;
  string content_kind = 5;
  repeated AssetRevisionRepresentationInput representations = 6;
  uint64 operation_timeout_ms = 7;
  reserved 8 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message AssetLifecycleRequest {
  string command_id = 1;
  string asset_id = 2;
  uint64 expected_revision = 3;
  uint64 operation_timeout_ms = 4;
  reserved 5 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message CreateProjectRequest {
  string command_id = 1;
  string name = 2;
  ProjectSpecInput specification = 3;
  uint64 operation_timeout_ms = 4;
  reserved 5 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message ReviseProjectSpecRequest {
  string command_id = 1;
  string project_id = 2;
  uint64 expected_revision = 3;
  ProjectSpecInput specification = 4;
  uint64 operation_timeout_ms = 5;
  reserved 6 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message ListProjectsRequest {
  uint32 page_size = 1;
  bytes cursor = 2;
  uint64 operation_timeout_ms = 3;
  reserved 4 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message CreateSubjectRequest {
  string command_id = 1;
  string kind = 2;
  string canonical_name = 3;
  uint64 operation_timeout_ms = 4;
  reserved 5 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message ListSubjectsRequest {
  uint32 page_size = 1;
  bytes cursor = 2;
  uint64 operation_timeout_ms = 3;
  reserved 4 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message CreateWorkItemRequest {
  string command_id = 1;
  string project_id = 2;
  WorkKindValue kind = 3;
  string code = 4;
  bytes specification_json = 5;
  repeated string subject_ids = 6;
  repeated string asset_ids = 7;
  uint64 operation_timeout_ms = 8;
  reserved 9 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message ReviseWorkRequest {
  string command_id = 1;
  string project_id = 2;
  string work_item_id = 3;
  uint64 expected_revision = 4;
  bytes specification_json = 5;
  repeated string subject_ids = 6;
  repeated string asset_ids = 7;
  uint64 operation_timeout_ms = 8;
  reserved 9 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message ListWorkRequest {
  string project_id = 1;
  uint32 page_size = 2;
  bytes cursor = 3;
  uint64 operation_timeout_ms = 4;
  reserved 5 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message CreateTakeRequest {
  string command_id = 1;
  string project_id = 2;
  string work_item_id = 3;
  string work_revision_id = 4;
  string primary_asset_id = 5;
  uint64 operation_timeout_ms = 6;
  reserved 7 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message TransitionTakeRequest {
  string command_id = 1;
  string project_id = 2;
  string work_item_id = 3;
  string work_revision_id = 4;
  string take_id = 5;
  uint64 expected_revision = 6;
  TakeTransitionValue transition = 7;
  optional string reason = 8;
  optional string related_take_id = 9;
  optional uint64 related_take_expected_revision = 10;
  uint64 operation_timeout_ms = 11;
  reserved 12 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message ReopenTakeRequest {
  string command_id = 1;
  string project_id = 2;
  string work_item_id = 3;
  string work_revision_id = 4;
  string terminal_take_id = 5;
  uint64 terminal_take_expected_revision = 6;
  string new_primary_asset_id = 7;
  uint64 operation_timeout_ms = 8;
  reserved 9 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}
message ListTakesRequest {
  string project_id = 1;
  string work_item_id = 2;
  string work_revision_id = 3;
  uint32 page_size = 4;
  bytes cursor = 5;
  uint64 operation_timeout_ms = 6;
  reserved 7 to 31;
  reserved "actor", "actor_principal", "principal", "admin", "credential", "backend_id", "locator", "cas_root";
}

message CreateAssetRevisionResult {
  string command_id = 1;
  string asset_id = 2;
  string asset_revision_id = 3;
  uint64 resulting_revision = 4;
  int64 created_at_seconds = 5;
  uint32 created_at_nanos = 6;
  bool replayed = 7;
  reserved 8 to 15;
}
message AssetLifecycleMutationResult {
  string command_id = 1;
  string asset_id = 2;
  uint64 resulting_revision = 3;
  AssetLifecycleValue lifecycle = 4;
  int64 updated_at_seconds = 5;
  uint32 updated_at_nanos = 6;
  bool replayed = 7;
  reserved 8 to 15;
}
message ProjectMutationResult {
  string command_id = 1;
  string project_id = 2;
  string project_spec_revision_id = 3;
  uint64 project_revision = 4;
  uint32 specification_sequence = 5;
  int64 updated_at_seconds = 6;
  uint32 updated_at_nanos = 7;
  bool replayed = 8;
  reserved 9 to 15;
}
message SubjectMutationResult {
  string command_id = 1;
  string subject_id = 2;
  uint64 subject_revision = 3;
  int64 created_at_seconds = 4;
  uint32 created_at_nanos = 5;
  bool replayed = 6;
  reserved 7 to 15;
}
message WorkMutationResult {
  string command_id = 1;
  string work_item_id = 2;
  string work_revision_id = 3;
  uint64 work_item_revision = 4;
  uint32 work_revision_sequence = 5;
  int64 updated_at_seconds = 6;
  uint32 updated_at_nanos = 7;
  bool replayed = 8;
  reserved 9 to 15;
}
message TakeMutationResult {
  string command_id = 1;
  string take_id = 2;
  uint32 ordinal = 3;
  TakeStateValue state = 4;
  string primary_asset_id = 5;
  uint64 take_revision = 6;
  optional string related_take_id = 7;
  int64 updated_at_seconds = 8;
  uint32 updated_at_nanos = 9;
  bool replayed = 10;
  reserved 11 to 15;
}

message AssetSummary {
  string asset_id = 1;
  string kind = 2;
  AssetLifecycleValue lifecycle = 3;
  uint64 revision = 4;
  int64 created_at_seconds = 5;
  uint32 created_at_nanos = 6;
  uint64 creation_commit_sequence = 7;
  reserved 8 to 15;
  optional int64 updated_at_seconds = 16;
  optional uint32 updated_at_nanos = 17;
  reserved 18 to 31;
}
message ProjectSpecView {
  string project_spec_revision_id = 1;
  uint32 sequence = 2;
  optional uint32 resolution_width = 3;
  optional uint32 resolution_height = 4;
  optional uint32 frame_rate_numerator = 5;
  optional uint32 frame_rate_denominator = 6;
  optional uint32 aspect_ratio_numerator = 7;
  optional uint32 aspect_ratio_denominator = 8;
  uint32 policy_schema_version = 9;
  bytes color_policy_json = 10;
  bytes audio_policy_json = 11;
  bytes quality_policy_json = 12;
  bytes privacy_policy_json = 13;
  bytes policy_sha256 = 14;
  reserved 15;
}
message ProjectView {
  string project_id = 1;
  string name = 2;
  uint64 revision = 3;
  int64 created_at_seconds = 4;
  uint32 created_at_nanos = 5;
  int64 updated_at_seconds = 6;
  uint32 updated_at_nanos = 7;
  uint64 creation_commit_sequence = 8;
  ProjectSpecView current_specification = 9;
  ProjectTrustValue effective_trust = 10;
  reserved 11 to 15;
}
message ListProjectsResult {
  uint64 snapshot_commit_sequence = 1;
  repeated ProjectView projects = 2;
  optional bytes next_cursor = 3;
  reserved 4 to 15;
}
message SubjectView {
  string subject_id = 1;
  string kind = 2;
  string canonical_name = 3;
  uint64 revision = 4;
  int64 created_at_seconds = 5;
  uint32 created_at_nanos = 6;
  uint64 creation_commit_sequence = 7;
  reserved 8 to 15;
}
message ListSubjectsResult {
  uint64 snapshot_commit_sequence = 1;
  repeated SubjectView subjects = 2;
  optional bytes next_cursor = 3;
  reserved 4 to 15;
}
message WorkView {
  string work_item_id = 1;
  string project_id = 2;
  WorkKindValue kind = 3;
  string code = 4;
  uint64 revision = 5;
  int64 created_at_seconds = 6;
  uint32 created_at_nanos = 7;
  int64 updated_at_seconds = 8;
  uint32 updated_at_nanos = 9;
  uint64 creation_commit_sequence = 10;
  string current_work_revision_id = 11;
  uint32 current_work_revision_sequence = 12;
  uint32 specification_schema_version = 13;
  bytes specification_json = 14;
  bytes specification_sha256 = 15;
  repeated string subject_ids = 16;
  repeated string asset_ids = 17;
  reserved 18 to 31;
}
message ListWorkResult {
  uint64 snapshot_commit_sequence = 1;
  repeated WorkView work_items = 2;
  optional bytes next_cursor = 3;
  reserved 4 to 15;
}
message TakeRelationshipView {
  string relationship_id = 1;
  TakeRelationshipKindValue kind = 2;
  string target_take_id = 3;
  reserved 4 to 15;
}
message TakeView {
  string take_id = 1;
  string work_revision_id = 2;
  uint32 ordinal = 3;
  TakeStateValue state = 4;
  string primary_asset_id = 5;
  uint64 revision = 6;
  int64 created_at_seconds = 7;
  uint32 created_at_nanos = 8;
  int64 updated_at_seconds = 9;
  uint32 updated_at_nanos = 10;
  repeated TakeRelationshipView outgoing_relationships = 11;
  reserved 12 to 15;
}
message ListTakesResult {
  uint64 snapshot_ordinal = 1;
  repeated TakeView takes = 2;
  optional bytes next_cursor = 3;
  reserved 4 to 15;
}

message CoreRequest {
  oneof operation {
    IngestAssetCopyRequest ingest_asset_copy = 1;
    GetLibraryStatusRequest get_library_status = 2;
    VerifyLibraryRequest verify_library = 3;
    ListIntegrityIssuesRequest list_integrity_issues = 4;
    InspectAssetRequest inspect_asset = 5;
    ListAssetsRequest list_assets = 6;
    MaterializeAssetRequest materialize_asset = 7;
    CreateAssetRevisionRequest create_asset_revision = 16;
    AssetLifecycleRequest retire_asset = 17;
    AssetLifecycleRequest restore_asset = 18;
    CreateProjectRequest create_project = 19;
    ReviseProjectSpecRequest revise_project_spec = 20;
    ListProjectsRequest list_projects = 21;
    CreateSubjectRequest create_subject = 22;
    ListSubjectsRequest list_subjects = 23;
    CreateWorkItemRequest create_work_item = 24;
    ReviseWorkRequest revise_work = 25;
    ListWorkRequest list_work = 26;
    CreateTakeRequest create_take = 27;
    TransitionTakeRequest transition_take = 28;
    ReopenTakeRequest reopen_take = 29;
    ListTakesRequest list_takes = 30;
  }
  reserved 8 to 15;
  reserved 31;
  reserved "actor", "actor_principal", "principal", "project_id", "admin", "credential";
}
message CoreResponse {
  oneof response {
    IngestAssetCopyResult ingest_asset_copy = 1;
    GetLibraryStatusResult get_library_status = 2;
    VerifyLibraryResult verify_library = 3;
    ListIntegrityIssuesResult list_integrity_issues = 4;
    InspectAssetResult inspect_asset = 5;
    ListAssetsResult list_assets = 6;
    MaterializeAssetResult materialize_asset = 7;
    ErrorEnvelope error = 15;
    CreateAssetRevisionResult create_asset_revision = 16;
    AssetLifecycleMutationResult retire_asset = 17;
    AssetLifecycleMutationResult restore_asset = 18;
    ProjectMutationResult create_project = 19;
    ProjectMutationResult revise_project_spec = 20;
    ListProjectsResult list_projects = 21;
    SubjectMutationResult create_subject = 22;
    ListSubjectsResult list_subjects = 23;
    WorkMutationResult create_work_item = 24;
    WorkMutationResult revise_work = 25;
    ListWorkResult list_work = 26;
    TakeMutationResult create_take = 27;
    TakeMutationResult transition_take = 28;
    TakeMutationResult reopen_take = 29;
    ListTakesResult list_takes = 30;
  }
  reserved 8 to 14;
  reserved 31;
}
```

`CoreRequest.operation` and `CoreResponse.response` use the §7 table in that exact
order. The two lifecycle request tags have distinct generated fields/types even
though both consume `AssetLifecycleRequest`; Project create/revise, Work
create/revise and Take create/transition/reopen similarly have distinct oneof fields
while reusing their response view types. Core generates AssetRevision,
Representation, Resource, ProjectSpecRevision, WorkRevision, relationship, event and
provenance IDs only after the absent-command decision; callers provide none of them.
Member ordinal is its zero-based position in the request resource. CreateAssetRevision
returns the new revision identity and revision number; generated graph child IDs are
retrieved through paginated `asset.inspect.v1`, avoiding an unbounded mutation
response.

Existing `AssetSummary` adds only `optional int64 updated_at_seconds = 16` and
`optional uint32 updated_at_nanos = 17`, then reserves 18..31. A protocol-1.2
session leaves both absent and preserves its exact observable response; protocol 1.3
sets both from the effective timestamp. The same version gate applies when the
summary is nested in existing ListAssets/InspectAsset results. Half-presence is never
emitted and is rejected by internal conversion tests.

### 7.2 Exact CLI grammar and output

The TASK-009 CLI surface is exactly the following. Options may appear in any order
except that each `--representation` starts a representation, each following
`--resource` starts one of its resources, and each following `--member` belongs to
that resource until the next resource/representation. Repeated IDs retain caller
order through parsing and are then duplicate-checked/canonically raw-ID sorted where
§6 requires it. `HEX` is lowercase even-length hex; `UUID` is canonical UUIDv7;
`U32/U64` are unsigned canonical decimal with no sign/whitespace/leading zero except
the literal `0`; `TOKEN` uses the governing token grammar.

```text
mengxia asset create-revision --command-id UUID --asset-id UUID
  --expected-revision U64 --parent-revision-id UUID{1..64}
  --content-kind TOKEN
  (--representation TOKEN (--resource TOKEN
     --member LOGICAL_NAME_HEX:BLOB_SHA256_HEX{1..4096}){1..64}){1..64}
  [operation/client transport options]
mengxia asset retire  --command-id UUID --asset-id UUID --expected-revision U64 [operation/client options]
mengxia asset restore --command-id UUID --asset-id UUID --expected-revision U64 [operation/client options]

mengxia project create --command-id UUID --name-hex HEX
  [--resolution U32xU32] [--frame-rate U32/U32] [--aspect-ratio U32/U32]
  --color-policy-json-hex HEX --audio-policy-json-hex HEX
  --quality-policy-json-hex HEX --privacy-policy-json-hex HEX [operation/client options]
mengxia project revise-spec --command-id UUID --project-id UUID --expected-revision U64
  [the same complete ProjectSpec options] [operation/client options]
mengxia project list [page/cursor/operation/client options]

mengxia subject create --command-id UUID --kind TOKEN --canonical-name-hex HEX [operation/client options]
mengxia subject list [page/cursor/operation/client options]

mengxia work create --command-id UUID --project-id UUID --kind scene|shot --code-hex HEX
  --specification-json-hex HEX [--subject-id UUID]{0..64} [--asset-id UUID]{0..64}
  [operation/client options]
mengxia work revise --command-id UUID --project-id UUID --work-item-id UUID
  --expected-revision U64 --specification-json-hex HEX
  [--subject-id UUID]{0..64} [--asset-id UUID]{0..64} [operation/client options]
mengxia work list --project-id UUID [page/cursor/operation/client options]

mengxia take create --command-id UUID --project-id UUID --work-item-id UUID
  --work-revision-id UUID --primary-asset-id UUID [operation/client options]
mengxia take transition --command-id UUID --project-id UUID --work-item-id UUID
  --work-revision-id UUID --take-id UUID --expected-revision U64
  --transition shortlist|select|approve|reject|supersede
  [--reason-hex HEX] [--related-take-id UUID --related-take-expected-revision U64]
  [operation/client options]
mengxia take reopen --command-id UUID --project-id UUID --work-item-id UUID
  --work-revision-id UUID --terminal-take-id UUID --expected-revision U64
  --new-primary-asset-id UUID [operation/client options]
mengxia take list --project-id UUID --work-item-id UUID --work-revision-id UUID
  [page/cursor/operation/client options]
```

`operation/client options` means optional `--operation-timeout-ms U64` followed by
the already accepted TASK-008 transport options; omission selects the resolved
ceiling. `page/cursor/operation/client options` adds `--page-size U32` and optional
`--cursor HEX`; CLI omission of page size sends the existing fixed default `32`,
while a raw wire value of zero remains `VALIDATION_ERROR`. An absent cursor selects
the first page. Unknown, repeated
singleton, missing, empty, half-paired or operation-inapplicable options fail locally
with exit 2/`VALIDATION_ERROR` and no connection. The graph parser rejects a member
before a resource, a resource before a representation, empty groups and any count or
frame overflow. Transition option presence follows §4.5 exactly.

Successful stdout is ASCII and newline terminated. Mutations emit one header
`MENGXIA_RESULT operation=<operation-id> replayed=<0|1>` followed by one
`<field>=<canonical-value>` line per response field in numeric tag order, excluding
the already represented `replayed`; no field is omitted except an absent optional
related Take. Lists emit
`MENGXIA_PAGE operation=<operation-id> snapshot=<u64> count=<u32>
next_cursor=<320-lowercase-hex|NONE>`, then one `ROW index=<u32>` block per row with
fields in the exact nested-message tag order. Repeated Subject/Asset IDs use
ascending raw-ID order; outgoing Take relationships use the exact kind/target order
from §8 and indexed keys
`subject_id.<n>`, `asset_id.<n>` and
`relationship.<n>={REOPENS|SUPERSEDES}:<relationship-uuid>:<target-take-uuid>`.
`ROW index` and every repeated-field `<n>` are zero-based decimal. Ordinary field
keys are the protobuf snake-case field names; a nested message is expanded in place
at its parent tag using dot-separated keys. Thus Project tag 9 is emitted as
`current_specification.project_spec_revision_id`,
`current_specification.sequence`, `current_specification.resolution_width`,
`current_specification.resolution_height`,
`current_specification.frame_rate_numerator`,
`current_specification.frame_rate_denominator`,
`current_specification.aspect_ratio_numerator`,
`current_specification.aspect_ratio_denominator`,
`current_specification.policy_schema_version`,
`current_specification.color_policy_json_hex`,
`current_specification.audio_policy_json_hex`,
`current_specification.quality_policy_json_hex`,
`current_specification.privacy_policy_json_hex` and
`current_specification.policy_sha256` in that exact nested tag order, before outer
Project tag 10 `effective_trust`. Every absent
optional scalar in a list row is still emitted at its position with literal `NONE`;
singular list-row fields are never otherwise omitted. A repeated field emits zero
lines when empty and otherwise emits exactly one indexed line per element. Enum
values use the exact uppercase
suffix tokens defined by the governing enum (`ACTIVE`, `SCENE`, `CANDIDATE`,
`UNTRUSTED`, and so on), without the protobuf type prefix.
Untrusted name/code/logical-name/reason/JSON values are always lowercase hex with a
`_hex` field suffix; digest bytes are lowercase 64-char hex; IDs are canonical UUID;
timestamps are separate signed-seconds/unsigned-nanos decimals. Stderr uses only the
accepted static error envelope, and failure exits retain the TASK-007/008 mapping.
Golden subprocess tests freeze help text, option ordering equivalence, exact stdout
bytes and the absence of raw metadata/control/ANSI sequences.

### 7.3 Complete API-010 dispositions

All fifteen operations require protocol 1.3 and the authenticated Library-owner
Client. Queries are side-effect free and commandless. Mutations are pure SQLite
transactions bound to the exact operation/principal/request digest and replay only
with the same command ID; they have no external effect. Validation occurs before
claim except authorized existence, ancestry, expected-revision and state checks,
which occur inside the transaction in §9 order.

| Operations | Additional validation/authorization | Side effect/result | Operation-specific failures |
|---|---|---|---|
| Asset revision create | complete bounded graph; ACTIVE Asset; exact parents, Blob custody and expected revision | immutable graph, Asset revision/update time, event and legacy result | `NOT_FOUND`, `CONFLICT`, `REVISION_EXHAUSTED`, custody corruption |
| Asset retire/restore | exact Asset and expected revision; exact legal lifecycle edge | lifecycle/revision/update time, one event/result; no byte/Location change | `NOT_FOUND`, `CONFLICT`, `INVALID_TRANSITION`, `REVISION_EXHAUSTED` |
| Project create/revise | complete bounded canonical ProjectSpec; revise proves Project and expected revision | immutable spec plus current pointer/revision/event/result | `NOT_FOUND`, `CONFLICT`, `REVISION_EXHAUSTED` |
| Project/Subject list | cursor and page/response caps; owner scope | current bounded page only | `VALIDATION_ERROR`, `STORAGE_CORRUPTION` |
| Subject create | bounded kind/name; no name-based identity inference | new global Subject/event/result | ID/sequence exhaustion |
| Work create/revise | exact Project; complete JSON and relationship sets; revise proves Work ancestry/revision | immutable WorkRevision, pointer/revision/relationships/event/result | `NOT_FOUND`, `CONFLICT`, `REVISION_EXHAUSTED` |
| Work list | exact Project ancestry and cursor scope | bounded current Work page | `NOT_FOUND`, `VALIDATION_ERROR`, `STORAGE_CORRUPTION` |
| Take create | exact Project/Work/WorkRevision and existing Asset | new candidate/ordinal/event/result | `NOT_FOUND`, ordinal/event exhaustion |
| Take transition | exact ancestry, option shape, expected revisions and §4.5 edge | atomic Take(s), bounded outgoing relationship, ordered event(s), result | `NOT_FOUND`, `CONFLICT`, `INVALID_TRANSITION`, exhaustion |
| Take reopen | exact terminal Take/revision; different existing Asset | new candidate and one REOPENS edge/event/result | `NOT_FOUND`, `CONFLICT`, `INVALID_TRANSITION`, exhaustion |
| Take list | exact ancestry and cursor scope | bounded current page with at most two outgoing edges per row | `NOT_FOUND`, `VALIDATION_ERROR`, `STORAGE_CORRUPTION` |

Every operation may additionally return the already registered authentication,
frame/decode, `DEADLINE_EXCEEDED`, `OPERATION_CANCELLED`, `STORAGE_BUSY`,
`STORAGE_IO_ERROR`, `STORAGE_CORRUPTION`, configuration and `INTERNAL_ERROR`
families under their existing static/redacted contracts. Pre-transaction deadline or
busy uses `RETRY_ACTION_SAME_COMMAND`; an unknown post-response outcome also requires
the same command ID. Existing exact replay is terminal success. Command-binding
mismatch uses `CONFLICT`/`RETRY_ACTION_NONE`; expected-revision conflict permits
`RETRY_ACTION_FRESH_COMMAND` only after a fresh read. Validation, authentication,
not-found, invalid-transition, corruption and configuration errors are not automatic
retries. There is no server retry. Disconnect/cancel after transaction admission
joins commit/rollback; the caller resolves uncertainty by exact-command replay.

Wire and CLI pagination semantics are exactly §8. Existing operation tags 1..7 and
all protocol 1.0/1.1/1.2 request/result behavior are unchanged. A 1.0..1.2 negotiated
session requesting tag 16..30 receives the existing unsupported-version/operation
error without dispatch or side effect.

Mutation responses are typed views of the exact §5 stored result and rehydrated
canonical row, never caller echo. Query rows are closed as follows:

| query | per-row observable projection |
|---|---|
| `project.list.v1` | Project ID/name/effective `UNTRUSTED`/revision/timestamps plus current ProjectSpecRevision ID, sequence, scalar fields, four canonical JSON values and digests |
| `subject.list.v1` | Subject ID/kind/name/revision/created timestamp |
| `work.list.v1` | WorkItem ID/kind/code/revision/timestamps plus current WorkRevision ID, sequence, format version, canonical JSON/digest and raw-ID-sorted Subject/Asset sets |
| `take.list.v1` | Take ID/WorkRevision ID/ordinal/state/primary Asset/revision/timestamps plus only its at-most-two outgoing relationship ID+kind+target tuples ordered REOPENS then SUPERSEDES (then target raw ID) |

IDs are canonical UUID strings at the CLI, timestamps use existing seconds/nanos
fields on wire and safe stable CLI rendering. Wire strings/JSON remain bounded UTF-8;
the CLI renders every untrusted name, code, reason and canonical JSON value as
lowercase hex with explicit field labels, never as raw terminal text. No
path/locator/SQLite or policy interpretation is returned. Historical
ProjectSpecRevision/WorkRevision lookup needed by a later Run is a typed store port
by exact ID; TASK-009 does not invent a generic history/CRUD endpoint.

Every operation is one authenticated Library-owner session, has one absolute
deadline, no server retry, no detached work and one terminal response. Mutation
timeout range is 100..5,000 ms under new typed
`MENGXIA_MAX_METADATA_OPERATION_TIMEOUT_MS` (default/maximum 5,000; tightening only).
Queries use the same range and also stop at the response-byte cap. Timeout before
transaction returns `DEADLINE_EXCEEDED`; once a pure SQLite transaction begins it
either commits/rolls back and is joined before response. A lost response is retried
with the same command ID. Cancellation never leaves a durable `CLAIMED` row for a
pure TASK-009 command.

## 8. Pagination and bounded response contract

Migration 0002 advances both completed TASK-008 cursor codecs before serving any
request. `ListAssets` cursor v2 remains 80 bytes with the exact TASK-008 layout,
operation discriminator 1 and checksum, but changes magic to hex
`4d584c4355523200` (`MXLCUR2\0`) and format version to 2. `InspectAsset` cursor v2
remains 208 bytes with its exact TASK-008 layout, operation discriminator 2,
phase/range rules and checksum, but changes magic to hex `4d58494355523200`
(`MXICUR2\0`) and format version to 2. Both v2 decoders are bound only to exact
migration generation 0002. Once the Library is 0002, v1 is always
`VALIDATION_ERROR`; an absent first-page cursor directly emits v2. No decoder
reinterprets old bytes. After an explicit offline restore, the old TASK-008 binary
opens exact 0001 and continues to use v1. Golden/concurrency tests run both old and
new binaries across upgrade/restore and prove no cross-generation acceptance.

`ListProjects`, `ListSubjects`, `ListWork` and `ListTakes` accept page size 1..64.
Encoded response content is additionally capped at 1,048,576 bytes before framing;
the adapter may return fewer rows and a cursor, but if one valid row alone exceeds
the cap the row's own canonical bounds are corrupt/incompatible and the operation
fails closed rather than looping.

All use one exact 160-byte opaque cursor:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 16 | per-operation magic |
| 16 | 2 | version = 1 |
| 18 | 2 | record length = 160 |
| 20 | 4 | flags = 0 |
| 24 | 16 | Library ID |
| 40 | 1 | scope kind: NONE=0, PROJECT=1, WORK_REVISION=2 |
| 41 | 7 | zero reserved |
| 48 | 16 | scope ID or all-zero |
| 64 | 8 | captured membership endpoint |
| 72 | 8 | last returned key |
| 80 | 32 | SHA-256 of exact filter/scope contract |
| 112 | 16 | zero reserved |
| 128 | 32 | SHA-256(bytes 0..128) |

The exact 16-byte magics are `MX9_PROJECTS_V1\0`, `MX9_SUBJECTS_V1\0`,
`MX9_WORKLIST_V1\0` and `MX9_TAKELIST_V1\0`. Project/Subject cursors require
`scope_kind=NONE` and an all-zero scope ID; Work requires `PROJECT` and its Project
ID; Take requires `WORK_REVISION` and its WorkRevision ID. The filter digest is
`SHA-256("MENGXIA_" || operation_id || "_FILTER_V1\0" || scope_kind_u8 ||
scope_id[16])`; operation ID is its exact lowercase ASCII registry value. There are
no hidden filters in V1. The checksum detects accidental/malformed cursors but is
not authorization: every request independently authenticates the channel and
revalidates its scope.

These four format-1 cursors are independently bound to exact migration generation
0002 because they did not exist under 0001. Any later accepted migration must issue a
new format/magic and reject these bytes rather than reinterpret them. Format numbers
are per operation family: their value 1 does not make them compatible with TASK-008
cursor v1.

Project/Subject/Work membership is ordered by immutable creation commit sequence;
Take membership is ordered by store-assigned ordinal. The first page captures the
current endpoint in the same read transaction. Continuations bind Library, operation,
scope and filter digest, require the endpoint still exists, and seek strictly after
the last returned key over the accepted ordered index. At most `page_size + 1`
membership-index entries and the corresponding bounded table rows are read; no
claim is made that every response column is stored in a covering index. Membership
is snapshot-frozen; mutable
revision/state fields are current and explicitly carry their revision. TASK-009 has
no delete, so missing snapshot members are corruption. Query-plan tests reject temp
sorts, unbounded residual filters and non-index scans.

`ListTakes` joins only relationships whose `source_kind='TAKE'`, source ID is the
current row and kind is `TAKE_REOPENS|TAKE_SUPERSEDES`. The two partial unique indexes
make this projection at most two rows. Output follows the existing source index order
`(relationship_kind, target_id)`, which is REOPENS before SUPERSEDES for the closed
tokens; no temp sort is permitted. Incoming relationships are not read, counted or
serialized. A duplicate outgoing kind or any endpoint/type mismatch is
`STORAGE_CORRUPTION`, not truncation.

Endpoint and last-key fields are unsigned u64 big-endian. Take ordinals are widened
losslessly from u32. The protobuf carries the exact 160 bytes; CLI input/output uses
the existing canonical lowercase hex convention (exactly 320 ASCII characters) and
rejects uppercase, wrong length or non-hex input before decode.

## 9. Transaction, idempotency and error precedence

All TASK-009 mutations are SQLite-only:

```text
authenticate channel
-> validate bounded scalars/JSON and compute semantic digest
-> writer admission
-> BEGIN IMMEDIATE
-> observe exact CommandRecord binding
-> replay/conflict without clock/entropy OR, only if absent, lazily sample all IDs
   and one timestamp through the Core-owned seams
-> insert CLAIMED
-> authorized existence/project-context checks
-> expected-revision/state checks
-> state + relationships + event(s) + result payload
-> validate replay view and event allocator
-> COMMIT
-> terminal response
```

Because claim and mutation share the transaction, a crash yields either no row/effect
or one complete replayable outcome. `RECOVERY_REQUIRED` is not a normal TASK-009
pure-command state. Existing ingest/materialize recovery behavior remains unchanged.
The exact-0002 startup validator therefore rejects any TASK-009 operation in
`CLAIMED` or `RECOVERY_REQUIRED`; only the already accepted external-effect
ingest/materialize operation/state matrices may survive a transaction. It also
rejects a new-operation terminal rejection whose safe code is outside the closed
ErrorCode registry.

The writer executes the new-command value factory only after the absent-row decision
while holding the same transaction; generated object/event/relationship IDs are
pairwise distinct and the one sampled timestamp is used for command, rows and
events. ID/clock failure rolls back with `ID_GENERATION_UNAVAILABLE`. Exact replay
does not call either seam, resample values or update timestamps. The deadline is
checked before transaction entry and again after new-value generation, before the
first insert.

Error precedence is:

1. transport authentication/version/frame/decode failure;
2. bounded syntax, JSON, cursor and timeout validation before claim;
3. existing command binding replay/conflict;
4. ID/time generation for an absent new command;
5. authorized object existence and exact Project/parent membership;
6. expected revision (`CONFLICT`);
7. legal state transition (`INVALID_TRANSITION`);
8. revision/ordinal/event-sequence exhaustion;
9. typed busy/I/O/corruption/internal store failure.

Not-found responses reveal only the already-authorized object type and supplied ID.
No raw SQL, JSON, reason, name, path, UID, digest or row content enters safe errors.
No new `ErrorCode` is required.

## 10. Security and authorization verification

- principal is only TASK-003's authenticated peer/Library owner UID;
- Project IDs select work context but never establish tenant or global identity
  ownership;
- every scoped operation proves the full Project -> WorkItem -> WorkRevision -> Take
  chain in the same transaction;
- cross-Project tests prove shared global Subject/Asset references are allowed while
  a Work/Take ID under the wrong Project context is denied without mutation;
- arbitrary JSON cannot grant trust, approval, rights, egress or authority;
- approval is an explicit Take transition by the authenticated V1 owner, not an
  Admin operation and not inferred from ProjectTrust;
- reasons and metadata are persisted only in bounded canonical fields and excluded
  from logs/errors/metric labels;
- no secret, path, backend/locator, SQLite handle or CAS capability is exposed;
- queues, page rows, response bytes, JSON, graph counts, events and deadlines are
  finite; retry is caller-controlled and bounded;
- all multi-row state/event/outcome changes are one transaction; no external effect
  or destructive behavior exists.

## 11. Observability and health

TASK-009 extends only ADR-0011's closed operation label registry with the fifteen
operation IDs in §7. Existing `CoreLogEvent`, static per-error metrics, transaction
latency, queue depth and outcome labels are reused. It adds no raw metadata field,
unbounded label, exporter or numeric performance SLO.

The operation event lifecycle remains admitted/started/completed/rejected and
records bounded duration, operation, outcome and safe error code where applicable.
TASK-009 local pure commands introduce no new startup recovery class and do not alter
readiness semantics. Migration failure prevents endpoint publication; a post-open
creative row/event/outcome invariant failure follows the existing fatal local-store
shutdown path.

## 12. Migration and corruption verification

The accepted implementation must prove:

- exact 0000/0001 byte/digest preservation and exact candidate 0002 bytes/hash;
- clean 0000 -> 0001 -> 0002 and existing populated 0001 -> 0002 upgrades;
- old `ASSET`, `ASSET_REVISION`, `LOCATION` success, rejection and recovery rows are
  byte-equivalent in shared columns and replay identically after upgrade;
- existing ingest/materialize operation-first replay remains exact;
- domain/provenance event IDs, commit order, FKs, append-only triggers and allocator
  remain exact;
- snapshot/intents cover absent, partial, complete, tampered, wrong inode/mode/ACL,
  WAL/SHM, disk-full and every SQL transaction failure prefix;
- restore of the verified pre-0002 snapshot plus previous binary reopens exact 0001;
- extra/missing table/index/trigger/view, changed SQL, migration row, payload/hash,
  typed ID/timestamp/revision/JSON, relationship endpoint, current pointer, sequence,
  event or command mapping fails closed;
- `quick_check`, `foreign_key_check`, exact schema allowlist and forced-index probes
  pass before worker admission.

The full reopen validator checks every table contract and boundedly proves singleton,
current pointer, sequence, command creator, relationship endpoint, selected-Take
uniqueness and event/result ownership invariants. Large-table semantic checks are
performed through indexed paged scans, not unbounded collection.

## 13. Candidate acceptance criteria

Canonical synchronization should retain `AC-011`, record the unscored AC-010
prerequisite boundary in §2.4, split
`AC-016` because its Attempt-history clause is not implementable before TASK-015,
and add these TASK-009-specific criteria in currently unused IDs:

```gherkin
AC-091
Given a populated exact migration-0001 Library and a verified durable snapshot/manifest pair
When migration 0002 succeeds, fails or is interrupted
Then 0000/0001 bytes remain immutable and every old command/event/FK replays exactly
And the Library exposes either exact 0001 recovery or exact verified 0002, never a
partially accepted schema.

AC-092
Given a Project or WorkItem at an expected revision
When its specification is revised
Then one immutable canonical revision is appended and the current pointer advances
atomically with one command outcome and DomainEvent
And prior revisions and JSON bytes remain unchanged.

AC-093
Given two Projects in one Library
When immutable WorkRevisions reference the same global Subject or Asset
Then both typed relationships are valid without transferring identity ownership
And using either Work/Take ID under the wrong Project context is denied.

AC-094
Given a Take in any V1 state
When a requested transition, explicit replacement or reopen is evaluated
Then only the exact transition table can commit with optimistic concurrency
And terminal Takes never mutate; reopen creates a new candidate and relationship.

AC-095
Given a completed TASK-009 command
When the exact command is replayed or its payload/event/canonical row is corrupted
Then exact binding returns the original typed result and no duplicate effect
And any mismatch conflicts or fails closed without disclosing another result.

AC-096
Given concurrent Project, Subject, Work or Take creation/mutation
When bounded list queries continue across pages
Then membership and ordering follow the accepted snapshot/keyset
And no item is duplicated, omitted, cross-scoped or returned beyond row/byte caps.

AC-097
Given an authenticated ordinary owner Client
When each TASK-009 semantic CLI/API operation executes
Then it is reachable, bounded and observable without generic CRUD, caller actor,
Admin, media bytes, raw storage authority or a multi-tenant claim.
```

`AC-010` and `REQ-003`: no PASS status in TASK-009; only the unscored prerequisite
evidence in §2.4, with terminal ownership retained by TASK-015 plus Provider
validation. `AC-041`: no-byte-change/domain-history contributor evidence only;
terminal owner TASK-022. `REQ-006`: immutable-input contributor evidence only.
`AC-016`: TASK-009 may record state/event `CONTRIBUTOR_PASS` only; TASK-015 retains
terminal ownership of its Attempt-history clause. `AC-011` may be terminal PASS here.

## 14. Stable candidate test registry

| Test ID | Mandatory evidence |
|---|---|
| `TEST-MIGRATION-009` | exact bytes/hash, clean/populated upgrade, retained manifest, immutable no-sidecar snapshot open, link-count/capacity/journal fault/SIGKILL matrix and offline restore |
| `TEST-SCHEMA-009` | complete object/column/index/FK/trigger/partial-index allowlist, including Take source cardinality, and negative mutations |
| `TEST-OUTCOME-009` | every v1 result codec/golden/hash/operation-kind matrix, optional future result ID and strict legacy non-null IDs |
| `TEST-REPLAY-009` | old and new exact replay/conflict/terminal outcomes across migration/restart |
| `TEST-EVENT-009` | shared allocator, append-only, payload TLV/hash and event/aggregate matrix |
| `TEST-DOMAIN-009` | Project/Subject/Work/Take values, immutability, caps and exhaustive transitions |
| `TEST-JSON-009` | UTF-8/duplicate/depth/node/key/string matrix; exact i64/u64 callbacks, finite f64 strict endpoints, overflowing integer-to-f64 rejection, exponent/fraction boundaries, canonicalization and hash |
| `TEST-CONFIG-009` | four-layer timeout and shared reserve priority, identical Store/Blob typed values, bounds/error matrix and pre-namespace failure |
| `TEST-PROTO-009` | exact 16..30 request/response tags/messages/descriptor, AssetSummary version gating, cursor generation and immutable 1.0/1.1/1.2 compatibility |
| `TEST-CLI-009` | every exact §7.2 semantic command/query grammar, stdout bytes, bounded output, redaction and stable prior help |
| `TEST-AUTH-009` | peer-derived owner, actor/Admin denial and complete cross-Project context negatives |
| `TEST-PROJECT-009` | create/revise/current pointer/overflow/replay atomicity |
| `TEST-SUBJECT-009` | global create/list and shared cross-Project reference semantics |
| `TEST-WORK-009` | create/revise/relationship/immutable spec and code uniqueness |
| `TEST-TAKE-009` | create, every legal/illegal edge, explicit supersede, terminal reopen and reason |
| `TEST-ASSET-LIFECYCLE-009` | revision API plus retire/restore, no byte/custody side effect |
| `TEST-CONCURRENCY-009` | duplicate command, expected revision, ordinal, selected-Take and shutdown races |
| `TEST-PAGINATION-009` | TASK-008 v1 rejection/v2 vectors, 0002-bound new cursors, concurrent commits/mutations, at-most-two outgoing Take edges, byte cap and query plans |
| `TEST-CORRUPTION-009` | typed rows, pointer/sequence/relationship/result/event/hash corruption |
| `TEST-RECOVERY-009` | exact migration/pure-transaction restart outcomes; no durable pure CLAIMED row |
| `TEST-ERROR-009` | precedence, static safe message/retry action and redaction canaries |
| `TEST-OBSERVABILITY-009` | closed labels/events/durations and metadata/reason exclusion |
| `TEST-LIFECYCLE-009` | bounded writer/read admission, disconnect/deadline/panic/join/shutdown |
| `TEST-ARCH-009` | dependency/file/public surface; no generic CRUD/Admin/CAS/unsafe/later-task edge |
| `TEST-SUPPLY-009` | exact JSON dependency features/MSRV/license/advisory/offline lock evidence |
| `TEST-DOC-009` | proposal/ADR/spec/plan/review/intake/AGENTS/AC/TEST/file-scope agreement |
| `TEST-ENDTOEND-009` | CLI -> authenticated daemon -> app -> store -> restart/replay for every operation family |

Developer gate runs every deterministic unit/integration/proto/CLI/architecture/doc
test and the complete retained repository baseline. Formal gate adds real APFS
migration snapshot/SIGKILL/fault/stress evidence and the retained second-UID job.
No required test may be skipped when marking DONE.

## 15. Canonical synchronization required before activation

One acceptance-only documentation change must:

1. accept ADR-0012 and close `REVIEW-GAP-005`;
2. copy exact Feature/Requirement/Decision/AC/TEST/file scope into Plan;
3. add AC-091..AC-097 as bare Gherkin definitions and all TEST rows in the mechanical
   formats required by `document_traceability.rs`;
4. record the unscored `AC-010`/`REQ-003` prerequisite, the `AC-016`, `AC-041`,
   `REQ-006` and `REQ-015` contributor/terminal-owner splits and the §2.7 audit
   refinement;
5. refine §8.2, §8.6.1, §8.7, §9.0/§9.1, §10.2/registry, §14.1, §15, §16, §18,
   §22 and the decision/review records;
6. add `MENGXIA_MAX_METADATA_OPERATION_TIMEOUT_MS` and fixed JSON/response caps;
7. mark only TASK-009 `IN_PROGRESS / TASK_009_ONLY` after explicit authorization;
8. keep TASK-010+, Admin, root rebind, Provider/Plugin and destructive behavior
   blocked.

Before any production edit, `scripts/verify-repository.sh docs`, `git diff --check`
and the retained developer baseline must pass on the synchronized start candidate.

## 16. Implementation order after acceptance

```text
STEP-1  canonical synchronization, ADR-0012, stable tests and exact gate script
STEP-2  immutable protocol-1.2 fixtures and migration candidate byte/hash lock
STEP-3  migration snapshot/intent authority and 0002 upgrade/reopen validator
STEP-4  extensible command/event codecs and old-result compatibility replay
STEP-5  pure domain values, canonical JSON and transition property tests
STEP-6  store ports/repositories and Asset lifecycle operations
STEP-7  Project/Subject/Work/Take app services, pagination and concurrency
STEP-8  additive protocol 1.3, daemon dispatch and thin CLI
STEP-9  observability/error/lifecycle/recovery integration
STEP-10 full developer/formal validation, diff/security review and per-AC evidence
STEP-11 reviewed macos-26 CI evidence and completion-only canonical record
```

Each step must leave prior tests green. No step authorizes the next task.

## 17. Active start record

```text
TASK009_CANONICAL_GATE: ACCEPTED
TASK009_LIFECYCLE: IN_PROGRESS
TASK009_IMPLEMENTATION_AUTHORITY: TASK_009_ONLY

SCOPE: TASK-009 ONLY — forward migration 0002, extensible bounded command/event
       outcomes, Asset revision/lifecycle product operations and Project/Subject/
       WorkRevision/Take semantic commands/queries.
FEATURES: FUNC-003, FUNC-004
REQUIREMENTS:
  REQ-003, REQ-004, REQ-006, REQ-007, REQ-008, REQ-010, REQ-011,
  REQ-012, REQ-013, REQ-014,
  DATA-001, DATA-007, DATA-009, DATA-010, DATA-011, DATA-012,
  API-001, API-002, API-003, API-008, API-010, API-011,
  SEC-005, SEC-013, SEC-014, SEC-017, SEC-020, SEC-021,
  REL-001, REL-004, REL-005, REL-006,
  OPS-001, OPS-002, OPS-003, OPS-004, CFG-001, CFG-003
PREREQUISITES: TASK-006 DONE; TASK-008 DONE; REVIEW-GAP-005 CLOSED;
               ADR-0012 ACCEPTED
DECISIONS:
  BASE-001, BASE-002, BASE-003, BASE-004, BASE-007, BASE-010,
  BASE-011, BASE-012, BASE-014, BASE-016, BASE-018, BASE-019,
  DEC-001, DEC-002, DEC-003, DEC-004, DEC-006, DEC-007, DEC-008,
  DEC-015, DEC-016, DEC-017, DEC-019, DEC-020, DEC-021,
  ADR-0001, ADR-0003, ADR-0004, ADR-0008, ADR-0009, ADR-0010,
  ADR-0011, ADR-0012
ACCEPTANCE: AC-011, AC-091, AC-092, AC-093, AC-094, AC-095,
            AC-096, AC-097
UNSCORED_PREREQUISITE_ONLY: AC-010; REQ-003
CONTRIBUTOR_ONLY: AC-016; AC-041; REQ-006; REQ-015
TESTS: all twenty-seven TEST-*-009 IDs in proposal §14
DEVELOPER_GATE: scripts/verify-task-009.sh developer
FORMAL_COMPLETION_GATE: scripts/verify-task-009.sh formal
AUTHORIZED_FILES: accepted proposal §3 exact list
FORBIDDEN: accepted proposal §3.1; TASK-010+ remains unauthorized
```

This record is active only together with canonical Specification v1.1.34 and the
exact Plan start record. It authorizes no file or behavior outside §3.

## 18. Independent review checklist

- recompute candidate SQL bytes/hash and execute it with bundled SQLite 3.53.4;
- populate every legacy command/result/event shape before upgrade and compare exact
  post-upgrade replay, not merely row counts;
- validate the foreign-keys-off rebuild protocol, checked capacity formula,
  DELETE-journal/WAL mode transitions, immutable no-sidecar snapshot authority,
  retained manifest and every inode/link-count crash prefix against actual TASK-004
  path/namespace behavior;
- independently recompute every result/event/cursor/digest golden vector;
- verify all indexes with `EXPLAIN QUERY PLAN` on realistic cardinalities;
- review Subject and relationship reachability, Project non-tenant semantics and
  Take explicit supersede/reopen behavior;
- verify JSON duplicate rejection/canonicalization, exact visitor-level numeric
  endpoints including overflowing integer tokens, and dependency supply facts;
- ensure CoreRequest 8..15/CoreResponse 8..14 remain reserved, response tag 15
  remains the error, new tags are exactly 16..30, TASK-008 cursor v1 is rejected
  under 0002 and 1.0/1.1/1.2 fixtures remain byte-stable;
- verify no operation requires Admin, Provider, CAS write, root rebind or later
  migration;
- verify exact file scope includes every mechanically required driver/test update;
- reject any TASK-009 PASS/CONTRIBUTOR_PASS claim for AC-010 or REQ-003 and any
  completion claim that marks full REQ-006 PASS;
- require complete local and reviewed formal CI evidence before DONE.

## 19. Current next action

TASK-009 is complete and its implementation authority is revoked. The next safe
action is TASK-010 pre-start analysis/document work only; no TASK-010 production
implementation is authorized by this completion record.

## 20. Formal completion evidence

```text
STATUS: PASS
EXACT_REVIEWED_HEAD: fa7a0047c95c8b8eba12e859284223a1a78f51e2
REVIEWED_MACOS_26_RUN: 34552988098
FORMAL_AGGREGATE: PASS / 10m18s
REAL_SECOND_UID: PASS / 1m16s
ACCEPTANCE: AC-011 PASS; AC-091 PASS; AC-092 PASS; AC-093 PASS;
            AC-094 PASS; AC-095 PASS; AC-096 PASS; AC-097 PASS
UNSCORED_PREREQUISITE_ONLY: AC-010; REQ-003
CONTRIBUTOR_ONLY: AC-016; AC-041; REQ-006; REQ-015
SECURITY: SEC-005 PASS; SEC-013 PASS; SEC-014 PASS; SEC-017 PASS;
          SEC-020 PASS; SEC-021 PASS
REQUIRED_UNEXECUTED_TESTS: NONE
TASK009_LIFECYCLE: DONE
TASK009_IMPLEMENTATION_AUTHORITY: NONE
```

Both `scripts/verify-task-009.sh developer` and `scripts/verify-task-009.sh formal`
passed on the exact reviewed head. The local/formal aggregates covered all
twenty-seven stable TASK-009 mappings, retained TASK-001 through TASK-008 gates,
workspace/Clippy/document/naming checks, supply-chain policy, migration recovery,
WAL/crash evidence and the formal 1/10/100 GiB generated-stream test. Reviewed
arm64 `macos-26` run `34552988098` passed the formal aggregate in 10m18s and the
separate real second-UID job in 1m16s.

The completion diff review found no change outside accepted proposal §3, no rewrite
of migrations 0000/0001, no unauthorized dependency or unsafe expansion, no secret,
root rebind, Admin, Provider/Plugin, Credential, Rights, destructive behavior or
TASK-010+ implementation. All twenty-seven TASK-009 TEST IDs passed;
required unexecuted tests: `NONE`. Lifecycle: TASK-009 is `DONE`; implementation authority is `NONE`.
