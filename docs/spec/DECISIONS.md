---
title: "梦夏（MengXia）决策日志"
project: "梦夏 / MengXia"
document_role: "Decision Log and ADR Index"
status: "ACTIVE"
version: "0.3.29"
date: "2026-09-01"
language: "zh-CN"
---

# 梦夏（MengXia）决策日志

本文件记录已接受决策、开放问题、规范冲突和 ADR 索引。详细规范仍以
`IMPLEMENTATION_SPEC.md` 为主要 Source of Truth。

## 已接受的基线决策

下列基线始于 canonical specification v1.0.1，并包含至 v1.1.28 的独立审查、foundation gate、TASK-001/TASK-002/TASK-004/TASK-003/TASK-005/TASK-006/TASK-007 completion、TASK-004-before-TASK-003 authority sequencing、post-TASK-007 correction，以及 accepted TASK-005/TASK-006/TASK-007/ADR-0010 contracts；完整约束与理由见当前规范、accepted supplements 和 Review 记录。

| ID | 决策 | 状态 | 来源 |
|---|---|---|---|
| `BASE-001` | V1 使用 Rust、Tokio、SQLite、proto3、JSON Schema 2020-12 与 Cargo Workspace | `ACCEPTED` | Implementation Spec §0.4 |
| `BASE-002` | 产品采用 local-first、vendor-neutral 架构 | `ACCEPTED` | Executive Summary |
| `BASE-003` | V1 不实现 UI，所有动作通过 CLI + Core API 完成 | `ACCEPTED` | §3 |
| `BASE-004` | Canonical persistence 使用 State + append-only events + rebuildable projections + Blob Storage | `ACCEPTED` | `DATA-001` |
| `BASE-005` | 第三方或未知 Native Plugin 必须受 OS 强制 sandbox，否则拒绝激活 | `ACCEPTED` | `SEC-002` |
| `BASE-006` | 真实 Provider Credential 和网络接入晚于安全隔离、Lease 与 Broker | `ACCEPTED` | Executive Summary |
| `BASE-007` | V1 为 single-Library、single-local-owner trust domain；Project 不是 tenant | `ACCEPTED V1` | `DEC-016`, review `REVIEW-002` |
| `BASE-008` | actor 只能由已认证 IPC channel 派生；request 不得声明 actor/Admin role | `ACCEPTED` | `DEC-017`, review `REVIEW-001` |
| `BASE-009` | 初始 IngestAsset vertical slice 仅支持非破坏性的 copy mode | `ACCEPTED V1` | `DEC-018`, review `REVIEW-009` |
| `BASE-010` | 有限 safety cap 在使用它的 task 前阻塞；性能 SLO 仍须测量 | `ACCEPTED` | `DEC-019`, review `REVIEW-008` |
| `BASE-011` | Foundation pin Rust/MSRV 1.98.0 and bundled SQLite 3.53.4 with verified official source/checksum | `ACCEPTED` | `ADR-0003`, `OQ-003` |
| `BASE-012` | arm64 macOS is the initial foundation platform; ordinary Client is UID/channel-derived; Admin and third-party Native Plugin remain disabled | `ACCEPTED V1 FOUNDATION` | `ADR-0004` |
| `BASE-013` | TASK-002..TASK-005 use finite configurable foundation caps; later Plugin/Provider/release caps remain open | `ACCEPTED PARTIAL` | `ADR-0005`, `OQ-006` |
| `BASE-014` | Stable TEST obligations are declared before task start, implemented during the owning task and executable with PASS evidence before DONE | `ACCEPTED` | Specification §0.5, `REVIEW-016` |
| `BASE-015` | First-create bootstrap accepts an absent or correctly owned empty target and rejects canonical/non-empty/unsafe targets | `ACCEPTED` | `ADR-0004`, `REVIEW-018` |
| `BASE-016` | Whole-V1 readiness and scoped task authorization are separate; completed TASK-001/TASK-002 evidence does not authorize a later task whose own gate is absent | `ACCEPTED` | `REVIEW-019`, Plan v0.3.7 |
| `BASE-017` | TASK-004 creates durable Library owner/lock context before TASK-003 activates local Client IPC; IPC consumes the context without depending on SQLite | `ACCEPTED` | user-selected Option A; Specification v1.1.8; TASK-003 gate analysis |
| `BASE-018` | TASK-005 local custody uses opaque source/root capabilities, atomic logical/physical reservation, exact-case no-clobber CAS, stable backend-instance identity and fail-closed cleanup; completion grants no later-task authority | `ACCEPTED / VERIFIED` | ADR-0007; Specification v1.1.18 through v1.1.21; TASK-005 supplement and formal run `33073580258` |
| `BASE-019` | Repository CI uses fail-closed docs/developer/formal scopes and a non-recursive component graph; code formal evidence retains every owned stable mapping and the separate real second-UID job | `ACCEPTED / LOCALLY VERIFIED` | ADR-0010; `REVIEW-CONFLICT-023` |

## 开放决策

不得在实现中静默替这些项目作决定。开始依赖相关选择的任务前，应补充候选方案、证据、影响和结论。

Canonical Open Question ID 以规范 §24 的 `OQ-*` 为准；本表不得建立第二套编号。

| ID | 问题 | 阻塞范围 | 状态 |
|---|---|---|---|
| `OQ-001` / `OQ-002` | arm64 macOS foundation 已接受；exact sandbox backend/version 与第三方 Native Plugin release claim | TASK-012、第三方 Native Plugin claim | `PARTIAL / LATER BLOCKING` |
| `OQ-003` | Rust/MSRV 与包含 WAL-reset 修复的 bundled SQLite 版本/编译选项/checksum | TASK-001、TASK-004 | `ACCEPTED / ADR-0003` |
| `OQ-004` | canonical Credential store | TASK-016、真实 Provider | `OPEN / BLOCKING` |
| `OQ-005` | 真实 Provider validation targets | TASK-018..TASK-020; 由 TASK-017 的 accepted Provider-selection ADR 关闭 | `OPEN / TASK-017 DECISION OUTPUT / BLOCKING IMPLEMENTATION` |
| `OQ-006` | TASK-002..TASK-005 foundation caps 已接受；Plugin/Provider caps、reference hardware 与 release SLO | TASK-011/TASK-012/TASK-016；release | `PARTIAL / LATER BLOCKING` |
| `OQ-007` | user-installed third-party code 是否可为 TRUSTED_NATIVE | policy/release claim | `OPEN / NON-BLOCKING WITH SAFE DEFAULT DENY/SANDBOX_ONLY` |
| `OQ-008` | retention、hold、orphan 与 raw observation policy | TASK-022、production | `OPEN / BLOCKING` |
| `OQ-009` | rights/data-classification schema | TASK-021、真实 egress | `OPEN / BLOCKING` |
| `OQ-010` | Foundation 明确禁用 Admin；未来 macOS Admin authority/user-presence mechanism | TASK-010/TASK-013/TASK-016/TASK-022 and any future storage-root rebind | `DEFERRED / ADMIN DISABLED / LATER BLOCKING` |

## 冲突记录

发现规范、仓库、官方协议或已接受决策之间的冲突时，追加以下模板：

```text
CONFLICT:
Source A:
Source B:
Recommended canonical decision:
Reason:
Impact:
Classification: EXPECTED_GAP | SPEC_STALE | REPO_STALE | CONFLICT | UNKNOWN
Status: OPEN | RESOLVED
```

### TASK-003/TASK-004 durable owner authority sequencing

