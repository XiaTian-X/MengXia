# ADR-0014: TASK-010 package foundation and managed executable boundary

- Status: ACCEPTED
- Date: 2026-09-11
- Owners: TASK-010 foundation; downstream TASK-012/TASK-013 integration
- Supersedes: no accepted ADR

## Context

The canonical TASK-010 description previously combined pure manifest inspection,
host-path RuntimeDependency verification, package persistence, grants, revocation,
Admin authority and later process activation. That combination blocked the pure
foundation on OQ-010 and did not provide a feasible macOS guarantee that a file
inspected through a retained descriptor would be the exact image later executed by
pathname.

The macOS 26.5 SDK exposes pathname-based execve and posix_spawn and exposes no
fexecve or execveat. A mutable absolute path therefore cannot be accepted as a
durable executable authority merely because it was hashed at inspection time.

## Decision

1. TASK-010 is a non-executing, non-persistent package foundation.
2. A V1 package is one canonical JSON manifest. PackageDigest is SHA-256 over all
   manifest bytes.
3. Runtime dependencies are logical declarations containing ID, role, target,
   length and digest. Manifest V1 contains no host path, URI, command, arguments,
   environment or shell fragment.
4. mengxia-plugin-package owns typed manifest/dependency/permission-request values.
   mengxia-plugin-security depends one-way on package and owns pure PermissionDiff
   evidence.
5. TASK-010 performs bounded in-memory inspection only. It owns no filesystem,
   Tokio, worker, admission, deadline, metrics, store, migration or product API.
6. TASK-012 must prove managed executable custody, exact managed-object-to-process
   binding and sandbox enforcement on real arm64 macOS before any third-party
   Native activation claim.
7. TASK-013 owns authenticated source acquisition, no-replace import into the
   managed content-addressed Plugin executable store, installation persistence,
   grants, revocations, migrations 0003/0004, audit and AC-027.
8. OQ-010 remains mandatory for TASK-013 privileged effects but does not block
   TASK-010 foundation or TASK-011 protocol work.
9. Every grant binds one exact PackageDigest. TASK-010 never inherits a grant or
   changes lifecycle state; downstream policy must persist a new decision or a
   separately auditable derived grant for every new digest.
10. Production JSON Schema validation is explicitly Draft 2020-12 and runtime
    offline. TASK-010 accepts proposal v0.2.3's final-manifest `jsonschema
    0.56.0`/`semver 1.0.28` dependency preflight, exact production/test edges and
    graph of 117 retained packages plus 40 added registry packages. MIT-0 remains
    outside the global allow list and is permitted only by the exact
    `borrow-or-share@0.2.4` license exception after a separate start record.
11. Untrusted instance failures from either JSON Schema or typed semantic checks
    have one `MANIFEST_INVALID` result; validator keyword/error iteration order is
    not observable. TASK-010 contributes only the requirement sub-scopes explicitly
    assigned in proposal v0.2.3 and cannot mark their later enforcement complete.
12. `IMPLEMENTATION_SPEC.md` §19.11 and §20.0.9 are the sole normative TASK-010
    acceptance and stable-test obligation sources; the proposal records IDs only.
13. Remaining `API-001` ownership is exact: TASK-011 Plugin transport proto3,
    TASK-014 Capability JSON Schema, TASK-015 Recipe JSON Schema, TASK-020
    namespaced extension JSON Schema and TASK-023 aggregate release evidence.
14. TASK-012 terminally owns AC-021/AC-022 and contributes sandbox evidence to
    AC-020/AC-023; TASK-013 terminally owns AC-020 and contributes Asset/Lease
    evidence to AC-023; TASK-016 terminally owns AC-023.
15. Lexically invalid JSON number grammar is `MALFORMED_JSON`; a lexically valid
    number outside V1's shortest unsigned `u64` subset is `MANIFEST_INVALID`.

## Consequences

- TASK-010 and TASK-011 can progress without prematurely selecting Admin authority.
- Package manifests remain portable across machines because source paths are not
  part of package identity.
- Installation and activation require a managed executable store and cannot run an
  arbitrary mutable source path.
- AC-027 and migration ownership move to TASK-013.
- TASK-010 cannot claim that declared dependency bytes exist or are executable.
- Failure to prove the TASK-012 launch binding keeps third-party Native activation
  disabled; it does not weaken the check to a last-moment pathname hash.

## Rejected alternatives

### Durable absolute path in the manifest

Rejected because it is machine-specific and mutable, and pathname-based launch
cannot atomically consume the descriptor that supplied the earlier digest evidence.

### Archive or directory package in V1

Rejected because it adds extraction traversal, duplicate-entry, compression,
metadata-normalization and executable-custody surfaces before they are needed.

### Complete grants/revocations in TASK-010

Rejected because Admin evidence and SecurityAuditEvent ownership are not available
until TASK-013. Foundation diff evidence is useful without those mutations.

### Gate A plus a blocking Gate B inside TASK-010

Rejected because TASK-011 needs only the foundation. Making TASK-010 completion
depend on Admin persistence would serialize unrelated work and recreate the
development-efficiency problem this split is intended to remove.

## Verification

- independent review of TASK-010 proposal v0.2.3: PASS 2026-09-11;
- canonical task/migration/AC/dependency synchronization;
- exact JSON Schema and SemVer isolated dependency/deny-policy graph review;
- architecture tests for one-way package/security dependencies;
- canonical ownership and fail-closed boundaries for TASK-012 managed executable
  custody/launch binding and TASK-013 atomic install/grant/revocation/audit.

Detailed platform prototypes, migration designs and composition plans are not
TASK-010 acceptance dependencies. They remain mandatory before their respective
TASK-012 or TASK-013 start records and cannot be inferred from this ADR.
