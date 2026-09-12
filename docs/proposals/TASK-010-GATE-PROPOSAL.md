---
title: "TASK-010 Plugin package foundation start-gate proposal"
project: "梦夏 / MengXia"
document_role: "Accepted TASK-010 implementation supplement"
status: "ACCEPTED_INCORPORATED_BY_CANONICAL_SPECIFICATION_1_1_44"
version: "0.2.3"
date: "2026-09-11"
canonical_specification_reviewed: "IMPLEMENTATION_SPEC.md v1.1.43"
repository_head_reviewed: "1505e62941bb967771856421316054c4c067d3b5"
---

# TASK-010 Gate Proposal

## 0. Gate verdict

TASK-010 is authorized only as the non-executing, non-persistent package
foundation defined here. Independent review reproduced the final Cargo graph,
lock hash, offline build and complete cargo-deny result without finding an
unresolved implementation blocker.

Version 0.2.3 retains the v0.2.2 supply contract and resolves the remaining
canonical-authority, downstream ownership and numeric-classification conflicts
recorded by `REVIEW-CONFLICT-052`. Acceptance and stable-test obligation prose now
has one canonical owner, every remaining `API-001` schema sub-scope and cross-task
security criterion has an explicit terminal owner, and valid but non-V1 JSON
numbers have one deterministic result.
The canonical Specification v1.1.44 and Plan v0.3.55 start record grant exact
STEP-1 authority; nothing in this supplement authorizes TASK-011 or a privileged
Plugin effect.

```text
TASK010_CANONICAL_GATE: ACCEPTED
TASK010_LIFECYCLE: IN_PROGRESS
TASK010_IMPLEMENTATION_AUTHORITY: TASK_010_FOUNDATION_ONLY
TASK010_PROPOSAL_VERSION: 0.2.3
TASK010_REPOSITORY_HEAD_REVIEWED: 1505e62941bb967771856421316054c4c067d3b5
TASK010_INDEPENDENT_REVIEW: PASS_2026_09_11
TASK010_UNRESOLVED_DESIGN_BLOCKERS: NONE
TASK010_DEPENDENCY_PREFLIGHT: PASS_ISOLATED_FINAL_MANIFEST_GRAPH
TASK010_REMAINING_GATES: NONE_BEFORE_STEP_1
```

No migration, package installation, durable grant/revocation, Admin operation,
process execution, sandbox behavior, daemon/CLI/proto surface or TASK-011 behavior
is authorized by this document.

## 1. Repository and authority baseline

- Committed `main` and `origin/main` are both
  `1505e62941bb967771856421316054c4c067d3b5`.
- TASK-001 through TASK-009 and MAINT-001 are complete with authority `NONE`.
- The current package/security/host/sandbox crates are placeholders.
- Migrations 0000, 0001 and 0002 are immutable completed-task inputs.
- The current docs gate and complete developer repository gate pass.
- `OQ-010` remains open and every Admin-sensitive operation remains disabled.

The missing TASK-010 implementation is an `EXPECTED_GAP`; completed-task code is
not stale and is not modified by this proposal.

## 2. Proposed resolution of the review findings

### 2.1 Task ownership

TASK-010 owns only:

1. exact manifest bytes and `PackageDigest`;
2. bounded duplicate-aware JSON parsing and embedded JSON Schema validation;
3. closed typed manifest values and canonical serialization;
4. immutable `RuntimeDependencyDeclaration` values;
5. pure, total `PermissionDiff` evidence;
6. negative/property/supply/architecture tests for those pure contracts.

TASK-010 does not own installation, persistent lifecycle state, package or grant
repositories, revocation enforcement, Admin authentication, executable custody,
sandboxing or process launch.

The exact requirement attribution is:

| ID | TASK-010 contribution | Completion ownership |
|---|---|---|
| `FUNC-006` | package-foundation contributor | remains incomplete until TASK-013 composition |
| `API-001` | completes only the V1 Manifest JSON Schema sub-scope | TASK-011 owns Plugin transport proto3; TASK-014 owns Capability JSON Schema; TASK-015 owns Recipe JSON Schema; TASK-020 owns namespaced extension JSON Schema; TASK-023 aggregates whole-requirement release evidence. Core transport proto3 is already owned by completed TASK-003/TASK-007/TASK-008/TASK-009. |
| `SEC-003` | proves requested permissions are typed evidence, never a grant | TASK-013 terminal enforcement owner |
| `SEC-010` | computes deterministic update-expansion evidence | TASK-013 terminal authorization owner |
| `SEC-016` | keeps publisher text untrusted and binds package bytes to a digest | TASK-013 terminal trust/grant owner |
| `SEC-017` | completes bounded parsing for this Manifest input boundary only | global requirement remains active at every later input boundary |
| `SEC-020` | completes the exact TASK-010 dependency graph review only | global supply review remains continuous and task-scoped |

TASK-010 completion must not mark the whole of `FUNC-006`, `API-001`, `SEC-003`,
`SEC-010`, `SEC-016`, `SEC-017` or `SEC-020` complete beyond the sub-scope stated
above.

