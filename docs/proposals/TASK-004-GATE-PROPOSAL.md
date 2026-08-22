# TASK-004 accepted implementation contract

> Status: **ACCEPTED / INCORPORATED BY CANONICAL SPECIFICATION v1.1.9**
>
> Date: 2026-08-22
>
> Scope: normative TASK-004 implementation supplement incorporated by reference from
> the canonical Specification. It authorizes only the exact TASK-004 start record and
> implementation scope below; it does not authorize TASK-003 or later capabilities.

## 1. Outcome

The accepted Option A makes `TASK-004` the active task before `TASK-003`. The
dependency graph remains acyclic. The contracts below are accepted as a normative
supplement to canonical Specification v1.1.9 and are bounded by the synchronized
TASK-004 start record in the Implementation Plan.

This revision closes the ambiguities identified by the review and permits the task
to start. It selects a stock-SQLite-compatible filesystem model, removes the bootstrap
owner/lock circular dependency, defines the exact local sys-crate patch boundary,
fixes queue and shutdown accounting, assigns exact error mappings, and expands the
data/crash/corruption evidence. Section 11 is the exact input incorporated into the
canonical start record; lifecycle authority remains in the Implementation Plan.

## 2. Evidence read

- `AGENTS.md`: implementation may not start while the current task's blocker or
  start gate is open.
- Specification v1.1.8 §0.5: stable Requirement/Decision/AC/TEST traceability and a
  synchronized task-start record are mandatory before implementation.
- Specification v1.1.8 §7 and §8.7: `mengxia-store-sqlite` owns storage mechanics;
  TASK-004 owns only immutable migration `0000_store_bootstrap`, while TASK-006 owns
  `0001_library_assets` and domain repositories.
- Specification v1.1.8 §16 and ADR-0005: writer queue default 256/range 16–4096,
  read connections default 4/range 1–16, and SQLite busy budget default 5000 ms with
  tightening-only range 1–5000 ms.
- ADR-0003: canonical operation bundles SQLite 3.53.4 from the accepted official
  amalgamation, verifies its official archive SHA3-256, compiles the accepted
  options, asserts exact runtime/source/options before mutation, and never uses
  system or Android SDK SQLite.
- ADR-0004: first create accepts an absent or correctly owned empty local APFS
  target; rejects canonical metadata, non-empty, symlink, owner/mode mismatch and
  unsupported filesystem targets; records the invoking effective UID once; then
  holds the Library lock for the daemon lifecycle.
- ADR-0005: full writer queue returns `BACKPRESSURE`; busy waits are bounded and
  return a typed result; bounded work must not detach.
- Repository at commit `1228f0a`: TASK-001/TASK-002 are complete; the SQLite store
  remains an architecture skeleton and has no SQLite binding or migrations.
- Reviewed compatible Rust packages are exact `rusqlite` 0.40.2 and
  `libsqlite3-sys` 0.38.2. Upstream 0.38.2 embeds SQLite 3.53.2, so its unmodified
  bundled source cannot satisfy ADR-0003.
- The locally inspected official SQLite 3.53.4 amalgamation reports source ID
  `2026-07-24 19:02:57 bf7c7f30031888f4e796e429ab3978879485813aaca6f641c7b33e4e09459bcc`.
  Its extracted `sqlite3.c` SHA-256 is
  `b1dd5d74ec7f29055a6684fa06fb3c2f6821c87dd38f9a458dfd2e8a1db28189`;
  `sqlite3.h` SHA-256 is
  `919e7f2e8ed1d8f56ac17b412b8971c76aa5d1a879752cc6058f75e7d5910e1d`.
- Apple's `acl_get_flagset_np(3)` contract accepts either an ACL object or ACL entry;
  the reviewed Apple Libc `acl_flag.c` correspondingly returns separate `a_flags`
  and `ae_flags` storage. The shim must therefore inspect and report both levels.
- The selected macOS 26.5 SDK declares `ACL_MAX_ENTRIES == 128` and the public
  `acl_dup`/`acl_clear_flags_np`/`acl_add_flag_np`/`acl_size`/`acl_copy_ext` APIs.
  Portable external-representation reconstruction in §6.1 detects unknown flag bits
  without importing Apple-private `aclvar.h` layout.
- Local toolchain evidence is Xcode 26.6 build `17F113`, SDK 26.5 and Apple clang
  21.0.0; §6.1 records the independently calculated compiler, SDK header and
  `libtool` SHA-256 digests.
- Local `lstat` evidence is `/` = directory UID 0/GID 0/mode `0755`,
  `/Applications` = directory UID 0/GID 80 (`admin`)/mode `0775`, and the selected
  `/Applications/Xcode.app` bundle = directory UID 0/GID 0/mode `0755`. Therefore a
  blanket ban on group-writable ancestors rejects the accepted host; §6.1 replaces
  it with one exact `/Applications` exception and an explicit build-host trust
  boundary rather than silently weakening every ancestor check.
- The current `.github/workflows/ci.yml` uses `runs-on: macos-15`. The GitHub-hosted
  arm64 `macos-15` inventory reviewed on 2026-08-22 provides Xcode only through
  26.3 (with 16.4 selected by default), so it cannot execute the pinned Xcode 26.6 /
  SDK 26.5 FFI gate. The corresponding `macos-26` inventory provides Xcode 26.6
  build `17F113` at `/Applications/Xcode_26.6.app` and macOS SDK 26.5. These mutable
  image inventories establish availability only; they are not toolchain identity
  evidence and never replace the fail-closed tuple/path/digest checks below.

## 3. Resolved gate findings

The original five blockers plus the subsequently identified architecture, security,
CI-environment and build-host-policy findings are resolved by this accepted
contract. Regression against any resolution reopens TASK-004; it does not silently
weaken or bypass the corresponding test.

### `TASK004-BLOCKER-001` — exact bundled SQLite build path

- Category: `DEPENDENCY`; severity: `CRITICAL`; scope: `TASK_SPECIFIC`.
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`.
- Resolution: the exact local-patch policy in §4 is accepted.
- Verification: locked source-pinned offline build, exact source/options assertions,
  and dependency/binary-link inspection in `TEST-SUPPLY-004`.

### `TASK004-BLOCKER-002` — immutable bootstrap/migration contract

- Category: `DATA_MODEL`; severity: `CRITICAL`; scope: `TASK_SPECIFIC`.
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`.
- Resolution: the complete schema, migration grammar, transaction,
  staging publish and typed reopen validation in §5–§6.
- Verification: `TEST-BOOTSTRAP-004`, `TEST-MIGRATION-004`,
  `TEST-RECOVERY-004`, `TEST-WAL-004` and `TEST-CORRUPTION-004`.

### `TASK004-BLOCKER-003` — Library filesystem/lock proof

- Category: `ARCHITECTURE`; severity: `HIGH`; scope: `TASK_SPECIFIC`.
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`.
- Resolution: §6's separate `mengxia-platform-fs` FFI boundary,
  component-by-component descriptor authority from `/`, safe parent bootstrap and
  the explicitly scoped fixed canonical path opened with `SQLITE_OPEN_NOFOLLOW`.
  No custom SQLite VFS or descriptor-relative SQLite filename is proposed.
- Verification: real-filesystem prefix/symlink/ACL cases in `TEST-PATH-004` plus
  two-process cases in `TEST-LOCK-004` and `TEST-BOOTSTRAP-004`.

### `TASK004-BLOCKER-004` — writer/read lifecycle and error taxonomy

- Category: `ARCHITECTURE`; severity: `HIGH`; scope: `TASK_SPECIFIC`.
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`.
- Resolution: §7's exact capacity, admission, cancellation,
  shutdown, read leasing and error-code mappings.
- Verification: `TEST-QUEUE-004` and `TEST-ERROR-004`.

### `TASK004-BLOCKER-005` — canonical registry/start gate absent

- Category: `SPECIFICATION`; severity: `CRITICAL`; scope: `TASK_SPECIFIC`.
- Owner: `WORK_SHOULD_FIX`.
- Resolution: Specification v1.1.9 incorporates §§4–10 and the stable registries;
  the Implementation Plan contains the synchronized §11 start record.
- Verification: `TEST-DOC-004` must reject shorthand/range IDs, inconsistent status,
  or missing references.

### `TASK004-BLOCKER-006` — unsafe code is not isolated in a platform/FFI crate

- Category: `ARCHITECTURE`; severity: `HIGH`; scope: `TASK_SPECIFIC`.
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`.
- Problem: placing an ACL FFI exception inside `mengxia-store-sqlite` conflicts with
  canonical Specification §17 and weakens that adapter's existing unsafe prohibition.
- Resolution: §4/§6's separate `mengxia-platform-fs` canonical crate,
  one private audited FFI module and safe descriptor-only API; keep store at
  `#![forbid(unsafe_code)]`.
- Verification: `TEST-ARCH-004` and `TEST-SUPPLY-004`.

### `TASK004-BLOCKER-007` — absolute path-prefix authority is incomplete

- Category: `SECURITY`; severity: `HIGH`; scope: `TASK_SPECIFIC`.
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`.
- Problem: `O_NOFOLLOW` on the final parent/database component does not protect an
  earlier symlink or changed name-to-inode binding in the absolute path prefix.
- Resolution: §6.2/§6.4's component-by-component walk from `/`,
  retained descriptor chain, mutation-authority policy, edge revalidation and exact
  root/same-eUID threat limitation.
- Verification: `TEST-PATH-004`, `TEST-BOOTSTRAP-004` and `TEST-ARCH-004`.

### `TASK004-BLOCKER-008` — pinned platform tuple is unavailable in current CI scope

- Category: `ENVIRONMENT`; severity: `HIGH`; scope: `TASK_SPECIFIC`.
- Owner: `WORK_SHOULD_FIX`.
- Problem: the current workflow uses `macos-15`, whose hosted arm64 image does not
  contain the Xcode 26.6 / SDK 26.5 tuple required by §6.1, while the previous
  TASK-004 implementation allowlist excluded `.github/workflows/ci.yml`. Strict
  implementation therefore could neither preserve scope nor produce formal CI
  evidence for the safety-critical platform shim.
- Resolution: §6.1's fail-closed `macos-26` CI selection and
  pre-Cargo tuple/path/digest preflight, add only the narrowly bounded workflow file
  change in §8; ADR-0006 plus `TEST-SUPPLY-004` own the
  resulting CI evidence. The runner label is discovery evidence, not proof.
- Verification: `TEST-SUPPLY-004` and `TEST-DOC-004` must fail if the workflow,
  ADR-0006, manifest, preflight tuple or implementation scope diverges.

### `TASK004-BLOCKER-009` — blanket ancestor-mode policy rejects accepted build hosts

- Category: `SECURITY`; severity: `HIGH`; scope: `TASK_SPECIFIC`.
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`.
- Problem: the previous §6.1 required every Xcode ancestor to be non-group/world-
  writable, but the accepted host uses standard `/Applications` UID 0/GID 80/mode
  `0775`; the only documented local and CI toolchain roots would fail before Cargo.
- Resolution: §6.1's exact component policy: `/` remains immutable;
  `/Applications` alone permits the standard root/admin `0775` metadata; the Xcode
  bundle and every resolved component actually used below it are owned by root or
  the recorded admin build eUID and remain non-group/world-writable. Root, GID 80
  administrators and the recorded build account are explicitly inside the
  trusted build-host boundary; any other writable ancestor or descendant fails.
- Verification: `TEST-SUPPLY-004` exercises the real local/CI positive cases and the
  complete synthetic UID/GID/type/mode/symlink matrix through the same pure metadata
  policy used by `build.rs`, without changing host filesystem permissions.

### `TASK004-GAP-001` — platform/path acceptance evidence is not closed

- Classification: `EXPECTED_GAP`; severity: `HIGH`; scope: `TASK_SPECIFIC`.
- Owner: `WORK_SHOULD_FIX`.
- Resolution: `AC-073` and `TEST-PATH-004` are accepted in the canonical
  registry/start record and require real arm64 macOS/APFS
  evidence rather than marker-only document checks.
- Verification: §9–§10 plus `TEST-DOC-004` lifecycle enforcement.

## 4. Accepted exact SQLite build policy

### 4.1 Exact packages and features

The accepted dependency boundary is:

```toml
mengxia-platform-fs = { path = "crates/mengxia-platform-fs", version = "=0.1.0" }
rusqlite = { version = "=0.40.2", default-features = false, features = ["bundled"] }
rustix = { version = "=1.1.4", default-features = false, features = ["fs", "process", "std"] }
sha2 = { version = "=0.11.0", default-features = false }
tokio = { version = "=1.53.1", default-features = false, features = ["sync"] }

[build-dependencies]
sha2 = { version = "=0.11.0", default-features = false }
```

`mengxia-store-sqlite` may depend on the internal `mengxia-platform-fs` safe API,
`rusqlite`, safe `rustix`, `sha2` and `tokio`; it retains
`#![forbid(unsafe_code)]` and has no direct `libc` dependency. The accepted new
`mengxia-platform-fs` FFI adapter depends only on safe `rustix`; its build script
uses the same exact `sha2` pin to verify §6.1's checked-in C inputs and selected
Apple toolchain, then directly invokes the fixed Apple tools and exact argv. It does
not use the environment-sensitive `cc` crate. It has no Rust `libc` dependency and
must not depend on SQLite, store, storage, application, domain, Tokio or a
composition root.

