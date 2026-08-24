---
title: "梦夏（MengXia）实施计划"
project: "梦夏 / MengXia"
document_role: "Living Implementation Plan"
status: "TASK_004_IN_PROGRESS"
version: "0.3.21"
date: "2026-08-24"
language: "zh-CN"
source_of_truth: "IMPLEMENTATION_SPEC.md v1.1.11"
review: "IMPLEMENTATION_REVIEW.md v1.1.21"
---

# 梦夏（MengXia）实施计划

本文档记录实施顺序、task gate 和验收证据。架构/语义以 `IMPLEMENTATION_SPEC.md` 为准，开放决策与例外以 `DECISIONS.md`/ADR 为准，当前可实现性与阻塞项以 `IMPLEMENTATION_REVIEW.md` 为准。

## 1. Status semantics

- `PENDING`: 依赖已知但尚未开始。
- `IN_PROGRESS`: 已满足 gate，正在实施；同一阶段最多一个状态 owner。
- `BLOCKED`: 存在明确未解决依赖/决策/Review finding；不得写依赖该项的实现。
- `DONE`: 实现、正向/负向/故障测试及证据均已完成。

Task 不得仅因文件存在或 happy-path 通过而标记 `DONE`。每个 task 完成记录必须列出 commit/worktree state、运行命令、结果、未执行测试及原因；security/recovery test 不得静默跳过。

## 2. Current State versus Target State

| Item | Current State | Target State | Classification |
|---|---|---|---|
| Git | initialized, branch `main` | retained | `FACT` |
| Source/workspace | TASK-001 Cargo workspace and TASK-002 foundation value/error baseline are implemented and verified | domain/runtime behavior implemented by owning tasks | `FACT / PARTIAL TARGET` |
| Schema/migrations | TASK-004 bootstrap migration `0000_store_bootstrap`, exact reopen `quick_check`/schema/PRAGMA allowlist and typed singleton validation are implemented and verified; product schema migrations beginning with `0001` remain absent | reviewed forward-only migrations | `FACT / PARTIAL TARGET` |
| Tests/CI | TASK-001 repository verification tests/scripts and arm64 macOS CI present | layered functional/security/recovery suites added by owning tasks | `FACT / PARTIAL TARGET` |
| Review | historical findings retained; TASK-004 corrections are accepted by Specification v1.1.11/ADR-0006, its start record is active, and its implementation plus complete deterministic corruption evidence are verified | reproduce every TASK-004 gate before DONE | `FACT / ACTIVE GATE` |
| Phase 0 decisions | OQ-003, early OQ-006 and foundation Client/Admin boundary accepted | retained until superseded | `DECISION / ACCEPTED` |

