# ADR-0013: Toolchain evolution and public-repository governance

- Status: ACCEPTED
- Date: 2026-09-11
- Applies to: post-TASK-009 repository/toolchain maintenance
- Authority: user-authorized MAINT-001; no product or TASK-010+ authority
- Supersedes: ADR-0006 only where this ADR explicitly replaces exact developer-host
  path/metadata predicates; its attested evidence and runtime authority boundaries
  remain in force

## Context

The repository is now public. Its existing layered CI provides developer feedback on
pull requests, but the exact formal and real-second-UID gates run only after code is
pushed to `main`. The repository also has no protected-main rule, scheduled security
scan, CodeQL setup, Dependabot security updates, secret scanning or push protection.
That permits a code change to enter `main` before the formal evidence for that change
exists and leaves a quiet repository unaware of newly published advisories.

The build boundary correctly separates ordinary developer evidence from exact formal
attestation, but its developer path policy accepts only two Xcode bundle names and
requires `/` and `/Applications` to equal one exact owner/group/mode tuple. A safe OS
or Xcode update can therefore reject local development even when ownership,
containment, compiler identity and ABI safety still hold. Exact Xcode/SDK/tool digests
are also repeated in the manifest, Rust build script, shell preflight and tests.

Finally, TEST-PROTO-001 requires a formal compiler comparison. The committed source,
descriptor and compiler artifact digests are checked, but no repository command
currently obtains the recorded protoc artifact, regenerates a descriptor in a
temporary directory and byte-compares it. The recorded protoc 35.1 artifact was
independently checked during this decision: with `proto/core/v1` as the sole proto
path and `handshake.proto` as the input, it reproduces the committed descriptor
SHA-256 `f417f10b0a30bb28b234398d9f1a54ef2489c2ee85c512eca68838b5732a9341`.

These are `REPO_STALE`/`SPEC_STALE` maintenance defects. They do not invalidate the
reviewed TASK-001 through TASK-009 product evidence and do not authorize TASK-010.

## Decision

1. Repository evidence has three outcomes, not three production build authorities:
   - `developer-compatible` enforces trusted path properties, Apple compiler
     identity, fixed arguments and ABI probes, records observed identity, and cannot
     satisfy formal/release evidence;
   - `attested` additionally matches the exact active versioned provenance manifest;
   - `upgrade-candidate` is a separate diagnostic command which may report only
     `COMPATIBLE_DEVELOPER`, `REATTEST_REQUIRED`, `UNSUPPORTED` or `UNVERIFIABLE`.
     It never sets the attested build class or edits repository/global tool state.
2. Developer Xcode discovery uses the system-selected developer directory and
   accepts only `Xcode.app/Contents/Developer` or a dotted-decimal
   `Xcode_<version>.app/Contents/Developer` bundle directly below `/Applications`
   after canonical containment, no-follow edge checks,
   trusted ownership, non-writability by untrusted principals, Apple clang identity
   and the existing ABI probes pass. Ambient `DEVELOPER_DIR`, `SDKROOT`, compiler,
   linker and flag overrides remain forbidden.
3. `/` and `/Applications` use monotonic safety predicates. Each must be a real
   root-owned directory and not world-writable. A group-writable `/Applications` is
   accepted only for numeric admin GID 80. A safer non-group-writable root-owned mode
   is accepted regardless of group. Selected Xcode descendants retain the stricter
   existing trusted-owner and no group/world-write rules.
4. `docs/provenance/macos-acl-ffi-toolchain-v1.toml` is the sole current
   machine-readable source for the exact attested tuple and input digests. The Rust
   and shell consumers use closed, bounded parsing and reject missing, duplicate,
   malformed or unknown top-level/input keys. An attested upgrade adds a new
   immutable versioned manifest and updates the active path; it never overwrites the
   reviewed v1 file. Formal CI selects its versioned Xcode directory from that same
   validated active manifest, so a re-attestation PR does not depend on the obsolete
   workflow path it is replacing.
5. Code-bearing pull requests must run developer, attested formal, real-second-UID
   and dependency-review evidence before merge. One unconditional `merge-gate`
   evaluates the change classification and conditional job results; branch rules
   require this stable aggregate. CodeQL results remain independently visible and
   reviewed, but are not a blanket required status while GitHub default setup omits
   fork pull requests; otherwise public external contributions would be impossible
   to merge. The exact merged `main` commit still reruns formal/second-UID evidence
   before a task may claim DONE.
6. Pull-request workflows use `pull_request`, never `pull_request_target`, keep the
   default token read-only, persist no checkout credentials and receive no secrets.
   Third-party actions use full commit SHA pins. A weekly schedule runs the formal,
   second-UID and current advisory checks so hosted-image or dependency drift is
   discovered without waiting for a product change. A deprecated action runtime is
   upgraded to a verified official release under the same full-SHA and dependency-
   review requirements; compatibility forcing is not treated as a stable solution.
7. The public repository enables CodeQL default setup, Dependabot alerts/security
   updates, secret scanning and push protection. Dependency version updates are
   review-only and never auto-merge. Protected `main` requires a pull request,
   successful gates, conversation resolution and linear history and blocks force
   pushes/deletion. While the repository has only one trusted reviewer, an external
   approval is not mandatory; CODEOWNERS becomes mandatory when a second trusted
   reviewer is available.
8. `minimum_deployment_target = "13.0"` is a link target, not proof that macOS 13 is
   supported. Runtime support is claimed only after TASK-023 executes a real
   minimum-OS installation/startup/ACL/APFS/IPC/SQLite/recovery matrix.
9. Volatile local-host observations are evidence artifacts, not recurring canonical
   decisions. A safe OS patch that passes developer gates does not require rewriting
   completed tasks, historic ADRs or accepted proposals.

## Authorized files

MAINT-001 may change only repository governance/current-state documents,
`.github/**`, `SECURITY.md`, repository verification scripts,
`crates/mengxia-platform-fs/build.rs`, and the narrow testkit CI/TASK-003/TASK-004/
document-traceability tests needed to prove this ADR. It may add toolchain/proto
verification scripts. It may not change Cargo manifests/lock, Rust/SQLite/protoc
versions, `third_party/**`, `proto/**`, migrations or production runtime behavior.
The untracked TASK-010 proposal is outside this authority.

## Verification

- `TEST-MAINT-CI-001`: exact PR/push/schedule matrix, stable unconditional aggregate,
  least privilege, SHA pins and no `pull_request_target`.
- `TEST-MAINT-TOOLCHAIN-001`: monotonic root/Application predicates, safe future
  Xcode developer acceptance, exact attested rejection and manifest tamper matrix.
- `TEST-MAINT-PROTO-001`: exact protoc artifact digest, isolated regeneration and
  byte comparison while ordinary offline Cargo builds remain compiler-independent.
- `TEST-MAINT-SUPPLY-001`: current advisory check plus scheduled evidence and
  review-only dependency update policy.
- `TEST-MAINT-DOC-001`: accepted ADR/findings/authority/completion synchronization
  without changing completed-task or TASK-010 lifecycle.

## Rollback

If the aggregate can incorrectly pass, the developer predicate admits an untrusted
path, formal attestation accepts manifest drift, or regeneration can use an ambient
compiler, do not enable/retain the branch rule. Revert the maintenance implementation
to ADR-0010/ADR-0006 behavior, keep product authority disabled, and record a corrected
maintenance finding. Hosted-image/tool unavailability is `UNVERIFIABLE` or
`REATTEST_REQUIRED`, never permission to weaken an exact check.