`libsqlite3-sys` is resolved exactly at 0.38.2 through `rusqlite` and patched to the
single repository-local path `third_party/libsqlite3-sys-0.38.2`. That path
must be added to `workspace.exclude` and to a narrow naming-policy allowlist; it must
not become a workspace package and does not authorize a broad `third_party/**`
exception. By contrast, `mengxia-platform-fs` is the accepted eighteenth canonical
workspace package, not a vendored exception. Accepting it requires an architecture
ADR plus synchronized Specification §6, workspace inventory, dependency/naming tests
and TASK-004 file scope. Historical TASK-001 completion
evidence remains evidence for its then-accepted 17-package bootstrap; the living
inventory test must be deliberately versioned, never silently weakened. No package
or dependency outside this exact start scope is authorized.

### 4.2 Local patch contents

The local copy preserves upstream license/provenance and contains the complete
0.38.2 crate needed by Cargo: manifest, `build.rs`, bindings, wrapper headers and
SQLite source directory. The reviewed patch must:

1. substitute only the accepted official SQLite 3.53.4 `sqlite3.c` and `sqlite3.h`;
2. keep normal builds on checked-in bindings—no bindgen, network or generated
   bindings are permitted in the canonical build;
3. make `build.rs` compile only the local accepted source and emit one
   `links = "sqlite3"` provider;
4. reject `LIBSQLITE3_SYS_USE_PKG_CONFIG`, `SQLITE3_LIB_DIR`, `SQLITE3_INCLUDE_DIR`,
   `SQLITE3_STATIC`, vcpkg, system headers/libraries, and environment-supplied C flags;
5. remove upstream optional extension defines/features not accepted by ADR-0003,
   including FTS, RTREE, DBSTAT, STAT4, URI filename handling and extension loading;
6. make the repository build script explicitly supply only the five ADR-0003
   defines plus the security tightening `SQLITE_OMIT_LOAD_EXTENSION`; and
7. retain a minimal reviewable diff and source/license digest manifest.

The repository-explicit SQLite C defines are exactly:

```text
SQLITE_THREADSAFE=1
SQLITE_DQS=0
SQLITE_DEFAULT_FOREIGN_KEYS=1
SQLITE_DEFAULT_WAL_SYNCHRONOUS=2
SQLITE_TRUSTED_SCHEMA=0
SQLITE_OMIT_LOAD_EXTENSION
```

The first five values are the accepted ADR-0003 flags. The sixth removes
`sqlite3_enable_load_extension` and loadable-extension entry points. Adding that
security-tightening flag requires explicit canonical acceptance before that change.
This statement constrains defines supplied by the repository build, not the complete
output of `sqlite3_compileoption_get()`: SQLite also reports amalgamation defaults,
limits and compiler/platform diagnostics.

### 4.3 Source and runtime proof

The source manifest records the ADR-0003 official archive SHA3-256, the exact
`sqlite3.c`/`sqlite3.h` SHA-256 values from §2, upstream URLs and licenses. Before any
Library mutation, runtime assertions require:

- `sqlite3_libversion_number() == 3053004`;
- exact `sqlite3_sourceid()` equal to the value in §2;
- all six repository-explicit options present with exact values through
  `sqlite3_compileoption_used`;
- `ENABLE_LOAD_EXTENSION`, `ENABLE_FTS1`, `ENABLE_FTS2`, `ENABLE_FTS3`,
  `ENABLE_FTS4`, `ENABLE_FTS5`, `ENABLE_RTREE`, `ENABLE_DBSTAT_VTAB`,
  `ENABLE_STAT4`, `ENABLE_COLUMN_METADATA` and `USE_URI` absent;
- every remaining option either exactly matches the reviewed checked-in
  `sqlite-compile-options-allowlist.txt`, or is the single diagnostic
  `COMPILER=<value>` entry recorded for the current approved host compiler; any
  unreviewed extra fails readiness; and
- per-connection hardening succeeds.

`cargo tree -d`, `cargo metadata`, native dependency inspection and a symbol/runtime
probe must show one SQLite sys crate and one accepted SQLite implementation. The
claim is **offline and source-pinned**, not universally hermetic: the build still
depends on the pinned Rust toolchain and host C compiler recorded by the repository.

## 5. Accepted immutable bootstrap and migration contract

### 5.1 Exact schema

The exact migration filename is `0000_store_bootstrap.sql`; the stored
`migration_name` is `0000_store_bootstrap`, derived only by removing the final
`.sql` suffix. The file is UTF-8 without BOM and repository-enforced LF. Its SHA-256
covers all exact committed bytes of `0000_store_bootstrap.sql` without filename,
path, whitespace or newline normalization.

```sql
CREATE TABLE schema_migrations (
    migration_sequence INTEGER PRIMARY KEY NOT NULL
        CHECK (migration_sequence BETWEEN 0 AND 9999),
    migration_name TEXT NOT NULL UNIQUE,
    sha256 BLOB NOT NULL CHECK (length(sha256) = 32),
    applied_at_seconds INTEGER NOT NULL,
    applied_at_nanos INTEGER NOT NULL
        CHECK (applied_at_nanos BETWEEN 0 AND 999999999)
) STRICT;

CREATE TABLE library_meta (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    library_id BLOB NOT NULL UNIQUE CHECK (length(library_id) = 16),
    owner_uid INTEGER NOT NULL CHECK (owner_uid BETWEEN 0 AND 4294967295),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL
        CHECK (created_at_nanos BETWEEN 0 AND 999999999)
) STRICT;
```

The committed SQL contains static DDL only. `library_id`, effective UID, timestamps,
migration sequence/name and checksum are inserted by bound parameters in the same
`BEGIN IMMEDIATE` transaction; no dynamic value is interpolated into SQL.

### 5.2 Migration identity and ordering

Every embedded migration filename matches exactly:

```text
four ASCII digits, underscore, one lowercase ASCII letter,
then zero through sixty-three lowercase ASCII letters, digits, or underscores,
then .sql
```

The parsed four-digit filename prefix equals `migration_sequence`; removing the
`.sql` suffix produces the only stored name. The embedded registry is unique and
contiguous from zero; TASK-004 contains only filename
`0000_store_bootstrap.sql`, sequence zero and stored name
`0000_store_bootstrap`. Applied records are queried with
`ORDER BY migration_sequence ASC` and must exactly equal the embedded registry by
sequence, name and digest. No missing, extra, duplicate, renamed or changed record is
accepted. Applied migration bytes are immutable.

### 5.3 Fresh bootstrap transaction

A fresh bootstrap attempt first completes pure `StoreConfig` validation and read-only parent
authorization. Before creating/modifying the Library root it calls an injectable
internal `BootstrapClock` seam exactly once, converts the returned `(seconds, nanos)`
through TASK-002 `Timestamp::from_unix_seconds_nanos`, and calls an independent
internal `LibraryIdSource` exactly once. Its production implementation delegates only
to TASK-002 `Id::try_new()`; tests inject typed `IdGenerationError` results without
exposing the private TASK-002 clock/entropy seams. Bootstrap-clock source/range/nanos
failure or any UUID clock/range/entropy failure returns
`ID_GENERATION_UNAVAILABLE` before root mutation. Public diagnostics use the existing
safe static error contract and retain no raw clock value.

The resulting one `Timestamp` and `library_id` are recorded in the durable intent.
Recovery validates and reuses both; it never resamples time or generates a replacement
ID for the same intent. A proven cleanup/restart creates a new intent and therefore
samples once for that new attempt.

`TEST-BOOTSTRAP-004`/`TEST-ERROR-004` inject a counting `BootstrapClock` and
`LibraryIdSource` to prove one call to each, identical timestamp fields in both rows
and intent reuse after restart. Negative cases cover bootstrap-clock source failure,
timestamp year overflow, invalid nanos, and injected TASK-002 pre-Unix/range/entropy
ID-generation errors; each returns
`ID_GENERATION_UNAVAILABLE`, exposes only safe static detail and leaves the target
absent/empty.

After §6 target validation, lock acquisition and staging database creation:

1. assert exact SQLite source/options before DDL;
2. set and verify connection hardening;
3. begin `BEGIN IMMEDIATE`;
4. execute the static bootstrap DDL;
5. insert exactly one `library_meta` using the same TASK-002-validated UUIDv7,
   bootstrap `Timestamp` and effective UID already committed in the intent;
6. insert sequence zero/name/checksum using that exact same timestamp—both tables'
   seconds and nanos must be byte-for-byte equal;
7. validate both rows within the transaction and commit;
8. checkpoint, close, validate and publish using §6.5.

A failed transaction never publishes a canonical database. TASK-004 creates no
`0001_library_assets` or domain/runtime table.

### 5.4 Reopen validation

Reopen first validates the filesystem/lock boundary, then opens the canonical DB
with `SQLITE_OPEN_NOFOLLOW`, and before admitting work verifies:

- exact runtime/options and all connection hardening;
- `PRAGMA quick_check` returns exactly one row whose value is `ok`;
- the complete `sqlite_schema` row set exactly matches this allowlist and no other
  row exists: tables `schema_migrations` and `library_meta`; indexes
  `sqlite_autoindex_schema_migrations_1` on `migration_name` and
  `sqlite_autoindex_library_meta_1` on `library_id`; no view, trigger, manual index,
  virtual/shadow table or `sqlite_sequence`;
- each table row's `tbl_name` and checked-in normalized `sql` text match exactly;
  each expected autoindex has the exact `tbl_name`, `sql IS NULL` and positive
  rootpage; table rootpage numbers need only be positive and are not hard-coded;
- `PRAGMA table_xinfo`, `index_list`, `index_xinfo` and `foreign_key_list` prove the
  exact columns, declared types, nullability, primary/unique/check constraints,
  index column/order/collation/partial flags, no foreign keys and `STRICT` shape;
- exactly one `library_meta` row exists;
- `library_id`, `library_meta.created_at` and
  `schema_migrations.applied_at` decode through TASK-002 typed validators;
- `library_meta.created_at_seconds/nanos` exactly equal
  `schema_migrations.applied_at_seconds/nanos` on every fresh or normal reopen;
- when a valid bootstrap intent remains, its timestamp also passes the same typed
  validator and both database rows exactly equal the intent seconds/nanos;
- stored owner UID equals the effective UID, root owner, lock-file owner and database
  file owner; and
- ordered migration records exactly match the embedded registry.

Any mismatch fails closed before writer/read admission. Repair, downgrade and
runtime-supplied migrations are out of scope.

## 6. Accepted macOS Library bootstrap/lock contract

### 6.1 Boundary and fixed names

TASK-004 uses safe APIs from exact `rustix` 1.1.4 plus stable
`std::fs::File::try_lock`; it adds no custom SQLite VFS. `rustix` does not expose
macOS extended-ACL enumeration, so the former all-safe-code claim is withdrawn.
Canonical acceptance must explicitly approve the separate platform/FFI adapter crate
`mengxia-platform-fs`. `mengxia-store-sqlite` keeps
`#![forbid(unsafe_code)]`; it cannot contain an unsafe exception, raw ACL symbol or
direct `libc` dependency. Apple ACL calls are absent from `libc` 0.2.189, so
`mengxia-platform-fs` does not hand-write those declarations in Rust or pretend that
the crate exports them. Instead, `build.rs` compiles exactly the checked-in
`include/mengxia_acl_shim.h`, `src/macos_acl_shim.c` and
`src/macos_acl_abi_probe.c` against `<sys/acl.h>` using the following closed
toolchain contract. No `cc` crate, shell, PATH lookup or response file participates.

Two explicit evidence classes avoid treating every developer build as a release
attestation:

- `developer` is the default local build class. It still uses the checked-in C
  inputs, fixed argv, absolute Apple tools, path confinement, safe ownership/mode
  policy and ABI assertions, but records rather than formally attests the exact
  Xcode/SDK/tool digests. Its output is suitable for local development and tests but
  MUST NOT satisfy `TEST-SUPPLY-004` or release evidence.
- `attested` is selected only by the formal CI/release verification command. It adds
  every exact tuple/path/digest and environment requirement below and emits an
  attestation record. Only an `attested` build produced by the reviewed CI job may
  satisfy `TEST-SUPPLY-004`; a caller-selected local value grants no authority.

- The only accepted Cargo target/host is `aarch64-apple-darwin`. `build.rs` invokes
  absolute `/usr/bin/xcode-select`, `/usr/bin/xcodebuild` and `/usr/bin/xcrun` with
  `Command::env_clear()` and an internal fixed locale. `xcrun --no-cache --sdk
  macosx` must resolve Xcode `26.6` build `17F113`, SDK `26.5`, and Apple clang
  `21.0.0 (clang-2100.1.1.101)`. The selected developer directory must be reached
  through exactly `/Applications/Xcode.app/Contents/Developer` or
  `/Applications/Xcode_26.6.app/Contents/Developer`; after symlink resolution it
  must equal one of those two reviewed roots, and every resolved tool/SDK path must
  remain below that canonical developer directory. Path metadata must match the
  closed build-host permission matrix below. `/Applications` is the sole writable-
  ancestor exception; it does not exempt the Xcode bundle or any component below it.
  Any other logical path, canonical root, identity or permission fails before
  compilation in `attested` mode. Developer mode accepts an Apple clang/SDK that
  passes the checked-in ABI probe and minimum deployment contract, records the exact
  identities/digests and never reports attested evidence.