TASK-011 may depend on a completed TASK-010 foundation without waiting for
`OQ-010`. TASK-012 owns the platform proof for managed executable custody plus
descriptor-to-launched-image binding and sandbox enforcement. TASK-013 owns the
authenticated install/grant/revoke composition, migrations 0003/0004 and AC-027's
durable `PENDING_APPROVAL` result. TASK-012 terminally owns AC-021/AC-022 and only
contributes OS-sandbox evidence to AC-020/AC-023; TASK-013 terminally owns AC-020
and contributes Asset Broker/Lease evidence to AC-023; TASK-016 terminally owns
the fully composed AC-023 result.

### 2.2 Package and executable identity

The V1 package is exactly one canonical UTF-8 JSON manifest. `PackageDigest` is
SHA-256 over every manifest byte. Executable and tool bytes are not embedded and
are not identified by a durable host path.

Each runtime dependency has a logical `dependency_id`, target, role, byte length
and SHA-256 digest. A later authenticated install request supplies an out-of-band
source handle for each ID. TASK-013 must copy from a descriptor-verified source
into a managed content-addressed Plugin executable store. TASK-012 must expose the
only platform launch primitive, and it consumes only the
corresponding managed object. TASK-013 composition may invoke that primitive only
after current digest, revocation, grant and sandbox checks.

This avoids treating a mutable absolute pathname as persistent authority. It also
avoids the false claim that macOS can execute an already-inspected descriptor:
the reviewed macOS 26.5 SDK exposes pathname-based `execve`/`posix_spawn` and no
`fexecve` or `execveat`. Before TASK-012 starts, a real arm64 macOS prototype must
prove the exact managed-object-to-process binding; otherwise third-party Native
activation remains unavailable.

### 2.3 Gate-A runtime boundary

TASK-010 library APIs accept bounded manifest bytes. They do not own Tokio,
threads, blocking filesystem calls, deadlines, cancellation, global admission or
Core metrics. Those are application/host concerns when external sources are later
acquired.

This makes TASK-010 deterministic and testable without inventing an interruptible
`BoundedPackageSource`. It also removes the need to modify `mengxia-platform-fs`
under this task.

### 2.4 PermissionDiff and grants

`PermissionDiff` is evidence only. It never creates or inherits a grant and never
changes lifecycle state. Its result is:

```text
UNCHANGED
CONTRACTION
EXPANSION
INCOMPARABLE_DENY
```

Total precedence is:

```text
INCOMPARABLE_DENY > EXPANSION > CONTRACTION > UNCHANGED
```

Every grant remains bound to one exact `PackageDigest`. TASK-013 must define the
complete transition table for a different digest:

- incomparable input is denied;
- expansion requires authenticated approval and becomes `PENDING_APPROVAL`;
- unchanged or contraction does not reuse the old row; TASK-013 may either require
  approval or create a new, separately persisted derived grant with explicit
  provenance and current-revocation checks.

TASK-010 returns the diff only and therefore cannot silently grant authority.

### 2.5 Error ownership

TASK-010 has an internal closed `PluginPackageError`; no public product
`ErrorCode` mapping is added in this task. The internal classes are exactly:

```text
INPUT_TOO_LARGE
RESOURCE_LIMIT_EXCEEDED
MALFORMED_JSON
DUPLICATE_KEY
MANIFEST_INVALID
NONCANONICAL_BYTES
INTERNAL_INVARIANT
```

`INPUT_TOO_LARGE` is reserved for raw input longer than 65,536 bytes.
`RESOURCE_LIMIT_EXCEEDED` covers decoded depth/node/string/count and checked-sum
limits. Empty input, BOM, invalid UTF-8, lexically invalid JSON number tokens such
as `+1`, `01`, `.1`, `NaN`, `Infinity` or a malformed exponent, all other invalid
JSON tokens, and trailing data are `MALFORMED_JSON`; a repeated key is always
`DUPLICATE_KEY` once its second key token is encountered. Every lexically valid JSON
number that is not a V1 shortest unsigned decimal fitting `u64`--including `-1`,
`-0`, `1.0`, `1e0`, `1e999` and an integer greater than `u64::MAX`--is
`MANIFEST_INVALID`. `MANIFEST_INVALID` also covers every successfully parsed
instance that fails the embedded schema, a field contract,
SemVer/capability/permission syntax, or a semantic/cross-field rule. The bounded
parser must classify number grammar and V1 range/form without relying on a generic
number-conversion error string or dependency-specific conversion order. The
implementation must not expose validation
library keyword/order as an error distinction. Surrounding JSON whitespace is
valid JSON but noncanonical and therefore becomes `NONCANONICAL_BYTES`; a second
root value or non-whitespace after the root is malformed trailing data and becomes
`MALFORMED_JSON`. Invalid embedded schema bytes or impossible typed/canonical state
are `INTERNAL_INVARIANT`. `diff_permissions` is infallible and expresses an
identity/comparator mismatch through `INCOMPARABLE_DENY`, not this error enum.

The error retains no rejected bytes or user-controlled string. Later product
composition maps malformed input to `VALIDATION_ERROR`, unavailable capability
resolution to `UNSUPPORTED_CAPABILITY`, filesystem I/O to `STORAGE_IO_ERROR`,
source mutation to a dedicated later-task class, policy denial to
`AUTHORIZATION_DENIED`, and timeout/cancellation to their existing exact codes.
TASK-010 must not collapse those later conditions into one validation error.

