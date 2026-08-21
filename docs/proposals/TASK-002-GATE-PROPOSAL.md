# TASK-002 start-gate proposal

> Status: **ACCEPTED / INCORPORATED IN CANONICAL v1.1.6**
>
> Date: 2026-08-21
>
> Scope: retained decision provenance for the `TASK-002` start gate
>
> Authority: the user accepted the modified bundle on 2026-08-21. Its normative
> contracts and IDs are incorporated in Specification v1.1.6 and Plan v0.3.6;
> canonical documents remain the definition sites. This retained proposal is
> provenance and does not authorize `TASK-003` or any later task.
> Sections 1–9 retain the proposal rationale; Sections 10–11 record its completed
> disposition.

## 1. Gate purpose

At proposal time, `TASK-001` was complete. Specification §0.5 and §4 require every later task to have
stable Feature, Requirement, Acceptance and Test references in a task-start record
before it becomes `IN_PROGRESS`. The then-current canonical documents did not provide
that registry for `TASK-002`.

This proposal supplies one reviewable bundle for that missing gate:

- exact public behavior for typed UUIDv7 IDs, SHA-256 digest values, timestamps and
  revision numbers;
- the minimal error-taxonomy baseline that can be implemented without guessing an
  operation contract;
- proposed immutable Acceptance and Test IDs;
- an exact, minimally featured dependency candidate set;
- negative/security tests and a copy-ready start record;
- explicit exclusions that prevent early implementation of later tasks.

Acceptance was recorded by incorporating its normative parts into the canonical
Specification and Plan and synchronizing every current-state entry document named in
§10. `TASK-002` remained blocked until that revision passed document traceability;
the canonical completion record now reports the later implementation evidence.

## 2. Evidence and current prerequisites

| Evidence | Current result |
|---|---|
| Root `AGENTS.md` | `TASK-001` complete; `TASK-002` is next but its start gate is required |
| Specification §18 | Goal and broad scope are present; stable Acceptance/Test IDs are absent |
| Plan §5 | `TASK-002` is `PENDING`; dependency is completed `TASK-001` plus accepted serialization limits |
| Review §7 `TASK-002` | Value types/error families are identifiable; task-start registry is still required |
| `ADR-0005` | Foundation finite limits applicable to `TASK-002` are accepted |
| Repository | clean `main` baseline at `60b3445`; `mengxia-types` and `mengxia-domain` are empty skeletons |

No open Provider, Admin, sandbox, Credential, rights, retention or production
deployment decision is a prerequisite for this value-object-only task.

## 3. Proposed scope boundary

### Included

- `mengxia-types`: opaque typed IDs, digest value, timestamp value, revision value,
  stable error-code enum and safe typed value errors.
- `mengxia-domain`: only the minimal typed domain-error baseline described below.
- Unit, property, malformed-input, boundary, architecture and supply-chain tests.
- Canonical documentation and task-lifecycle evidence required to start and finish
  this task.

### Excluded

- Protobuf, Serde DTOs, SQLite row mappings, schemas and migrations.
- File/stream SHA-256 computation; that remains owned by `TASK-005`.
- Command handlers, IPC, authentication, authorization, persistence, retries,
  idempotency records or pagination.
- Provider/Plugin types, transport error mapping, HTTP status mapping, logging or
  metrics implementation.
- A domain clock service, public/global `Timestamp::now`, or MengXia-owned mutable
  global ID generator. The fallible ID generator may read OS time and entropy only
  through the exact private path defined below.
- Any behavior from `TASK-003` or later.

## 4. Proposed public contracts

All four values are opaque: their representation fields remain private. They do not
implement `Default`, do not expose dependency-specific types in public signatures,
and implement only the traits listed here. `mengxia-types` re-exports the four value
types and their public error types from its crate root; their internal module layout is
not public API. `mengxia-domain` re-exports `DomainError` from its crate root. Domain
Object, wire DTO and database Row remain separate types with explicit later-task
mappers.

### 4.1 `Id<T>`

Public behavior:

- A generated or parsed value is a non-nil RFC 9562 UUID with RFC variant and
  version 7. Construction cannot produce or retain another UUID version.