```text
CONFLICT:
Source A: TASK-003 required peer UID to equal the recorded Library owner UID and was listed before TASK-004.
Source B: ADR-0004 and TASK-004 assign first durable Library owner creation, Library lock and bootstrap lifecycle to TASK-004.
Recommended canonical decision: execute TASK-004 before TASK-003; TASK-003 receives an already-open Library context and never persists, infers or reads owner authority from request/configuration.
Reason: avoids both a store-boundary violation and a temporary authentication root that would weaken ADR-0004.
Impact: only the dependency order changes. Stable Task IDs/scopes remain; TASK-006/TASK-007/TASK-011 and later dependency semantics remain intact and the graph stays acyclic.
Classification: CONFLICT
Resolution evidence: user accepted Option A on 2026-08-21; repository document dependency tests enforce the accepted edges and reject cycles.
Status: RESOLVED / BASE-017
```

### TASK-005 default Blob root versus TASK-004 exact namespace

```text
CONFLICT:
Source A: canonical configuration fixes `MENGXIA_BLOB_ROOT` default to `<library_root>/storage`.
Source B: completed TASK-004 opens and synchronizes only the exact `.mengxia.lock + library.sqlite3` completed state.
Recommended canonical decision: accept ADR-0007 and the TASK-005 supplement; permit one exact descriptor-verified `storage` directory only beside the completed canonical state, retain every bootstrap/recovery denial, and bound enumeration before collecting an unexpected fourth candidate.
Reason: changing the default causes downstream configuration drift, while a permissive namespace would weaken completed filesystem authority.
Impact: TASK-005 depended on TASK-004 and implemented the narrow platform/store compatibility change with retained regression tests. Commits `88e7b3413db5607651f2c842f6d0c1f03d513968` and `f516faafe50707b88f51f25c03be07f917f8943f` passed reviewed formal run `33073580258`; TASK-004 remains DONE and TASK-005 completion grants no later authority.
Classification: CONFLICT plus canonical TASK-005 dependency/scope `SPEC_STALE`; the implementation gap is now closed and verified.
Status: RESOLVED / BASE-018 / ADR-0007 / IMPLEMENTED / VERIFIED / SPECIFICATION v1.1.20
```

### `BASELINE-007` TASK-005 completion-state synchronization residue

```text
CONFLICT:
Source A: Specification v1.1.19 §0.4 and the historical CONFLICT-004 disposition still described TASK-005 as awaiting reviewed CI or as an implementation EXPECTED_GAP.
Source B: commits 88e7b3413db5607651f2c842f6d0c1f03d513968 and f516faafe50707b88f51f25c03be07f917f8943f passed reviewed macos-26 formal CI run 33073580258; commit 13a9cc9 recorded TASK-005 DONE and authority NONE.
Recommended canonical decision: update only current-state and resolved-disposition prose to the verified completed state while preserving explicitly historical pre-start and pre-CI records.
Reason: current-state residue can make a later Codex reopen completed work or misread implementation authority, while rewriting historical records would destroy evidence chronology.
Impact: Specification v1.1.20, Review v1.1.30, Plan v0.3.30 and Intake v1.3.25 synchronize current facts; TASK-005 evidence, contract and behavior do not change.
Classification: SPEC_STALE
Status: RESOLVED
```

### `REVIEW-CONFLICT-008` TASK-013 accepted dependency and AC ownership synchronization

```text
CONFLICT:
Source A: Specification v1.1.19 TASK-013 omitted TASK-007 and the accepted AC-024/AC-026/AC-028 ownership details.
Source B: the accepted TASK-003 supplement and Plan require TASK-013 to depend on TASK-007, TASK-009 and TASK-012, own terminal AC-028, and implement only the named privileged-dispatch denial contribution without claiming AC-029.
Recommended canonical decision: synchronize the Specification TASK-013 body exactly to the already accepted supplement and Plan mapping.
Reason: TASK-007 supplies CommandRecord/principal binding required for honest AC-028 completion; this is an accepted dependency correction, not new implementation authority.
Impact: future TASK-013 start-gate analysis receives the correct prerequisite and acceptance set; no task starts and no architecture changes.
Classification: SPEC_STALE
Status: RESOLVED
```

### `REVIEW-CONFLICT-009` TASK-010 Admin gate synchronization

```text
CONFLICT:
Source A: Specification v1.1.19 TASK-010 listed only TASK-001 and TASK-002 as dependencies.
Source B: Specification OQ-010, the minimum Plugin Admin operation registry and Plan require accepted Admin evidence before install, approve, activate or revoke privileged flows.
Recommended canonical decision: add the scoped OQ-010 dependency to the TASK-010 body without claiming that non-executing package inspection itself proves Admin authority.
Reason: the task as planned contains privileged mutations, but a broader statement would incorrectly block evidence-only inspection semantics.
Impact: TASK-010 remains BLOCKED; no Admin endpoint or implementation is authorized.
Classification: SPEC_STALE
Status: RESOLVED
```

### `REVIEW-CONFLICT-010` OQ-005 Provider-selection gate ownership

```text
CONFLICT:
Source A: Plan v0.3.29 and Decisions v0.3.19 listed OQ-005 as a prerequisite blocking TASK-017.
Source B: Specification TASK-017 exists to choose the concrete validation Providers and accept their ADRs, while OQ-005 explicitly blocks TASK-018 implementation rather than the selection task itself.
Recommended canonical decision: TASK-017 depends on TASK-016 and closes OQ-005 through accepted Provider-selection ADRs; OQ-005 blocks TASK-018 through TASK-020 implementation until then.
Reason: making a decision an input to the task that owns that decision creates a circular gate.
Impact: TASK-017 remains currently BLOCKED by TASK-016, but its future entry gate is no longer circular; no Provider is selected by this correction.
Classification: CONFLICT
Status: RESOLVED
```

### `REVIEW-CONFLICT-011` TASK-005 completion AC evidence mapping

```text
CONFLICT:
Source A: Plan v0.3.30 marked AC-075 through AC-079 PASS but attached the source, capacity, publish, backend-identity and cancellation explanations to the wrong AC numbers.
Source B: Specification §19.8 and the accepted TASK-005 supplement §12 define AC-075 as descriptor authority, AC-076 as stable non-destructive source, AC-077 as bounded verified streaming/control, AC-078 as finite admission/capacity and AC-079 as durable no-clobber publish including Location identity.
Recommended canonical decision: retain every PASS result and executable TEST record, but realign each completion explanation and named evidence to its canonical AC definition.
Reason: the tests passed, but an incorrect evidence-to-acceptance explanation can mislead later audits and downstream contract work.
Impact: Plan v0.3.31 corrects documentation only; no test outcome, implementation behavior or completed-task lifecycle changes.
Classification: SPEC_STALE
Status: RESOLVED
```

### `BASELINE-008` completed-foundation current-state residue

```text
CONFLICT:
Source A: Intake v1.3.25 still classified TASK-004 as active slices with formal supply-chain PASS pending, and Review v1.1.30's current finding-disposition row omitted completed TASK-005; Plan also abbreviated TASK-010's OQ-010 scope to install/approve.
Source B: reviewed run 32695815747 proves TASK-004 DONE, reviewed run 33073580258 proves TASK-005 DONE, and the canonical TASK-010 gate covers install, approve, activate and revoke privileged flows.
Recommended canonical decision: synchronize only current-state/disposition and gate-scope prose while preserving historical pre-completion records.
Reason: these are stale summaries that can cause a later reviewer to reopen completed work or understate an Admin gate.
Impact: Review v1.1.31, Plan v0.3.31 and Intake v1.3.26 become consistent with Specification v1.1.21; no later task is authorized.
Classification: SPEC_STALE
Status: RESOLVED
```