### 2.6 Crate direction

Dependency direction is frozen:

```text
mengxia-plugin-package  -> mengxia-types
mengxia-plugin-security -> mengxia-plugin-package, mengxia-types
```

`mengxia-plugin-package` owns the neutral typed manifest, dependency declaration
and permission request values. `mengxia-plugin-security` owns the pure diff.
Neither crate depends on app, ports, Tokio, platform-fs, store, host, sandbox,
plugin proto, a Provider SDK or the other direction.

## 3. Exact V1 manifest contract

### 3.1 Package bytes

The package is one canonical UTF-8 JSON document with:

- no BOM, leading/trailing whitespace, comments or trailing bytes;
- no duplicate object keys;
- no archive, directory, symlink, embedded executable or external schema reference;
- a maximum of 65,536 bytes;
- exact equality with the canonical serializer output.

Object keys are ordered by raw UTF-8 bytes. Arrays use the field-specific ordering
below. No optional whitespace is emitted. JSON strings emit every non-control
Unicode scalar directly as UTF-8 except `"` and `\\`. Those two characters use
`\\"` and `\\\\`; U+0008/U+0009/U+000A/U+000C/U+000D use
`\\b`/`\\t`/`\\n`/`\\f`/`\\r`; every other U+0000..U+001F scalar uses one
lowercase `\\u00xx`; `/` and all other scalars are not escaped. V1 permits
unsigned integer values only, encoded as shortest decimal. Floating-point and
arbitrary-precision numbers are rejected.

### 3.2 Canonical example shape

```json
{"$schema":"https://schemas.mengxia.local/plugin/manifest-v1.schema.json","capabilities":["media.video.image_to_video@1"],"manifest_version":1,"plugin_id":"example.transcoder","publisher":"example-untrusted-label","requested_permissions":[{"kind":"broker.asset.read@1","scope":"run-inputs"}],"runtime_dependencies":[{"byte_length":1234,"dependency_id":"entrypoint","role":"PLUGIN_ENTRYPOINT","sha256":"1111111111111111111111111111111111111111111111111111111111111111","target":"aarch64-apple-darwin"}],"version":"1.0.0"}
```

The digest is a canonical declaration fixture, not evidence that dependency bytes
have been acquired. The accepted schema and golden fixture become byte-authoritative.

### 3.3 Field contracts

- `$schema` is the exact identifier above and is never fetched.
- `manifest_version` is exactly integer `1`.
- `publisher` is 1..128 ASCII bytes from `[A-Za-z0-9._-]`; it is untrusted display
  metadata and never selects policy.
- `plugin_id` is 1..128 lowercase ASCII bytes, starts with `[a-z]`, and contains
  only `[a-z0-9._-]`.
- `version` is 1..64 ASCII bytes accepted by `semver::Version::parse`; build
  metadata is forbidden and `Version::to_string()` must equal the input. Prerelease
  identifiers are permitted only when that exact round-trip succeeds.
- a capability ID is 5..128 lowercase ASCII bytes in
  `<dot-separated-name>@<positive-u32>` form. The name has 2..8 non-empty segments;
  each segment is 1..32 bytes, starts with `[a-z]` and continues with
  `[a-z0-9_]*`. The version is shortest unsigned decimal in 1..=u32::MAX. IDs are
  sorted/unique by ASCII bytes.
  Parsing a syntactically valid ID grants nothing; later capability resolution
  rejects an ID absent from the installed provider registry.
- V1 accepts one permission kind: `broker.asset.read@1`, whose only scope is
  `run-inputs`. The list may be empty, is sorted/unique by the complete canonical
  object, and adding any entry is expansion. New kinds require a manifest-schema
  version or an accepted additive registry decision with a total comparator.
- `dependency_id` is 1..64 lowercase ASCII bytes, begins with `[a-z]`, and uses
  `[a-z0-9_-]`. Runtime dependencies are sorted/unique by `dependency_id`.
- `role` is `PLUGIN_ENTRYPOINT` or `TOOL`; exactly one entrypoint exists.
- `target` is exactly `aarch64-apple-darwin` in V1 foundation.
- `byte_length` is 1..1,073,741,824 and aggregate declared bytes are at most
  4,294,967,296.
- `sha256` is 64 lowercase hexadecimal characters and is not all zero.
- every object rejects unknown properties independently in schema and typed parsing.

## 4. Parser and JSON Schema contract

The checked-in schema uses JSON Schema Draft 2020-12, has an exact `$id` equal to
`https://schemas.mengxia.local/plugin/manifest-v1.schema.json`, sets
`additionalProperties: false` at every object, and is embedded in production.
Remote/file resolution, custom side-effecting formats and runtime schema loading
are forbidden. Production compilation must explicitly select `Draft202012`, call
the validator builder's `offline()` mode and call
`should_validate_formats(false)`; it must not rely only on Cargo feature selection
or a dependency default. An external `$ref` negative fixture must fail without
filesystem or network activity even if a future dependency edge accidentally
enables a resolver feature.