Current plan state: `TASK_004_IN_PROGRESS`. TASK-001 and TASK-002 are verified complete. User-selected Option A makes TASK-004 active so it can create durable Library owner/lock context before TASK-003 activates Client IPC. Specification v1.1.11 incorporates the accepted TASK-004 supplement. The verified implementation now includes SQLite/config/bootstrap/runtime hardening, exact SQLite/platform error classification, the complete bootstrap-only reopen schema validator, macOS path/ACL authority, absent/empty root creation, one durable exclusive lock with post-lock revalidation and explicit same-description release, and the exact version-one 256-byte intent codec plus descriptor-relative exclusive create, short-write loop and intent/root fsync ordering. Golden/independent-checksum/corruption tests and `BootstrapFsOps` failure traces prove the exact returned prefix is preserved without cleanup or canonical creation. Reopen now acquires the lock before bounded descriptor-relative intent read, repeats inode/size/ACL/root-state proofs, and exposes typed namespace states whose intent bytes are separately decoded and authority-verified; malformed state is preserved. A verified valid intent is then re-read, synchronized intent-before-root, re-read again, and used to create only the fixed owner-only empty staging file with `O_EXCL|O_NOFOLLOW` before the second root sync. Fault traces prove each failure preserves lock, exact intent and only the allowed absent/empty staging state; pre-existing staging is never overwritten. The staging SQLite slice now opens only that fixed existing file without `CREATE`, applies the configured finite busy timeout and accepted hardening, commits the exact bootstrap rows in one immediate transaction, truncates/checkpoints and closes WAL, validates recognized sidecars, fsyncs the closed staging inode, reopens read-only for exact intent/schema/typed-row verification, then performs one bounded checkpoint/close cleanup so the pre-publish state contains no sidecars. Config/authority mismatch and unsafe WAL/SHM metadata fail closed before unauthorized mutation. The ordered publish slice revalidates the exact intent and staging inode, creates canonical only through descriptor-relative `linkat`, proves both names identify the same inode, then performs root fsync, staging unlink, root fsync, intent unlink and final root fsync before canonical read-only schema/identity validation and closed-file sync. Fault injection covers every publish syscall prefix; tampered staging never creates canonical. Closed restart recovery now reacquires the pre-existing lock and handles intent-only, complete staging, canonical-plus-staging same-inode, canonical-plus-intent and canonical-only states, always revalidating SQLite/intent metadata before cleanup and returning the retained authority plus typed metadata. Valid-intent staging recovery additionally accepts only the exact owner-only staging/WAL/SHM name set, lets bundled SQLite recover the last committed WAL state, publishes an exact complete bootstrap, or proves `quick_check` plus an empty `sqlite_schema` before descriptor-relative staging/sidecar cleanup and returning retained lock-only `NeedsFreshBootstrap`. Cleanup fault injection preserves every completed namespace prefix; committed-but-tampered or unsafe-sidecar evidence is never deleted. Real killed-writer fixtures now prove missing/malformed SHM recovery, valid commit plus incomplete WAL tail, valid commit plus uncommitted frames and pure rollback cleanup. Before SQLite recovery, a lock-bound descriptor-relative WAL reader performs a 1,024-frame-bounded format/salt/checksum-chain scan; damage to payload, salt or checksum required by the acknowledged bootstrap commit returns corruption and preserves intent/staging/WAL evidence instead of being misclassified as an empty rollback. Reopen validation now adds forced semantic probes for both expected unique indexes because `quick_check` alone does not detect every bit-flipped index payload; malformed/truncated databases, table/index cell flips, forbidden schema objects/shapes and typed row/timestamp/identity matrices return corruption. The standalone test-only WAL-reset regression uses four independent connections, retained snapshots, concurrent writers/checkpointer, 16 fixed seeds by 256 cycles, salt observations, final TRUNCATE/reopen/checksum assertions and an independent 30-second subprocess watchdog. The bounded lifecycle retains Library authority across one dedicated writer and the exact configured read workers, serializes FIFO insertion, dequeue and shutdown-gate close, counts only queued-not-started writers, gives read workers immediate non-waiting leases, and exposes only fixed crate-internal bootstrap verification commands. Cancellation drops only result interest; shutdown revokes queued writers with the exact safe error, joins running transactions/reads and every worker, validates/checkpoints/closes canonical, then releases the Library lock. Queue boundary, FIFO, cancellation, read saturation, concurrent submit/shutdown, worker panic/join failure and post-shutdown lock reacquisition tests pass. Distinct published inodes map to corruption and tampered canonical/intent states are preserved. ADR-0006 fixes the finite macOS FFI/build-evidence boundary, and the synchronized start record authorizes only the exact TASK-004 scope. The same-OS SIGKILL matrix runs 29 producer/recovery subprocess cases across all 23 acknowledged boundaries, including seven deterministic point-7 short-write prefixes. The deterministic corruption matrix now explicitly covers database/page/quick-check damage, complete schema/autoindex/migration/metadata/intent identity corruption, WAL required-commit damage, filesystem ownership/mode/type/APFS/ACL evidence and every mutable prefix depth. All fourteen local gate mappings and the retained baseline pass under the exact tuple; formal reviewed CI attestation remains pending, so TASK-004 is not DONE. TASK-003 and every later task remain unauthorized. The whole-V1 review verdict remains `NOT READY FOR CODEX`; that product-wide verdict does not block this explicitly authorized slice.

## 3. Phase 0 — intake, evidence and decision gate

Status: `DONE` for the foundation scope required by TASK-001..TASK-005.

Required outputs:

1. `PROJECT_INTAKE_REPORT.md` with repository/toolchain/host filesystem facts and discrepancy classifications: `DONE` and refreshed after the foundation toolchain/platform decisions; refresh again when those facts change.
2. Accepted `OQ-003` decision: `DONE` in ADR-0003—Rust/MSRV 1.98.0 and bundled SQLite 3.53.4 with source/options/checksum.
3. Accepted incremental `OQ-006` caps: `DONE` for TASK-002..TASK-005 in ADR-0005; later caps close before their consuming tasks.
4. Accepted target platform and ordinary Client peer-auth contract: `DONE` in ADR-0004; Admin features explicitly remain disabled until a later OQ-010 ADR.
5. A doc traceability check covering the normative namespaces and canonical-definition/range rules in Specification §0.5. Current documentation correction defines TASK-001 IDs; `TEST-DOC-001` makes the check reproducible during TASK-001 and requires PASS before that task is `DONE`.
6. `IMPLEMENTATION_REVIEW.md` updated so the foundation decisions needed to assess TASK-001..TASK-005 are recorded: `DONE`; TASK-004's accepted contract closes its gate findings, while each later task still requires its own start-gate audit.

Completion condition: the first task's full dependency set is accepted and there are no unrecorded conflicts. Phase 0 does not choose Provider, secret store or sandbox prematurely unless evidence is available.

## 4. Task start and completion contract

Before starting any task, Codex MUST answer from the documents:

```text
Feature/Requirement/AC/TEST IDs; exact files and dependency direction;
trusted and untrusted inputs; authenticated principal and authorization;
persisted state/transaction/effect boundaries; timeout/retry/idempotency;
positive, negative, concurrency, abuse and failure tests; completion evidence.
```

