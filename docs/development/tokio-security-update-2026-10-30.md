# Tokio security update — 2026-10-30

Classification: `REPO_STALE`. PR #20's supply-chain check found the existing
Tokio 1.53.1 pin affected by RUSTSEC-2026-0386. The user explicitly authorized
fixing this dependency, rechecking PR #20, and merging only after checks pass.
This is an incident-specific security patch under ADR-0016, not a new product
task or a relaxation of the supply policy.

## Evidence and scope

- [RustSec advisory](https://github.com/RustSec/advisory-db/blob/main/crates/tokio/RUSTSEC-2026-0386.md)
  identifies unsafe I/O ownership assumptions in the old safe AsyncFd constructors.
- [Tokio 1.53.3 release](https://github.com/tokio-rs/tokio/releases/tag/tokio-1.53.3)
  supplies the patch in the existing minor release series; the advisory marks
  versions at or above 1.53.3 as patched.
- [Upstream change](https://github.com/tokio-rs/tokio/pull/8587) deprecates the old
  constructors and introduces unsafe registration APIs with explicit FD-lifetime
  obligations. Repository Rust sources do not call AsyncFd, so no product API or
  unsafe-code migration is required here. This source observation is not a claim
  that the affected dependency was safe, nor a reason to ignore the advisory.
- Cargo.toml pins exactly 1.53.3 with unchanged features. Cargo.lock changes only
  the Tokio version and registry checksum; no other package or edge changes.
- The immutable TASK-010 lock fixture and its SHA-256 remain unchanged. The
  TASK-011 inventory validator permits only the exact later Tokio version/checksum
  replacement, with all historical dependency edges and other records preserved.
  Regression negatives reject rollback, another patch version, checksum/source
  tampering, edge changes, unrelated packages and a changed historical version.
- No advisory ignore, CI weakening, protocol/migration change, native execution
  qualification or unrelated local work is included.

## Validation

On 2026-10-30, `scripts/check-supply-chain.sh` passed all four cargo-deny
categories and the security-event gate. The complete
`scripts/verify-repository.sh developer` gate passed on arm64 macOS, including
workspace build/check/Clippy/tests, retained task gates, the updated inventory
test and its eight rejection controls, toolchain regressions and shared supply.
An independent lock-record comparison confirmed only the Tokio version/checksum
changed; the historical fixture still hashes to
`302df8141acee77aa58ecb796a53ecbb4faf9f6cd55dc384667bb08e725c0b2e`.

The first local aggregate stopped in the CLI fixture because the temporary
worktree name made its socket path 106 bytes long. A shell trace isolated the
path-validation rejection. The worktree was moved to a shorter path and the
complete aggregate rerun passed, without changing a test or product limit.
This was a validation-environment issue, not a Tokio failure or a skipped test.

Validation results and exact PR/main checks are recorded with
[PR #20](https://github.com/XiaTian-X/MengXia/pull/20). The security-event inventory
records resolution of this dependency advisory only; it does not claim complete
security coverage or full product qualification.