Validation order is exact:

1. inspect byte length without allocation: length above 65,536 is
   `INPUT_TOO_LARGE`; empty input, BOM or invalid UTF-8 is `MALFORMED_JSON`;
2. bounded duplicate-aware parse; the first encountered lexical/structural event
   determines `MALFORMED_JSON`, `DUPLICATE_KEY`, `RESOURCE_LIMIT_EXCEEDED` or the
   exact valid-JSON-but-non-V1-number `MANIFEST_INVALID` result from §2.5;
3. validate against the embedded schema; every instance failure is
   `MANIFEST_INVALID`, independent of validator error iteration order;
4. convert to closed typed values and enforce semantic/cross-field rules; every
   rejection at this stage is also `MANIFEST_INVALID`;
5. canonical serialize and require exact byte equality;
6. hash the exact accepted bytes;
7. return an immutable `InspectedPluginPackage`.

The implementation must not use `serde_json::Value` deserialization as its only
duplicate-key detector. The accepted malformed corpus independently covers every
boundary, escape, duplicate, integer and unknown-field case.

The reviewed dependency candidate is:

```toml
jsonschema = { version = "=0.56.0", default-features = false }
semver = { version = "=1.0.28", default-features = false, features = ["std"] }
```

The isolated preflight and future STEP-1 use exactly these manifest edges, with no
additional direct, dev, build or feature-forwarding edge:

```toml
# root Cargo.toml [workspace.dependencies], added entries only
jsonschema = { version = "=0.56.0", default-features = false }
semver = { version = "=1.0.28", default-features = false, features = ["std"] }

# crates/mengxia-plugin-package/Cargo.toml
[dependencies]
jsonschema.workspace = true
mengxia-types.workspace = true
semver.workspace = true
serde.workspace = true
serde_json.workspace = true
sha2.workspace = true

# crates/mengxia-plugin-security/Cargo.toml
[dependencies]
mengxia-plugin-package = { path = "../mengxia-plugin-package", version = "=0.1.0" }
mengxia-types.workspace = true

# crates/mengxia-testkit/Cargo.toml, added dev dependencies only
[dev-dependencies]
mengxia-plugin-package = { path = "../mengxia-plugin-package", version = "=0.1.0" }
mengxia-plugin-security = { path = "../mengxia-plugin-security", version = "=0.1.0" }
```

Property/boundary coverage uses deterministic generated loops and fixtures inside
the listed source/test files; TASK-010 adds no `proptest`, fuzzing or other test
dependency edge.

`jsonschema` default features include HTTP/file resolution and therefore must
remain disabled. The corrected 2026-09-11 preflight copied the current workspace
and lock, applied every exact production/test edge above, and selected 40 new
registry packages with no removal or existing-version change.
Rust 1.98 workspace/all-target compilation passed both online and subsequently
offline. The direct crate checksums are
`jsonschema=b6a806f80c1f5560431009ce5ec29b59d38f950e0cd7db5f1b8925e6d8104e21`
and `semver=8a7852d02fc848982e0c167ef163aaff9cd91dc640ba85e263cb1ce46fae51cd`;
the final-manifest candidate lock SHA-256 is
`302df8141acee77aa58ecb796a53ecbb4faf9f6cd55dc384667bb08e725c0b2e`.

The unmodified repository `deny.toml` correctly rejected the candidate for one
new MIT-0 license, exact transitive default features and the unavoidable retained
`foldhash`/`getrandom`/`r-efi` version pairs. A second isolated run passed all
advisory, bans, license and source checks only after adding the exact per-crate and
feature/version exceptions described in §4.1. This preflight is review evidence,
not a repository lock or implementation change. The formal start record may
authorize applying exactly that dependency-policy delta in STEP-1. Any selected
package/version/feature/checksum deviation, any resolver/HTTP/TLS/IDNA feature, any
advisory ignore, or any broader policy relaxation stops before manifest
implementation and returns to review. A partial hand-written validator is not
silently substituted for JSON Schema 2020-12.

### 4.1 Exact preflight `deny.toml` delta

STEP-1 may add only these policy records, and the real lock must retain the exact
versions below:

- keep `MIT-0` out of the global `licenses.allow` list and add only
  `licenses.exceptions = [{ allow = ["MIT-0"], crate =
  "borrow-or-share@0.2.4" }]` for that exact crate/version;
- add documented duplicate skips for `foldhash 0.1.5`, `getrandom 0.3.4` and
  `r-efi 5.3.0`; their coexisting versions are respectively `0.2.0`, `0.4.3` and
  `6.0.0`;
- extend the existing exact `syn 2.0.119` feature record only with `full`, `visit`
  and `visit-mut`;
- add the following exact feature records (comma-separated values are the complete
  enabled set, not a partial allow-list):

