# ADR-0003: Foundation Rust toolchain and bundled SQLite

- Status: ACCEPTED
- Date: 2026-08-21
- Closes: `OQ-003`

## Context

MengXia is a new Rust workspace with no backward-compatibility or existing MSRV constraint. The store design requires WAL and multiple connections, so it must not use SQLite versions affected by the WAL-reset corruption bug. The visible host `sqlite3` is Android SDK 3.50.6 and is neither approved nor safe for this design.

Official evidence checked on 2026-08-21:

- Rust's official release index and release announcement identify Rust 1.98.0, released 2026-08-20, as current stable: <https://blog.rust-lang.org/2026/08/20/Rust-1.98.0/>.
- SQLite's official download page identifies 3.53.4 as the current release and publishes the 3.53.4 amalgamation checksum: <https://www.sqlite.org/download.html>.
- SQLite's WAL documentation states the WAL-reset bug affects releases through 3.51.2 and is fixed in 3.51.3 and later: <https://www.sqlite.org/wal.html#the_wal_reset_bug>.
- SQLite documents compile-time options and runtime inspection through `PRAGMA compile_options`: <https://www.sqlite.org/compile.html>.

## Decision

Foundation toolchain:

- Pin Rust `1.98.0` for build and MSRV.
- Use Rust edition 2024.
- Install with rustup's minimal profile plus `rustfmt` and `clippy`.
- CI and local verification use the pinned toolchain, not an unpinned `stable` alias.

SQLite:

- Bundle SQLite `3.53.4` from the official `sqlite-amalgamation-3530400.zip` release artifact; system SQLite is forbidden for canonical store operation.
- Expected official SHA3-256 of the downloaded archive is `628a44cfe82c66aed1ccbbe85a562d2e33ebe64b3288981ed76285612227934e`.
- Compile with the following accepted options:
  - `SQLITE_THREADSAFE=1`
  - `SQLITE_DQS=0`
  - `SQLITE_DEFAULT_FOREIGN_KEYS=1`
  - `SQLITE_DEFAULT_WAL_SYNCHRONOUS=2`
  - `SQLITE_TRUSTED_SCHEMA=0`
- Do not enable optional SQL extensions unless a later dependency review proves necessity.
- On every canonical connection, enable/verify foreign keys, WAL, `synchronous=FULL`, defensive mode and trusted schema off; keep load-extension capability disabled.
- Startup must assert exact SQLite version/source ID and required compile options before accepting mutation.

TASK-001 may install the approved Rust toolchain and obtain verified SQLite tooling/source. TASK-004 owns the final bundled-library integration and runtime assertions.

## Consequences

- No legacy Rust compatibility is claimed for V1.
- Updating Rust or SQLite requires dependency/advisory review and an ADR or recorded dependency decision with new checksums/evidence.
- The Android SDK SQLite and macOS system SQLite may be used only for non-canonical diagnostics; they cannot open or mutate a MengXia Library.
- The downloaded SQLite CLI is developer tooling and does not replace the bundled application library.

## Verification

- `rustc --version`, `cargo --version`, `cargo fmt --version` and `cargo clippy --version` resolve through Rust 1.98.0.
- Downloaded SQLite artifacts match the published SHA3-256 before extraction/use.
- Bootstrap rejects a system/affected/mismatched SQLite runtime.
- `sqlite3_libversion_number`, source ID, thread-safety and `PRAGMA compile_options` match the accepted build.
- WAL/checkpoint concurrency regression and migration/reopen tests run against the bundled build.