If any answer affects correctness/security/data integrity and is absent, classify and record it; do not infer it. Every external/process/file side effect requires a durable intent/recovery answer before implementation.

At task start, stable AC/TEST obligations must already be defined and copied into the task-start record; their repository commands may be created by that task. At task completion, every referenced TEST ID must map to a deterministic repository command/target/check and have recorded PASS evidence. This staged contract prevents both prose-only completion and a bootstrap circular dependency.

## 5. Task traceability and order

Detailed task bodies are normative in Specification §18. This table adds the required living status, traceability, file scope, security implications and do-not-change constraints.

| Task | Status | Feature / Requirements | Dependencies / decision gates | Likely files | Acceptance and tests | Security implications / Do not change |
|---|---|---|---|---|---|---|
| `TASK-001` Repository bootstrap | `DONE` | FUNC-001; SEC-020, DATA-006 | Phase 0 and OQ-003 accepted | root Cargo/toolchain/deny config, crate skeletons | AC-050, AC-051, AC-052, AC-053, AC-054; TEST-BOOT-001, TEST-BOOT-002, TEST-ARCH-001, TEST-NAME-001, TEST-SUPPLY-001, TEST-DOC-001 | Pin fixed SQLite path; no historical names or relaxed dependency edges |
| `TASK-002` Core values/error baseline | `DONE` | FUNC-001; REQ-001, API-010, DATA-012; SEC-017, SEC-020 | TASK-001; BASE-011, BASE-013, BASE-014; ADR-0003, ADR-0005; accepted REVIEW-GAP-003 | workspace deps/lock, `mengxia-types`, domain/errors, TASK-002 tests/docs | AC-055, AC-056, AC-057, AC-058, AC-059; TEST-TYPE-001, TEST-PARSE-001, TEST-TIME-001, TEST-ERROR-001, TEST-ARCH-002, TEST-SUPPLY-002, TEST-DOC-002 | Exact accepted codecs/fallible generator/errors/deps only; no Provider/Plugin/proto/Serde/DB/storage behavior or raw input/secret retention |
| `TASK-003` IPC, framing, Client identity | `PENDING` | FUNC-001; API-001, API-002, API-003, API-008, API-009, API-010; SEC-005, SEC-013, SEC-014; CFG-003 | TASK-002; TASK-004 durable owner/lock context; ADR-0004 peer-auth; ADR-0005 frame cap; Admin disabled | proto/core, framing, daemon, CLI | AC-028, AC-029; stable TASK-003 registry still required; fuzz, actor spoof, unauthorized peer, cap±1, no-TCP | Actor server-derived; Admin disabled; IPC consumes opened Library context and never depends on SQLite/inferred owner authority |
| `TASK-004` SQLite/migration engine | `IN_PROGRESS` | FUNC-001; DATA-001, DATA-005, DATA-006, DATA-007, DATA-011; REL-001; SEC-017, SEC-020, SEC-021; CFG-001, CFG-003 | TASK-002; BASE-011, BASE-013, BASE-014, BASE-015, BASE-017; DEC-017, DEC-020, DEC-021, DEC-022; ADR-0001, ADR-0003, ADR-0004, ADR-0005, ADR-0006; accepted implementation supplement | exact supplement §8 scope | AC-065, AC-066, AC-067, AC-068, AC-069, AC-070, AC-071, AC-072, AC-073; fourteen stable tests | Bootstrap lifecycle only; exact bounded scope; no TASK-003/Admin/later schema/capability |
| `TASK-005` BlobStorage/CAS primitives | `PENDING` | FUNC-002; DATA-002, DATA-003, DATA-004, DATA-013; PERF-001; SEC-017, SEC-021 | TASK-002; ADR-0005 stream/I/O/hash/staging caps | ports, local storage | source/symlink/disk-full/crash/O(buffer), cap±1 | Stable handles/root confinement; no source deletion; no canonical Asset ownership |
| `TASK-006` Asset domain/persistence | `PENDING` | FUNC-002, FUNC-003; REQ-001, REQ-002, REQ-004, REQ-005, REQ-008, REQ-011, REQ-012; DATA-009, DATA-010, DATA-011 | TASK-004, TASK-005 | domain/app/store, complete immutable `0001_library_assets` | AC-002, AC-005..AC-008, FK/sequence/concurrency/event tests | No migration rewrite after apply; Blob dedup never merges Asset |
| `TASK-007` copy-only ingest slice | `PENDING` | FUNC-002; REQ-010, REQ-013; API-003, API-010; REL-005 | TASK-003, TASK-006 | app/proto/CLI/store/storage | AC-001..AC-009, E2E, concurrent duplicate, all crash points | Copy only; reject adopt/reference; physical durability before registration |
| `TASK-008` verify/recovery | `PENDING` | FUNC-001, FUNC-010; REL-004, REL-008; OPS-004 | TASK-007 | daemon/app/store/storage/CLI | corruption matrix, provider-offline restart AC-015, startup cost | Deep verify explicit; unrelated local work allowed in degraded mode |
| `TASK-009` Project/Work/Take | `PENDING` | FUNC-004; REQ-003, REQ-004, REQ-006, REQ-007, REQ-012, REQ-014; SEC-014 | TASK-006 | domain/app/store/proto, `0002_projects_work` | AC-010, AC-011, AC-016; transition/concurrency/cross-Project tests | Project not tenant/Asset owner; no generic CRUD/direct state assignment |
| `TASK-010` Plugin package/Manifest | `BLOCKED` | FUNC-006; SEC-003, SEC-009, SEC-010, SEC-016, SEC-020 | TASK-001, TASK-002; OQ-010 for install/approve | package/security/schema, `0003_plugin_packages` | AC-027; schema/tamper/publisher spoof/dependency tests | VERIFIED does not authenticate publisher; exact digest grant only |
| `TASK-011` Plugin protocol/hostile fixture | `BLOCKED` | FUNC-006, FUNC-007; API-004; REL-001, REL-006; SEC-017, SEC-021 | TASK-003, TASK-010; frame/log/process caps | plugin proto/framing/host/testkit | malformed/flood/crash/timeout/queue cap suite | Private channel only; bounded stdout/stderr/frames; no Core/Admin handle |
| `TASK-012` exact OS sandbox | `BLOCKED` | FUNC-007; SEC-001, SEC-002, SEC-005, SEC-021 | TASK-011; OQ-001, OQ-002; resource caps | platform sandbox/host/security tests | AC-020..AC-023 + mandatory real hostile suite | All required dimensions ENFORCED or deny; no backend-name/self-report shortcut |
| `TASK-013` Lease/Asset Broker/audit | `BLOCKED` | FUNC-006, FUNC-007, FUNC-010; SEC-004, SEC-005, SEC-006, SEC-019; DATA-011 | TASK-009, TASK-012; OQ-010 | security/host/brokers/store, `0004_plugin_security` | AC-024, AC-026, AC-029; caller/race/revoke/audit tests | Caller/channel/run/digest binding; CAS path hidden; ordinary Client cannot grant |
| `TASK-014` controlled FFmpeg Plugin | `PENDING` | FUNC-005, FUNC-007; SEC-009, SEC-017, SEC-020; REL-006 | TASK-013; accepted executable digest/resource caps | plugin/tool + contracts | timeout/cancel/malformed media/digest/output verify | argv only, no shell/PATH/DB/CAS; output untrusted until verified |
| `TASK-015` Recipe/Run runtime | `PENDING` | FUNC-005; REQ-006, REQ-009; API-005, API-006, API-007; DATA-010; REL-002, REL-003, REL-004, REL-005, REL-006, REL-007, REL-008 | TASK-009, TASK-014; job/queue/deadline caps | runtime/domain/app/store, `0005_runtime` | AC-012..AC-016, AC-031; DAG/attempt/crash/partial-success tests | Persist intent before effect; UNKNOWN no blind retry; retry creates Attempt |
| `TASK-016` Secret/Network Brokers | `BLOCKED` | FUNC-007, FUNC-008; SEC-006, SEC-007, SEC-011, SEC-012, SEC-015, SEC-017, SEC-021; CFG-002 | TASK-013, TASK-015; OQ-004, OQ-010; egress/cost/size caps | brokers/security/config | AC-023, AC-025, AC-029, AC-044; SSRF/rebinding/redirect/canary rotation tests | No generic proxy/raw static secret to sandbox; no real egress before pass |
| `TASK-017` Provider selection gate | `BLOCKED` | FUNC-008, FUNC-012; API-005, API-006, API-007 | TASK-016; OQ-005 | ADRs, adapter README/contracts | current official source/auth/state/idempotency/webhook evidence | No implementation from memory; no generic webhook listener |
| `TASK-018` CLI Provider | `PENDING` | FUNC-008, FUNC-012; API-005, API-006, API-007; REL-002, REL-003, REL-006, REL-007 | TASK-017 | selected plugin/adapter | fake CLI + opt-in real contract, kill/timeout/env tests | verified executable/env; durable external ID; no shell |
| `TASK-019` HTTP Provider | `PENDING` | FUNC-008, FUNC-012; SEC-011, SEC-015, SEC-017; REL-002, REL-003, REL-006, REL-007 | TASK-017 | selected adapter/brokers | mock server + optional real; retry/rate/redirect/webhook-if-enabled | Broker-only egress; bounded streaming; adapter-specific callback contract |
| `TASK-020` Local/Hybrid/interoperability | `PENDING` | FUNC-012; G-001, G-003, G-008; DATA-008 | TASK-017, TASK-019 | adapter/integration/export | AC-030; reopen without plugin; relationship/resolve/register contracts | No Core schema change or provider type leakage |
| `TASK-021` Rights/classification/clearance | `BLOCKED` | FUNC-009; G-009, REQ-015, DATA-012; SEC-014, SEC-017, SEC-019 | TASK-009, TASK-013; OQ-009 | rights/domain/app/proto/store, `0006_rights_classification` | AC-040; correction/conflict/egress/cross-Project/audit E2E | UNKNOWN/CONFLICTED never implicit ALLOW; Provenance != Rights |
| `TASK-022` retention/GC/Purge | `BLOCKED` | FUNC-011; REQ-015; DATA-010, DATA-011; SEC-018, SEC-019 | TASK-008, TASK-013, TASK-021; OQ-008 | admin/app/storage/store/CLI | AC-041..AC-043; preview/hold/last-copy/concurrency/crash tests | Purge disabled until policy; exact target set; no retirement→delete inference |
| `TASK-023` release gate | `BLOCKED` | all enabled FUNC and P0 requirements | enabled tasks complete; OQ-006 release SLO part | CI/docs/evidence/ops | all mandatory suites, upgrades, benchmarks, fresh advisories | No silent skip/fabricated SLO/unsupported security claim |