- `docs/provenance/macos-acl-ffi-toolchain-v1.toml` pins the accepted logical and
  canonical developer-directory allowlist to exactly the two paths above, plus the
  exact component metadata/mask rules from the build-host permission matrix, the
  privileged root/admin trust-boundary statement, Xcode/build/SDK/clang identities,
  every C input digest, clang digest
  `7def90dd8829726686213a747fc5bff1583df933dae5edc55d755479e0bfe00a`, and SDK
  `sys/acl.h` digest
  `9511f84f0abe1e108e10979900d4fea8567534aef78f0984f7050c49f6c29ff7`, plus Apple
  `libtool` digest
  `229eb9d8027953d2aee0590f983eed587d52bdd1ebc21114a62ce693f77b03f1`.
  The build script verifies these with exact `sha2` before any compilation.
- The clang invocation is a checked-in ordered argv: resolved absolute clang;
  `-target arm64-apple-macos13.0`; the resolved versioned SDK via one `-isysroot`;
  exactly one repository include directory; `-std=c11`; `-fvisibility=hidden`;
  `-fno-common`; `-fPIC`; `-fstack-protector-strong`; `-O2`; `-g0`;
  `-D_FORTIFY_SOURCE=2`; and the accepted warning set with `-Werror`. A checked-in
  export macro gives default visibility only to the two version-one shim functions.
  No other `-I`, `-F`, `-include`, `-D`, plugin, wrapper,
  sanitizer, linker or code-generation flag is allowed. Absolute `xcrun` resolves
  Apple `libtool`, which receives a fixed argv and internally supplied
  `ZERO_AR_DATE=1` to produce the one static archive.
- In both evidence classes, before tool discovery, `build.rs` rejects C/Objective-C
  tool, include, SDK, linker/archive and test overrides: `CC`, `CFLAGS`, `CPPFLAGS`, `CPATH`,
  `C_INCLUDE_PATH`, `CPLUS_INCLUDE_PATH`, `OBJC_INCLUDE_PATH`, `SDKROOT`,
  `DEVELOPER_DIR`, `TOOLCHAINS`, `MACOSX_DEPLOYMENT_TARGET`, `ARCHFLAGS`, `LD`,
  `LDFLAGS`, `LIBRARY_PATH`, `AR`, `ARFLAGS`, `RANLIB`, `RANLIBFLAGS`, `NM`,
  `STRIP`, `CRATE_CC_*`, `CC_*`,
  `CFLAGS_*`, `CPPFLAGS_*`, all `<target>_{CC,CFLAGS,CPPFLAGS,AR,ARFLAGS}` and
  `{HOST,TARGET}_{CC,CFLAGS,CPPFLAGS,AR,ARFLAGS}` forms, `ZERO_AR_DATE`, and every
  `MENGXIA_ACL_*` variable except the verification command's exact
  `MENGXIA_ACL_BUILD_CLASS=attested`; `MENGXIA_ACL_TESTING` is always rejected.
  Attested mode additionally rejects `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`,
  nonempty `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`,
  `CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS`,
  `CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER`, `BINDGEN_EXTRA_CLANG_ARGS`,
  `CLANG_PATH` and `COMPILER_PATH` are also rejected. Cargo-required `OUT_DIR`,
  `HOST`, `TARGET`, `PROFILE`
  and `OPT_LEVEL` are validated but never translated into C flags. Developer mode
  may use ordinary Cargo/Rust wrapper and Rust flags because they cannot alter the
  fixed C argv; the evidence record marks the build non-attested.
- The script writes
  `OUT_DIR/mengxia-acl-build-command-v1.json` containing the tool real paths and
  identities, input/output digests, complete ordered compile/archive argv and exact
  child environment. `TEST-SUPPLY-004` independently reconstructs the allowlist,
  hashes the outputs and proves each rejected override fails before clang executes.

The build-host permission matrix is exact:

| Component | Required accepted metadata | Rejection rule |
|---|---|---|
| `/` | real directory, never a symlink; UID `0`, GID `0`, mode exactly `0755` | any type, UID, GID or mode difference |
| `/Applications` | real directory, never a symlink; UID `0`, GID `80` (`admin`), mode exactly `0775` | any type/UID/GID/mode difference, including world-write; this is the sole group-write exception |
| selected logical Xcode bundle | `/Applications/Xcode.app` may be the canonical real directory or a symlink owned by UID `0` or the recorded build eUID only when its canonical target is the accepted versioned bundle; `/Applications/Xcode_26.6.app` must be a canonical real directory | any other symlink/target, non-directory canonical target or owner outside the accepted owner set |
| canonical Xcode bundle and every canonical path component actually traversed to `Contents/Developer`, clang, `libtool`, the SDK root and `sys/acl.h` | real component or in-bundle symlink resolving inside the same canonical bundle; every canonical directory/file owner is UID `0` or the recorded build eUID and `(mode & 0o022) == 0` | group/world writable canonical component, owner outside the accepted owner set, escape from the bundle, broken/looping link or metadata/read failure |

The standard `/Applications` exception is justified only by an explicit build-host
trust boundary: before Xcode/tool discovery, `build.rs` invokes only absolute
`/usr/bin/id -u`, `/usr/bin/id -g` and `/usr/bin/id -G` with `env_clear()`, fixed
locale, fixed argv and bounded strict decimal output parsing; it rejects execution,
nonzero status, malformed/duplicate/out-of-range values or a primary GID missing
from the returned supplementary-group set. `/usr/bin/id` is identity discovery, not
a compiler input, and its path metadata must be a root-owned non-writable component
under the same pure policy. The accepted component-owner set is exactly UID `0` plus that build
eUID; a non-root build eUID is accepted only when its recorded groups include numeric
GID `80` (`admin`). OS root and members of GID 80 are privileged build-host
administrators and are trusted not to mutate the selected Xcode bundle or its aliases
concurrently. This explicitly includes every repository/dependency subprocess
launched with the build account's credentials; source pinning and review—not this
mode check—must establish their trustworthiness. A malicious/concurrent root, admin
or same-credential build process is out of scope. Ordinary non-admin local users
remain outside that authority. This
exception does not claim that `0775` is generally safe for toolchain ancestors and
cannot be applied to another path, group or mode.

`build.rs` in both evidence classes and the CI preflight evaluate this matrix immediately before tool
identity/digest checks; `build.rs` repeats metadata, canonical-path, identity and
digest checks after producing the archive and fails rather than emitting link
metadata if anything changed. These repetitions detect accidental image drift but
do not pretend to defeat an in-scope privileged administrator. The implementation
places the matrix in the same pure metadata-policy function shared by production
`build.rs` and `TEST-SUPPLY-004`; tests pass synthetic `lstat`/canonicalization
records to that function and never chmod, chown or replace real `/Applications`.

The formal CI gate is also closed and fail-closed:

- TASK-004 changes the existing arm64 workflow job to `runs-on: macos-26`. Before
  any Rustup, Cargo, build-script or repository gate command, the workflow executes
  exactly
  `/usr/bin/sudo /usr/bin/xcode-select --switch
  /Applications/Xcode_26.6.app/Contents/Developer`.
  It then verifies `uname -m == arm64`, the selected developer-directory
  allowlist/ownership/mode above, the build eUID/GID/supplementary groups and
  accepted owner set, Xcode `26.6` build `17F113`, SDK `26.5`, Apple
  clang `21.0.0 (clang-2100.1.1.101)`, and all manifest-pinned tool/header digests.
  The inline preflight starts with `set -eu`, invokes only absolute system tool
  paths and compares against checked-in literal expectations also checked for exact
  equality with the manifest by `TEST-DOC-004`. It explicitly selects the attested
  build class. A mismatch stops the job before
  Cargo; there is no fallback to another Xcode, SDK, compiler or runner image.
- `macos-26` is a mutable hosted-runner label and is only an availability selector,
  not toolchain evidence. The preflight records `ImageOS`, `ImageVersion`,
  `RUNNER_OS`, `RUNNER_ARCH`, `sw_vers`, selected logical and canonical developer
  roots, the exact tuple and digests in the CI log. Missing provenance variables are
  recorded as unavailable but never substitute for tuple validation. If a future
  image removes or replaces Xcode 26.6, CI fails closed until a reviewed manifest,
  proposal and ADR change; it must not silently accept the newer image toolchain.
- After preflight, CI runs the unchanged TASK-001 repository gates and the complete
  TASK-004 gates, including `TEST-SUPPLY-004`. The workflow change may not loosen
  checkout pinning, permissions, timeouts, locked/offline requirements, tests or
  advisory policy. `TEST-DOC-004` verifies the workflow uses the sole accepted
  runner label, performs preflight before every Cargo invocation and contains no
  fallback branch.

The build also rejects a non-Apple target, an SDK older than macOS 10.13
(`__MAC_OS_X_VERSION_MAX_ALLOWED < 101300`), non-8-bit bytes, a C `int` other than
32 bits, a failed static assertion or any compiler warning. Changing the manifest,
toolchain tuple, ABI evidence or accepted flag universe requires review rather than
automatic regeneration.

The checked-in public-to-Rust C ABI is exactly version one:

```c
#define MENGXIA_ACL_ABI_V1 1u
struct mengxia_acl_summary_v1 {
    uint32_t abi_version;       /* byte offset 0 */
    uint32_t entry_count;       /* byte offset 4 */
    uint32_t allow_count;       /* byte offset 8 */
    uint32_t deny_count;        /* byte offset 12 */
    uint32_t acl_flags;         /* byte offset 16; ACL object only */
    uint32_t entry_flags_or;    /* byte offset 20; ACEs only */
    uint32_t inheritable_count; /* byte offset 24; ACE count only */
    uint32_t external_size;     /* byte offset 28; portable bytes */
    int32_t os_errno;           /* byte offset 32; zero on success */
    uint32_t reserved;          /* byte offset 36; must be zero */
};
uint32_t mengxia_acl_abi_version_v1(void);
int32_t mengxia_acl_inspect_fd_v1(
    int32_t fd, struct mengxia_acl_summary_v1 *out);
```

The structure is exactly 40 bytes with alignment 4. Return statuses are fixed as
`0=OK`, `1=INVALID_ARGUMENT`, `2=OS_ERROR`, `3=MALFORMED_ITERATION`,
`4=UNKNOWN_TAG`, `5=UNKNOWN_SDK_RESULT`, `6=ENTRY_LIMIT_EXCEEDED` and
`7=UNKNOWN_FLAG_BITS`; no other value is accepted by Rust. A null output pointer or
negative descriptor is `INVALID_ARGUMENT`. For a non-null output pointer the shim
zeroes all 40 bytes first,
then sets `abi_version=1`; counts/flags/size are authoritative only on `OK`, `reserved`
must remain zero, and `os_errno` is the captured nonnegative `errno` only for
`OS_ERROR` (zero for every other status). Rust rejects a wrong ABI version, nonzero
reserved field, impossible count relation or unknown status before policy use.
`acl_flags` is read by calling `acl_get_flagset_np` on the `acl_t` itself and
normalizes only `DEFER_INHERIT=1<<0` and `NO_INHERIT=1<<1`. `entry_flags_or` is
produced by a separate call on every `acl_entry_t` and normalizes only
`ENTRY_INHERITED=1<<0`, `FILE_INHERIT=1<<1`, `DIRECTORY_INHERIT=1<<2`,
`LIMIT_INHERIT=1<<3` and `ONLY_INHERIT=1<<4`. `inheritable_count` counts entries
carrying any of the latter four propagation flags; ACL-level flags are never
misreported as entry flags.

The C probe includes the SDK header and `_Static_assert`s
`ACL_TYPE_EXTENDED==0x100`, `ACL_FIRST_ENTRY==0`, `ACL_NEXT_ENTRY==-1`,
`ACL_MAX_ENTRIES==128`, `ACL_EXTENDED_ALLOW==1`, `ACL_EXTENDED_DENY==2`, and the
exact SDK flag values
`1<<0`, `1<<17`, `1<<4`, `1<<5`, `1<<6`, `1<<7`, `1<<8`. It uses the SDK spelling
`ACL_ENTRY_ONLY_INHERIT`, not an invented alias. The probe also asserts every struct
offset, size/alignment and fixed-width integer size. Version one additionally fixes
`MENGXIA_ACL_EXTERNAL_MAX_V1` at 16,384 bytes. The SDK-side types and function
signatures consumed by the C compiler are exactly:

```c
struct _acl; struct _acl_entry; struct _acl_flagset;
typedef struct _acl *acl_t;
typedef struct _acl_entry *acl_entry_t;
typedef struct _acl_flagset *acl_flagset_t;
extern acl_t acl_get_fd_np(int fd, acl_type_t type);
extern int acl_get_entry(acl_t acl, int entry_id, acl_entry_t *entry_p);
extern int acl_get_tag_type(acl_entry_t entry_d, acl_tag_t *tag_type_p);
extern int acl_get_flagset_np(void *obj_p, acl_flagset_t *flagset_p);
extern int acl_get_flag_np(acl_flagset_t flagset_d, acl_flag_t flag);
extern int acl_clear_flags_np(acl_flagset_t flagset_d);
extern int acl_add_flag_np(acl_flagset_t flagset_d, acl_flag_t flag);
extern acl_t acl_dup(acl_t acl);
extern int acl_valid(acl_t acl);
extern ssize_t acl_size(acl_t acl);
extern ssize_t acl_copy_ext(void *buf_p, acl_t acl, ssize_t size);
extern int acl_free(void *obj_p);
```

They are included from the SDK, never duplicated as an independent production
header; this block is the reviewed ABI contract. The shim is the sole code that
calls them. Required interpretations follow the SDK contracts: pointer-returning
functions return non-null or fail with null/`errno`; `acl_get_entry` returns `1` for
an entry, `0` at end and `-1` on error; flag/tag/validation/mutation/free functions
return `0` or `-1`; `acl_get_flag_np` returns `1` present, `0` absent or `-1` error;
and size/copy return a nonnegative byte count or `-1`. Any other result is status 5.

Inspection is finite and ordered:

1. obtain and `acl_valid` the original ACL, then query its ACL-object flagset for
   exactly `ACL_FLAG_DEFER_INHERIT` and `ACL_FLAG_NO_INHERIT`;
2. iterate entries separately, querying only `ACL_ENTRY_INHERITED`,
   `ACL_ENTRY_FILE_INHERIT`, `ACL_ENTRY_DIRECTORY_INHERIT`,
   `ACL_ENTRY_LIMIT_INHERIT` and `ACL_ENTRY_ONLY_INHERIT`; after 128 accepted entries,
   exactly one additional `ACL_NEXT_ENTRY` returning an entry immediately yields
   `ENTRY_LIMIT_EXCEEDED`—no counter approaches integer overflow and no 129th entry
   is inspected;
3. require `acl_size(original)` in `1..=16_384`, duplicate with `acl_dup`, validate
   it, serialize both ACLs with portable big-endian `acl_copy_ext`, and require exact
   size and byte equality before modification;
4. on the duplicate only, clear the ACL-object flagset and re-add exactly the two
   recognized ACL flags observed on the original; then walk original/duplicate
   entries in lockstep, clear each duplicate entry flagset and re-add exactly the five
   recognized entry flags observed on its original peer; and
5. serialize the reconstructed duplicate again. Exact equality with the original is
   the proof that neither the ACL-object flag word nor any entry flag word contained
   an unknown bit. A difference is `UNKNOWN_FLAG_BITS`; a count/order mismatch is
   `MALFORMED_ITERATION`. The shim never reads Apple-private `_acl`, `a_flags`,
   `_acl_entry` or `ae_flags` layout.

The two external-representation buffers are each fixed at 16,384 bytes, so memory
and compare work are bounded. Every successfully allocated original ACL, duplicate
ACL and buffer is released exactly once on every exit; cleanup continues after the
first failure, and a cleanup failure becomes `OS_ERROR` only when no earlier error
already owns the result.

The Rust side declares only these two functions in one private `macos_ffi` module:

```rust
#[repr(C)]
struct MengxiaAclSummaryV1 {
    abi_version: u32, entry_count: u32, allow_count: u32, deny_count: u32,
    acl_flags: u32, entry_flags_or: u32, inheritable_count: u32,
    external_size: u32, os_errno: i32, reserved: u32,
}
unsafe extern "C" {
    fn mengxia_acl_abi_version_v1() -> u32;
    fn mengxia_acl_inspect_fd_v1(fd: i32, out: *mut MengxiaAclSummaryV1) -> i32;
}
```

`mengxia-platform-fs` uses `#![deny(unsafe_code)]` and places
`#[allow(unsafe_code)]` only on that private module. No public item exposes raw file
descriptors, ACL pointers, C types or platform constants. A Rust layout test repeats
the size/alignment/offset assertions. The inspector core takes a private hidden C
backend table; the exported production function always passes one fixed real-SDK
table. A separate C test executable links the same object and calls only that hidden
core with deterministic fakes—there is no production/test preprocessor branch,
fake macro or ambient include path. Empty, allow, deny, both ACL-object flags, all
five entry flags, unknown ACL/entry bits detected by external-representation
reconstruction, null/error/unexpected results, exact 128 entries, immediate 129th
entry rejection, size `0`/`16_384`/`16_385`, duplicate/serialization mismatch, every
allocation cleanup failure and repeated calls prove bounded status conversion and
exactly-once cleanup. Real-APFS tests remain separately mandatory.
`TEST-ARCH-004` rejects `unsafe`, raw ACL symbols, `libc` or
`#[allow(unsafe_code)]` in `mengxia-store-sqlite`, rejects unsafe outside the single
Rust FFI module, and rejects ACL SDK symbols outside the shim/probe. Failure to build
or run either ABI probe is `UNVERIFIABLE`, never a fallback. This accepted package,
C shim and narrow Rust exception are governed by ADR-0006.

The crate exposes safe `BorrowedFd`/`OwnedFd`-based operations that return opaque
`ValidatedAbsolutePath`/`MacOsObjectSecurity` evidence. `ValidatedAbsolutePath` has
private fields, no public unchecked constructor and no `Clone`, `Copy`, serialization,
display or caller-selected child operation; it owns the retained descriptor chain.
It can mint only a non-`Clone`, non-`Copy`, lifetime-bound `FixedSqliteChildPath<'_>`
for enum variants `Canonical` and `BootstrapStaging`. The token's fields and
constructor are private and its sole byte-level interface is the `AsRef<Path>` borrow
required by stock `rusqlite`. This trait **does permit** any Rust holder to copy or
format the path; the token is not claimed to provide confidentiality or type-level
non-extractability. Its type guarantees are limited to unforgeable construction and
the two fixed basenames.

Store is trusted first-party code under ADR-0004. Repository policy—not the Rust
type system—requires it to borrow the path only in one private `stock_sqlite_open`
module, forbids owned/string conversion, persistence and logging, and requires
post-open revalidation before returning. `TEST-ARCH-004` maintains an exact source
allowlist: outside the platform implementation and that consumer, no production file
may name `FixedSqliteChildPath`; inside the consumer it rejects `PathBuf::from`,
`to_path_buf`, `to_owned`, `into`, `display`, formatting/logging macros and any call
other than the allowlisted `Connection::open_with_flags`. A committed negative
fixture that performs `token.as_ref().to_path_buf()` must compile successfully and
then fail this repository architecture lint, proving the constraint is architectural
rather than falsely compile-time. Separate compile-fail fixtures cover only private
constructor/field access and arbitrary-basename forgery. The platform crate remains
SQLite-independent.

The only internal names owned by TASK-004 are:

```text
.mengxia.lock
.mengxia.bootstrap-intent
library.sqlite3
library.sqlite3-wal
library.sqlite3-shm
.library.sqlite3.bootstrap
.library.sqlite3.bootstrap-wal
.library.sqlite3.bootstrap-shm
```

The Library root must be an absolute local APFS directory, exactly mode `0700`, a
real directory rather than a symlink, owned by the invoking effective UID and have
no extended ACL entries.
`.mengxia.lock`, `.mengxia.bootstrap-intent`, canonical database and staging files
must be regular non-symlink files, exactly mode `0600`, with the same owner.
Group/world permission bits are zero and every recognized file/sidecar has an empty
extended ACL. The APFS mount must not carry `MNT_IGNORE_OWNERSHIP`.

### 6.2 Target validation and owner source

`O_NOFOLLOW` on one `open` protects only that call's final target component; this
contract does not treat it as whole-path protection. Before any mutation,
`mengxia-platform-fs` parses the already lexically validated absolute Library root
into exact nonempty components and creates a `ValidatedAbsolutePath` by walking from
an opened `/` anchor. For every existing component through the final parent it calls
descriptor-relative `openat` with directory, no-follow and close-on-exec flags using
the previously opened directory—not a growing absolute string—and retains the
resulting `OwnedFd`, exact component bytes, device and inode. Any symlink,
non-directory, missing intermediate component, changed name binding or unsupported
metadata fails before root creation.

Every retained prefix directory must be local APFS with
`f_flags & MNT_IGNORE_OWNERSHIP == 0`, owned by root or the daemon effective UID,
and have no group/world write bit. Every ACL first passes §6.1's portable-
representation unknown-bit proof. For an ancestor or final parent,
`allow_count == 0`, `deny_count == entry_count`, ACL-object `DEFER_INHERIT` is
forbidden, and the only permitted ACL-object bit is `NO_INHERIT`. Deny-only entries
are permitted because the standard macOS home directory may carry a protective
`deny delete` entry and such an ACE cannot grant namespace mutation. Unknown tags,
unknown object/entry flag bits, unreadable/malformed ACLs, unexpected SDK results or
more than 128 entries fail closed.

The **final parent** is stricter: it must be owned by the daemon effective UID, give
the owner read/write/search, have no group/world write bit and additionally require
`inheritable_count == 0`; therefore no entry may carry file-inherit,
directory-inherit, limit-inherit or only-inherit. `ENTRY_INHERITED` alone is historical
provenance, not a propagation flag, and is accepted on a deny-only entry. Zero ACL
entries is accepted; a non-inheritable deny-only entry such as macOS's protective
home-directory `deny delete` is also accepted. Therefore
no ACE can be copied to the new root and no ACL principal gains namespace mutation;
within §6.4's declared root/same-eUID exclusions, no other principal can add, delete
or replace the Library-root name. Any failed check returns
`STORAGE_CONFIGURATION_ERROR` before root creation.

`ValidatedAbsolutePath::revalidate_chain()` freshly reopens every component from its
retained predecessor with the same flags, repeats mount/UID/mode/ACL checks and
requires exact device/inode equality at every edge. Merely `fstat`-ing the old handles
is insufficient because it would not prove that the names still resolve to those
handles. The chain is revalidated immediately before root creation/open, after root
creation/open, after Library lock acquisition, and immediately before and after every
stock-SQLite fixed-path open. A changed component or name-to-inode relationship fails
closed.

The retained final-parent descriptor and fixed final basename are the only authority
for descriptor-relative `mkdirat`/`openat`-style bootstrap operations. An absent root
is created only beneath that handle; an existing root is opened no-follow through
that handle. Only after the complete chain and root pass does the platform crate
yield the immutable canonical absolute bytes that stock SQLite may consume under
§6.4. Caller path text is never re-resolved or re-normalized by store code.

Every newly created or reopened root, lock, intent, database, WAL and SHM entry is
inspected by its already-open no-follow descriptor before it can authorize the next
operation. Its own `fstatfs` must again prove local APFS with ownership checking
enabled. The exact UID/mode/type checks above are mandatory, and the two-level ACL
summary must be completely empty: `entry_count == 0`, `acl_flags == 0` and
`entry_flags_or == 0`.
This post-create inspection proves that no inheritance or concurrent ACL
replacement changed the new object. Any ACL entry—including owner-only deny/allow
entries—is rejected rather than interpreted, keeping the accepted policy exactly
"BSD owner/mode with no extended ACL". The process never strips or rewrites an ACL;
it fails closed and preserves evidence.

For an absent target, the invoking effective UID is the sole bootstrap owner
candidate. The root is created `0700`, synchronized, reopened without following
symlinks, and its owner must equal that candidate. For an existing empty target, its
owner must equal the invoking effective UID. Durable `library_meta.owner_uid` does
not exist yet and is never consulted to authorize creation; after publish, reopen
checks compare all filesystem owners to the durable value. This removes the owner
bootstrap cycle.

An acceptable first-entry root is empty or contains only a valid unlocked
`.mengxia.lock`; the lock-only state is an expected result of a crash after lock-file
durability and before intent creation. Any staging name without the valid durable
intent described in §6.5 has no cleanup authority and fails closed without deletion.
Unknown entries, canonical metadata that fails normal reopen, wrong types, links,
owners, modes, filesystem or changed inode/device identities also fail closed.

### 6.3 Lock lifecycle

`.mengxia.lock` may be created only while descriptor-relative enumeration proves the
Library root is otherwise empty: no canonical DB, intent, staging file or sidecar
exists. It is created/opened no-follow as a regular `0600` file owned by the
bootstrap effective UID. If any canonical/intent/staging entry exists but the lock
name is absent, return `STORAGE_CONFIGURATION_ERROR` without creating a replacement
or opening SQLite. Another process may still hold the unlinked old lock inode, so
recreation in that state would permit split-brain.

First creation has this mandatory ordering with no intent/staging operation between
steps: create the lock entry exclusively; acquire the exclusive lock on that same
file description; verify its local-APFS/ownership-checking/type/UID/mode/empty-ACL
evidence; `fsync` the lock file; `fsync` the
Library-root directory; revalidate root/lock device and inode; then re-enumerate the
entire root while still holding the lock and require the exact lock-only state. Only
after both syncs and the locked re-enumeration may §6.5 create intent. A failure at
any step returns the mapped safe error and creates no intent or staging entry.

After creation the application never unlinks or replaces `.mengxia.lock`. The
process obtains a non-blocking exclusive advisory lock and holds the same open file
description for the entire `OpenedLibraryAuthority` lifetime. Contention maps to
`CONFLICT`; there is no stealing, wait loop or timeout.