| Crate | Exact enabled features |
|---|---|
| `ahash 0.8.12` | `default,getrandom,runtime-rng,serde,std` |
| `bumpalo 3.20.3` | `default` |
| `data-encoding 2.11.1` | `alloc,default,std` |
| `email_address 0.2.9` | `default,serde,serde_support` |
| `fancy-regex 0.19.1` | `default,perf,std,unicode,variable-lookbehinds` |
| `fluent-uri 0.4.1` | `alloc,default,impl-error,serde,std` |
| `hashbrown 0.17.1` | `allocator-api2,default,default-hasher,equivalent,inline-more,raw-entry` |
| `jsonschema-value 0.56.0` | `default,serde_json` |
| `lock_api 0.4.14` | `atomic_usize,default` |
| `micromap 0.3.0` | `default` |
| `num-bigint 0.4.8` | `default,std` |
| `num-traits 0.2.19` | `default,i128,std` |
| `parking_lot 0.12.5` | `default` |
| `percent-encoding 2.3.2` | `alloc,default,std` |
| `redox_syscall 0.5.18` | `default,userspace` |
| `referencing 0.56.0` | `default` |
| `regex 1.13.1` | `default,perf,perf-backtrack,perf-cache,perf-dfa,perf-inline,perf-literal,perf-onepass,std,unicode,unicode-age,unicode-bool,unicode-case,unicode-gencat,unicode-perl,unicode-script,unicode-segment` |
| `serde 1.0.229` | `alloc,default,derive,serde_derive,std` |
| `serde_derive 1.0.229` | `default` |
| `serde_json 1.0.150` | `default,float_roundtrip,std` |
| `strum 0.28.0` | `default,derive,std,strum_macros` |

No other license, skip or feature entry is part of the reviewed delta. Target-only
entries remain audited because the repository supply graph is intentionally broader
than the initial arm64 macOS runtime target.

### 4.2 Exact added registry inventory

The final-manifest preflight adds exactly these 40 registry packages to the current
117-package graph. Name, version and registry checksum are normative STEP-1 input.
For each row in listed order, normalize as
`<package-name-and-version>\t<lowercase-checksum>\n`; the SHA-256 of the resulting
UTF-8 bytes is exactly
`0d66568ef018ff7d68083bb8570a861bf24d40861a85d8965bda1a00c3283c7b`:

| Package | crates.io checksum |
|---|---|
| `ahash 0.8.12` | `5a15f179cd60c4584b8a8c596927aadc462e27f2ca70c04e0071964a73ba7a75` |
| `allocator-api2 0.2.21` | `683d7910e743518b0e34f1186f92494becacb047c7b6bf616c96772180fef923` |
| `bit-set 0.8.0` | `08807e080ed7f9d5433fa9b275196cfc35414f66a0c79d864dc51a0d825231a3` |
| `bit-vec 0.8.0` | `5e764a1d40d510daf35e07be9eb06e75770908c27d411ee6c92109c9840eaaf7` |
| `borrow-or-share 0.2.4` | `dc0b364ead1874514c8c2855ab558056ebfeb775653e7ae45ff72f28f8f3166c` |
| `bytecount 0.6.9` | `175812e0be2bccb6abe50bb8d566126198344f707e304f45c648fd8f2cc0365e` |
| `data-encoding 2.11.1` | `4583a4551df46e2792f82ceeac45e850d2e2d5debba0b91f102385cda5b11f06` |
| `email_address 0.2.9` | `e079f19b08ca6239f47f8ba8509c11cf3ea30095831f7fed61441475edd8c449` |
| `fancy-regex 0.19.1` | `52e0387578e845beb7a1acff126228499f26cb18edf12919cc513bb863266464` |
| `fluent-uri 0.4.1` | `bc74ac4d8359ae70623506d512209619e5cf8f347124910440dbc221714b328e` |
| `foldhash 0.2.0` | `77ce24cb58228fbb8aa041425bb1050850ac19177686ea6e0f41a70416f56fdb` |
| `fraction 0.17.0` | `e246562084dde8ebbcc943b261c406ce4f68e5032ec28029a251a47d6a295500` |
| `jsonschema 0.56.0` | `b6a806f80c1f5560431009ce5ec29b59d38f950e0cd7db5f1b8925e6d8104e21` |
| `jsonschema-regex 0.56.0` | `cb862addfa7782108933abcf842fe25334523a9df5b89ea4d9620e3dd7b42181` |
| `jsonschema-value 0.56.0` | `a05cd404c5ff6e2731dbbf7e290a27417750acfb59e329560a1d0af384c93fb0` |
| `lock_api 0.4.14` | `224399e74b87b5f3557511d98dff8b14089b3dadafcab6bb93eab67d3aace965` |
| `micromap 0.3.0` | `c2a86d3146ed3995b5913c414f6664344b9617457320782e64f0bb44afd49d74` |
| `num 0.4.3` | `35bd024e8b2ff75562e5f34e7f4905839deb4b22955ef5e73d2fea1b9813cb23` |
| `num-bigint 0.4.8` | `c89e69e7e0f03bea5ef08013795c25018e101932225a656383bd384495ecc367` |
| `num-cmp 0.1.0` | `63335b2e2c34fae2fb0aa2cecfd9f0832a1e24b3b32ecec612c3426d46dc8aaa` |
| `num-complex 0.4.6` | `73f88a1307638156682bada9d7604135552957b7818057dcef22705b4d509495` |
| `num-integer 0.1.47` | `7ce2d95d4b3734dc35aa2f45e1aa22cd416814592a4f9d9205e11affd5b8e10b` |
| `num-iter 0.1.46` | `c92800bd69a1eac91786bcfe9da64a897eb72911b8dc3095decbd07429e8048b` |
| `num-rational 0.4.2` | `f83d14da390562dca69fc84082e73e548e1ad308d24accdedd2720017cb37824` |
| `outref 0.5.2` | `1a80800c0488c3a21695ea981a54918fbb37abf04f4d0720c453632255e2ff0e` |
| `parking_lot 0.12.5` | `93857453250e3077bd71ff98b6a65ea6621a19bb0f559a85248955ac12c45a1a` |
| `parking_lot_core 0.9.12` | `2621685985a2ebf1c516881c026032ac7deafcda1a2c9b7850dc81e3dfcb64c1` |
| `percent-encoding 2.3.2` | `9b4f627cb1b25917193a259e49bdad08f671f8d9708acfd5fe0a8c1455d87220` |
| `redox_syscall 0.5.18` | `ed2bf2547551a7053d6fdfafda3f938979645c44812fbfcda098faae3f1a362d` |
| `ref-cast 1.0.27` | `7e440fb4e4b4147295338efb76001ab9e4efc0e5839df2c47fc5ac2381d365c3` |
| `ref-cast-impl 1.0.27` | `92ecd8964f8453721699a1ed72037b0db49ce2f5a5138486ee89bed6f67cdf3a` |
| `referencing 0.56.0` | `a3b4a92fac7e28c27de3ad26df2ecb9e652f227b258e845af034052e5b22c96c` |
| `scopeguard 1.2.0` | `94143f37725109f92c262ed2cf5e59bce7498c01bcc1502d7b9afe439a4e9f49` |
| `semver 1.0.28` | `8a7852d02fc848982e0c167ef163aaff9cd91dc640ba85e263cb1ce46fae51cd` |
| `strum 0.28.0` | `9628de9b8791db39ceda2b119bbe13134770b56c138ec1d3af810d045c04f9bd` |
| `strum_macros 0.28.0` | `ab85eea0270ee17587ed4156089e10b9e6880ee688791d45a905f5b1ca36f664` |
| `unicode-general-category 1.1.0` | `0b993bddc193ae5bd0d623b49ec06ac3e9312875fdae725a975c51db1cc1677f` |
| `uuid-simd 0.8.0` | `23b082222b4f6619906941c17eb2297fff4c2fb96cb60164170522942a200bd8` |
| `version_check 0.9.5` | `0b928f33d975fc6ad9f86c8f283853ad26bdd5b10b7f1542aa2fa15e2289105a` |
| `vsimd 0.8.0` | `5c3082ca00d5a5ef149bb8b555a72ae84c9c59f7250f013ac822ac2e49b19c64` |