### TASK-001 start record — 2026-08-21

- Scope: `TASK-001` only; repository bootstrap, empty crate/binary skeletons and its deterministic verification gates. No TASK-002 domain behavior or later feature is authorized.
- Feature/Requirements: `FUNC-001`; `SEC-020`; `DATA-006` (the TASK-001 obligation is limited to preserving the accepted bundled SQLite pin/boundary; runtime integration remains owned by TASK-004).
- Decisions/gates read: `OQ-003`; `DEC-001`; `DEC-007`; `DEC-019`; `DEC-020`; `BASE-001`; `BASE-011`; `BASE-014`; `BASE-016`; `ADR-0003`.
- Acceptance obligations: `AC-050`; `AC-051`; `AC-052`; `AC-053`; `AC-054`.
- Verification obligations: `TEST-BOOT-001`; `TEST-BOOT-002`; `TEST-ARCH-001`; `TEST-NAME-001`; `TEST-SUPPLY-001`; `TEST-DOC-001`.
- Planned file scope: root `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `deny.toml`, `.gitignore`; `.github/workflows/ci.yml`; `scripts/verify-task-001.sh`, `scripts/check-supply-chain.sh`; empty manifests/targets under `crates/` and `bins/`; TASK-001-only verification code/fixtures in `crates/mengxia-testkit/`; this living task record.
- Dependency direction: `domain/types/events -> ports -> application -> infrastructure adapters -> daemon/CLI composition roots`; forbidden edges remain exactly Specification §5.3 and are enforced against Cargo metadata plus a negative fixture.
- Trust/security answers: Cargo manifests, lockfiles, repository paths, Git inventory, advisory data and tool output are validation inputs and are not trusted merely because parsing succeeds. TASK-001 introduces no authenticated principal, authorization decision, tenant context, secret, persistence transaction, network/service runtime, retry, idempotent command or destructive behavior. External advisory availability is bounded by the verification process and MUST fail as `UNVERIFIABLE`, never PASS.
- Test/evidence plan: positive workspace/metadata/build/fmt/Clippy/test checks; forbidden-edge negative fixture; canonical-name and tracked-file hygiene checks; fail-closed locked source/license/advisory check; stable-ID positive and negative parser checks. Concurrency, runtime retry/idempotency, authentication and tenant tests are not applicable to an empty repository skeleton.
- Completion evidence required: every listed TEST ID maps to a deterministic command and passes; every listed AC is checked against command/diff evidence; `SEC-020` passes; no new regression, unresolved applicable blocker, scope expansion or architecture drift remains.

### TASK-001 completion record — 2026-08-21

- Scope result: only repository bootstrap, empty canonical crate/binary skeletons, CI/policy configuration and TASK-001 verification infrastructure were added. No TASK-002 behavior or later capability was implemented.
- Commit/worktree evidence: the repository baseline change containing this record is limited to the reviewed code/document/config candidate inventory; ignored `.DS_Store` and Cargo `target/` content is excluded, and the tracked worktree must be clean when the commit is handed off.
- `TEST-BOOT-001`: `PASS` — `scripts/verify-task-001.sh TEST-BOOT-001`; arm64 host, Rust/Cargo 1.98.0 and locked metadata verified.
- `TEST-BOOT-002`: `PASS` — `scripts/verify-task-001.sh TEST-BOOT-002`; format, build, check, Clippy with warnings denied and all workspace tests passed for all targets/features.
- `TEST-ARCH-001`: `PASS` — `scripts/verify-task-001.sh TEST-ARCH-001`; allowed internal graph passed, the committed `domain -> application` fixture and explicit forbidden infrastructure edges were rejected, and pure crates explicitly forbid unsafe code.
- `TEST-NAME-001`: `PASS` — `scripts/verify-task-001.sh TEST-NAME-001`; the 17-package canonical inventory, approved code/document/config path allowlist, candidate tracked-file hygiene and explicit nested-build/editor/log/coverage/environment ignore matrix passed; ignored pre-existing `.DS_Store` and Cargo `target/` files remain untracked.
- `TEST-SUPPLY-001`: `PASS` — `scripts/verify-task-001.sh TEST-SUPPLY-001`; cargo-deny 0.20.2, lock/source/license/bans/current RustSec checks passed; the deterministic unavailable simulation returned `UNVERIFIABLE` with exit status 2.
- `TEST-DOC-001`: `PASS` — `scripts/verify-task-001.sh TEST-DOC-001`; canonical definitions/references/ranges/task lifecycle/dependencies passed, while unknown/duplicate/malformed/unmet negative cases were rejected.
- `AC-050`: `PASS` — pinned toolchain, locked complete metadata and all-target build/check/test evidence from `TEST-BOOT-001` and `TEST-BOOT-002`.
- `AC-051`: `PASS` — metadata direction, committed negative fixture, explicit forbidden-edge cases and pure-crate unsafe prohibition evidence from `TEST-ARCH-001`.
- `AC-052`: `PASS` — canonical package/binary/path inventory, Git candidate-inventory hygiene and required `.gitignore` coverage evidence from `TEST-NAME-001`.
- `AC-053`: `PASS` — exact internal dependency versions, Cargo.lock, fail-closed deny policy, fresh advisory fetch and explicit unavailable result from `TEST-SUPPLY-001`.
- `AC-054`: `PASS` — deterministic closed stable-ID registry, complete TASK-001 record and negative traceability cases from `TEST-DOC-001`.
- Security result — `SEC-020`: `PASS`; no third-party production dependency was added, CI actions/tool versions are exact-pinned, and advisory unavailability cannot be reported as clean. Runtime SQLite hardening under `DATA-006` remains correctly deferred to TASK-004; TASK-001 preserves Rust 1.98.0 and SQLite 3.53.4 decision traceability.
- Baseline/diff result: `BASELINE-003` records and resolves the stale pre-TASK-001 Current State descriptions; no new regression remains, and review found no secret, debug bypass, public API behavior, migration, architecture drift or later-task implementation.

### TASK-002 start record — 2026-08-21

- Scope: `TASK-002` only; typed UUIDv7 IDs, SHA-256 digest value, UTC timestamp, RevisionNo and the minimal safe typed error baseline. No TASK-003 or later behavior is authorized.
- Feature/Requirements: `FUNC-001`; `REQ-001`; `API-010`; `DATA-012`; `SEC-017`; `SEC-020`.
- Decisions/gates read: `BASE-011`; `BASE-013`; `BASE-014`; `ADR-0003`; `ADR-0005`; `REVIEW-GAP-003`; completed `TASK-001` evidence.
- Acceptance obligations: `AC-055`; `AC-056`; `AC-057`; `AC-058`; `AC-059`.
- Verification obligations: `TEST-TYPE-001`; `TEST-PARSE-001`; `TEST-TIME-001`; `TEST-ERROR-001`; `TEST-ARCH-002`; `TEST-SUPPLY-002`; `TEST-DOC-002`.
- Planned file scope: root/workspace Cargo dependency declarations and lockfile; `crates/mengxia-types`; the minimal error module in `crates/mengxia-domain`; TASK-002-only tests/verification wiring; synchronized canonical/current-state lifecycle records. No proto, schema, migration, persistence, storage, Provider, Plugin, IPC or binary behavior.
- Public contract: exact Specification §8.1.1 and §14 contracts incorporated from the user-accepted `docs/proposals/TASK-002-GATE-PROPOSAL.md`.
- Security answers: all textual values are untrusted, exactly bounded, strictly parsed and canonicalized; error values retain no raw rejected input. This task has no authenticated operation, authorization decision, tenant, secret store, persistence transaction, network/process/file side effect, retry, idempotent command, migration or destructive behavior. ID generation is concurrency-tested and uses direct fallible OS time/entropy with the stateless UUID builder; it uses no MengXia-owned or dependency-owned shared counter/generator.
- Dependency answer: exact `uuid` 1.24.1/std, `getrandom` 0.4.3/std, `time` 0.3.55/std+formatting+parsing and dev-only `proptest` 1.11.0/std pins/features are accepted with default features disabled and must pass the existing fail-closed lock/license/advisory policy.
- Completion evidence required: every listed TEST ID maps to a deterministic command and passes; every listed AC and applicable security requirement has evidence; TASK-001 baseline remains green; no new regression, scope expansion, public API drift or unresolved applicable blocker remains.

### TASK-002 completion record — 2026-08-21

- Commit/worktree state: branch `main`, parent baseline commit `60b344571874946dc7fb77936ca1d50f40cb045d`; this record is part of the reviewed TASK-002 candidate and therefore does not self-claim its resulting commit hash. The completion handoff must report that hash and a clean tracked worktree.
- `TEST-TYPE-001`: `PASS` — `scripts/verify-task-002.sh TEST-TYPE-001`; UUIDv7 generation, marker typing, trait surface, byte/text round trips, deterministic time/entropy failure seams and parallel uniqueness sample passed.
- `TEST-PARSE-001`: `PASS` — `scripts/verify-task-002.sh TEST-PARSE-001`; malformed, noncanonical, non-ASCII, wrong UUID version/variant and numeric/time boundary inputs were rejected.
- `TEST-TIME-001`: `PASS` — `scripts/verify-task-002.sh TEST-TIME-001`; UTC year/nanosecond bounds, unique canonical `Z` text and checked revision exhaustion passed.
- `TEST-ERROR-001`: `PASS` — `scripts/verify-task-002.sh TEST-ERROR-001`; all 25 stable codes round-trip exactly and typed value/domain errors expose only static safe diagnostics.
- `TEST-ARCH-002`: `PASS` — `scripts/verify-task-002.sh TEST-ARCH-002`; exact public dependency surface, forbidden production edges and marker-mismatch compile-fail fixture passed.
- `TEST-SUPPLY-002`: `PASS` — `scripts/verify-task-002.sh TEST-SUPPLY-002`; exact pins/features/lock/license/advisory graph passed, production resolves only `getrandom` 0.4.3, dev-only `proptest` resolves 0.3.4, and unavailable advisory data fails as `UNVERIFIABLE`.
- `TEST-DOC-002`: `PASS` — `scripts/verify-task-002.sh TEST-DOC-002`; stable IDs, start/completion records and synchronized current-state markers passed positive and stale-state negative checks.
- `AC-055`: `PASS` — `TEST-TYPE-001`, `TEST-PARSE-001` and `TEST-ARCH-002` prove opaque marker-safe UUIDv7 values, canonical codecs and fallible stateless generation.
- `AC-056`: `PASS` — `TEST-TYPE-001` and `TEST-PARSE-001` prove exact 32-byte/lowercase-hex digest behavior without hashing APIs.
- `AC-057`: `PASS` — `TEST-TIME-001` and `TEST-PARSE-001` prove bounded UTC timestamps, unique canonical text and checked revisions.
- `AC-058`: `PASS` — `TEST-ERROR-001` proves the exact stable error taxonomy, typed mappings and input-safe diagnostics.
- `AC-059`: `PASS` — `TEST-ARCH-002` and `TEST-SUPPLY-002` prove the accepted public/dependency boundary and fail-closed supply policy.
- `SEC-017`: `PASS` — strict bounded parsers reject ambiguous/noncanonical/untrusted input and errors retain no rejected input; `TEST-PARSE-001` and `TEST-ERROR-001` provide negative evidence.
- `SEC-020`: `PASS` — exact dependency pins/features, lockfile, license/source/bans/advisory checks and the auditable dev-only duplicate path pass `TEST-SUPPLY-002` and `TEST-ARCH-002`.
- Baseline comparison: `scripts/verify-task-001.sh` passed after TASK-002, including format/build/check/Clippy/all-workspace tests, architecture/naming/supply/document gates. No pre-existing failure was reclassified and no new regression remains.
- Diff/scope result: only accepted TASK-002 values/errors, exact dependency policy, fixtures/tests/scripts and synchronized lifecycle documents changed. No proto, Serde, schema, migration, persistence, storage, IPC, binary, Provider, Plugin, authentication/authorization or destructive behavior was added; no test was deleted or weakened.
- Unexecuted required tests: none. TASK-003 and later tasks remain unauthorized and were not started.

### Accepted TASK-004-before-TASK-003 sequencing — 2026-08-21

- Decision: user selected Option A from `docs/proposals/TASK-003-GATE-PROPOSAL.md`.
- Reason: TASK-003 must compare peer UID with the durable recorded Library owner, while TASK-004 owns first-create owner persistence and the Library lock.
- Boundary: TASK-004 produces an opened Library context containing durable owner/lock authority; TASK-003 consumes that context only. Proto/framing/IPC crates cannot depend on SQLite or infer owner authority from eUID, environment, CLI or request fields.
- Stable-ID guarantee: no Task ID is renumbered or split. This is an execution dependency correction, not a Feature/Requirement/AC reassignment.
- Downstream invariants: TASK-006 continues to depend on TASK-004 and TASK-005; TASK-007 continues to depend on TASK-003 and TASK-006; TASK-011 continues to depend on TASK-003 and TASK-010. The dependency graph must remain acyclic.
- Current authorization: TASK-004 is `IN_PROGRESS` under Specification v1.1.11, ADR-0006, the accepted supplement and the exact start record below. No TASK-003 or later behavior is authorized.

### TASK-004 start record — 2026-08-22

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
TESTS: TEST-SQLITE-004; TEST-CONFIG-004; TEST-BOOTSTRAP-004; TEST-PATH-004;
  TEST-MIGRATION-004; TEST-LOCK-004; TEST-QUEUE-004; TEST-ERROR-004;
  TEST-RECOVERY-004; TEST-WAL-004; TEST-CORRUPTION-004; TEST-ARCH-004;
  TEST-SUPPLY-004; TEST-DOC-004
AUTHORIZED: Cargo.toml; Cargo.lock; narrow workspace/naming/supply policy metadata;
  .github/workflows/ci.yml for the exact attested macos-26 gate only;
  crates/mengxia-store-sqlite/**; crates/mengxia-platform-fs/**;
  docs/provenance/macos-acl-ffi-toolchain-v1.toml;
  migrations/sqlite/0000_store_bootstrap.sql;
  third_party/libsqlite3-sys-0.38.2/**; TASK-004 tests/scripts/fixtures;
  synchronized canonical documents; the canonical error registry/module only for
  STORAGE_BUSY and STORAGE_CONFIGURATION_ERROR
FORBIDDEN: TASK-003 transport/daemon/CLI/Admin; migration 0001 or later;
  domain repositories; Blob/CAS, Provider, Plugin, Credential, Project, Rights,
  GC/Purge or product behavior; raw SQL API; custom VFS; system SQLite; unbounded
  wait/retry/detached work; architecture/public-interface expansion
```