For every existing lock, pre-lock enumeration is only discovery evidence. After the
lock is acquired, the process must revalidate parent/root/lock identities and perform
a fresh complete enumeration under the lock; all recovery and reopen decisions use
only that post-lock snapshot. A pre-lock snapshot is never carried into mutation.

An unlocked stale lock-file inode after a crash is reused after full type/owner/mode
validation. It is not deleted. On reopen, the held lock, validated root identity and
durable owner form the immutable authority later consumed by TASK-003.

### 6.4 Stock SQLite path model

SQLite is not given a directory descriptor and this contract makes no claim that
SQLite opens its database/WAL/shared-memory files descriptor-relatively. After the
root/lock boundary is accepted, it receives only the immutable absolute staging or
canonical filename produced by `ValidatedAbsolutePath` plus a fixed internal
basename. Opens include `SQLITE_OPEN_NOFOLLOW`; URI filenames, caller-selected paths
and store-side string joining are disabled. `SQLITE_OPEN_NOFOLLOW` is defense in
depth for the final database component, not evidence about any prefix component.

Immediately before each SQLite open, `revalidate_chain()` proves every prefix
name-to-inode edge and the root descriptor again; immediately afterward it repeats
that proof and validates every recognized DB/WAL/SHM name descriptor-relatively from
the retained `0700` root. Because every ancestor denies namespace mutation to
non-root/non-owner principals, the final parent is daemon-owned with no `ALLOW` or
inheritable ACE (a non-inheritable deny-only ACL is permitted), and the root is
daemon-owned `0700` with an empty ACL, an untrusted different-UID process cannot
replace a prefix or database name during the stock-SQLite call.

This is an explicit threat-boundary argument, not an atomic descriptor-relative
SQLite open. A root process or arbitrary process already running with the daemon's
same effective UID can still race an absolute-path SQLite open; containment against
either is not claimed by ADR-0004 or this contract. If that threat enters scope, the
fixed-path model must be rejected and a separately reviewed custom VFS or OS
containment design is required. Pre/post revalidation may detect accidental or
out-of-boundary replacement but is not presented as preventing a same-eUID race.

### 6.5 Durable bootstrap intent

Names, file type, UID and mode alone never grant cleanup permission. Before creating
any staging name, the lock holder creates `.mengxia.bootstrap-intent` with
exclusive/no-follow `0600` semantics, writes one **exactly 256-byte** version-one
binary record, fsyncs the record, then fsyncs the Library root. All multi-byte
integers are big-endian. The signed seconds field is two's-complement `i64`; every
other integer is unsigned. The byte layout is normative:

| Offset | Width | Field / required value |
|---:|---:|---|
| 0 | 8 | ASCII magic `MXBTINT1` |
| 8 | 2 | version `1` (`u16`) |
| 10 | 2 | fixed-header length `76` (`u16`) |
| 12 | 4 | total record length `256` (`u32`) |
| 16 | 8 | root `st_dev` converted losslessly to `u64` |
| 24 | 8 | root `st_ino` converted losslessly to `u64` |
| 32 | 4 | bootstrap effective UID (`u32`) |
| 36 | 4 | flags/reserved; all zero |
| 40 | 16 | TASK-002 UUIDv7 canonical network-order bytes |
| 56 | 8 | TASK-002 timestamp Unix seconds (`i64`) |
| 64 | 4 | timestamp nanoseconds (`u32`) |
| 68 | 4 | migration sequence; exactly zero (`u32`) |
| 72 | 1 | migration-name byte length; exactly `20` |
| 73 | 1 | canonical-basename byte length; exactly `15` |
| 74 | 1 | staging-basename byte length; exactly `26` |
| 75 | 1 | reserved; zero |
| 76 | 32 | migration-name slot: ASCII `0000_store_bootstrap`, then zero padding |
| 108 | 32 | canonical slot: ASCII `library.sqlite3`, then zero padding |
| 140 | 32 | staging slot: ASCII `.library.sqlite3.bootstrap`, then zero padding |
| 172 | 32 | SHA-256 of exact checked-in `0000_store_bootstrap.sql` bytes |
| 204 | 20 | reserved bytes; all zero |
| 224 | 32 | SHA-256 over bytes `[0, 224)` only |

`st_dev`/`st_ino` values that cannot convert losslessly to the declared unsigned
width, a nonzero flag/reserved/padding byte, a length/slot disagreement, embedded
NUL before the declared length, non-ASCII slot content or trailing nonzero padding
invalidates the record. No native struct serialization, alignment padding, host
endianness, path string or caller-controlled text is permitted.

`TEST-BOOTSTRAP-004` commits a binary golden fixture with device
`0x0102030405060708`, inode `0x1112131415161718`, UID `501`, UUID bytes
`01890f1de00070008000000000000001`, seconds `1700000000`, nanos `123456789` and
migration-digest bytes `00..1f`. The exact expected 256-byte hex is:

```text
4d584254494e54310001004c0000010001020304050607081112131415161718
000001f50000000001890f1de00070008000000000000001000000006553f100
075bcd1500000000140f1a00303030305f73746f72655f626f6f747374726170
0000000000000000000000006c6962726172792e73716c697465330000000000
0000000000000000000000002e6c6962726172792e73716c697465332e626f6f
747374726170000000000000000102030405060708090a0b0c0d0e0f10111213
1415161718191a1b1c1d1e1f0000000000000000000000000000000000000000
61d3132622fa1ef1e69b1062be3b1a0eb4af990ce36153a041f7a4dce8a180f7
```

The golden record checksum is
`61d3132622fa1ef1e69b1062be3b1a0eb4af990ce36153a041f7a4dce8a180f7`.
Tests require encode-to-golden, decode-golden-to-fields and one mutation at every
field, reserved region, length, padding and checksum boundary. A second independent
test implementation recomputes the SHA-256 coverage range; encode/decode round trips
alone are insufficient evidence.

No path string or caller-controlled text is stored. A short write, invalid length,
magic/version, typed UUID, root identity, UID, basename, migration identity or record
checksum is **unproven intent**: return `STORAGE_CONFIGURATION_ERROR`, preserve every
entry and require explicit operator inspection. A valid intent is authoritative only
inside the accepted same-effective-UID threat boundary from §6.4. The UUID in a
committed staging/canonical `library_meta` must exactly match the intent UUID. The
intent timestamp must first pass TASK-002 typed validation; when intent exists, both
database timestamp columns must equal it exactly in seconds and nanos.

Fresh bootstrap never opens the canonical filename with `CREATE`:

1. make the intent durable as above;
2. create `.library.sqlite3.bootstrap` exclusively/no-follow at `0600`, verify its
   handle, and fsync the root; SQLite opens that existing fixed path without
   `SQLITE_OPEN_CREATE`;
3. run §5.3; force WAL checkpoint/truncate, close every connection, require staging
   WAL/SHM sidecars absent, fsync the staging DB, and re-open read-only for §5.4 plus
   exact intent UUID/migration matching;
4. publish with descriptor-relative `linkat` from the verified staging inode to the
   absent `library.sqlite3` name, fsync the root, unlink the staging name, fsync the
   root, unlink the verified intent, and fsync the root again; and
5. reopen canonical no-follow and verify inode/device, mode, owner, schema, runtime
   and migration evidence before admission.

The implementation never rewrites or updates an intent record. A recovery that must
restart bootstrap removes only proven staging entries authorized by a valid intent,
fsyncs the root, removes that same verified intent, fsyncs again, and begins with a
new UUID/intent. Existing canonical data is never replaced.

### 6.6 Complete recovery state table

The root is enumerated before lock creation/open. A lock may be created only for the
first row below; every later row requires the pre-existing validated lock to be held.
The complete retained prefix chain, parent/root and every entry must pass §6.2's
component-edge, no-follow, local-APFS, ownership-checking-enabled,
type/owner/mode/ACL policy. “Fail closed” means no automatic permission rewrite,
create, unlink or mutation.

| Durable entries after crash/open | Required evidence | Recovery action and result |
|---|---|---|
| Empty root, lock absent | root is otherwise empty and safe | create lock once, acquire it and start bootstrap |
| Lock only | valid regular unlocked lock | expected crash state; start a new intent without deleting lock |
| Canonical, intent, staging or sidecar present but lock absent | lock authority is missing and an unlinked inode may still be held | `STORAGE_CONFIGURATION_ERROR`; do not recreate lock, open SQLite or mutate anything |
| Valid intent, no staging/canonical | intent record passes §6.5 | re-fsync intent and root, then create the authorized staging file and continue |
| Valid intent plus empty/partial staging, no canonical | staging names are exact and intent-authorized; DB cannot pass §5.4 | unlink only the proven staging set, fsync, remove verified intent, fsync, then restart with a new intent |
| Valid intent plus committed valid staging, no canonical | §5.4 passes and Library UUID/migration digest/timestamp match intent | checkpoint/close/fsync if needed, then publish |
| Valid intent plus canonical and staging names for the same inode | canonical passes §5.4 and UUID/migration/timestamp match intent | fsync root, unlink staging, fsync, unlink intent, fsync, then normal reopen |
| Valid intent plus valid matching canonical, no staging | canonical passes §5.4 and UUID/migration/timestamp match intent | unlink verified intent, fsync root, then normal reopen |
| Canonical plus lock, no intent/staging | canonical passes §5.4 | normal reopen; missing SHM is not an error by itself |
| Invalid/truncated intent | none reliable | `STORAGE_CONFIGURATION_ERROR`; fail closed, delete nothing |
| Staging or staging sidecar without valid intent | none reliable | `STORAGE_CONFIGURATION_ERROR`; fail closed, delete nothing |
| Canonical and staging are distinct inodes, or valid intent UUID/migration/timestamp does not match DB | conflicting authority | `STORAGE_CORRUPTION`; fail closed, delete nothing |
| Unknown entry, ownership-disabled mount, a §6.2-disallowed parent ACL, any root/internal-file ACL entry or object flag, or any wrong type/owner/mode/root identity | unsafe configuration | `STORAGE_CONFIGURATION_ERROR`; fail closed, rewrite/delete nothing; a conforming non-inheritable deny-only ancestor/final-parent ACL is not an error |

This table governs both injected boundary crashes and naturally interrupted writes.
An interruption while the intent itself is being written may leave an invalid intent;
that outcome is safe but intentionally requires operator action because provenance
cannot be proven.

### 6.7 Exact crash/durability claim boundary

TASK-004 V1 claims **process-termination recovery only**: a daemon subprocess may be
killed at any enumerated boundary, then a new process on the same still-running macOS
instance observes the resulting filesystem state and follows §6.6. In this contract,
“durable” means explicitly synchronized and restart-visible for that tested process
model; it does not mean verified physical-media ordering.

The evidence is split into three non-interchangeable suites:

1. **SIGKILL visibility:** the parent test waits for an explicit child acknowledgement
   sent only after the named syscall returned successfully, sends `SIGKILL`, waits for
   process death and reopens on the same OS. Every returned create/write/link/unlink
   has the one exact namespace/byte state in §10.1; absence or rollback of a returned
   syscall is not an allowed SIGKILL result.
2. **Syscall failure and ordering:** a sealed `BootstrapFsOps` fault seam records the
   exact call trace and injects one failure before or after no kernel call. It proves
   no later operation is attempted, the mapped error is stable, and required
   `fsync`/enumeration ordering holds. This suite does not pretend a failed syscall
   partially succeeded unless a separate real-filesystem fault fixture proves that
   exact documented result.
3. **OS/power durability:** kernel panic, OS crash, power loss and physical-media
   ordering remain `UNVERIFIABLE` and supply no acceptance evidence.

Kernel panic, OS crash, sudden power loss, storage-controller volatile cache loss and
device failure are not claimed. Ordinary `fsync`, SQLite `synchronous=FULL` and the
ordered calls remain mandatory, but this contract does not assert that they provide
power-loss ordering on macOS. It does not enable SQLite `fullfsync`, call
`F_FULLFSYNC`, or report a power-loss test as PASS.

If V1 later requires OS-crash/power-loss durability, a separate accepted ADR must
define the exact macOS `F_FULLFSYNC`/SQLite `fullfsync` policy for DB, WAL, intent and
directory metadata; error mapping; supported storage hardware; and a real fault-
injection/power-cut verification method. Until then such evidence is
`UNVERIFIABLE`, not silently inferred from subprocess-kill tests.

## 7. Accepted bounded connection lifecycle and errors

### 7.1 Typed `StoreConfig` DTO and pure validation boundary

TASK-004 does not implement or test a substitute production resolver. The future
`mengxiad` composition boundary owns Specification §16 precedence—CLI flag, then
environment, then Library config, then compiled safe default—together with one-time
source capture, selected-source error priority, textual parsing and default
materialization. `mengxia-store-sqlite` never reads CLI arguments, environment
variables or a config file, never examines a raw source string and never chooses a
default. TASK-004 owns only a pure validator over one already selected typed
`ResolvedStoreConfig` DTO and returns an immutable `StoreConfig` with private fields
and typed accessors.

