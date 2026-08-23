# ADR-0006: macOS filesystem FFI and build-evidence boundary

- Status: ACCEPTED
- Date: 2026-08-22
- Applies to: `TASK-004`, `mengxia-platform-fs`, formal arm64 macOS CI
- Complements: `ADR-0003`, `ADR-0004`, `ADR-0005`

## Context

TASK-004 needs macOS extended-ACL information that the accepted safe Rust dependency
surface does not expose. A small checked-in C shim is therefore unavoidable. The
shim is security-sensitive because its result participates in deciding whether a
Library filesystem path is safe to bootstrap and open.

The initial gate draft also required every used Xcode component to be owned by root
and applied exact release-attestation checks to every developer build. That rule was
stricter than the declared threat model and was incompatible with legitimate hosted
runner images that install Xcode under the runner administrator account. It added
development friction without protecting against root, administrator or same-account
processes, which ADR-0004 already places outside the V1 containment claim.

## Decision

1. `mengxia-store-sqlite` remains `#![forbid(unsafe_code)]`. All macOS ACL FFI and
   the checked-in C shim live in the downward-only `mengxia-platform-fs` crate. The
   Rust FFI module is private and exposes only reviewed safe authority/summary types.
2. The C source, public C header, ABI probe, exported-symbol allowlist and compiler
   argv are checked in. No shell, ambient `PATH`, response file, `cc` crate, bindgen
   or Rust `libc` ACL declarations participate.
3. Builds have two evidence classes:
   - `developer` is the default. It enforces checked-in inputs, fixed C argv,
     absolute Apple tools, path confinement, safe owner/mode checks and ABI probes.
     It records actual tool identities/digests and cannot satisfy release or
     `TEST-SUPPLY-004` attestation.
   - `attested` is selected by the reviewed formal CI/release verification command.
     It additionally enforces the exact Xcode 26.6 build `17F113`, SDK 26.5, Apple
     clang 21.0.0 tuple, manifest paths/digests, cleaned child environment and
     fail-closed hosted-image preflight. Only CI evidence from this class can satisfy
     `TEST-SUPPLY-004`.
4. Before Xcode/tool discovery, the build obtains its effective UID, primary GID and
   supplementary groups only through absolute `/usr/bin/id -u`, `-g` and `-G` with
   cleared environment, fixed locale/argv, bounded strict parsing and root-owned
   non-writable path metadata. `/` must remain root/root `0755`; `/Applications` is the
   sole exception and must be root/admin `0775`. Every selected Xcode bundle/tool/SDK
   component must be owned by root or the recorded build eUID and must not be group-
   or world-writable. A non-root build eUID is accepted only when its recorded groups
   include numeric GID 80 (`admin`). Symlinks must use the same owner set and resolve
   within the accepted bundle.
5. Root, GID-80 administrators and processes running with the build account's
   credentials are inside the trusted build-host boundary. This decision makes no
   containment claim against malicious/concurrent root, administrator or same-eUID
   processes.
6. Formal CI uses the reviewed arm64 `macos-26` job, switches to the exact versioned
   Xcode path before Cargo, records image/tool/identity evidence and fails closed on
   drift. A mutable runner label proves availability only, never tool identity.
7. The complete normative details, hashes, command line, environment rules, ABI and
   negative matrices are in the accepted
   `docs/proposals/TASK-004-GATE-PROPOSAL.md`, incorporated by canonical
   Specification v1.1.11.

## Consequences

- Developers can compile and test without claiming that their machine produced a
  release attestation.
- Runtime Library path/ACL/lock/SQLite protections are unchanged and remain strict.
- A locally selected `attested` string grants no authority; accepted evidence is the
  complete reviewed CI job result.
- CI image drift fails explicitly and requires a reviewed manifest/ADR update.
- Supporting a different OS, architecture, compiler family or custom SQLite VFS
  requires a new decision; it is not an implicit fallback.

## Verification

- `TEST-ARCH-004` verifies unsafe isolation, dependency direction, ABI layout,
  exported symbols and the sole trusted SQLite path consumer.
- `TEST-SUPPLY-004` distinguishes developer records from formal attestation and
  verifies exact tool/source/digest/environment/owner policy plus negative cases.
- `TEST-DOC-004` keeps this ADR, the accepted task contract, canonical registries,
  CI scope and TASK-004 lifecycle record synchronized.