### `REVIEW-CONFLICT-012` TASK-003 endpoint pre-publication crash recovery

```text
CONFLICT:
Source A: Specification AC-064 and TEST-ENDPOINT-003 require deterministic endpoint collision/recovery lifecycle, including crash recovery; the accepted TASK-003 endpoint contract requires cleanup to revalidate the exact socket edge before removal.
Source B: runtime endpoint publication bound the final pathname before applying mode 0600, while stale recovery and the ModeSocket failure cleanup accepted only an already-0600 socket. A crash or injected failure in that window could therefore leave an owned socket that the next daemon start refused to recover.
Recommended canonical decision: retain final publication mode 0600 and the 0700 owner-only runtime directory, but allow recovery inspection of an unpublished socket edge to accept any mode only when the exact runtime authority, owner UID, socket type, empty ACL, identity stability and refused-connect proof all validate. Capture and compare the just-bound inode identity for in-process rollback; keep ordinary client validation strict at mode 0600.
Reason: process-global temporary umask mutation is racy in a multithreaded daemon. The bounded unpublished-edge rule closes recovery without weakening the published endpoint contract or claiming cross-UID reachability through the 0700 parent.
Impact: runtime endpoint recovery and TEST-ENDPOINT-003 evidence are corrected; AC-062 peer-UID authentication is unchanged.
Classification: REPO_STALE
Status: RESOLVED / implementation and regression evidence required in this correction set
```

### `REVIEW-CONFLICT-013` TASK-007 protocol intent drift

```text
CONFLICT:
Source A: accepted TASK-007 proposal v0.1.3 assigns HANDSHAKE_ONLY=0 and SINGLE_COMMAND=1, requires the legacy handshake client to send HANDSHAKE_ONLY, and requires the legacy handshake server to reject command intent.
Source B: the current proto assigned an extra UNSPECIFIED=0 value and shifted the accepted values to 1/2; request_handshake sent UNSPECIFIED and serve_handshake did not validate intent.
Recommended canonical decision: return the schema and both legacy protocol helpers to the accepted proposal values and validation behavior; regenerate the checked-in descriptor and provenance evidence.
Reason: the proposal is accepted authority and there is no accepted ADR or specification change authorizing the drift.
Impact: TASK-007 protocol implementation and tests change; existing TASK-003 wire fields remain backward-compatible because omitted proto3 intent decodes as HANDSHAKE_ONLY=0.
Classification: CONFLICT / implementation side REPO_STALE
Status: RESOLVED / implementation and regression evidence required in this correction set
```

### `REVIEW-CONFLICT-014` retained TASK-003 evidence and CLI gate regression

```text
CONFLICT:
Source A: TASK-003 is DONE and its retained TEST-PROTO-001, TEST-HANDSHAKE-001 and CLI evidence must continue proving the accepted 1.0 contract independently of later extensions.
Source B: TASK-007 replaced the TASK-003 full-file proto/descriptor hashes with current extended hashes, and the retained daemon help assertion required the two legacy options to remain adjacent, so insertion of --blob-root caused the formal TASK-003 gate to fail even though both legacy options remained present.
Recommended canonical decision: make TASK-003 evidence assert the stable legacy schema/wire surface and legacy CLI tokens rather than the mutable full extended artifact or option adjacency; move exact current-artifact provenance ownership to TASK-007.
Reason: a completed task's evidence must remain meaningful without forbidding compatible extension, and the formal dependency chain must detect behavioral regressions rather than harmless help ordering.
Impact: retained tests and scripts change; no TASK-003 runtime capability is reopened or expanded.
Classification: REPO_STALE plus evidence-ownership CONFLICT
Status: RESOLVED / regression evidence required in this correction set
```

### `REVIEW-CONFLICT-015` cross-kind core ID byte reuse

```text
CONFLICT:
Source A: Specification §1.1 states that core IDs MUST NOT reuse the same raw bytes across object kinds, and AC-084 requires Asset/Revision/Representation/Resource identity invariants.
Source B: AssetGraph registration validated relationships and same-kind duplication but did not reject an AssetId, RevisionId, RepresentationId and ResourceId constructed from the same raw bytes.
Recommended canonical decision: validate raw-byte pairwise uniqueness for every object ID introduced by one managed registration before mutating the graph, and retain a regression test using equal bytes under different ID types.
Reason: strong Rust wrapper types prevent accidental type interchange but do not themselves enforce the normative cross-object byte-uniqueness invariant.
Impact: invalid registrations fail with validation error; valid persisted representations and schemas do not change.
Classification: REPO_STALE
Status: RESOLVED / implementation and regression evidence required in this correction set
```

### `REVIEW-GAP-004` orphan requirement ownership

```text
CONFLICT:
Source A: OPS-001 through OPS-003, API-011, PERF-002 and SEC-008 are normative V1 requirements.
Source B: the task plan did not name an implementation owner for those requirements, allowing them to remain outside task start/completion gates.
Recommended canonical decision: assign OPS-001 through OPS-003 and SEC-008 to TASK-013 audit/observability foundation, API-011 to TASK-008 query/list foundations and PERF-002 to TASK-023 release verification; every owning task must add stable acceptance/test evidence before start.
Reason: explicit ownership prevents normative requirements from becoming release-time surprises while leaving their concrete metrics and still-OPEN policy choices to the owning gate.
Impact: future task scope and prerequisite documentation are synchronized; this record does not start those tasks or invent OPEN thresholds.
Classification: EXPECTED_GAP
Status: RESOLVED / planning ownership only
```

### `REVIEW-CONFLICT-016` TASK-007 handshake compatibility and error fidelity

```text
CONFLICT:
Source A: accepted TASK-007 proposal v0.1.4 preserves the existing HANDSHAKE_ONLY
          negotiation predicate, requires only SINGLE_COMMAND to use an exact 1.1
          range, and retains the protocol-1.0 ID_GENERATION_UNAVAILABLE envelope.
Source B: the daemon dispatcher accepted HANDSHAKE_ONLY only when min=max=0, while
          the retained predicate accepts min=0 with max>=0; correlation-ID failure
          closed the 1.1 handshake without its retained error envelope, and the 1.1
          client coerced every handshake error envelope to version unsupported.
Recommended canonical decision: share the retained handshake rejection envelope,
          restore the legacy min<=max/min=0 predicate in the daemon dispatcher,
          keep exact 1.1 for SINGLE_COMMAND, and validate/preserve the received
          protocol-1.0 handshake error code on the 1.1 client path.
Reason: additive protocol 1.1 may not narrow a valid completed 1.0 negotiation or
        lose the accepted stable error classification before an operation starts.
Impact: core-proto session implementation and compatibility/error regressions only;
        descriptor bytes, operation semantics and protocol versions do not change.
Classification: REPO_STALE
Status: RESOLVED / implementation and regression evidence required in this review set
```

### `REVIEW-CONFLICT-017` Library-config post-read authority revalidation