`ResolvedStoreConfig` contains an optional platform `PathBuf` for the required
`library_root`, `usize` write-queue capacity, `usize` read-connection count and a
millisecond `u64` busy timeout, plus a non-secret `ConfigSource` enum for each value.
It has private fields and checked test/composition constructors; it contains no raw
CLI/environment/config-file strings. The store validator rejects a missing root,
empty/non-Unicode/non-absolute/root path, any `.`/`..` component or NUL. Filesystem,
symlink, APFS and authority checks remain §6's descriptor-based responsibility;
lexical validation is not authorization.

The composition resolver must eventually supply `256`, `4` and `5000` only when
every higher-priority layer is absent. TASK-004 does not claim to verify that future
selection. Its pure validator independently enforces the following final typed
values before constructing `StoreConfig`:

| Canonical key | Typed value | Accepted inclusive range |
|---|---|---|
| `MENGXIA_LIBRARY_ROOT` | normalized absolute `LibraryRoot` | required; no default; §6 authorization follows |
| `MENGXIA_DB_WRITE_QUEUE` | `usize` queue capacity | 16 through 4096 |
| `MENGXIA_DB_READ_CONNECTIONS` | `usize` worker count | 1 through 16 |
| `MENGXIA_DB_BUSY_TIMEOUT_MS` | nonzero `Duration` milliseconds | 1 through 5000; values above 5000 are forbidden expansion of the tightening-only cap |

The DTO cannot represent empty/whitespace/signed/non-decimal/leading-zero textual
forms or integer overflow: rejecting those forms belongs to the production resolver,
not to TASK-004 evidence. A missing/invalid DTO field, zero or out-of-range final
value returns `STORAGE_CONFIGURATION_ERROR` with the static safe message and no raw
key/value/path. Pure store validation completes before parent authorization can lead
to root, lock, intent, staging or SQLite mutation.

`TEST-CONFIG-004` exercises only the TASK-004-owned DTO/validator boundary: missing,
empty, non-Unicode, relative, root, dot-segment and NUL Library paths; each exact
numeric lower/upper bound and adjacent rejection; zero; busy value `5001` or larger;
private immutable fields/accessors; source-free deterministic validation; and every
failure preceding any Library-root entry. It proves store/worker constructors cannot
receive raw configuration or perform source I/O. It contains no layered-resolver
fixture and provides no PASS evidence for production precedence.

The later TASK-003 canonical gate owns the executable production proof. Before
TASK-003 can start, its canonical plan row and stable AC/TEST registries must include
`CFG-001` and `CFG-003` and must assign observable tests for real daemon startup:
`CLI > environment > Library config > default`, single capture, required Library-root
source, invalid selected source never falling through, exact textual grammar and
overflow handling, versioned defaults, error priority, immutable DTO handoff and
failure before opening the Library. No new stable ID is fabricated in this contract;
TASK-003 canonicalization must allocate and trace the IDs. TASK-004 may complete its
store-only scope without claiming that later composition behavior exists.

### 7.2 Writer and read admission

- One dedicated blocking worker owns the only write connection.
- `MENGXIA_DB_WRITE_QUEUE` counts queued-not-started commands only. The in-flight
  command is excluded, so at most `configured capacity + 1` commands are admitted.
- Admission is non-blocking FIFO. The linearization point is successful insertion
  into the bounded queue while the admission gate is open. Before that instant the
  caller owns the command; capacity returns `BACKPRESSURE` and a closed gate returns
  `STORAGE_IO_ERROR` with safe reason `store shutting down`, with no ownership
  transfer. After that instant the store owns the admitted command.
- Every admitted command has one result channel and exactly one terminal disposition:
  it is started and commits/rolls back, or the unique shutdown-before-start exception
  in §7.3 returns `STORAGE_IO_ERROR`. Caller cancellation only drops receiver
  interest; it never revokes ownership or cancels an admitted command. No transaction
  or blocking task detaches.
- Exactly `MENGXIA_DB_READ_CONNECTIONS` blocking workers each own one read-only
  connection. Read admission is immediate try-acquire with no waiter queue; if all
  are occupied, it returns `BACKPRESSURE`.
- SQLite busy handling installs one finite `sqlite3_busy_timeout` for `SQLITE_BUSY`
  only and asserts shared-cache mode is disabled. `MENGXIA_DB_BUSY_TIMEOUT_MS` bounds
  the handler's cumulative requested sleep target; it is not a guaranteed minimum
  wait or an exact wall-clock deadline. SQLite may return `BUSY` without invoking the
  handler when waiting could deadlock, and OS scheduling may make elapsed wall time
  exceed requested sleep. TASK-004 adds no retry loop after SQLite returns.

TASK-004 exposes only typed internal bootstrap/migration seams. It exposes no raw
`Connection`, arbitrary SQL, path, migration hook, or speculative domain repository.

### 7.3 Shutdown

Shutdown's linearization point is an atomic admission-gate close serialized with
queue insertion. No insertion can linearize after it. A command inserted before it
is admitted even if shutdown observes it still queued. Shutdown is the sole
store-initiated revocation exception: queued commands not yet dequeued by the writer
are removed exactly once and their result channels receive `STORAGE_IO_ERROR` with
safe reason `store shutting down`. A command dequeued before the shutdown point is
the current command and must commit or roll back; all already-running reads finish.
Workers are joined, connections close, and only then is the Library lock released.

Shutdown guarantees no detached work and preserves transaction/lock ordering. It
does **not** promise an absolute wall-clock join bound for host filesystem I/O;
ADR-0005's finite SQLite busy budget remains enforced. Forced process termination is
handled by SQLite recovery and §6.5 on next open.

### 7.4 Exact error mapping

| Condition | Stable error code |
|---|---|
| Live Library lock contention | `CONFLICT` |
| Writer queue full or every read worker occupied | `BACKPRESSURE` |
| Submission sees closed admission gate, or shutdown revokes an admitted but not-started writer command | `STORAGE_IO_ERROR` with exact safe reason `store shutting down` |
| Any SQLite `BUSY` primary/extended result, returned immediately or after zero or more busy-handler sleeps | `STORAGE_BUSY`; no minimum-wait assumption and no TASK-004 retry after return |
| SQLite `LOCKED`, including `LOCKED_SHAREDCACHE`, while each worker exclusively owns one connection and shared cache is disabled | `INTERNAL_ERROR`; fail the store closed because the architecture invariant was violated |
| Migration checksum/order/schema mismatch, malformed DB/WAL, failed integrity check | `STORAGE_CORRUPTION` |
| Missing required Library-root DTO field, or malformed/non-Unicode/zero/out-of-range resolved `StoreConfig` input | `STORAGE_CONFIGURATION_ERROR`; fail before Library-root mutation |
| Unsupported filesystem/runtime/source/options, unsafe/unresolved path-prefix edge, `MNT_IGNORE_OWNERSHIP`, a §6.2-disallowed parent ACL, any root/internal-file ACL entry or object flag, or unsafe type/owner/mode | `STORAGE_CONFIGURATION_ERROR` |
| Bootstrap clock seam failure; invalid/out-of-range timestamp; UUIDv7 clock/range/entropy failure | `ID_GENERATION_UNAVAILABLE`; because sampling precedes root mutation, create no Library-root entry |
| SQLite `CORRUPT`, `NOTADB` or invalid format/integrity result | `STORAGE_CORRUPTION` |
| SQLite `CANTOPEN`, `IOERR`, `FULL`, `READONLY`, `PERM` or `NOMEM`; filesystem create/open/read/write/fsync/checkpoint failure | `STORAGE_IO_ERROR`; roll back/close and do not retry inside TASK-004 |
| SQLite `MISUSE`, unexpected `CONSTRAINT`, `INTERNAL`, `ABORT`, `INTERRUPT`, `SCHEMA`, unknown primary result; worker panic/join failure or invariant defect | `INTERNAL_ERROR`; fail the affected store closed |

Result mapping first normalizes `sqlite3_extended_errcode()` to its primary code.
An unrecognized extended code with a known primary code inherits that primary's
fail-closed mapping and records only the numeric code in restricted diagnostic
evidence, never the SQL/path. An unknown primary code maps to `INTERNAL_ERROR` and
closes the affected store. `CONSTRAINT` during controlled bootstrap is not user
validation: all inputs were already typed, so it indicates an implementation/schema
invariant failure. Reopen-detected invalid rows/schema remain `STORAGE_CORRUPTION`.

The two accepted additions are the canonical §14.1 rows:

| Code | Rust variant | Source | Retryable | API exposure | Log level | Metric | Exact safe message |
|---|---|---|---|---|---|---|---|
| `STORAGE_BUSY` | `StorageBusy` | any SQLite `BUSY` primary/extended result; the busy handler may be bypassed or stop after its cumulative requested-sleep target | conditional; caller may retry only with bounded delay and fresh admission | code plus generic retry guidance; no SQL/path/lock holder | WARN | `storage_busy_total` | `storage is temporarily busy` |
| `STORAGE_CONFIGURATION_ERROR` | `StorageConfigurationError` | TASK-004 missing/invalid resolved DTO field; later composition missing required source or invalid selected source; unsupported SQLite identity/options/filesystem; unresolved/changed prefix authority; ownership-checking-disabled volume; ACL metadata disallowed by §6.2; unsafe root/internal-file ACL or other metadata; or unproven recovery ownership | no until host/configuration/operator state changes | code plus generic corrective action; no raw setting/path/UID/ACL principal | ERROR | `storage_configuration_errors_total` | `storage configuration is unsupported or unsafe` |

TASK-004 implementation adds the variants to `ErrorCode` in this order after
`StorageCorruption`, make `as_str()` emit the exact uppercase strings, make
`FromStr` accept only those exact strings, include them once in the existing
exhaustive round-trip test vector, and preserve the static safe messages above
through typed storage-to-domain mapping.
Unknown/case-varied spellings remain `VALIDATION_ERROR`; parsing never retains the
rejected input. Provider/network timeout codes must not be reused for local storage.
Public diagnostics include safe category and correlation only; raw paths, SQL, row
data, source snippets, credentials and secrets are redacted.

Specification v1.1.9 incorporates these rows and their ordering. The production enum
is updated only inside the authorized TASK-004 implementation scope.

## 8. Exact authorized implementation scope

Requirements:

- `FUNC-001`
- `DATA-001`, `DATA-005`, `DATA-006`, `DATA-007`, `DATA-011`
- `REL-001`
- `SEC-017`, `SEC-020`, `SEC-021`
- `CFG-001`, `CFG-003`

Decisions:

- `BASE-011`, `BASE-013`, `BASE-014`, `BASE-015`, `BASE-017`
- `DEC-017`, `DEC-020`, `DEC-021`, `DEC-022`
- `ADR-0001`, `ADR-0003`, `ADR-0004`, `ADR-0005`, `ADR-0006`

The eighteenth canonical package/FFI boundary is accepted by `ADR-0006`. That ADR
governs the checked-in C shim/ABI and unsafe isolation **and** the CI execution
environment: arm64 `macos-26` selection, exact pre-Cargo Xcode/build/SDK/clang
tuple/path/digest preflight, recorded hosted-image provenance, and fail-closed image
drift behavior are one reviewable platform supply-chain boundary.

Authorized files/directories:

- `Cargo.toml`, `Cargo.lock`, narrow workspace/naming/supply-chain policy metadata;
- `.github/workflows/ci.yml`, limited to selecting the arm64 `macos-26` runner,
  executing §6.1's exact fail-closed platform preflight before Cargo, and adding the
  TASK-004 gates while preserving all existing security and TASK-001 gates;
- `crates/mengxia-store-sqlite/**`;
- new `crates/mengxia-platform-fs/**`, containing the sole private audited
  macOS filesystem FFI module and safe path-authority API;
- `docs/provenance/macos-acl-ffi-toolchain-v1.toml`, containing only the accepted
  C input/toolchain identities and digests from §6.1;
- `migrations/sqlite/0000_store_bootstrap.sql`;
- exact `third_party/libsqlite3-sys-0.38.2/**` local patch and provenance;
- TASK-004 tests and synchronized canonical documents; and
- canonical error registry/module only for the two accepted storage error codes.

Explicitly out of scope: TASK-003 transport/daemon/CLI/Admin work; migration 0001 or
later; domain repositories; Blob/CAS, Provider, Plugin, Credential, Project, Rights,
GC/Purge or product behavior; raw SQL APIs; data repair/downgrade; custom VFS;
network access; system SQLite fallback; unbounded wait/retry/work.

## 9. Stable acceptance registry

### `AC-065` — exact source-pinned SQLite and hardening

The locked offline build links only accepted SQLite 3.53.4 source/options through the
exact reviewed sys-crate patch. Runtime/source/options and per-connection hardening
are verified before mutation; extensions, URI paths and system fallbacks are absent.
Formal CI selects arm64 `macos-26` but treats the mutable runner label only as an
availability mechanism: the exact Xcode/build/SDK/clang paths, identities and
digests and the closed build-host permission matrix—including the sole exact
`/Applications` root/admin `0775` exception—must pass §6.1 preflight before any
Cargo command.

### `AC-066` — fail-closed first-create authority