- `Id::<T>::try_new() -> Result<Id<T>, IdGenerationError>` is the only production
  generator. It reads `SystemTime::now()` privately, rejects time before the Unix
  epoch or outside the UUIDv7 48-bit millisecond field, fills exactly ten random bytes
  with the direct fallible `getrandom::fill` API, and passes those values to
  `uuid::Builder::from_unix_timestamp_millis`. It does not call `Uuid::now_v7`, does
  not catch a dependency panic and has no MengXia-owned or dependency-owned shared
  counter/context.
- `Id::<T>::from_bytes(bytes: [u8; 16]) -> Result<Id<T>, ValueError>` validates
  variant, version and non-nil status.
- `Id::<T>::to_bytes(self) -> [u8; 16]` returns the exact canonical bytes used by
  future explicit Row mappers.
- `FromStr` accepts exactly 36 ASCII bytes in lowercase hyphenated form
  (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`). Uppercase, simple, braced, URN,
  whitespace-padded, nil, non-RFC-variant and non-v7 forms are rejected.
- `Display` emits that same lowercase canonical form; parsing its output round-trips.
- The public trait set is `Clone`, `Copy`, `Eq`, `Ord`, `Hash`, `Debug`, `Display`
  and `FromStr`, without requiring those traits on marker type `T`.
- Different marker types have no implicit conversion. No public API converts
  `Id<A>` into `Id<B>`.
- Generation is safe to call concurrently and promises UUIDv7 validity and practical
  uniqueness, not global strict monotonicity or a total creation order. Durable
  ordering remains a separate data contract.

Rejected alternatives:

- accepting every textual form understood by the UUID crate;
- silently normalizing uppercase or whitespace;
- exposing `uuid::Uuid` in the public MengXia contract;
- using `Uuid::now_v7`, whose process-shared ordering context and infallible surface do
  not satisfy this task's explicit no-hidden-global-state and typed-failure contract;
- implementing `Default` or accepting nil as a sentinel.

### 4.2 `Sha256Digest`

Public behavior:

- The value is exactly 32 opaque bytes and is not an ID type.
- `Sha256Digest::from_bytes(bytes: [u8; 32]) -> Sha256Digest` accepts every 32-byte
  value; a zero byte-array is not used as a sentinel and is therefore not specially
  rejected.
- `Sha256Digest::to_bytes(self) -> [u8; 32]` returns the exact bytes for future
  explicit Row mappers.
- `FromStr` accepts exactly 64 lowercase ASCII hexadecimal characters. Uppercase,
  mixed case, prefixes, separators, whitespace and non-hex input are rejected rather
  than normalized.
- `Display` emits exactly 64 lowercase hex characters and round-trips through
  `FromStr`.
- The public trait set is `Clone`, `Copy`, `Eq`, `Ord`, `Hash`, `Debug`, `Display`
  and `FromStr`.
- No hashing API and no `sha2` dependency is introduced in `TASK-002`.

The strict lowercase rule follows Specification §1.1's wire form and avoids multiple
text representations for the same digest.

### 4.3 `Timestamp`

Public behavior:

- The value is an instant normalized to UTC with nanosecond precision.
- Its allowed range is UTC year 0001 through 9999, matching the safe intersection
  needed by the future Protobuf timestamp mapper.
- `Timestamp::from_unix_seconds_nanos(seconds: i64, nanos: u32)
  -> Result<Timestamp, ValueError>` rejects nanoseconds above `999_999_999` and
  instants outside that range.
- `Timestamp::unix_seconds(self) -> i64` and
  `Timestamp::subsec_nanoseconds(self) -> u32` expose dependency-neutral components.
- `FromStr` accepts only the canonical UTC grammar
  `YYYY-MM-DDTHH:MM:SS[.fraction]Z`: uppercase `T`/`Z`, exactly four year digits,
  one to nine fractional digits only when non-zero, and no trailing fractional
  zeros. Leap-second text, local offsets (including `+00:00`), spaces, lowercase
  suffixes and out-of-range values are rejected.
- `Display` emits that unique shortest form while retaining full nanosecond
  precision; parsing its output round-trips.
- The public trait set is `Clone`, `Copy`, `Eq`, `Ord`, `Hash`, `Debug`, `Display`
  and `FromStr`.
- No `now()` or local-time API is provided. A later clock port owns time acquisition
  so domain tests need not depend on a global clock.

This task provides a canonical Core text codec only. It does not implement Serde,
Protobuf or SQLite conversion.

### 4.4 `RevisionNo`

Public behavior:

- The value is an opaque `u64` optimistic-concurrency revision.
- `RevisionNo::INITIAL` is `0`; the first successful mutation advances it to `1`.
- `RevisionNo::new(value: u64) -> RevisionNo` and `RevisionNo::get(self) -> u64`
  perform explicit construction/access. There is no `Default`.
- `RevisionNo::checked_next(self) -> Result<RevisionNo, RevisionOverflow>` returns
  the typed error at `u64::MAX`; wrapping and saturation are forbidden.
- `FromStr` accepts `0` or a non-zero ASCII decimal without sign, whitespace or
  leading zeroes, and rejects overflow.
- `Display` emits the same canonical decimal form.
- The public trait set is `Clone`, `Copy`, `Eq`, `Ord`, `Hash`, `Debug`, `Display`
  and `FromStr`.

### 4.5 Error baseline

`mengxia-types` defines the following exact `#[non_exhaustive] ErrorCode` variants.
The first twenty retain Specification §14.1; the final five close already-written
canonical error references and the two value-boundary outcomes made explicit by this
proposal. Canonical acceptance must add the final five rows to §14.1 rather than
leaving a second registry here.

```text
Rust variant                 Stable string
ValidationError              VALIDATION_ERROR
AuthenticationError          AUTHENTICATION_ERROR
AuthorizationDenied          AUTHORIZATION_DENIED
NotFound                     NOT_FOUND
Conflict                     CONFLICT
InvalidTransition            INVALID_TRANSITION
SourceModifiedDuringIngest   SOURCE_MODIFIED_DURING_INGEST
StorageIoError               STORAGE_IO_ERROR
StorageCorruption            STORAGE_CORRUPTION
ProviderValidation           PROVIDER_VALIDATION
InvalidCredential            INVALID_CREDENTIAL
ProviderRateLimited          PROVIDER_RATE_LIMITED
ProviderTimeout              PROVIDER_TIMEOUT
ProviderUnavailable          PROVIDER_UNAVAILABLE
SubmissionUnknown            SUBMISSION_UNKNOWN
PluginProtocolViolation      PLUGIN_PROTOCOL_VIOLATION
SandboxUnavailable           SANDBOX_UNAVAILABLE
PluginRevoked                PLUGIN_REVOKED
Backpressure                 BACKPRESSURE
InternalError                INTERNAL_ERROR
CommandInProgress            COMMAND_IN_PROGRESS
AdminAuthUnavailable         ADMIN_AUTH_UNAVAILABLE
UnsupportedCapability        UNSUPPORTED_CAPABILITY
IdGenerationUnavailable      ID_GENERATION_UNAVAILABLE
RevisionExhausted            REVISION_EXHAUSTED
```

The five canonical §14.1 rows added by acceptance are:

| Code/family | Source | Retryable | API exposure | Log level | Metric |
|---|---|---:|---|---|---|
| `COMMAND_IN_PROGRESS` | durable command claim | caller may retry with bounded delay | safe retry guidance only | INFO | `command_in_progress_total` |
| `ADMIN_AUTH_UNAVAILABLE` | platform/Admin authority | no until accepted evidence becomes available | generic safe platform limitation | WARN | `admin_auth_unavailable_total` |
| `UNSUPPORTED_CAPABILITY` | declared Provider/Plugin capability contract | no until adapter/configuration changes | safe capability identifier after authorization | INFO | `unsupported_capability_total` |
| `ID_GENERATION_UNAVAILABLE` | OS clock or entropy | conditional after platform condition changes | generic safe message | ERROR | `id_generation_failures_total` |
| `REVISION_EXHAUSTED` | optimistic revision counter | no | generic safe message and authorized object type | ERROR/ALERT | `revision_exhaustion_total` |

Exact public error shapes:

```rust
#[non_exhaustive]
pub enum ValueError {
    InvalidId,
    InvalidDigest,
    InvalidTimestamp,
    InvalidRevision,
    UnknownErrorCode,
}

#[non_exhaustive]
pub enum IdGenerationError {
    ClockBeforeUnixEpoch,
    TimestampOutOfRange,
    EntropyUnavailable,
}

pub struct RevisionOverflow;

#[non_exhaustive]
pub enum DomainError {
    InvalidValue(ValueError),
    IdGeneration(IdGenerationError),
    RevisionOverflow(RevisionOverflow),
}
```

Contract:

- `ErrorCode::as_str(self) -> &'static str`, `Display` and `FromStr<Err =
  ValueError>` use only the exact mappings above. Unknown, case-folded or
  whitespace-padded codes return `ValueError::UnknownErrorCode`.
- `Id<T>`, `Sha256Digest`, `Timestamp` and `RevisionNo` implement
  `FromStr<Err = ValueError>` and return respectively `InvalidId`, `InvalidDigest`,
  `InvalidTimestamp` and `InvalidRevision`; parsing errors never retain input.
- `ErrorCode` implements `Clone`, `Copy`, `Eq`, `Ord`, `Hash`, `Debug`, `Display` and
  `FromStr`; it is a classification value, not itself a `std::error::Error`.
  `ValueError`, `IdGenerationError` and `RevisionOverflow` implement `Clone`, `Copy`,
  `Eq`, `Debug`, `Display` and `std::error::Error`.
- Safe `Display` text is fixed by the table below. Derived/implemented `Debug`
  contains only enum/type and variant names.
- The enum does not expose a fixed `is_retryable()` boolean. Specification §14.1
  makes retryability contextual for several families, so a context-free boolean
  would encode incorrect policy.
- `mengxia-domain::DomainError::code()` maps invalid values to `VALIDATION_ERROR`, ID
  generation failures to `ID_GENERATION_UNAVAILABLE`, and revision exhaustion to
  `REVISION_EXHAUSTED`. Its `Display` delegates to the inner safe static diagnostic,
  and its `std::error::Error::source()` returns the inner error. `DomainError`
  implements `Debug`, `Display` and `std::error::Error`; no broader conversion blanket
  is part of TASK-002's public API.
- No raw input, secret, path, Provider payload, external error string or arbitrary
  user-provided message is stored in these error values.
- Operation-specific safe field errors, authorization reasons, retry directives,
  HTTP/Protobuf mapping, correlation IDs and logging/metrics remain with the task
  that owns the corresponding operation boundary. Defining the three already-used
  operation code strings above does not implement those later operation contracts.

| Error variant | Exact safe `Display` |
|---|---|
| `ValueError::InvalidId` | `invalid typed UUIDv7` |
| `ValueError::InvalidDigest` | `invalid SHA-256 digest` |
| `ValueError::InvalidTimestamp` | `invalid timestamp` |
| `ValueError::InvalidRevision` | `invalid revision number` |
| `ValueError::UnknownErrorCode` | `unknown error code` |
| `IdGenerationError::ClockBeforeUnixEpoch` | `system clock is before the Unix epoch` |
| `IdGenerationError::TimestampOutOfRange` | `system clock is outside the UUIDv7 range` |
| `IdGenerationError::EntropyUnavailable` | `operating-system entropy is unavailable` |
| `RevisionOverflow` | `revision number is exhausted` |

## 5. Proposed stable acceptance registry

These identifiers became canonical only through their incorporated definitions in
Specification v1.1.6; this provenance copy is not a definition site.

### `AC-055` — Typed UUIDv7 identity

All generated and accepted typed IDs are non-nil RFC-variant UUIDv7 values; exact
canonical text and 16-byte forms round-trip; malformed/non-canonical/non-v7 input is
rejected; marker types cannot be implicitly interchanged; clock/range/entropy failure
returns a typed safe error without constructing an ID or panicking in MengXia code.

### `AC-056` — Canonical digest value

Every 32-byte digest round-trips to exactly 64 lowercase hex characters; uppercase,
mixed, malformed and incorrectly sized text is rejected; the type adds no hashing,
Provider, transport or storage behavior.

### `AC-057` — Canonical timestamp and revision values

UTC timestamps and revision numbers round-trip in their unique canonical forms;
offset/non-canonical/out-of-range timestamps, non-canonical/overflowing revisions and
revision increment exhaustion fail with typed safe errors and never wrap or map to an
unexpected internal bug.

### `AC-058` — Stable safe error baseline

Every accepted error code, including the already-referenced operation codes and the
new ID-generation/revision-boundary codes, has one exact stable string form; unknown
codes are rejected; value/domain errors are typed and do not retain or expose rejected
input, secrets, paths or arbitrary payloads; no context-free retry policy is guessed.

### `AC-059` — Scope and dependency isolation

The implementation uses only the accepted minimal dependencies/features, preserves
the canonical Cargo dependency direction, and introduces no Provider, Plugin,
transport, Protobuf, Serde, database, filesystem, network or later-task behavior.

## 6. Proposed stable test registry

The repository command/target may be implemented by `TASK-002`, but each obligation
must be independently named in output and have PASS evidence before `DONE`.

| Test ID | Verification obligation | Required evidence |
|---|---|---|
| `TEST-TYPE-001` | Property tests for UUIDv7 generation, marker separation and exact text/byte round trips | deterministic test target, compile-fail marker fixture and generated-case count/result |
| `TEST-PARSE-001` | Table/property tests reject malformed, non-canonical, wrong-version, wrong-length, overflow and cap-boundary inputs | positive/negative matrix and exit status |
| `TEST-TIME-001` | Timestamp range, UTC canonicalization, fractional precision and revision boundary/overflow tests | min/max, subsecond and overflow cases |
| `TEST-ERROR-001` | Full code-string round trip, unknown-code rejection, typed mapping and safe-display/redaction tests | all codes/variants covered; canary input absent from `Display`/`Debug` |
| `TEST-ARCH-002` | Cargo/public-surface check proves allowed dependency direction and absence of Provider/Plugin/proto/DB/Serde coupling | metadata plus source/public API assertions |
| `TEST-SUPPLY-002` | Exact dependency/feature/lock/license/advisory delta passes the existing fail-closed supply policy | versions/features, cargo-deny result and unavailable-advisory behavior |
| `TEST-DOC-002` | TASK-002 IDs, task-start record, lifecycle state and explicit references satisfy closed traceability | deterministic positive and negative document tests |

Minimum test content:

- Property strategies cover arbitrary `[u8; 32]`, valid v7 byte layouts, timestamps
  across the accepted range and arbitrary `u64` revisions.
- Negative cases cover length minus/at/plus the exact parser bound: ID 35/36/37,
  digest 63/64/65, timestamp through 30 bytes with malformed/extra suffix cases,
  and revision 19/20/21 digits plus numeric overflow. Every text parser also receives
  non-ASCII/multibyte UTF-8 cases at misleading character and byte lengths.
- At least one parallel generation test validates version/variant and duplicate
  absence without claiming strict generation order. Private deterministic seams cover
  pre-epoch, 48-bit timestamp overflow and entropy-source failure without replacing
  the production OS sources or adding global mutable hooks.
- A compile-fail fixture proves `Id<A>` cannot be passed, assigned or converted to
  `Id<B>`; source-text inspection alone is not sufficient marker-separation evidence.
- A canary rejected string resembling a secret/path/URL is absent from public error
  `Display` and `Debug` output.
- Existing `TASK-001` verification remains green; tests are not removed or relaxed.

## 7. Proposed dependency set

Official crates.io metadata was queried on 2026-08-21. All candidates have an MSRV
below the repository's accepted Rust 1.98.0 and use licenses already compatible with
the existing `deny.toml` policy.

| Dependency | Proposed pin/features | Scope | Official metadata evidence |
|---|---|---|---|
| `uuid` | `=1.24.1`, `default-features = false`, `features = ["std"]` | `mengxia-types` runtime | Apache-2.0 OR MIT; MSRV 1.85.0; stable parsing/formatting plus the lower-level v7 `Builder`, without `Uuid::now_v7` or its RNG/context path |
| `getrandom` | `=0.4.3`, `default-features = false`, `features = ["std"]` | `mengxia-types` runtime | MIT OR Apache-2.0; MSRV 1.85.0; direct fallible OS entropy API used by `Id::try_new` |
| `time` | `=0.3.55`, `default-features = false`, `features = ["std", "formatting", "parsing"]` | `mengxia-types` runtime | MIT OR Apache-2.0; MSRV 1.88.0 |
| `proptest` | `=1.11.0`, `default-features = false`, `features = ["std"]` | dev-only property tests | MIT OR Apache-2.0; MSRV 1.85; omits default fork/timeout/bit-set features |

Dependency rules if the proposal is accepted:

- Declare exact pins in `[workspace.dependencies]`, consume them through workspace
  inheritance and commit the updated `Cargo.lock`.
- Review transitive changes with `cargo tree -e features` and the existing
  fail-closed supply-chain command.
- Do not add `hex`, `sha2`, `serde`, `thiserror`, `anyhow`, `rand`, another randomness
  abstraction or a second property-testing framework. The accepted direct
  `getrandom` dependency exists only to make entropy failure typed instead of using
  `uuid`'s infallible generator. Hex/error implementations are small and local;
  hashing and DTO serialization belong to later tasks.
- Any version or feature change from this table reopens the gate before code.

## 8. Security applicability

| Concern | Proposed TASK-002 disposition |
|---|---|
| Untrusted input | Applicable: exact ASCII length/grammar checks precede semantic parsing; non-canonical forms fail closed |
| Authorization / tenant context | Not applicable: no operation or tenant boundary is introduced; V1 remains one local Library |
| Secret or sensitive error leakage | Applicable: errors retain no rejected input and safe output is canary-tested |
| Timeout / retry / idempotency | Not applicable: pure bounded value operations have no external effect; no retry policy is guessed |
| Concurrency | Applicable only to ID generation: direct OS time/entropy plus the stateless UUID builder, no MengXia/shared UUID counter; parallel validity/uniqueness test required |
| Destructive behavior / migration | Not applicable and prohibited by scope |
| Logging/redaction | No logger is introduced; error values themselves must be safe to display/debug |
| Supply chain | Applicable through exact pins, minimal features, lockfile, licenses and fresh advisory evidence |

Traceability proposed for the start record: `FUNC-001`; `REQ-001`; `API-010` (only
the explicit no-operation/no-side-effect applicability record); `DATA-012` (bounded
untrusted textual values and no raw payload); `SEC-017`; `SEC-020`; `BASE-011`;
`BASE-013`; `BASE-014`; `ADR-0003`; `ADR-0005`.

## 9. Copy-ready canonical task-start record

The following record is a proposal. It must be copied into the canonical Plan only
after the public-contract and dependency choices above are accepted.

```markdown
### TASK-002 start record — 2026-08-21

- Scope: `TASK-002` only; typed UUIDv7 IDs, SHA-256 digest value, UTC timestamp,
  RevisionNo and the minimal safe typed error baseline. No TASK-003 or later behavior
  is authorized.
- Feature/Requirements: `FUNC-001`; `REQ-001`; `API-010`; `DATA-012`; `SEC-017`;
  `SEC-020`.
- Decisions/gates read: `BASE-011`; `BASE-013`; `BASE-014`; `ADR-0003`; `ADR-0005`;
  completed `TASK-001` evidence.
- Acceptance obligations: `AC-055`; `AC-056`; `AC-057`; `AC-058`; `AC-059`.
- Verification obligations: `TEST-TYPE-001`; `TEST-PARSE-001`; `TEST-TIME-001`;
  `TEST-ERROR-001`; `TEST-ARCH-002`; `TEST-SUPPLY-002`; `TEST-DOC-002`.
- Planned file scope: root/workspace Cargo dependency declarations and lockfile;
  `crates/mengxia-types`; the minimal error module in `crates/mengxia-domain`;
  TASK-002-only tests/verification wiring; synchronized canonical/current-state
  lifecycle records listed in this proposal §10. No proto, schema, migration,
  persistence, storage, Provider, Plugin, IPC or binary behavior.
- Public contract: exact forms and boundaries are those accepted from
  `docs/proposals/TASK-002-GATE-PROPOSAL.md` and incorporated into the canonical
  Specification before this record becomes active.
- Security answers: all textual values are untrusted, exactly bounded, strictly
  parsed and canonicalized; error values retain no raw rejected input. This task has
  no authenticated operation, authorization decision, tenant, secret store,
  persistence transaction, network/process/file side effect, retry, idempotent
  command, migration or destructive behavior. ID generation is concurrency-tested
  and uses direct fallible OS time/entropy with the stateless UUID builder; it uses no
  MengXia-owned or dependency-owned shared counter/generator.
- Dependency answer: exact `uuid`, `getrandom`, `time` and dev-only `proptest`
  pins/features are accepted and must pass the existing fail-closed
  lock/license/advisory policy.
- Completion evidence required: every listed TEST ID maps to a deterministic command
  and passes; every listed AC and applicable security requirement has evidence;
  TASK-001 baseline remains green; no new regression, scope expansion, public API
  drift or unresolved applicable blocker remains.
```

## 10. Canonical revision disposition

Acceptance authorized only the following documentation revision before code; all
items were completed before the canonical TASK-002 start record became active:

1. After explicit user acceptance, change this proposal header to `ACCEPTED /
   INCORPORATED`, record the canonical revision evidence, and retain the file
   only as provenance; it never becomes a competing canonical definition site.
2. Add a `REVIEW-GAP-003` record to `docs/spec/DECISIONS.md`, classified
   `EXPECTED_GAP`, recording that the previously absent TASK-002 public contract and
   stable registry were accepted through this gate. This is not an ADR and does not
   alter an existing `CONFIRMED` architecture decision.
3. Add the exact public signatures/error shapes above, the five new §14.1 error rows,
   `AC-055` through `AC-059` and the seven proposed Test definitions to
   `docs/spec/IMPLEMENTATION_SPEC.md`. Replace its broad `TASK-002` prose with
   references to those contracts, exact dependency candidates and exclusions.
4. Update `docs/spec/IMPLEMENTATION_PLAN.md`: replace the TASK-002 row's prose-only
   acceptance column, add the task-start record above and change TASK-002 from
   `PENDING` to `IN_PROGRESS` only in the same candidate revision that completes all
   remaining synchronization steps below.
5. Update `docs/spec/IMPLEMENTATION_REVIEW.md` so its readiness narrative, finding
   disposition and TASK-002 simulation state say that the start gate is satisfied and
   only TASK-002 is authorized. The whole-V1 verdict remains `NOT READY FOR CODEX` and
   every later gate remains unchanged.
6. Update root `AGENTS.md` and the `PROJECT_INTAKE_REPORT.md` first-safe-next-action
   text so neither still says the TASK-002 gate is missing. Preserve the repository
   fact that TASK-002 behavior is absent until implementation actually lands; do not
   pre-claim product behavior or completion.
7. Run the existing generic `TEST-DOC-001` against the synchronized candidate
   documents; it must accept the new canonical definitions/start record and continue
   to reject unknown, duplicate, malformed-range and lifecycle-invalid fixtures. The
   dedicated `TEST-DOC-002` command/output and its stale-current-state negative fixture
   are created during TASK-002, as allowed by Specification §0.5, and must pass before
   `DONE`; do not create them as a circular pre-start requirement or weaken
   `TEST-DOC-001`.
8. Re-run the complete `TASK-001` baseline after the canonical-only revision. A failed
   gate leaves TASK-002 unauthorized and must be corrected before production-code or
   manifest changes begin.
9. Pause again if any incorporated wording differs materially from this accepted
   proposal; classify the difference as `EXPECTED_GAP`, `SPEC_STALE`, `REPO_STALE`,
   `CONFLICT` or `UNKNOWN` instead of silently resolving it in code.

## 11. Approval disposition

The user accepted the modified bundle on 2026-08-21. The canonical documents then
incorporated the contract and created the TASK-002 start record; the later completion
record is the authority for implementation evidence. No approval remains pending for
TASK-002, and this disposition does not authorize `TASK-003` or any later task.