```text
CONFLICT:
Source A: accepted TASK-007 proposal v0.1.4 requires the retained config file and
          selected parent edge to be rechecked after bounded positional reading;
          any ownership, type, link-count, mode, ACL, identity or metadata mismatch
          must fail before bytes enter the resolver.
Source B: the reader rechecked directory components and final-edge inode identity,
          but its opened-file post-read comparison covered only device/inode/size/
          mtime and did not revalidate owner, type, link count, mode, ACL or ctime.
Recommended canonical decision: apply the complete owner-only regular-file policy
          to the opened descriptor before and after reading and to the freshly
          reopened final edge, comparing identity, size, mtime and ctime snapshots.
Reason: the authority proof must cover the state at return, not only the state seen
        before reading, and chmod/link/metadata races must fail closed.
Impact: platform config reader and deterministic post-read mutation evidence only;
        configuration syntax, precedence and accepted stable files do not change.
Classification: REPO_STALE
Status: RESOLVED / implementation and regression evidence required in this review set
```

### `REVIEW-CONFLICT-018` product-session permit lifetime

```text
CONFLICT:
Source A: accepted TASK-007 proposal v0.1.4 bounds resident product sessions from
          the atomic handshake-to-session transfer through the sole terminal
          response write/flush/close step.
Source B: the daemon explicitly dropped its client-session semaphore permit after
          joined application completion but before constructing and writing the
          terminal CoreResponse.
Recommended canonical decision: retain the owned session permit until the response
          write/close attempt completes, including the bounded transport-loss path.
Reason: releasing it early makes response-owning product sessions fall outside the
        configured residency cap and weakens the lifecycle invariant used by
        admission and shutdown reasoning.
Impact: daemon permit lifetime and lifecycle regression evidence only; no capacity,
        retry, persistence or wire contract changes.
Classification: REPO_STALE
Status: RESOLVED / implementation and regression evidence required in this review set
```

### `REVIEW-CONFLICT-019` retained test-fixture namespace reproducibility

```text
CONFLICT:
Source A: Plan §7 requires completed-task evidence to remain reproducible on the supported platform, including a long-lived developer host.
Source B: TASK-003 HOME endpoint fixtures and TASK-004 path fixtures used only process ID plus a process-local counter; panic or SIGKILL could preserve a directory, and a later reused PID/counter failed at creation with AlreadyExists before exercising the implementation.
Recommended canonical decision: retain the exact real owner-only path/ACL and crash semantics, but allocate each test-run namespace with a run nonce plus bounded exclusive-create collision retries; use RAII cleanup for normal/panic paths. A crash harness may remove only directories whose ownership it can prove; any retained SIGKILL residue must remain harmless to later runs.
Reason: stale test evidence must not create a false product regression, while cleanup must never guess ownership or weaken the filesystem authority test.
Impact: test-only fixture allocation/cleanup and regressions; no production endpoint, Library path, ACL, recovery or public behavior changes.
Classification: REPO_STALE
Status: IMPLEMENTED / TWO CONSECUTIVE TASK-003 GATES PASS / TASK-007 DEVELOPER AND LOCAL FORMAL PASS / REVIEWED CI PENDING
```

### `REVIEW-CONFLICT-020` seven-object managed-completion ID uniqueness

```text
CONFLICT:
Source A: Specification §1.1 and the accepted TASK-006/TASK-007 contracts reject cross-kind raw-ID reuse for every object introduced by one managed registration.
Source B: the prior REVIEW-CONFLICT-015 correction checked Asset, AssetRevision, Representation and Resource only, while TASK-007 also generates Location, DomainEvent and ProvenanceEvent IDs for the same completion.
Recommended canonical decision: validate pairwise uniqueness across all seven generated IDs before constructing or accepting ExternalIngestCompletion, retaining the existing four-graph-ID domain check as defense in depth.
Reason: UUIDv7 collision probability does not waive a normative invariant or deterministic injected-source test.
Impact: duplicate generated IDs become ID_GENERATION_UNAVAILABLE before registration; forged/bypassing completion values fail validation before transaction mutation. Valid results, schemas and migrations do not change.
Classification: REPO_STALE
Status: IMPLEMENTED / TARGETED AND COMPLETE PACKAGE TESTS PASS / TASK-007 DEVELOPER AND LOCAL FORMAL PASS / REVIEWED CI PENDING
```

### `REVIEW-CONFLICT-021` complete Asset operation ownership

```text
CONFLICT:
Source A: FUNC-003, REQ-013 and the minimum operation registry require ingested Assets to be inspectable, listable and materializable, with CreateAssetRevision/RetireAsset/RestoreAsset also reachable through semantic product operations.
Source B: completed TASK-006 supplied persistence only, TASK-007 supplied ingest only, and the prior TASK-008/TASK-009 bodies did not own the remaining product operations.
Recommended canonical decision: TASK-008 owns InspectAsset/ListAssets/MaterializeAsset with API-010/API-011, destination-authority and cleanup gates; TASK-009 owns CreateAssetRevision/RetireAsset/RestoreAsset while consuming the TASK-006 persistence foundation.
Reason: the CLI-only V1 must be able to observe and retrieve what it ingests without retroactively expanding TASK-007.
Impact: future task scope and pre-start contracts are corrected; no TASK-008/009 code is authorized by this planning correction.
Classification: CONFLICT / SPEC_STALE; current code absence is EXPECTED_GAP
Status: RESOLVED / PLANNING OWNERSHIP ONLY
```

### `REVIEW-CONFLICT-022` Core observability foundation ownership

```text
CONFLICT:
Source A: OPS-001/OPS-002 are P0 all-runtime requirements, OPS-003 applies to the metric schema, and TASK-008 owns distinct health semantics through OPS-004.
Source B: REVIEW-GAP-004 assigned OPS-001 through OPS-003 only to TASK-013, whose Plugin/Admin dependencies would leave TASK-008 and TASK-009 runtime paths without the common structured/redacted foundation.
Recommended canonical decision: TASK-008 owns the Core structured-log, redaction, bounded-label and health baseline; TASK-013 contributes Plugin/Broker/audit-specific fields and retains SEC-008 policy ownership.
Reason: observability and content-derived authority are different concerns; moving SEC-008 earlier would be an incorrect conflation, while delaying all Core telemetry creates avoidable retrofit work.
Impact: future task ownership only; no log payload, metric backend, threshold or Admin surface is invented here.
Classification: SPEC_STALE
Status: RESOLVED / SUPERSEDES REVIEW-GAP-004 FOR OPS-001..OPS-003 ONLY
```

### `REVIEW-CONFLICT-023` layered non-recursive CI evidence

```text
CONFLICT:
Source A: completed task evidence must remain reproducible and code-bearing formal
          candidates must retain every owned stable mapping, platform check and
          formal-only fault/stress/scaling obligation.
Source B: unrestricted push plus pull-request triggers can duplicate one commit,
          while TASK-007's recursive predecessor graph repeats workspace, document,
          naming and supply work and applies the same formal cost to docs-only edits.
Recommended canonical decision: accept ADR-0010's fail-closed docs/developer/formal
          trigger matrix and one repository baseline plus one component run per task;
          restrict docs-only classification to AGENTS.md, docs/spec and
          docs/proposals; treat machine-consumed docs/provenance and unknown future
          subtrees as code; preserve standalone task defaults and the separate
          second-UID formal job.
Reason: runtime evidence should be proportional to changed risk, and a future task
        must not increase CI time by recursively replaying every prior aggregate.
Impact: workflow, verification scripts, orchestration tests and synchronized docs
        only; no stable mapping, product behavior, migration or later-task authority.
Classification: REPO_STALE / CONFLICT
Status: IMPLEMENTED / LOCAL DOCS-DEVELOPER-FORMAL PASS / REVIEWED CI PENDING / ADR-0010
```

### `REVIEW-GAP-005` extensible durable command outcomes