Absent and correctly owned empty APFS targets bootstrap through the exact staging
protocol and durable versioned intent. Lock-only and every proven intent/staging
state follow §6.6; unproven content, symlink, wrong type/owner/mode and unsupported
filesystem, ownership-disabled volume, any parent `ALLOW`/unknown/malformed ACL,
unknown object/entry flag bit, ACL-object `DEFER_INHERIT`, entry/representation
bound violation, any inheritable final-parent ACE, or any root/internal-file ACL
entry/object flag fail without
automatic permission rewriting, deletion or canonical replacement. A conforming
non-inheritable deny-only ancestor/final-parent ACL is accepted. Parent mutation
authority and absence of inheritable final-parent ACLs are proven before first
create. A missing lock beside any canonical/recovery entry fails without recreation.

### `AC-067` — deterministic immutable migration

Fresh bootstrap and reopen enforce the exact static schema, parameterized metadata,
filename grammar, sequence, exact-byte checksum and typed singleton validation.
Tamper, gaps, extras, duplicates, malformed values and partial transactions fail.

### `AC-068` — exclusive durable Library lifecycle

One process holds the verified lock/authority for the opened Library lifetime.
Contention is finite; stale unlocked locks and every defined crash state either
resume through proven intent or fail closed without cleanup authority, lock stealing,
split-brain or replacement of canonical data under the §6.7 process-termination
claim boundary.

### `AC-069` — bounded storage concurrency

Exactly one writer and the fixed read workers obey the declared queue accounting,
admission linearization, sole shutdown-before-start exception, busy policy,
cancellation and shutdown contracts. No work, transaction, connection or lock is
detached.

### `AC-070` — bootstrap-only architecture boundary

TASK-004 creates only `schema_migrations` and `library_meta`, exposes no raw SQL or
later-task capability, and adds only approved exact dependencies/local source.
`mengxia-store-sqlite` retains `#![forbid(unsafe_code)]`; unavoidable unsafe is
isolated in the separate accepted `mengxia-platform-fs` FFI crate and only its one
private macOS module. The platform crate exposes only safe owned/borrowed-descriptor
types and has no SQLite/store/application dependency. TASK-004 introduces neither a
custom VFS nor system SQLite.

### `AC-071` — crash/corruption safety and diagnostics

Every crash point and WAL/SHM/corruption case has an explicit expected result:
recover the last complete commit, continue a proven bootstrap, or fail readiness
without unauthorized cleanup. Error mappings are exact and public diagnostics
contain no SQL, rows, raw paths, credentials or secrets. This AC makes no OS-crash,
power-loss or physical-media ordering claim.

### `AC-072` — typed pre-mutation store DTO validation

The Library root and three TASK-004 DB values reach the store only through one
already selected typed DTO, which the pure validator converts to immutable
`StoreConfig`. The store performs no source I/O, environment capture, precedence,
text parsing or default selection. Missing/invalid DTO fields, malformed paths,
zero, out-of-range or loosening values fail with `STORAGE_CONFIGURATION_ERROR`
before any Library-root mutation. This AC deliberately does not claim production
four-layer resolution; that observable ownership is assigned to TASK-003 in §7.1.

### `AC-073` — whole-prefix path authority and platform FFI isolation

The Library path is opened component-by-component from `/` using retained
descriptor-relative no-follow directory opens. Every prefix edge is revalidated by
name and inode before/after stock SQLite opens; `O_NOFOLLOW` on the database is never
treated as prefix protection. Different-UID namespace mutation is denied by the
accepted mount/owner/mode/ACL policy, while root/same-eUID races remain explicitly
outside the V1 threat claim. Store code stays unsafe-free and consumes only the safe
`mengxia-platform-fs` authority object.

## 10. Stable test registry and required matrices

| Test ID | Observable proof |
|---|---|
| `TEST-SQLITE-004` | exact version/source/options, forbidden extensions/system linkage and connection hardening |
| `TEST-CONFIG-004` | §7.1 resolved DTO/path/range/tightening/immutability/pre-mutation matrix and source-I/O-free pure store validator; explicitly no production precedence PASS claim |
| `TEST-BOOTSTRAP-004` | absent/empty/lock-only/intent golden-wire-format/metadata/type/owner/mode/ACL/inheritance/ownership-disabled/symlink/filesystem matrix and cleanup-authority denial |
| `TEST-PATH-004` | real-APFS component walk from `/`; symlink/non-directory at every prefix depth; root/eUID ownership and allow-vs-deny ACL policy; name-to-inode replacement before revalidation; fixed-path derivation; no SQLite call after failed proof; explicit same-eUID/root non-claim |
| `TEST-MIGRATION-004` | static DDL, bound metadata, grammar/order/checksum, exact complete `sqlite_schema`/autoindex allowlist, fresh/reopen/gap/extra/duplicate/tamper/rollback |
| `TEST-LOCK-004` | two-process exclusion, finite conflict, stale unlocked file, missing-name/no-recreate split-brain denial, crash/restart and authority identity |
| `TEST-QUEUE-004` | writer cap-minus-one/cap/cap-plus-one, admission/shutdown race linearization, exact terminal disposition, read saturation, cancellation and joined shutdown |
| `TEST-ERROR-004` | every §7.4 SQLite primary/extended, config, timestamp/ID generation, shutdown and worker condition maps to its exact stable code and redacted detail |
| `TEST-RECOVERY-004` | exact same-OS SIGKILL visibility at every §10.1 point plus separate `BootstrapFsOps` failure/order traces for disk-full/fsync/checkpoint/open; no power-loss PASS claim |
| `TEST-WAL-004` | §10.2 WAL/SHM result matrix plus §10.3 multi-connection WAL-reset/checkpoint concurrency regression |
| `TEST-CORRUPTION-004` | complete §10.4 deterministic corruption matrix with exact fail-before-admission codes |
| `TEST-ARCH-004` | eighteenth-package inventory and dependency direction; store retains `forbid(unsafe_code)`/no libc; exact C shim plus one private audited Rust FFI module in `mengxia-platform-fs`; C/Rust ABI layout and symbol allowlist; private-construction compile-fail fixtures; path-copy fixture compiles but repository lint rejects it; every rusqlite open confined to the sole trusted consumer; no raw type escape, raw SQL/IPC/later capability/custom VFS |
| `TEST-SUPPLY-004` | developer builds record identities and cannot claim attestation; exact attested pins/features/licenses/digests/local diff; no `cc` crate; exact Xcode/SDK/clang paths, versions and digests; full ordered command/environment JSON; ABI probe; every named/plain/target-prefixed C compiler/flag/include/SDK/link/archive/fake override rejected before tool execution and attested Rust wrapper/flag overrides rejected; no Rust `libc` ACL dependency; offline source-pinned build and one SQLite link; formal CI uses arm64 `runs-on: macos-26`, switches only to `/Applications/Xcode_26.6.app/Contents/Developer`, records hosted-image provenance and eUID/GID/groups, selects the attested class, passes the exact tuple/path/digest preflight before Cargo and provides no fallback; real local and CI positives prove `/` UID0/GID0/0755, the sole `/Applications` UID0/GID80/0775 exception, and root-or-recorded-build-eUID non-group/world-writable bundle/used descendants; the same pure policy receives synthetic wrong type/owner/GID/every mode bit, non-admin non-root build eUID, world-write, exception-at-another-path, bundle/internal group-write, alias escape/broken/looping symlink, metadata failure and post-archive mutation records and must reject each before link metadata; tuple/path/digest substitutions still prove fail-before-Cargo behavior |
| `TEST-DOC-004` | full stable IDs, status/start/completion lifecycle and downstream dependency invariants |

### 10.1 Exact crash injection points

`TEST-RECOVERY-004` sends `SIGKILL` to a subprocess at each boundary and reopens in a
new process on the same still-running macOS instance; it does not simulate kernel
crash or power loss:

1. root directory created/fsynced, before parent directory fsync;
2. parent directory fsynced, before lock creation;
3. lock entry created and acquired, before lock-file fsync;
4. lock file fsynced, before root-directory fsync;
5. root directory fsynced, before locked re-enumeration;
6. locked re-enumeration proves lock-only, before intent creation;
7. intent entry created, and after each deterministic returned prefix write, before
   the full record write completes;
8. full intent record written, before intent-file fsync;
9. intent file fsynced, before root-directory fsync;
10. intent and root fsynced, before staging creation;
11. staging file created, before root-directory fsync;
12. staging entry durable, before SQLite open;
13. WAL/hardening verified, before `BEGIN IMMEDIATE`;
14. static DDL executed, before singleton insert;
15. singleton inserted, before migration row insert;
16. migration row inserted, before commit;
17. committed, before checkpoint/truncate;
18. closed and staging file fsynced, before `linkat` publish;
19. canonical hard link created, before root-directory fsync;
20. root directory fsynced, before staging unlink;
21. staging name unlinked, before root-directory fsync;
22. root directory fsynced, before intent unlink; and
23. intent unlinked, before final root-directory fsync.

For this table, the child acknowledges only after the operation named to the left of
“before” has returned successfully. Therefore the expected same-OS namespace state
is exact, not an OS-crash durability alternative and not “cleanup if validation
fails”:

| Crash point | Permitted durable observation on restart | Expected result |
|---|---|---|
| 1 | empty root exists | open and validate that exact empty root; do not accept absence as SIGKILL evidence |
| 2 | empty root | create/reuse lock and continue |
| 3 | lock-only root; lock contains exactly the bytes written before acknowledgement (normally zero) | acquire/validate the existing unlocked lock; absence is failure of this SIGKILL test |
| 4 | lock-only root | acquire/validate the existing unlocked lock; absence is failure of this SIGKILL test |
| 5 | lock only | acquire and repeat the mandatory post-lock revalidation/enumeration |
| 6 | lock only | reuse unlocked validated lock, re-enumerate under lock and then create intent |
| 7 | intent exists with exactly the acknowledged prefix length/bytes, including the zero-length create-only variant | it is unproven intent and returns `STORAGE_CONFIGURATION_ERROR`; absence or different bytes fail the SIGKILL fixture |
| 8 | complete valid 256-byte intent exists | revalidate, fsync intent and root, then continue; absent/partial bytes fail this SIGKILL fixture |
| 9 | complete valid 256-byte intent exists | validate and continue; absent/partial bytes fail this SIGKILL fixture |
| 10 | valid durable intent, no staging | create authorized staging |
| 11 | valid intent plus authorized empty staging | perform §6.6 proven cleanup/restart; absent staging fails this SIGKILL fixture |
| 12 | valid intent plus empty staging | proven cleanup/restart, never generic name-based cleanup |
| 13, 14, 15, 16 | valid intent plus incomplete/rolled-back staging | SQLite recovery attempt; if §5.4 cannot pass, proven cleanup/restart |
| 17 | valid intent plus committed staging/WAL | recover SQLite to its last valid commit, then validate and publish |
| 18 | valid intent plus complete validated staging | publish |
| 19 | canonical and staging names both exist and identify the same inode, plus valid intent | finish ordered publish cleanup; canonical absence or distinct inodes fail this SIGKILL fixture |
| 20 | canonical/staging same inode plus intent | finish ordered staging and intent cleanup |
| 21 | canonical plus intent; staging name is absent | finish intent cleanup; staging presence fails this SIGKILL fixture |
| 22 | valid canonical plus intent | remove verified matching intent and reopen |
| 23 | valid canonical; intent and staging names are absent | normal reopen; intent presence fails this SIGKILL fixture |

No case admits a partial schema, replaces canonical data or deletes an entry whose
ownership is not proven by a valid intent. Mid-operation states outside these exact
SIGKILL boundary observations follow §6.6 and may intentionally require operator
action. Separate `BootstrapFsOps` tests cover syscall errors/order; OS/power-loss
namespace alternatives remain `UNVERIFIABLE` and are not permitted results here.

### 10.2 WAL and SHM expected-result matrix

Fixtures record the exact acknowledged commit sequence before mutation so recovery
can distinguish a valid previous commit from lost/corrupt durable state.

| Injected/observed state with no live connection | Expected result and code |
|---|---|
| Clean main DB; no WAL or SHM | normal reopen succeeds |
| Valid committed WAL; SHM absent | SQLite rebuilds transient SHM, recovers the commit, `quick_check` and sequence assertions pass |
| Valid DB/WAL; stale or malformed SHM bytes | SQLite recreates/rebuilds SHM; success is required if final integrity and commit sequence match |
| Valid last commit followed by well-formed uncommitted WAL frames | trailing frames are ignored; reopen returns the last committed sequence |
| Valid last commit followed by incomplete bytes or an invalid-checksum uncommitted frame | WAL scan stops at the first invalid frame; reopen returns the last valid commit |
| Valid WAL reset with a new salt/checksum chain | reopen succeeds and returns every acknowledged commit in the new chain |
| Invalid checksum/salt/frame within content required for an acknowledged commit, or committed sequence missing after recovery | `STORAGE_CORRUPTION`; fail before admission |
| Malformed/truncated main DB or failed `quick_check` after SQLite recovery | `STORAGE_CORRUPTION`; fail before admission |
| WAL/SHM symlink, wrong type, owner or mode | `STORAGE_CONFIGURATION_ERROR`; SQLite is not allowed to open it |
| Permission, disk-full or open/read/write/checkpoint/fsync failure | `STORAGE_IO_ERROR` |