- Accepted contract: `docs/proposals/TASK-004-GATE-PROPOSAL.md`, incorporated by
  Specification v1.1.11. The supplement's exact matrices control where this compact
  record summarizes behavior.
- Entry evidence: TASK-001/TASK-002 remain green; Option A/BASE-017 and ADR-0003,
  ADR-0004, ADR-0005, ADR-0006
  are accepted; AC-065..AC-073 and all fourteen TEST IDs are canonically defined;
  the developer/attested build distinction prevents local builds from claiming
  formal supply-chain PASS.
- Completion rule: do not mark DONE until each AC and TEST above has reproducible
  PASS evidence and the complete pre-existing workspace baseline remains green.
- Gate evidence update — 2026-08-24: `scripts/verify-task-004.sh` now maps all
  fourteen TEST IDs to executable commands, retains format/check/Clippy/full-workspace
  baselines, and is invoked by the reviewed arm64 `macos-26` workflow. The complete
  script passes locally with the exact Xcode/SDK/clang tuple under attested-class
  validation, including environment rejection and compile/lint negative fixtures.
  Per §6.1, this local run is not formal attestation; TASK-004 remains `IN_PROGRESS`
  until the reviewed CI job produces the required external PASS record.
- Code-review correction — 2026-08-24: the accepted §5.3 first-create clock/UUID
  production orchestration and `ID_GENERATION_UNAVAILABLE` mapping were absent even
  though the previous local gate passed (`REPO_STALE`). Fresh bootstrap now performs
  read-only parent authorization, samples each source exactly once before any root
  mutation, persists the one identity/timestamp through owner open and restart, and
  has absent/empty-root failure evidence for clock, timestamp and UUID source errors.
  `TEST-BOOTSTRAP-004` and `TEST-ERROR-004` include these cases; formal CI evidence
  remains pending.