```text
CONFLICT:
Source A: immutable migration 0001 intentionally permits only ASSET, ASSET_REVISION and LOCATION result kinds for its completed Asset persistence scope.
Source B: TASK-009 and later commands require additional durable replayable result shapes, and SQLite cannot widen the existing CHECK constraint in place.
Recommended canonical decision: make an accepted forward-only extensible outcome design a TASK-009 pre-start gate; preserve 0001 bytes and prove existing outcome/event/FK replay through the migration. Prefer one stable extensibility mechanism over per-result-kind table rebuilds.
Reason: this is expected schema evolution, not permission to rewrite an applied migration or discover the strategy while implementing 0002.
Impact: TASK-009 proposal/migration tests only; completed TASK-006/007 rows and behavior remain valid.
Classification: EXPECTED_GAP
Status: OPEN / BLOCKS TASK-009 START, NOT TASK-008
```

### `BASELINE-001` Git repository 初始化

```text
CONFLICT:
Source A: Implementation Specification v1.0.1 记录当前工作区没有 Git 元数据。
Source B: 2026-08-20 已按用户要求初始化本地 Git repository，默认分支为 main。
Recommended canonical decision: 更新仓库现状描述；不改变任何目标架构或 CONFIRMED 决策。
Reason: 这是完成 repository bootstrap 后产生的正常状态变化。
Impact: 规范中的仓库现状改为“Git 已初始化，源代码结构尚未初始化”。
Classification: SPEC_STALE
Status: RESOLVED
```

### `BASELINE-002` 文档基线 commit history

```text
CONFLICT:
Source A: PROJECT_INTAKE_REPORT v1.2.1 still stated that the initialized repository had no commits.
Source B: Git history contains the reviewed documentation baseline commit created on 2026-08-21.
Recommended canonical decision: refresh only the Current State evidence; keep TASK-001 as the first implementation/CI bootstrap task.
Reason: a documentation commit does not mean source, tests or CI exist, but “no commits” is no longer factual.
Impact: PROJECT_INTAKE_REPORT v1.2.2 records commit history without changing target architecture or implementation readiness.
Classification: SPEC_STALE
Status: RESOLVED
```

### `BASELINE-003` TASK-001 后仓库现状

```text
CONFLICT:
Source A: AGENTS.md、Implementation Specification/Review、Implementation Plan Current State 与 PROJECT_INTAKE_REPORT 仍把仓库描述为仅有文档、没有 Cargo workspace、测试或 CI，且把 TASK-001 记为下一步。
Source B: 当前工作区已存在 TASK-001 的 Cargo workspace、17 个 canonical package/binary skeleton、repository verification tests/scripts、CI、Cargo.lock 与 policy 配置；Implementation Plan 已将 TASK-001 标记为 DONE。
Recommended canonical decision: 将所有 Current State 证据同步为“TASK-001 bootstrap 已实现并验证；尚无 TASK-002 domain behavior、schema、migration 或产品能力”，保留后续 task gate，不把空骨架误报为功能实现。
Reason: TASK-001 完成是正常仓库状态变化；陈旧入口会使后续 Codex 重复 bootstrap 或错误判断 task authorization。
Impact: 同步更新 AGENTS.md、Specification v1.1.5、Review v1.1.5、Plan v0.3.5 与 Intake Report v1.3.0；TASK-002 仍为 PENDING，开始前必须建立其稳定 AC/TEST registry 与 start record。
Classification: SPEC_STALE
Status: RESOLVED
```

### `BASELINE-004` TASK-004 本地门禁后 canonical current-state 与 future-task mapping

```text
CONFLICT:
Source A: Specification v1.1.11 的 Current State/Repository Map 仍写 17 个 package、无 schema/migration/Library owner-lock，并在 §18 将 TASK-007 收窄到 `AC-001..AC-006`；将跨后续 Broker/Secret task 的 `AC-024..AC-026` 放入 TASK-012 测试范围。
Source B: commit abe48db 后仓库已有第 18 个 mengxia-platform-fs package、完整 TASK-004 本地实现与 14-ID 本地 PASS；Plan 的 TASK-007 为 `AC-001..AC-009`，TASK-010 为 `AC-027`，TASK-012 为 `AC-020..AC-023`；formal reviewed CI attestation 仍未产生。
Recommended canonical decision: 只同步可观察的 Current State、repository map 与既有 AC 语义映射；TASK-004 保持 IN_PROGRESS，AC-065/TEST-SUPPLY-004/final DONE 继续等待正式 reviewed CI evidence；不改变任何生产实现、迁移字节、依赖顺序、权限边界或后续 task authorization。
Reason: 陈旧 canonical facts 会误导后续实现，而本地证据不能替代 accepted formal CI boundary。
Impact: Specification v1.1.12、Review v1.1.22、Plan v0.3.22、Intake v1.3.17、accepted supplement/ADR references 与 document traceability 同步；实现行为不变。
Classification: SPEC_STALE
Status: RESOLVED
```

### `BASELINE-005` TASK-004 formal CI Xcode distribution provenance

```text
CONFLICT:
Source A: accepted manifest/preflight pinned clang and libtool bytes observed from the local Mac App Store Xcode 26.6 installation and treated them as the formal CI tuple.
Source B: reviewed `macos-26-arm64` image `20260728.0273.1` installs the same Xcode 26.6 build 17F113 from runner-images `Xcode_26.6_Universal` XIP, but its clang/libtool bytes have different stable SHA-256 values; the original CI stopped fail-closed before Cargo.
Recommended canonical decision: retain separate evidence classes; keep default local builds non-attested, and pin only the formal attested profile to the reviewed runner XIP tool bytes. Emit observed non-secret tuple/digests before comparison without accepting fallback values.
Reason: distribution-equivalent version banners do not prove byte identity. Formal evidence must be established on its actual CI distribution, while local development must remain usable without pretending to provide release attestation.
Impact: Specification v1.1.13, ADR-0006 clarification, accepted supplement, provenance manifest, preflight/build constants and supply/document tests synchronize. Runtime behavior, migration bytes, public API and downstream authorization are unchanged.
Classification: CONFLICT
Status: RESOLVED
```

### `BASELINE-006` TASK-004 formal completion evidence

```text
CONFLICT:
Source A: canonical current-state records kept TASK-004 IN_PROGRESS while awaiting corrected reviewed CI attestation.
Source B: commit bfcb151 and reviewed GitHub Actions run 32695815747 passed the exact runner-XIP preflight, retained TASK-001 baseline and all fourteen TASK-004 gates.
Recommended canonical decision: mark TASK-004 DONE with per-TEST, per-AC and applicable security PASS evidence; keep TASK-003 PENDING until its separate stable registry/start gate is reviewed.
Reason: the final external evidence requirement is satisfied, but task completion never grants implicit authority to a dependent task.
Impact: Specification v1.1.14、Review v1.1.24、Plan v0.3.24、Intake v1.3.19、AGENTS current state and document traceability synchronize. No runtime behavior, migration, public API or later implementation changes.
Classification: SPEC_STALE
Status: RESOLVED
```

### `REVIEW-GAP-003` TASK-002 public contract and stable start registry