SHM is never treated as canonical database content and its absence alone is not
corruption. Manually deleting a SHM while another process is live is not a supported
operation and is excluded by the held Library lock.

### 10.3 Required WAL-reset concurrency regression

`TEST-WAL-004` uses a standalone test-only engine database in an isolated APFS temp
directory outside every canonical Library root. It links the exact bundled SQLite
3.53.4 and applies the same connection hardening, but it is never opened through the
Library bootstrap/reopen API. Test-only static DDL creates exactly:

```sql
CREATE TABLE wal_reset_probe (
    sequence INTEGER PRIMARY KEY NOT NULL,
    writer INTEGER NOT NULL,
    payload BLOB NOT NULL,
    checksum BLOB NOT NULL CHECK (length(checksum) = 32)
) STRICT;
```

That DDL lives only in the integration-test fixture, never in a migration or
production module. `TEST-ARCH-004` proves canonical TASK-004 migrations/reopen still
permit only `schema_migrations` and `library_meta` and that production code cannot
reference `wal_reset_probe`.

The fixture disables auto-checkpoint and opens at least four independent connections:
writer A, writer B, a dedicated checkpointer and a reader holding a snapshot. Each
acknowledged transaction inserts one sequence plus deterministic payload/checksum,
providing the test data model without changing canonical schema. The targeted
schedule must:

1. commit a numbered/checksummed baseline and retain the reader snapshot;
2. commit additional numbered transactions from alternating writer connections;
3. call `sqlite3_wal_checkpoint_v2` with `SQLITE_CHECKPOINT_FULL` while the retained
   reader prevents all frames from being backfilled/reset, recording the returned
   log/backfill counts and the exact allowed result below;
4. release the reader and repeat `SQLITE_CHECKPOINT_FULL` until backfill equals the
   WAL frame count, then enter an explicit `SQLITE_CHECKPOINT_RESTART` boundary;
5. coordinate the next writer commit, the first post-checkpoint WAL reset and another
   full checkpoint across test barriers; finish with `SQLITE_CHECKPOINT_TRUNCATE`,
   while preserving the configured finite busy budget; and
6. close all connections, reopen, run `quick_check`, checkpoint again, and assert the
   exact set/order/checksum of every acknowledged commit with no loss or duplicate.

The mandatory bounded stress phase executes 16 fixed deterministic schedule seeds,
each with 256 checkpoint/reset/commit cycles. It records WAL header salt/reset
observations only at quiescent barriers, repeats final reopen/integrity assertions for
every seed, and fails on any missing/duplicate acknowledged commit, corruption,
unbounded busy wait or unexpected SQLite result. An optional longer soak may add
randomized schedules but cannot replace this deterministic evidence.

Every checkpoint call has a phase-specific allowed result:

| Checkpoint call | Allowed immediate result | Required eventual assertion |
|---|---|---|
| `FULL` with retained reader/writer contention | `SQLITE_OK` with backfill lower than log frame count, or immediate/handler `SQLITE_BUSY` | no integrity assertion until contention is released; no retry occurs inside production code |
| `FULL` after reader release but while a scheduled writer is active | `SQLITE_OK` or bounded `SQLITE_BUSY` | after the writer barrier, one quiescent `FULL` must return `SQLITE_OK` with backfill equal to log frame count |
| `RESTART` at the coordinated writer/reset race | `SQLITE_OK` or bounded `SQLITE_BUSY` | after writers quiesce, a separate `RESTART` must return `SQLITE_OK` before the next cycle |
| Final quiescent `TRUNCATE` | only `SQLITE_OK` | WAL is zero-length/absent as documented and final reopen assertions pass |

`SQLITE_LOCKED`, `MISUSE`, corruption, an unknown result, a busy-handler sleep policy
violation or failure to reach the required quiescent result fails the test. The test
subprocess has an independent 30-second watchdog to detect hangs; that watchdog is
test infrastructure, not a production I/O or SQLite wall-clock guarantee. Bounded
repeated checkpoint calls are part of this scheduler only; they do not add a
production retry loop or weaken §7.4 mappings.

### 10.4 Deterministic corruption matrix

`TEST-CORRUPTION-004` independently covers only states expected to fail: malformed
database header/page; truncated database; bit-flipped table/index content;
checksum/salt/frame damage required for an acknowledged WAL commit; `quick_check`
failure; missing/extra/wrong-shape/non-STRICT bootstrap tables; wrong singleton count;
extra trigger, view, manual/partial/expression index, virtual/shadow table or
`sqlite_sequence`; missing/renamed/wrong-column expected autoindex;
invalid UUID bytes; timestamp seconds one below/above the TASK-002 bounds; each row's
invalid nanos;
`library_meta.created_at` differing from `schema_migrations.applied_at`; valid intent
timestamp differing from either or both rows; wrong owner UID; migration missing,
extra, duplicate sequence/name, sequence/name-prefix mismatch, non-contiguous order,
invalid filename grammar and checksum mismatch; canonical/staging distinct inodes;
intent UUID/migration/timestamp mismatch; every version-one wire field/length/
reserved/padding/checksum mutation; unproven/invalid intent; unauthorized staging/
sidecars; `MNT_IGNORE_OWNERSHIP`; an ancestor/final-parent `ALLOW` ACE,
unknown/malformed ACL tag, ACL-object or entry flag bit, ACL-object `DEFER_INHERIT`,
the 129th entry, external representation larger than 16,384 bytes or differing after
known-flag reconstruction, any inheritable ACE on the final parent, or any ACL entry
or object flag on the Library root/internal files; symlink or non-directory at each absolute-path
prefix depth; prefix owner/mode/ACL violation; name-to-inode replacement detected
before the SQLite call; and wrong file type/owner/mode. Separate positive fixtures
prove that a non-inheritable deny-only ACL on an ancestor or final parent is accepted
when every other §6.2 condition passes. Data/integrity damage and a valid-intent-to-database
identity/timestamp mismatch return `STORAGE_CORRUPTION`; unsafe or unproven
filesystem/recovery metadata returns `STORAGE_CONFIGURATION_ERROR`; every case fails
before writer/read admission with redacted detail and no automatic cleanup or ACL
rewriting.

All runtime/filesystem/lock/crash/corruption evidence uses isolated real SQLite files
and subprocesses; mocks alone cannot satisfy it. Required macOS/APFS evidence reports
`UNVERIFIABLE` on unsupported hosts rather than passing via skip.

### 10.5 Required whole-prefix and FFI-boundary matrix

`TEST-PATH-004` runs on a real local APFS tree and fixes one expected outcome per
case:

| Case | Expected evidence/result |
|---|---|
| Safe `/`-anchored root/eUID-owned chain, non-writable to group/world | every component is retained and exact edge revalidation succeeds |
| Symlink substituted at each individual prefix depth, final parent, Library root or database target | `STORAGE_CONFIGURATION_ERROR` before root/SQLite mutation; final-target `SQLITE_OPEN_NOFOLLOW` is not credited for prefix cases |
| Regular file/FIFO/socket or missing component in an intermediate position | fail before mutation; no fallback string resolution |
| Prefix owner neither root nor daemon eUID, group/world-writable prefix, `MNT_IGNORE_OWNERSHIP`, malformed/unknown ACL tag or flag bit, ACL-object `DEFER_INHERIT`, or any `ALLOW` ACE | fail closed with redacted configuration error |
| Non-inheritable deny-only ancestor/final-parent ACL, including macOS `deny delete`; ACL-object flags zero or only `NO_INHERIT` | accepted if every other check passes; proves the policy does not reject the standard protective home ACL merely because an ACL exists |
| Any inheritable ACE on the final parent | rejected before root creation, including an inheritable deny ACE |
| Component renamed/replaced after initial walk but before the explicit pre-SQLite revalidation seam | device/inode edge mismatch; SQLite-open seam is not called and no target is mutated |
| C/Rust ABI version/layout/constants and every SDK return shape | checked-in C static assertions plus Rust offset tests pass against the selected SDK; ACL-object and entry flags are distinct; portable reconstruction detects unknown bits |
| FFI null/error/malformed iteration/bounds and cleanup paths | safe typed error; 128 entries complete, the 129th fails immediately; external buffers never exceed 16,384 bytes; every non-null original/duplicate/buffer is freed once, including cleanup failure and repeated calls; no unwind/raw escape |
| Fixed canonical/staging child passed to stock SQLite | only the private `stock_sqlite_open` consumer may take the lifetime-bound token and temporarily borrow `&Path`; pre/post revalidation both execute |
| Attempt to forge a child token or choose a basename | compile failure from private construction surface |
| Attempt to copy/persist/log a borrowed path or open rusqlite elsewhere | valid Rust may compile, but the repository architecture lint must fail; this is not credited as a type-system guarantee |
| Attempt by store code to construct authority, import `libc`, use `unsafe` or access raw platform symbols | compile/architecture failure |

Real symlink, file-kind, chmod and name-replacement cases are mandatory. ACL tag/flag
branches require real macOS ACL fixtures where the test account may create them plus
independent deterministic wrapper tests; inability to create required APFS/ACL
fixtures is `UNVERIFIABLE`, not PASS. The suite does not attempt or claim prevention
of a root/same-eUID race after final revalidation, because §6.4 excludes that threat.

## 11. Canonical start-record inputs

This is the enumerated input copied into the canonical Implementation Plan start
record. The Plan owns lifecycle status; this supplement owns the detailed contract.
Every stable ID is spelled separately:

```text
TASK: TASK-004
STATUS: IN_PROGRESS
DEPENDENCIES: TASK-002; BASE-011; BASE-013; BASE-014; BASE-015; BASE-017;
  DEC-017; DEC-020; DEC-021; DEC-022; ADR-0001; ADR-0003; ADR-0004;
  ADR-0005; ADR-0006
REQUIREMENTS: FUNC-001; DATA-001; DATA-005; DATA-006; DATA-007; DATA-011;
  REL-001; SEC-017; SEC-020; SEC-021; CFG-001; CFG-003
ACCEPTANCE: AC-065; AC-066; AC-067; AC-068; AC-069; AC-070; AC-071;
  AC-072; AC-073
TESTS: TEST-SQLITE-004; TEST-CONFIG-004; TEST-BOOTSTRAP-004; TEST-MIGRATION-004;
  TEST-PATH-004; TEST-LOCK-004; TEST-QUEUE-004; TEST-ERROR-004; TEST-RECOVERY-004;
  TEST-WAL-004; TEST-CORRUPTION-004; TEST-ARCH-004; TEST-SUPPLY-004;
  TEST-DOC-004
FORBIDDEN: TASK-003 transport; later migrations and repositories; Blob, Provider,
  Plugin, Admin or destructive behavior; raw SQL API; custom VFS; system SQLite;
  unbounded wait, retry or detached work; architecture or public-interface expansion
```

Canonical Specification/Decisions/Review/Plan versions and status are synchronized;
`TEST-DOC-004` verifies every reference and the active start record.

## 12. Downstream non-regression proof

Option A changes only the prerequisite that TASK-003 depends on TASK-004. TASK-004
still depends on TASK-002. TASK-006 still depends on TASK-004 and TASK-005. TASK-007
still depends on TASK-003 and TASK-006. TASK-011 still depends on TASK-003 and
TASK-010. The graph is acyclic.

TASK-004's cross-task output remains an internal opened Library context containing
verified owner/lock authority. TASK-003 later consumes it without importing SQLite.
TASK-006 later adds the complete immutable `0001_library_assets` migration. TASK-005
remains independently executable after TASK-002. The accepted
`mengxia-platform-fs` is a downward-only infrastructure/FFI leaf: store may depend on
it; it cannot depend on store/storage/application; TASK-005 may reuse its safe path
authority later only through its own gate. No downstream Task ID/order, public
product interface or acceptance ownership is moved or renumbered.

Configuration ownership is deliberately split without creating a dependency cycle:
TASK-004 owns the source-free `ResolvedStoreConfig` DTO validator and all store-side
pre-mutation bounds; TASK-003 later owns the `mengxiad` production resolver and real
startup integration. When the TASK-003 draft is canonicalized, its plan row must add
`CFG-001` alongside its existing `CFG-003`, and its newly allocated stable AC/TEST
entries must cover every production behavior enumerated in §7.1. TASK-004's
`AC-072`/`TEST-CONFIG-004` cannot be cited as that evidence.

## 13. Acceptance closure

1. Specification v1.1.9 incorporates this complete contract by reference and owns
   the canonical task/status/error-registry synchronization.
2. ADR-0006 accepts the checked-in C shim/unsafe boundary, developer-versus-attested
   evidence classes, arm64 `macos-26` CI contract, exact attested preflight and the
   explicit root/admin/build-eUID trust boundary.
3. The Implementation Plan contains the synchronized TASK-004 `IN_PROGRESS` start
   record with the full §11 ID set and exact file/forbidden scope.
4. TASK-003 canonicalization—not TASK-004 implementation—must add
   `CFG-001`/`CFG-003` plus stable production resolver/startup AC and TEST ownership
   to its canonical registry; do not treat `TEST-CONFIG-004` as proxy evidence.
5. Implementation remains limited to §8. TASK-003, Admin, Provider, Plugin, product
   schema and every later capability remain unauthorized.