## 5. RuntimeDependencyDeclaration

The manifest declaration contains no pathname, URI, command, arguments,
environment or shell fragment:

```text
dependency_id
role: PLUGIN_ENTRYPOINT | TOOL
target: aarch64-apple-darwin
byte_length
sha256
```

The declaration is immutable identity metadata. TASK-010 proves only that it is
canonical, bounded and included in `PackageDigest`; it does not assert the named
bytes are installed or executable.

Later source acquisition must be explicit and descriptor-first, with no ambient
`PATH` or shell. It must prove source stability, import into managed custody without
replacement, rehash the managed object, and persist the binding atomically with the
authenticated install/grant decision. Raw source paths are not durable package
fields and never appear in a Plugin-visible API.

## 6. PermissionDiff contract

Diffing requires the same exact `publisher` and `plugin_id`; a different pair is
not an update and deterministically returns `INCOMPARABLE_DENY` with reason
`PACKAGE_IDENTITY_MISMATCH` and an empty change list. Publisher equality is
grouping evidence only, not authentication.

- adding a permission, capability or dependency is `EXPANSION`;
- removing one is `CONTRACTION` unless another change expands authority;
- changing a dependency role, target, digest or length is `EXPANSION`;
- changing the entrypoint is `EXPANSION`;
- an unknown/invalid V1 permission kind, scope or value is rejected by inspection
  and therefore can never enter this typed diff;
- a future accepted manifest type without an accepted total cross-version
  comparator returns `INCOMPARABLE_DENY` with reason
  `COMPARATOR_UNAVAILABLE` rather than guessing;
- changing only the manifest `version` is `UNCHANGED` permission evidence; a
  version change combined with authority changes never reduces their classification;
- mixed changes use the total precedence in §2.4.

The result contains a classification, a closed incomparable reason (`NONE`,
`PACKAGE_IDENTITY_MISMATCH`, `COMPARATOR_UNAVAILABLE`) and bounded typed changes,
but no grant or lifecycle transition. A change has namespace `PERMISSION`,
`CAPABILITY` or `RUNTIME_DEPENDENCY`, disposition `ADDED`, `REMOVED` or `CHANGED`,
and its canonical identifier. RuntimeDependency fields changing under the same
`dependency_id` produce one `CHANGED` entry, not an added/removed pair.