```text
CONFLICT:
Source A: Specification v1.1.5 and Plan v0.3.5 identified TASK-002's broad value/error goal but did not define stable AC/TEST IDs, exact public signatures, fallible ID generation, canonical parser boundaries or the minimal dependency feature set required before implementation.
Source B: The user-reviewed TASK-002 start-gate proposal fixes those contracts, adds the five error taxonomy rows already required by canonical operation prose or the accepted value boundaries, and supplies a complete start record and verification matrix.
Recommended canonical decision: incorporate the accepted proposal into Specification v1.1.6 and Plan v0.3.6, authorize TASK-002 only, and retain every TASK-003+ gate.
Reason: this closes the mandatory pre-start traceability/public-contract gap without changing an accepted authority, persistence, Provider or destructive-operation architecture decision.
Impact: AC-055 through AC-059 and seven TASK-002 TEST obligations become canonical; exact uuid/getrandom/time/proptest pins and features are accepted for this task; synchronized Current State documents mark TASK-002 IN_PROGRESS only after TEST-DOC-001 and the TASK-001 baseline pass.
Classification: EXPECTED_GAP
Resolution evidence: user accepted the modified `docs/proposals/TASK-002-GATE-PROPOSAL.md` on 2026-08-21; official locked crate source/metadata confirmed the specified Builder/fallible entropy APIs, licenses and MSRVs.
Status: RESOLVED
```

### `REVIEW-CONFLICT-001` Caller-supplied actor 与 channel-bound identity

```text
CONFLICT:
Source A: Implementation Specification v1.0.1 CommandEnvelope 允许 caller 填写 actor_principal。
Source B: 同一规范 SEC-005/§12.3 要求 authority domain 与 channel/process-bound identity，且不信任 self-report。
Recommended canonical decision: reserve wire field 3；Core 仅从 authenticated channel 派生 PrincipalContext。
Reason: caller field 无法作为认证或审计证据。
Impact: TASK-003 增加 actor spoof/peer/Admin negative tests；Admin 未解决 OQ-010 时禁用。
Classification: CONFLICT
Status: RESOLVED / BASE-008
```

### `REVIEW-CONFLICT-002` 初始 ingest mode 与 Managed custody

```text
CONFLICT:
Source A: TASK-007 列出 copy/adopt/reference，但未定义 destructive 与 custody 语义。
Source B: DATA-002/DATA-003 要求 Managed Asset 有 verified durable user-controlled Location 且 physical durability 先于注册。
Recommended canonical decision: initial vertical slice 仅 copy；reference 明确为 UNMANAGED，adopt 需独立 destructive contract。
Reason: 防止源文件移动/删除或非 durable reference 被误报为 Managed。
Impact: TASK-007 与 AC-009 修订；未来新增 mode 需 ADR/contract。
Classification: CONFLICT
Status: RESOLVED / BASE-009
```

### `REVIEW-CONFLICT-003` Migration ownership

```text
CONFLICT:
Source A: TASK-004 创建 0001_library_assets.sql；TASK-006 后续仍需修改它。
Source B: DATA-007 与 §22.1 禁止修改已合并 migration bytes。
Recommended canonical decision: TASK-004 只拥有 0000_store_bootstrap；TASK-006 一次性拥有完整 0001_library_assets。
Reason: 保证 checksum 与 upgrade fixture 可重现。
Impact: migration 序号与 task ownership 已修订。
Classification: CONFLICT
Status: RESOLVED
```

### `REVIEW-GAP-001` Phase 0 blocking decisions

```text
CONFLICT:
Source A: IMPLEMENTATION_PLAN v0.1.0 可被理解为 Phase 0 后直接初始化 Cargo workspace。
Source B: OQ-003/OQ-006 阻塞 TASK-001/TASK-003/TASK-004，OQ-010 阻塞 Admin authority；SQLite 官方记录当前 WAL-reset fixed-version requirement。
Recommended canonical decision: Phase 0 blocks implementation until the current task's dependent OQ/Review gates close. ADR-0003..ADR-0005 now allow TASK-001 to begin while later Admin, third-party Native Plugin, Credential, Provider and destructive capabilities remain fail-closed at their own gates.
Reason: Codex may not guess security versions, identity mechanisms or resource limits; accepted foundation evidence is sufficient only for TASK-001..TASK-005.
Impact: Phase 0 foundation scope is complete without prematurely closing OQ-002/OQ-004/OQ-005/OQ-008/OQ-009/OQ-010 or later OQ-006 portions.
Classification: EXPECTED_GAP
Resolution evidence: user accepted the recommended foundation boundary on 2026-08-21; ADR-0003..ADR-0005 record exact evidence and behavior.
Status: RESOLVED
```

### `REVIEW-CONFLICT-004` First-create Library bootstrap vs disabled Admin

```text
CONFLICT:
Source A: ADR-0004 disables every Admin-sensitive operation until OQ-010 closes.
Source B: TASK-004 and InitializeLibrary require a path to create LibraryMeta/owner state and apply deterministic startup migrations before an authenticated Client channel exists.
Recommended canonical decision: first-create bootstrap is a one-shot internal daemon lifecycle path authorized by the daemon effective UID plus strict local parent/target checks; deterministic checksummed forward migrations are internal startup lifecycle. Neither is an ordinary Client/Admin RPC. Manual/destructive migration administration remains disabled.
Reason: avoids a bootstrap-authentication cycle without weakening the Admin boundary or exposing DB access to CLI.
Impact: Specification v1.1.2, ADR-0004 and TASK-004 define target denial tests and fail-closed behavior.
Classification: CONFLICT
Status: RESOLVED
```

### `REVIEW-GAP-002` TASK-001 stable acceptance/test IDs

```text
CONFLICT:
Source A: Specification/Plan required a task to reference stable acceptance/test IDs before implementation.
Source B: TASK-001 used only natural-language “workspace builds”, “architecture test” and “smoke build” labels.
Recommended canonical decision: define immutable AC/TEST identifier rules; bind TASK-001 to AC-050 through AC-054 and its six TEST IDs; require declaration before start and executable PASS evidence before DONE.
Reason: deterministic completion cannot depend on mutable prose, but requiring commands before TASK-001 creates them would be circular.
Impact: TASK-001 remains next; its obligations are stable at start and its repository checks must exist and pass before completion.
Classification: EXPECTED_GAP
Resolution evidence: `REVIEW-016`; Specification v1.1.4 / Review v1.1.4 / Plan v0.3.4.
Status: RESOLVED
```

### `REVIEW-CONFLICT-005` Accepted foundation caps missing from configuration model

```text
CONFLICT:
Source A: ADR-0005 accepts finite configurable decode, DB, ingest, staging and free-space boundaries; Specification §17 forbids handler/adapter magic constants.
Source B: Specification v1.1.3 §16 omitted several accepted keys and did not state which values could only tighten.
Recommended canonical decision: expose every configurable ADR-0005 boundary as a typed key and make maximum/reserve widening impossible without a new decision.
Reason: otherwise implementers must hard-code or invent configuration/range behavior.
Impact: §16 and ADR-0005 now define matching defaults, ranges and tightening-only semantics; invalid combinations disable the dependent operation.
Classification: CONFLICT
Resolution evidence: `REVIEW-017`; Specification v1.1.4 and clarified ADR-0005.
Status: RESOLVED
```

### `REVIEW-CONFLICT-006` Empty bootstrap target acceptance

```text
CONFLICT:
Source A: ADR-0004 and Specification §12.3 allow an absent or correctly owned empty first-create target.
Source B: TASK-004 and ADR-0004 verification said bootstrap rejects an “existing” target, which also includes the allowed empty target.
Recommended canonical decision: reject existing canonical metadata, non-empty content and unsafe ownership/mode/symlink/filesystem states, while accepting an absent or correctly owned empty target.
Reason: target existence alone is not the authority/integrity risk; ambiguous wording would produce incompatible bootstrap implementations.
Impact: TASK-004 and ADR-0004 verification now use the same denial matrix.
Classification: CONFLICT
Resolution evidence: `REVIEW-018`; Specification v1.1.4 and clarified ADR-0004.
Status: RESOLVED
```