## 6. Phases and gates

| Phase | Tasks | Entry gate | Exit evidence |
|---|---|---|---|
| 0 Intake/decisions | documentation only | repository inspected | `DONE` for recorded foundation decisions; TASK-004's task-local gate is accepted and active; later tasks retain their own gates |
| 1 Foundation authority/data | TASK-001..TASK-004 | Phase 0 complete | build + authenticated IPC + fixed/hardened SQLite evidence |
| 2 Managed custody | TASK-005..TASK-009 | Phase 1 | copy-ingest/recovery/domain E2E and crash evidence |
| 3 Plugin authority | TASK-010..TASK-013 | Admin/platform/caps decisions | hostile protocol + exact sandbox + lease/audit evidence |
| 4 Runtime/brokers | TASK-014..TASK-016 | Phase 3 | durable runtime, secret/egress/SSRF/canary evidence |
| 5 Provider/rights/destructive | TASK-017..TASK-022 | relevant OQ decisions | Provider portability, rights and destructive lifecycle E2E |
| 6 Release | TASK-023 | enabled scope complete | P0 traceability manifest and benchmark/security evidence |

Security is not deferred to one phase: authenticated identity, validation, boundedness, error/redaction and audit foundations enter at the first task that exposes the relevant boundary.

## 7. Change and completion rules

- A structural dependency/authority/data decision change is recorded in `DECISIONS.md` and, when long-lived or difficult to reverse, an ADR before code.
- Applied migration bytes are immutable. A later task adds a new migration; it never edits an applied one.
- A task may remain `DONE` only while its evidence is reproducible on the supported platform/version. A security regression reopens the task/gate.
- `PROJECT_INTAKE_REPORT.md` is refreshed for the TASK-001-complete bootstrap state and foundation toolchain/platform evidence; it MUST be refreshed again when repository or host facts change and MUST distinguish empty boundaries from implemented product behavior.
- `IMPLEMENTATION_REVIEW.md` must be updated after Phase 0 and before release with a second full review and final readiness verdict.