Changes are unique and sorted by namespace in the order above, then identifier
ASCII bytes, then disposition `ADDED < REMOVED < CHANGED`. For two valid V1
packages the maximum is 193 entries: one permission union entry, 128 disjoint
capability IDs and 64 disjoint dependency IDs. Identity/comparator incomparable
results contain no changes. `UNCHANGED`, `CONTRACTION` and `EXPANSION` always use
reason `NONE`.

## 7. Public API boundary

Candidate surface:

```rust
pub fn inspect_manifest(bytes: &[u8])
    -> Result<InspectedPluginPackage, PluginPackageError>;

pub fn diff_permissions(
    old: &InspectedPluginPackage,
    candidate: &InspectedPluginPackage,
) -> PermissionDiff;
```

`PermissionDiff`, `PermissionChange`, their classification/reason/namespace/
disposition enums and their slices are immutable closed values. The function does
not return `Result`: all pairs of inspected packages, including identity mismatch,
have the exact result defined in §6.

Returned fields are accessed through bounded typed accessors. Raw JSON trees,
mutable vectors, raw dependency paths, process/file/SQLite handles and grant
constructors do not escape.

No proto tag, operation ID, CLI subcommand, daemon handler, socket or product API
is added by TASK-010.

## 8. Boundedness

| Resource | Hard ceiling |
|---|---:|
| manifest bytes | 65,536 |
| JSON depth | 16 |
| JSON nodes | 4,096 |
| one string | 4,096 bytes |
| capabilities | 64 |
| requested permissions | 1 |
| RuntimeDependency declarations | 32 |
| one declared dependency | 1 GiB |
| aggregate declared dependency bytes | 4 GiB |
| diff entries | 193 |

The root JSON value has depth 1. Entering an object or array increments depth by
one; scalar children do not add nesting depth. Every object, array and scalar value
counts as one node; object member names do not count as nodes. The string ceiling
applies to decoded UTF-8 bytes of every member name and string value, before a value
is copied into a typed field. Raw escape spelling does not change that decoded
limit. Byte-length aggregation uses checked `u64` addition before collection
allocation; any overflow or cap breach is `RESOURCE_LIMIT_EXCEEDED`.

The raw byte cap is checked before any input-proportional allocation. During parse,
node/string/list counts are checked before the next retained value is pushed or a
larger collection is reserved. Declared byte lengths are accumulated with checked
`u64` arithmetic before constructing the typed dependency collection, and no memory
allocation is sized from a declared dependency byte length. Since the API is
synchronous in-memory computation, TASK-010 defines no runtime concurrency, worker,
queue, deadline or cancellation configuration.

## 9. Acceptance and stable tests

`IMPLEMENTATION_SPEC.md` §19.11 is the sole normative source for TASK-010
acceptance prose. `IMPLEMENTATION_SPEC.md` §20.0.9 is the sole normative source
for TASK-010 stable-test obligations and required evidence. This proposal records
only the exact ownership/cross-reference IDs so two copies cannot drift.

```text
TASK010_ACCEPTANCE_IDS: AC-098,AC-099,AC-100
TASK010_TEST_IDS: TEST-MANIFEST-010,TEST-PACKAGE-010,TEST-DEPENDENCY-010,TEST-DIFF-010,TEST-PUBLISHER-010,TEST-BOUNDS-010,TEST-ARCH-010,TEST-SUPPLY-010,TEST-DOC-010
TASK010_ACCEPTANCE_AUTHORITY: IMPLEMENTATION_SPEC.md §19.11
TASK010_TEST_AUTHORITY: IMPLEMENTATION_SPEC.md §20.0.9
```

AC-027 remains owned by TASK-013 because its `staged` and durable
`PENDING_APPROVAL` state is not created here.

Migration, persistence, replay, restart-corruption, Admin and executable-custody
tests belong to TASK-012/013; terminal EgressAuthorization and Network/Asset Broker
composition belongs to TASK-016. None is falsely counted as TASK-010 evidence.

## 10. Exact candidate file scope

After independent acceptance, TASK-010 may modify only:

```text
Cargo.toml
Cargo.lock
deny.toml
AGENTS.md
crates/mengxia-plugin-package/Cargo.toml
crates/mengxia-plugin-package/src/lib.rs
crates/mengxia-plugin-package/src/manifest.rs
crates/mengxia-plugin-package/src/dependency.rs
crates/mengxia-plugin-security/Cargo.toml
crates/mengxia-plugin-security/src/lib.rs
crates/mengxia-plugin-security/src/permission_diff.rs
crates/mengxia-testkit/Cargo.toml
crates/mengxia-testkit/tests/architecture.rs
crates/mengxia-testkit/tests/ci_orchestration.rs
crates/mengxia-testkit/tests/document_traceability.rs
crates/mengxia-testkit/tests/task_010_foundation.rs
crates/mengxia-testkit/tests/fixtures/task_010/**
schemas/plugin/manifest-v1.schema.json
scripts/verify-task-010.sh
scripts/verify-repository.sh
.github/workflows/ci.yml
docs/proposals/TASK-010-GATE-PROPOSAL.md
docs/spec/adr/ADR-0014-task-010-package-foundation.md
docs/spec/IMPLEMENTATION_SPEC.md
docs/spec/DECISIONS.md
docs/spec/IMPLEMENTATION_REVIEW.md
docs/spec/IMPLEMENTATION_PLAN.md
docs/spec/PROJECT_INTAKE_REPORT.md
```