### `REVIEW-CONFLICT-007` Whole-V1 verdict vs scoped TASK-001 authorization

```text
CONFLICT:
Source A: the mandated final review contract restricts functional/security/Codex verdicts to exact whole-product enum values and requires NOT READY FOR CODEX while blockers remain.
Source B: Review v1.1.3 placed custom foundation/task-scoped READY values in those rows although later V1 features remained BLOCKED.
Recommended canonical decision: report whole-V1 as FUNCTIONAL/SECURITY CONDITIONALLY READY and CODEX NOT READY FOR CODEX; report READY FOR TASK-001 separately as the current authorized slice.
Reason: readiness scope must neither bypass later gates nor prevent safe foundation progress.
Impact: Review v1.1.4 and Plan v0.3.4 distinguish the overall verdict from task authorization; all later gates remain unchanged.
Classification: CONFLICT
Resolution evidence: `REVIEW-019`.
Status: RESOLVED
```

## TASK-003 gate acceptance — 2026-08-25

The user accepted `docs/proposals/TASK-003-GATE-PROPOSAL.md` as the normative
TASK-003 supplement. Its exact wire/framing/decode-depth contract, 5-second deadline,
32-handshake admission cap, opaque opened-Library seam, owner-only marked runtime
namespace, two-command CLI, descriptor-first offline generation policy, formal
second-UID completion evidence and five-AC/eleven-TEST registry are binding. TASK-003
alone is `IN_PROGRESS`; no later task is authorized.

The accepted error conflict preserves existing storage and Provider/Plugin meanings
and adds three local IPC codes with the exact Specification §14.1 rows:

TASK003_ERROR_TAXONOMY_CONFLICT: ACCEPTED
TASK003_ERROR_CODES_ADDED: IPC_TRANSPORT_ERROR; PROTOCOL_VERSION_UNSUPPORTED; DEADLINE_EXCEEDED
TASK003_STORAGE_IO_SOURCE_PRESERVED: filesystem/backend
TASK003_UNSUPPORTED_CAPABILITY_SOURCE_PRESERVED: declared Provider/Plugin capability contract

The accepted cross-task AC correction records contributor evidence without permitting
premature completion or implementation:

TASK003_AC_OWNERSHIP_CONFLICT: ACCEPTED
TASK003_AC_028_CONTRIBUTORS: TASK-003; TASK-007
TASK003_AC_028_TERMINAL_OWNER: TASK-013
TASK003_AC_029_CONTRIBUTORS: TASK-003; TASK-013; TASK-016; TASK-022
TASK003_AC_029_TASK013_BRANCHES: PLUGIN_GRANT; AUDIT_EXPORT; MANUAL_MIGRATION_ADMIN
TASK003_AC_029_TASK016_BRANCHES: CREDENTIAL
TASK003_AC_029_TASK022_BRANCHES: PURGE
TASK003_AC_029_TERMINAL_OWNER: TASK-023

TASK003_CANONICAL_GATE: ACCEPTED
TASK003_SPECIFICATION_VERSION: 1.1.17
TASK003_LIFECYCLE: DONE
TASK003_PROPOSAL: docs/proposals/TASK-003-GATE-PROPOSAL.md

### TASK-003 formal executable preflight path — 2026-08-26

```text
CONFLICT:
Source A: the accepted TASK-003 supplement required absolute /usr/bin/test for the formal second-UID executable preflight.
Source B: reviewed macOS 26 CI run 32912547078 and the accepted developer host both report /usr/bin/test absent and /bin/test present.
Recommended canonical decision: use absolute /bin/test -x for that private formal preflight only.
Reason: preserve the exact no-PATH, cleared-environment executable check with the platform's real utility path.
Impact: no production behavior, authority boundary, public interface, dependency, AC/TEST ownership or later-task authorization changes.
Classification: SPEC_STALE
Status: RESOLVED / SPECIFICATION v1.1.16
```

### TASK-003 completion evidence — 2026-08-26

- Accepted result: TASK-003 is `DONE` under Specification v1.1.17 without changing
  the previously accepted wire, identity, filesystem authority, configuration,
  dependency or later-task boundaries.
- Evidence: implementation commit
  `4f7bf27855b05c5080790aae3221ee10ae662431`; reviewed GitHub Actions run
  `32914222948`; formal job `task-003-second-uid`; runner `macos-26`; result `PASS`.
- Authority consequence: this closes TASK-003 only. TASK-005 and every later task
  remain unauthorized until their own canonical start gates are accepted.

## TASK-005 gate acceptance — 2026-08-26

Independent review accepts `docs/proposals/TASK-005-GATE-PROPOSAL.md` and ADR-0007
as the normative pre-start TASK-005 contract. The decision fixes opaque local source
and root authority, config-authority binding, bounded control/streaming, atomic
logical plus physical remaining-byte admission, exact-case APFS namespace,
no-clobber/full-sync promotion, stable Location backend identity, cleanup-failed
runtime closure, exact error/retry mapping and fixed crash/fault registries.

The same review corrects four planning defects before implementation: TASK-005 now
depends on completed TASK-004; the default `storage` namespace compatibility is
completed-state-only and bounded; the 85-byte internal locator gives Blob roots a
937-byte absolute limit; and formal gate aggregation invokes the retained
TASK-001/002/004 baseline once through `verify-task-003.sh` rather than recursively.

Lifecycle consequence: the contract, AC-074 through AC-081 and all seventeen TEST
IDs are canonical. The later explicit activation recorded below changes TASK-005 to
`IN_PROGRESS` without changing this accepted contract. TASK-006 and later tasks
remain unauthorized.

TASK005_CANONICAL_GATE: ACCEPTED
TASK005_SPECIFICATION_VERSION: 1.1.18
TASK005_LIFECYCLE: DONE
TASK005_IMPLEMENTATION_AUTHORITY: NONE
TASK005_PROPOSAL: docs/proposals/TASK-005-GATE-PROPOSAL.md

Activation evidence: on 2026-08-27 the user explicitly authorized implementation
after review of the synchronized contract. Proposal §16.1 was copied into the Plan;
TASK-005 alone became `IN_PROGRESS`. This changed no accepted contract and granted no
TASK-006 or later authority.

Implementation evidence update — 2026-08-27: the exact TASK-005 implementation and
all seventeen executable TEST mappings now pass both the local developer gate and a
complete local `formal` candidate run on APFS, including 30 KILL points, 78 fault
seams and generated 1/10/100 GiB O(buffer) evidence. This is not the reviewed
`macos-26` CI attestation required for `DONE`; lifecycle and authority therefore
remain unchanged.

Completion evidence — 2026-08-27: implementation commit
`88e7b3413db5607651f2c842f6d0c1f03d513968` plus gate correction commit
`f516faafe50707b88f51f25c03be07f917f8943f` passed reviewed `macos-26`
GitHub Actions run `33073580258`. The earlier run `33072816350` correctly exposed a
`REPO_STALE` TASK-005 Clippy invocation that retained the attested FFI class while
Cargo set its legitimate workspace wrapper; the correction reused the established
TASK-001/004 Clippy-only boundary and did not weaken the fail-closed build script.
All seventeen TEST IDs, retained gates and both CI jobs pass. TASK-005 is `DONE`,
implementation authority was `NONE` until TASK-006 received its independent gate.

## TASK-006 gate acceptance — 2026-08-28

