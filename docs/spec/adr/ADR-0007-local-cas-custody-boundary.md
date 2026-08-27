# ADR-0007: Local CAS custody and capability boundary

- Status: ACCEPTED
- Date: 2026-08-26
- Applies to: `TASK-005` and the local-custody portions consumed by `TASK-006`,
  `TASK-007` and `TASK-008`
- Normative detail: `docs/proposals/TASK-005-GATE-PROPOSAL.md`

## Context

TASK-005 must turn an untrusted local pathname into digest-verified durable byte
custody without giving application, CLI or Plugin code a raw CAS path or filesystem
authority. The canonical default `<library_root>/storage` conflicts with TASK-004's
previous exact two-entry completed Library namespace, and later persistence needs a
stable way to distinguish a renamed root from a copied or recreated storage instance.

Large-file ingest also crosses several coupled safety boundaries: source mutation,
bounded memory, concurrent logical and physical capacity reservation, cooperative
cancellation, no-clobber publish, crash prefixes and uncertain staging cleanup. If
these are left to implementation inference, TASK-006/007 would need to redesign the
port or persist an unstable Location identity.

## Decision

1. The accepted TASK-005 supplement is the exact implementation contract. TASK-005
   remains `PENDING` until its separate start record activates that contract.
2. Production local sources are opaque non-forgeable capabilities opened only by
   `LocalBlobStorage`; the ports-only trait uses an associated source type. A
   cooperative non-blocking control seam is present now, while TASK-007 owns product
   deadline duration, authenticated cancellation and outcome mapping.
3. `OpenedLibrary` is the only production minting path for an opaque Blob-root
   authority. It may share only a private strong Library-lock lease. Config and
   authority carry the same opaque root-request identity and a mismatch fails before
   workers or root mutation.
4. TASK-004's completed Library namespace may additionally contain one exact safe
   `storage` directory only beside `.mengxia.lock + library.sqlite3`. Earlier and
   recovery states remain unchanged. Library enumeration becomes bounded as part of
   this compatibility correction; TASK-004 remains `DONE` while TASK-005 owns the
   implementation and retained regression evidence.
5. The local CAS layout is `sha256-v1/<aa>/<bb>/<digest>.blob`; staging is private and
   random. Every internal name is descriptor-relative, no-follow, exact-case proven
   on case-insensitive APFS and same-device. Publish is `NOREPLACE`; an existing
   canonical object is rehashed and never overwritten.
6. Streaming freezes the source descriptor length, reads exactly that many bytes,
   performs one bounded EOF probe, revalidates the descriptor and selected name edge,
   and accepts a stable zero-byte regular file. Stable pre-existing hard links are
   allowed; link or selected-name mutation is detected. Source content or metadata is
   never modified.
7. Admission is one atomic non-blocking operation covering ingest, I/O worker, hash
   worker, logical staging and physical remaining-byte reservations. Current free
   space already accounts for written/orphan blocks; it must additionally cover the
   reserve plus every active remaining byte. No admitted operation later returns
   backpressure.
8. Cleanup is `revalidate -> unlink -> full-sync staging directory`. A failure at any
   cleanup step closes the runtime to new admission because on-disk/accounting truth
   is no longer sufficient for continued mutation. TASK-005 never deletes
   prior-process orphans; TASK-008 may reconcile and TASK-022 alone may later delete
   after OQ-008.
9. A local Location locator is relative and stable. Its backend ID hashes the Library
   UUID plus Blob-root device/inode, so same-inode rename preserves identity while
   copy/recreation/cross-volume movement creates a new instance. TASK-006 persists
   the values opaquely; TASK-007 owns verified transactional rebinding and TASK-008
   owns later verification/reconciliation.
10. Software durability requires the supplement's ordered `F_FULLFSYNC` calls and
    same-OS SIGKILL/fault evidence. No CI result is represented as physical
    power-loss proof.

The fixed local caps are those in ADR-0005 plus eight staging-name attempts, 4,096
observed staging entries and eight retries per logical interrupted read/write/EOF
probe. Source paths may use 1,023 bytes, but a Blob root is limited to 937 bytes so
the fixed 85-byte locator plus separator remains within macOS `PATH_MAX`.

## Consequences

- TASK-005 gains a narrow dependency on completed TASK-004/platform authority; the
  graph remains acyclic.
- `mengxia-platform-fs` and `mengxia-store-sqlite` receive only the exact private
  lease/namespace seams listed by the supplement. SQLite schema, SQL, recovery and
  TASK-003 IPC behavior remain frozen.
- TASK-006 can define persistence against stable backend/locator values without
  owning filesystem semantics. Domain code must keep the locator opaque.
- Cancellation, root movement and orphan handling have forward-compatible seams, but
  no product operation, DB registration, automatic reconciliation, deletion or GC is
  authorized by this ADR.
- A cleanup failure sacrifices same-runtime availability in favor of known capacity
  and namespace truth; restart is required.

## Verification

- AC-074 through AC-081 and the seventeen stable TASK-005 TEST IDs in the accepted
  supplement are mandatory before TASK-005 may become `DONE`.
- Developer feedback and formal completion evidence are separate. Only
  `scripts/verify-task-005.sh formal` may emit the canonical per-ID PASS evidence and
  it aggregates prior task gates exactly once.
- Real APFS tests cover exact-case aliases, source/name mutation, same-inode rename,
  copied/recreated roots, concurrent capacity reservation and cleanup uncertainty.
- The fixed KILL-005 and FAULT-005 registries are immutable after task activation
  except through a reviewed proposal/ADR/specification change.