The workflow file is listed only to change the stale display name from TASK-001
through TASK-009 to TASK-001 through TASK-010. Classifier, job behavior, permissions
and protected-branch evidence do not change. `mengxia-platform-fs`, store, migration,
app, ports, proto, binaries, host and sandbox are deliberately excluded.

`deny.toml` is authorized only for the reviewed `jsonschema 0.56.0` closure: keep
MIT-0 out of global `licenses.allow` and add its exact per-crate exception only for
`borrow-or-share@0.2.4`; record exact-feature sets for its unavoidable
transitive defaults; and document the retained `foldhash 0.1/0.2`, `getrandom
0.3/0.4` and `r-efi 5/6` pairs. It must not lower a lint level, add an advisory
ignore, allow an unknown source/license family, use a wildcard exception or exempt
an unreviewed version. The exact resulting file must make the isolated and real
workspace `cargo deny check` pass.

## 11. Accepted canonical synchronization

The accepted review change:

- accept ADR-0014 and this exact package/dependency/ownership contract;
- retain AC-098..AC-100 as the TASK-010 foundation criteria and AC-027 under TASK-013;
- move migrations 0003/0004 and their tables to TASK-013;
- make TASK-011 depend on `TASK010_FOUNDATION_DONE`;
- add TASK-012's managed executable custody/launch proof and TASK-013 install owner;
- accept the final-manifest preflight graph and exact `deny.toml` delta as STEP-1's only
  dependency-policy authority; the repository lock is not changed before start;
- add every stable test ID and exact file whitelist;
- keep `OQ-010` open and Admin/installation/activation disabled;
- creates the exact active start record after independent review passes.

## 12. Authorized implementation order

```text
STEP-1  Apply only the exact preflighted manifest edges, lock and deny delta and run
        supply/offline gates; require 117 retained packages plus the exact 40 added
        registry packages and candidate lock hash; stop before manifest code if the
        real graph differs or any gate fails.
STEP-2  Freeze schema plus independently computed canonical/digest fixtures.
STEP-3  Implement bounded duplicate-aware parse and schema validation.
STEP-4  Implement closed typed manifest and dependency declaration values.
STEP-5  Implement canonical serializer and PackageDigest.
STEP-6  Implement pure PermissionDiff and exhaustive lattice tests.
STEP-7  Run task gate, complete workspace, Clippy and developer repository gate.
STEP-8  Open a code PR and require dependency review plus formal macOS evidence.
STEP-9  Record completion and revoke TASK-010 authority to NONE.
```

No step starts TASK-011 automatically.

## 13. Active start record

```text
TASK010_CANONICAL_GATE: ACCEPTED
TASK010_LIFECYCLE: IN_PROGRESS
TASK010_IMPLEMENTATION_AUTHORITY: TASK_010_FOUNDATION_ONLY
TASK010_PROPOSAL_VERSION: 0.2.3
TASK010_ADR: ADR-0014_ACCEPTED
TASK010_OQ010: OPEN_ADMIN_DISABLED_NOT_A_FOUNDATION_DEPENDENCY
TASK010_ALLOWED_SCOPE: proposal §10 exactly
TASK010_FORBIDDEN: filesystem/store/migration/app/ports/proto/CLI/daemon/Admin/install/activation/execution/TASK-011+
TASK010_ACCEPTANCE: AC-098,AC-099,AC-100
TASK010_TESTS: IMPLEMENTATION_SPEC.md §20.0.9 exact registry; proposal §9 IDs only
TASK010_BASELINE: exact reviewed head plus proposal §4 final-manifest dependency preflight
```

This record is active only together with canonical Specification v1.1.44 and Plan
v0.3.55. It grants no authority outside §10.

## 14. Independent review checklist

- one manifest byte identity, no archive/extraction surface;
- no durable host path or shell/PATH field;
- duplicate and unknown security data cannot disappear;
- schema engine has no reachable network/file resolver and production calls
  `offline()` explicitly;
- capability syntax is consistently `name@version`;
- package/security dependency direction is one-way and acyclic;
- diff returns typed, sorted evidence only and is total for mixed changes and
  identity mismatch;
- no grant, revocation row or lifecycle transition exists in TASK-010;
- managed executable custody and path-based macOS launch risk are owned before
  third-party activation;
- every cap has one exact counting definition, a boundary test and checked arithmetic;
- exact dependencies pass offline/supply gates;
- completed migrations and TASK-001..009 behavior remain unchanged.

## 15. Current next action

```text
READINESS: READY_TO_IMPLEMENT_TASK_010_FOUNDATION
NEXT_SAFE_ACTION: execute STEP-1 and stop if the real graph differs
PRODUCTION_CODE_CHANGE: TASK_010_FOUNDATION_ONLY
MIGRATION_CHANGE: FORBIDDEN
ADMIN_OR_PLUGIN_EXECUTION: FORBIDDEN
```