The corrected v0.2.1 candidate passed independent feasibility, security and
downstream-compatibility review. The user then accepted it and authorized execution.
ADR-0008 and proposal v0.2.2 fix the exact immutable 0001 bytes, normalized revision
parents, shared event allocator, typed domain/DTO/row separation, store-derived
owner principal, external-versus-pure transaction split, prior-runtime fail-closed
claim recovery, fixed result references and bounded single-writer lifecycle.

TASK006_CANONICAL_GATE: ACCEPTED
TASK006_SPECIFICATION_VERSION: 1.1.22
TASK006_LIFECYCLE: DONE
TASK006_IMPLEMENTATION_AUTHORITY: NONE
TASK006_ERROR_CODES_ADDED: OPERATION_CANCELLED
TASK006_PROPOSAL: docs/proposals/TASK-006-GATE-PROPOSAL.md

The exact start record in Plan v0.3.32 is the only authority. It grants no product
IPC, source/CAS orchestration, destructive operation, dependency change, unsafe/FFI
expansion, TASK-007 or later behavior.

Completion evidence — 2026-08-29: implementation commit
`60b6616c20d677632ca25b8b72340fc3a639db54` plus review correction commit
`10455605556984e48def16efc27fb52338109944` passed reviewed arm64 `macos-26`
GitHub Actions run `33257331689`. The earlier run `33256714550` exposed a
`REPO_STALE` retained TASK-005 lock-lifetime race: a fork-to-exec window could leave
a duplicate Blob-lock descriptor alive after logical authority dropped. The private
guard now explicitly unlocks on drop and retains a regression with a surviving
duplicate descriptor; no public interface, dependency, migration, architecture
boundary or TASK-006 contract changed. All fourteen TASK-006 TEST IDs, AC-082 through
AC-090, SEC-017/SEC-020/SEC-021, retained gates and both CI jobs pass with no required
unexecuted test. TASK-006 is `DONE`, implementation authority is `NONE`, and TASK-007
and every later task remain unauthorized.

## TASK-007 root-rebind ownership correction and gate acceptance — 2026-08-30

```text
CONFLICT:
Source A: ADR-0007 and the prior Specification edge-case row assigned verified
          copied/recreated/cross-volume Blob-root rebinding to TASK-007.
Source B: TASK-007 is the single `asset.ingest.v1` copy slice, while immutable 0001
          can replay only ASSET/ASSET_REVISION/LOCATION results and no accepted
          ordinary-Client operation can represent an authenticated multi-Location
          backend rebind.
Recommended canonical decision: TASK-007 proves same-inode rename and fails closed
          on a changed backend without rewrite. TASK-008 may verify/report only. A
          future rebind requires a separate command/result/transaction/restart gate
          after OQ-010 establishes Admin authority.
Reason: implicit startup rewriting violates DATA-009; partial or unverified rewriting
        violates DATA-002/DATA-003; inventing an Admin-equivalent ordinary operation
        violates API-009.
Impact: Specification, Plan, ADR-0007 and ADR-0009 align on fail-closed ownership;
        no migration changes and no current/future Task ID is silently assigned the
        mutation. TASK-008 remains able to add degraded verification/reporting.
Classification: CONFLICT
Status: RESOLVED / ADR-0009
```

Independent review of TASK-007 proposal v0.1.2 additionally found and v0.1.3 fixed
the real SQLite writer fatal-gate mapping, the exact protocol safe-message allowlist
and complete start-record Decision references. The user authorized correction and
TASK-007-only implementation on 2026-08-30.

TASK007_CANONICAL_GATE: ACCEPTED
TASK007_SPECIFICATION_VERSION: 1.1.25
TASK007_LIFECYCLE: DONE
TASK007_IMPLEMENTATION_AUTHORITY: NONE
TASK007_PROPOSAL: docs/proposals/TASK-007-GATE-PROPOSAL.md

Completion evidence — 2026-08-31: the exact implementation/review head
`084f8269d0e9421bf909ae7d9a44e83cae3e9a9a` passed the complete local developer
aggregate and reviewed arm64 `macos-26` GitHub Actions run `33401785647`. The formal
TASK-007 aggregate and retained real second-UID job both passed. The preceding run
`33399903641` exposed only `REPO_STALE` verification defects: TASK-007 had not reused
the accepted Clippy-only escape from the attested FFI class, and same-process listener
drop tests modeled crash recovery with a nondeterministic kernel close window. The
correction reused the established TASK-004/005/006 build boundary and real
child-process `SIGKILL` fixtures without changing production behavior or stale-socket
acceptance. All nineteen TEST IDs and `AC-001` through `AC-009` pass, required
unexecuted tests are `NONE`, TASK-007 is `DONE`, and implementation authority is
`NONE`. No root rebind, Admin or TASK-008+ authority is granted.

## ADR 索引

TASK-004 gate acceptance on 2026-08-22 resolves the remaining build-host mismatch:
formal CI retains exact source/tool/path/digest evidence, while ordinary developer
builds are explicitly non-attested. Xcode components may be owned only by root or
the recorded admin build eUID and may never be group/world writable. This is the
accepted finite build-host boundary in `ADR-0006`; runtime Library path/ACL/lock
requirements are unchanged. The accepted TASK-004 supplement and synchronized Plan
start record authorize TASK-004 alone.

TASK-004 implementation evidence additionally found that SQLite 3.53.4 accepts the
mandatory `SQLITE_TRUSTED_SCHEMA=0` compiler define but does not publish that define
through `sqlite3_compileoption_get`/`PRAGMA compile_options`. Requiring a nonexistent
registry row was `SPEC_STALE`, not an implementation failure. The accepted correction
keeps the define and proves both `SQLITE_DBCONFIG_TRUSTED_SCHEMA=false` and
`PRAGMA trusted_schema=0` on every connection; the other five repository defines
remain compile-option assertions. This changes no security boundary.

重大、长期或难以逆转的决策应建立单独 ADR，并在此登记。

| ADR | 标题 | 状态 | 日期 |
|---|---|---|---|
| `ADR-0001` | V1 local authority and identity boundary | `ACCEPTED` | 2026-08-20 |
| `ADR-0002` | Copy-only initial Managed Asset ingest | `ACCEPTED` | 2026-08-20 |
| `ADR-0003` | Foundation Rust toolchain and bundled SQLite | `ACCEPTED` | 2026-08-21 |
| `ADR-0004` | arm64 macOS foundation Client authority and deferred Admin | `ACCEPTED` | 2026-08-21 |
| `ADR-0005` | Foundation finite safety caps | `ACCEPTED` | 2026-08-21 |
| `ADR-0006` | macOS filesystem FFI and build-evidence boundary | `ACCEPTED` | 2026-08-22 |
| `ADR-0007` | Local CAS custody and capability boundary | `ACCEPTED` | 2026-08-26 |
| `ADR-0008` | Asset persistence and durable command ledger | `ACCEPTED` | 2026-08-28 |
| `ADR-0009` | Copy-ingest session and orchestration boundary | `ACCEPTED` | 2026-08-30 |
| `ADR-0010` | Layered non-recursive CI orchestration | `ACCEPTED` | 2026-09-01 |

建议命名：`docs/spec/adr/ADR-0001-short-title.md`。

## ADR 最小模板

```markdown
# ADR-NNNN: 标题

- Status: PROPOSED | ACCEPTED | SUPERSEDED | REJECTED
- Date: YYYY-MM-DD
- Supersedes: 可选

## Context

## Decision

## Consequences

## Verification
```
