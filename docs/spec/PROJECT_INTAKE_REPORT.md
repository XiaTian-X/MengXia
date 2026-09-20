---
title: "梦夏（MengXia）项目接管与仓库基线报告"
status: "REVIEWED_FOUNDATION_IN_PROGRESS_NO_PRODUCT_AUTHORITY"
version: "1.3.74"
date: "2026-09-20"
---

# 项目接管与仓库基线报告

## Current local observation — 2026-09-20

CURRENT_PROJECT_NEXT_ACTION: COMPLETE_REVIEWED_NATIVE_FOUNDATION

PR preparation follow-up: remote main still resolves to
4fcf3470a6a98ad09feaf12152cee8c69740e467, with no open PR at intake. Repository is
public; classic main protection requires Merge gate, strict up-to-date checks and
linear history, including administrators. CodeQL default setup is configured for
actions/c-cpp/rust. These are read-only observations, not configuration changes.
The user's continuation now permits the scoped normal commit/push/PR stage; earlier
no-submission statements describe the previous local checkpoint, not this stage.

Pure-foundation local implementation checkpoint: reviewed_admission.rs, its exports,
eight integration tests and scoped lifecycle validation are implemented. Full local
developer gate exited 0 on the current dirty worktree; exact commands, environment
and exclusions are in reviewed-native plan §7. No migration, dependency, protocol,
R0 probe, system setting, commit/push or plugin enablement change. Ledger is LOCAL_PASS
but IN_PROGRESS; reviewed hosted PR/main and real second UID remain pending.

Foundation start accepted (2026-09-20): the user's “开始下一步” authorizes
REVIEWED_NATIVE_FOUNDATION_ONLY implementation under reviewed plan §5. Scope status
is IN_PROGRESS, product authority NONE; old strict TASK-012 remains BLOCKED and
TASK-010/TASK-011 DONE is unchanged. Complete the pure evaluator, scoped accounting
and local regression, then obtain reviewed formal PR/main evidence before scoped
DONE. Earlier draft-only/routing statements below are historical for this scope,
not a prohibition on its accepted pure implementation. No signature, package
installation, process launch, grant/lease, DB migration or external write is enabled.

ADR-0021 accepts reviewed-only native admission with explicit residual
host-memory/DoS risk and retained OS filesystem/network/IPC, custody, Broker and
lifecycle boundaries. It is not all-ENFORCED SANDBOX_ONLY or blanket TRUSTED_NATIVE.
The current work is the accepted pure admission foundation in
REVIEWED-NATIVE-PLUGIN-DEVELOPMENT-PLAN.md §5, with scoped implementation authority only. Product authority remains
NONE, no candidate runtime is qualified, and unknown/unreviewed packages stay
disabled. Necessary scoped refactoring is permitted with regression/compatibility
evidence. Existing DONE, protocol bytes and applied migrations remain unchanged.
The earlier R0-B/VM-candidate routing below is historical and superseded; research
results remain evidence, not a reason to repeat the same memory probes. Ubuntu
remains deferred. Hard-memory feasibility blocks the strict profile, not the new
pure admission contract; real reviewed execution still requires its retained
properties and finite-budget gate.

The earlier planning checkpoint recorded a policy change, not a new host probe. Existing worktree changes
were present at takeover; no production file, migration, package/wire artifact,
dependency, system setting or historical evidence was modified by this planning
work. The subsequent pure implementation and local evidence are recorded below and
in reviewed-native plan §7; this historical paragraph does not describe that code change.

Reviewed-native planning verification — 2026-09-20: documentation driver passed
28 tests (traceability 7, naming 4, CI orchestration 5, CI evidence 12); architecture
4/4, retained TASK-010 foundation 9/9 and TASK-011 foundation 19/19 passed. Targeted
document-traceability Clippy with warnings denied, cargo fmt check and diff check
passed. The route validator covers the new ADR/plan and rejects reverting the
accepted direction; new negative mutations reject authority expansion, hard-memory
mislabeling, inherited strict PASS, relaxed unknown-package/secret/revocation
rules, weakened old AC wording and pure-foundation overclaims. These are local
planning/compatibility checks, not the new runtime qualification or complete
workspace/Formal/second-UID/hosted evidence. Existing TASK-011 self-authored hostile
fixtures ran under their retained test harness; no third-party package was run.

Code inspection also confirmed inspect_manifest hashes canonical manifest bytes;
the reviewed contract now separately binds final artifact and dependency closure
without changing PackageDigest or old golden bytes. No Git commit was created.

### Earlier observations (historical routing, retained evidence)

Latest read-only comparison: MACOS-NATIVE-EXECUTION-BOUNDARY-COMPARISON.md records
Apple SDK/public APIs, pinned Containerization source and existing package/session
contracts. No VM/extension was started, no runnable image downloaded, no signing,
installation, dependency or production change. Darwin package target remains
unchanged. API availability is not host configuration validation or qualification.

Latest batch: MACOS-NATIVE-R0B-002.md records six bounded self-process observations
at /tmp/mengxia-r0b002.Nvr2e5ch on the same tuple. memorystatus returned EPERM;
DATA rejected a new mapping but allowed existing-mapping footprint growth beyond
headroom. No root, signing, registration or product changes. Existing-mapping,
shared/kernel total coverage is still unqualified; next assess execution-boundary
candidates rather than repeat these API probes. Batch authority has ended.

New finite R0-B observations: MACOS-NATIVE-R0B-001.md records the current tuple,
source/binary hashes and private temporary results for a self-authored Seatbelt
probe. No system registration/signing/mount/configuration or product code changed.
The observed AS/physical-growth counterexample does not qualify a native backend.

ADR-0019 accepts macOS/Ubuntu as independently qualified editions with shared core
contracts. This is planning, not runtime evidence. No Ubuntu host was inspected:
distribution version, architecture, kernel, filesystem, toolchain and enforcement/
delegation capabilities remain UNKNOWN. The user now defers Ubuntu until macOS
completion: no Ubuntu intake/version choice is needed now; no VM is required later.
ADR-0020 now prioritizes controlled built-ins and defers third-party Native support.
The latest scheduling amendment prioritizes bounded R0-B native feasibility research
following MACOS-NATIVE-R0B-002.md section 5; TASK-015 drafting is not the immediate action. This is
not production implementation or a rerun of the same unavailable memory API.
The built-in-first scope decision remains, not new runtime
evidence; no built-in execution profile or product implementation start is accepted.
Existing macOS records do not prove Linux
support, and the Ubuntu CI classifier is not a native product-validation job.

Head remains `4fcf3470a6a98ad09feaf12152cee8c69740e467`. The active host is now
arm64 macOS 27.0 / 26A428 with full Xcode 27.0 / 27A266a and SDK 27.0, rechecked
with sw_vers, xcodebuild and xcrun. Older "current" host statements below are dated
historical snapshots and do not describe today's selected Xcode/SDK. Their formal
commit/run evidence remains unchanged.

On 2026-09-15, maintained cold-native verification, workspace all-target/all-feature
tests, fast checks, docs and shared Cargo supply checks passed at this head on the
new tuple. Native evidence was generated under a fresh candidate directory, not
inferred from cached builds. ATTESTATION_MATCH was NO; developer compatibility was
demonstrated within those checks, not formal/release or TASK-012 qualification.
Formal scaling and real second UID were not executed by the workspace invocation;
tool-advisory coverage remains partial/unknown. These are dated results, not a
claim that all commands were rerun on 2026-09-20.

The unaccepted, deferred TASK-012 draft v0.2.6 retains v0.2.2's changed sandbox-exec identity and
local memory/guest-query/process-group/re-exec counterexamples. Its correction is
documentation-only. Production spawn, sandbox, installation and activation remain
unauthorized; the hard-memory feasibility gate remains open. No system rollback,
formal CI manifest update or reopening of completed tasks follows from this upgrade.

Fresh 2026-09-20 feasibility check: the same OS/Xcode tuple was re-observed;
a temporary C program calling only its own task_set_phys_footprint_limit(128 MiB)
as an ordinary user returned 8. RLIMIT_AS and RLIMIT_RSS both equal 5. This is
negative primitive-availability evidence, not a resource-cap or full hostile test.
No Ubuntu intake, system change, new dependency or runtime implementation occurred.
Qualification sequencing and macOS-only DONE interpretation were corrected in
documentation; no backend or new implementation start is accepted.

Bounded Native R0 observation — 2026-09-20: the user paused TASK-012 and authorized
only the seven-file R0-A research harness. Its 11 synthetic negative checks passed;
12 closed cases each ran three times on the current tuple, producing 36 structurally
valid expected observations with confirmed child cleanup and no unbounded stress.
Evidence under ignored `target/native-research/evidence.doKYy0hM` is 13,803 bytes
and explicitly records HEAD `4fcf3470a6a98ad09feaf12152cee8c69740e467`
with dirty=YES. It is developer evidence, not a commit/release attestation.

That is the initial historical record. The later audit identified eight harness/
test defects, including three malformed negative fixtures. It does not qualify
the corrected implementation. Renewed schema-2 observations and bounded fault
regressions are separately recorded in the R0 design section 9.4; no old artifact
was overwritten or upgraded in place.

Read-only R0-B preflight found the ExtensionFoundation SDK APIs and zero valid
code-signing identities. Current `/usr/bin/sandbox-exec` SHA-256 is
`58839ef01b4eef8aac0d2aa8f9d1c074ae45aafe3533965b030672450064acc8`
with CDHash `1266e34f192210d9f1ac6dbc9cf297ecff93e568`, different from historical
macOS 26 evidence. No registration, signing, profile execution, external/media
binary or system-state change occurred. R0 is complete with `INCONCLUSIVE`
convergence; TASK-012 remains paused/BLOCKED/NONE and TASK-015 planning remains the
then-planned independent product action, since superseded by the explicit R0-B
research priority recorded above.

Built-in scope decision verification — 2026-09-20: after accepting ADR-0020,
`scripts/verify-repository.sh docs` passed all 25 tests (document traceability 4,
naming 4, CI orchestration 5, CI evidence 12). The new documentation regression
rejects restoring the Native prerequisite to pure planning, removing FFmpeg's
execution prerequisite, bypassing Provider prerequisites, implicitly accepting
resource risk, enabling third-party Native and duplicate scope rows. These are
planning/traceability checks, not product runtime security tests. `cargo fmt --all
--check` and targeted document-traceability Clippy with warnings denied passed.
No product code, migration, protocol, dependency/tool pin or CI workflow changed
in this scope decision; no full Formal, second-UID or hosted CI run was performed.

## Historical intake records

Current-action correction verification — 2026-09-20: the routing correction
synchronizes ten entry/route documents on drafting TASK-015 PLAN_FOUNDATION only.
Native candidate technical evidence remains unchanged; its v0.2.6 update corrects
deferred routing. The pure foundation output type is still an explicit start-gate
decision, not a completed ExecutionPlan implementation. No product authority,
production code, migration, protocol or dependency changes are introduced.
Verification: document_traceability 6/6 and the complete docs driver 27/27 passed;
targeted Clippy with warnings denied, format and diff whitespace checks passed.
Negative cases retain the correct current-action header while injecting each stale
body instruction, and also reject missing/duplicate/divergent action declarations
and reverting the accepted direction to pending. Full Formal/hosted CI was not run.

Scoped-start/Broker correction verification — 2026-09-20: the subsequent two-finding
fix passed `scripts/verify-repository.sh docs` (26 tests), the architecture target
(4 tests), and `mengxia-domain` tests/doctests (14 tests). Format, targeted
document-traceability Clippy with warnings denied, and diff whitespace checks passed.
The added regression checks the three accounting stages in Specification/Plan/ADR,
rejects a premature validator requirement, rejects reintroducing the Broker
persistence/execution dependency cycle or skipping persisted prerequisites, and
checks scoped migration ordering plus qualification/product boundaries. These are
planning regressions, not an implemented scoped lifecycle engine or execution proof.
No product code, migration bytes, protocol, dependency, tool pin or CI workflow was
changed by this correction. Full Formal, real second UID and hosted CI were not run.


2026-09-13 ACL correction completion intake: PR #10 merged implementation head
9da5125449a9cbeb77bae446cb2ab08b40fe7880 as main
632a2aac725001332fb8541806abe9cdcfcc65ee. Reviewed PR/main runs
34739311995 / 34739722410 each passed exactly 155 expected aggregate IDs;
CodeQL runs 34739311120 / 34739721926 passed all three analyses. Exact checkout
and tree evidence is recorded in the plan's ACL correction completion section.
Status DONE; BUILD_ACL_CORRECTION_ONLY revoked; implementation/product authority
NONE. Completion changes only documentation/versions; all previous facts below
retain their historical attribution. CLT 27 is not selected or newly certified.

Historical 2026-09-13 ACL correction intake: baseline c2a1f50337488250cc3433ea3d9b925ca728554a.
CLT 27.0 was installed separately; selected Xcode remains 26.6 / 17F113, SDK 26.5.
The preceding unchanged-code review passed a fresh native candidate and 154 local
FAST_PASS obligations. Its ACL counterexample (0500 tool copy plus everyone write)
is REPO_STALE, not a CLT compatibility regression. User now authorizes only the
bounded build ACL correction; product authority remains NONE. Previous completion
facts below retain their exact historical SHA/run attribution.
Local correction checkpoint: shared Shell/Rust ACL policy implemented; 12 toolchain
tests, clean native candidate, 154 exact developer FAST_PASS IDs, documentation and
hot Fast passed. No tool version, lock, native source, attestation or CI-topology
change. Tests ran before commit; reviewed PR/main evidence remains pending.

2026-09-13 MAINT-003 completion intake: implementation merged by PR #8 as main
7b6fa4f17373ab94aa86057d8ddc9b4d23c23b8c. Reviewed PR/main runs
34731852394 / 34732388943 passed all 155 expected aggregate IDs; CodeQL runs
34731851724 / 34732388829 passed. Tested PR merge-ref was
36cef451a8403b3b509ea9d8565c6811bd6eefcf; its tree and implementation head
229ca19be28f1f618d42b45f4807d31270a9af74 match merged main exactly.
Completion worktree changes only canonical documentation/records; generated tools,
builds and logs remain ignored. Maintenance/product authority is NONE.
Second real hosted Xcode evidence and exact scope limits: MAINT-003 plan §14.
PR #3 closed automatically after configuration merged; security update settings
remain enabled and main protection is unchanged. No tool version was upgraded.

Historical pre-implementation MAINT-003 intake: main b2fa8d52580a58f014da97e9473e647681911389,
with only the preceding planning documents dirty. One /Applications/Xcode.app
installation is available; no second local Xcode evidence is claimed. Official
cargo-deny 0.20.2 arm64 archive SHA-256 fe67d82a10d8597a3549364cb733a3f9cc1bfff9031b7ae46384a9f2a72090c3
matches GitHub release metadata and the official .sha256 asset; its extracted
executable SHA-256 is 5f65c07c459c9514f0c97cc2e2fb6b120daef2d95aee31062cab4816cf027eb1.
No tool version or system setting was changed at that checkpoint. ADR-0016 then
authorized bounded MAINT-003 implementation; that temporary authority is now revoked.

TASK003_CANONICAL_GATE: ACCEPTED
TASK003_SPECIFICATION_VERSION: 1.1.17
TASK003_LIFECYCLE: DONE
TASK003_PROPOSAL: docs/proposals/TASK-003-GATE-PROPOSAL.md

TASK005_CANONICAL_GATE: ACCEPTED
TASK005_SPECIFICATION_VERSION: 1.1.18
TASK005_LIFECYCLE: DONE
TASK005_IMPLEMENTATION_AUTHORITY: NONE
TASK005_PROPOSAL: docs/proposals/TASK-005-GATE-PROPOSAL.md

TASK006_CANONICAL_GATE: ACCEPTED
TASK006_SPECIFICATION_VERSION: 1.1.22
TASK006_LIFECYCLE: DONE
TASK006_IMPLEMENTATION_AUTHORITY: NONE
TASK006_PROPOSAL: docs/proposals/TASK-006-GATE-PROPOSAL.md

TASK007_CANONICAL_GATE: ACCEPTED
TASK007_SPECIFICATION_VERSION: 1.1.25
TASK007_LIFECYCLE: DONE
TASK007_IMPLEMENTATION_AUTHORITY: NONE
TASK007_PROPOSAL: docs/proposals/TASK-007-GATE-PROPOSAL.md

TASK008_CANONICAL_GATE: ACCEPTED
TASK008_SPECIFICATION_VERSION: 1.1.30
TASK008_LIFECYCLE: DONE
TASK008_IMPLEMENTATION_AUTHORITY: NONE
TASK008_PROPOSAL: docs/proposals/TASK-008-GATE-PROPOSAL.md

TASK009_CANONICAL_GATE: ACCEPTED
TASK009_SPECIFICATION_VERSION: 1.1.34
TASK009_LIFECYCLE: DONE
TASK009_IMPLEMENTATION_AUTHORITY: NONE
TASK009_PROPOSAL: docs/proposals/TASK-009-GATE-PROPOSAL.md

TASK010_CANONICAL_GATE: ACCEPTED
TASK010_LIFECYCLE: DONE
TASK010_IMPLEMENTATION_AUTHORITY: NONE
TASK010_PROPOSAL: docs/proposals/TASK-010-GATE-PROPOSAL.md

TASK011_CANONICAL_GATE: ACCEPTED
TASK011_LIFECYCLE: DONE
TASK011_IMPLEMENTATION_AUTHORITY: NONE
TASK011_PROPOSAL: docs/proposals/TASK-011-GATE-PROPOSAL.md
TASK011_PROPOSAL_VERSION: 0.1.2
TASK011_DECISION: ADR-0017 ACCEPTED

本报告只记录只读检查得到的 Current State，不把当前开发机工具或目录当成 Target State 决策。

## Repository facts

MAINT-002: DONE / NONE under ADR-0015. Reviewed PR run 34676854969 covers head
9291075a30325d86acf4943b7ef0ab76e0b91cad (actual merge-ref checkout
e55c3e0fd04680c3679047b0983aacfe222af7b6). Merged-main run 34677363307 covers
19e2e613728c2a2c11f6c3dfc185b04e3a625316. Both full required evidence sets passed;
native, second-UID and supply checkouts matched their respective event SHA. PR and
main code trees are identical. Product/frozen-input diff is zero and main protection
remains strict with required Merge gate and enforced administrators. Performance and
closed-scope completion evidence are in the MAINT-002 plan §12. Draft suppression
is NOT_ENABLED; no product authority or later-task dependency was added.

| Observation | Evidence | Classification | Impact |
|---|---|---|---|
| Git repository 已初始化，branch 为 `main`，已有文档基线 commit history；TASK-001 bootstrap 属于包含本报告的 repository baseline change | `git status --short --branch`; `git log -1`; reviewed candidate inventory | `FACT / BASELINE CHANGE` | 提交前后均须核对 worktree 与 commit evidence，不得把忽略文件或未暂存文件误报为已提交内容 |
| TASK-001/TASK-002 已完成；workspace 现有 18 个 canonical package，TASK-004 已加入固定 SQLite 3.53.4、精确错误映射、bootstrap-only schema/index/typed-row reopen validator、macOS path/ACL/root/lock authority、pre-mutation clock/UUID first-create orchestration、intent codec/durable-create/post-lock-read、valid-intent empty-staging、staging SQLite bootstrap、ordered publish、closed restart recovery、authorized incomplete/WAL recovery、bounded required-commit WAL classification、23-point/29-case same-OS SIGKILL recovery、bounded connection lifecycle、complete deterministic corruption matrix 与 16×256 WAL-reset stress slices | locked Cargo metadata; repository candidate inventory; TASK-001/TASK-002 evidence; TASK-004 scoped diff and complete local gates; reviewed runner-XIP formal CI run `32695815747` | `FACT / VERIFIED` | TASK-004 `DONE`；正式 supply-chain PASS 来自 reviewed CI attestation；后续 task 不因该完成状态自动获权 |
| TASK-003 的 framed proto3 handshake、server-derived Client identity、受保护 runtime endpoint、CLI/config composition 与 bounded joined lifecycle 已实现；该 task 本身不包含产品 ingest/domain 能力 | scoped TASK-003 diff review; `scripts/verify-task-003.sh`; successful CI run `32914222948`; formal job `task-003-second-uid`; `TEST-IPC-MACOS-001: PASS` | `FACT / VERIFIED` | TASK-003 DONE；真实 second-UID evidence 只来自 reviewed formal CI；后续消费者必须保持其 authority boundary |
| TASK-005 exact-scope ports/local-storage/platform implementation 已完成：opaque source/root authority、bounded worker/admission、stream/hash/write、durable no-clobber CAS、orphan/recovery、Location descriptor 与 joined shutdown；本地门禁和 reviewed formal CI 通过 | commits `88e7b3413db5607651f2c842f6d0c1f03d513968`, `f516faafe50707b88f51f25c03be07f917f8943f`; `scripts/verify-task-005.sh formal`; reviewed run `33073580258`; Specification v1.1.19; ADR-0007 | `FACT / VERIFIED` | TASK-005 `DONE`，authority `NONE`；其完成未自动授权 TASK-006，后者已通过独立 gate 完成 |
| TASK-006 Asset domain、typed DTO/row、CommandRecord/event persistence、immutable 0001、recovery 与 bounded writer lifecycle 已实现并完成审查 | commits `60b6616c20d677632ca25b8b72340fc3a639db54`, `10455605556984e48def16efc27fb52338109944`; `scripts/verify-task-006.sh formal`; reviewed arm64 `macos-26` run `33257331689`; Specification v1.1.23; ADR-0008 | `FACT / VERIFIED` | TASK-006 `DONE`，authority `NONE`；其边界由 completed TASK-007 精确消费 |
| TASK-007 additive protocol 1.1 copy-only ingest、bounded claim/CAS/registration、CLI/daemon composition、idempotency 与 crash recovery 已实现并完成审查 | exact head `084f8269d0e9421bf909ae7d9a44e83cae3e9a9a`; `scripts/verify-task-007.sh developer`; reviewed arm64 `macos-26` run `33401785647`; proposal v0.1.4; ADR-0009; Specification v1.1.27 | `FACT / VERIFIED` | TASK-007 `DONE`，authority `NONE`；migration、root rebind、Admin、TASK-008+ 仍禁止 |
| Post-TASK-007 review found stale PID/counter fixture collisions and incomplete seven-object raw-ID uniqueness enforcement; the exact correction set is implemented and verified | Decisions REVIEW-CONFLICT-019/020; exact commit `7c361399211d4551f16b1397195d7ad6f7e05479`; targeted/package/local formal gates; reviewed run `33482363576` | `REPO_STALE / CORRECTION VERIFIED` | Temporary correction authority is revoked to `NONE`; TASK-008+ remains unauthorized |
| Pre-ADR-0010 baseline CI ran on every push and pull request and the latest formal script recursively repeated prior workspace/document/supply gates | pre-correction workflow and task-script comparison recorded by `REVIEW-CONFLICT-023` | `REPO_STALE / CONFLICT / HISTORICAL` | ADR-0010 limits the correction to layered, fail-closed, non-recursive CI orchestration maintenance; stable task evidence and product behavior remain intact |
| ADR-0010 layered CI correction is implemented and verified | classifier negative matrix, including machine-consumed `docs/provenance/**`; orchestration regressions; local docs/developer/formal repository drivers; reviewed run `33482363576` with formal aggregate and separate real second-UID job PASS | `VERIFIED` | temporary CI maintenance authority is `NONE`; later code candidates retain the same two-job formal requirement |
| TASK-008 bounded read/verify/materialize, Core observability/health, durable recovery and protocol 1.2 CLI/daemon composition are implemented and verified | exact head `7aeb032a75edbe85050cf470d910bc53a85d74cf`; complete local repository developer gate; reviewed arm64 `macos-26` run `34188886713`; proposal v0.2.5; ADR-0011; Specification v1.1.31 | `FACT / VERIFIED` | TASK-008 `DONE`, authority `NONE`; migration, root rebind, Admin and TASK-009+ remain forbidden |
| TASK-009 protocol 1.3 creative ledger, migration 0002, Asset lifecycle and Project/Subject/Work/Take semantic surface are implemented and verified | exact head `fa7a0047c95c8b8eba12e859284223a1a78f51e2`; complete local developer/formal gates; reviewed arm64 `macos-26` run `34552988098`; proposal v0.1.5; ADR-0012 | `FACT / VERIFIED` | TASK-009 `DONE`, authority `NONE`; TASK-010+ and all privileged/destructive capabilities remain unauthorized |
| Post-TASK-009 audit found incomplete current-schema creative validation, a stale verifier operation registry, missing ListWork scope existence and insufficient named test mappings; the bounded correction is implemented | Decisions `REVIEW-CONFLICT-032`..`036`; exact correction `c3fa74a`; focused store suites, Clippy, complete local gates; reviewed exact-descendant run `34559210695`; retained MAINT-001 runs `34565503807`/`34566194911` | `REPO_STALE / CORRECTION VERIFIED / REVIEWED CI COVERAGE` | no migration/protocol/dependency/authority expansion; original TASK-009 delivery evidence remains separately attributed |
| Public-repository/toolchain review found post-merge-only formal validation, no protected-main/security-analysis settings, brittle developer Xcode metadata/name equality, repeated attestation values, absent executable protoc regeneration and no idle-repository scan; MAINT-001 implemented and verified the bounded correction | ADR-0013; PR `#1`; final PR run `34565503807`; merge `2dd5bcb76a8eb6b804ef55b10d78dd715bdaebe4`; merged-main run `34566194911`; `REVIEW-CONFLICT-037`..`REVIEW-GAP-044` | `CORRECTION VERIFIED / MAINT-001 DONE` | authority revoked to `NONE`; no product, migration, protocol artifact, dependency or TASK-010+ authority |
| Dependabot security updates, secret scanning, push protection and private vulnerability reporting are enabled; CodeQL default setup analyzes Actions, C/C++ and Rust; strict protected `main` requires the repository-owned `Merge gate` because default setup excludes fork PRs | GitHub settings/default-setup/protection API read-back; CodeQL runs `34563494593`, `34565501863`, `34566195268`; alert 1 reviewed and dismissed as a `#[cfg(test)]` deterministic-UUID assertion false positive; current open code/dependency/secret alerts: zero | `FACT / VERIFIED / EXTERNAL CAPABILITY GAP` | retain Clippy, cargo-deny, dependency review and repository security tests; do not require CodeQL globally or add `pull_request_target` while fork scans are unavailable |
| The initial MAINT-001 PR run reported that checkout v4's Node.js 20 runtime is deprecated and compatibility-forced to Node.js 24 | PR run `34563606468`; official checkout v7.0.1 release/tag/verified-commit/action metadata | `REPO_STALE / EXTERNAL TOOLING DEPRECATION / REVIEW-CONFLICT-043` | update only to the official full-SHA-pinned Node.js 24 action and rerun dependency/workflow gates; no product code or toolchain attestation changes |
| CodeQL default setup runs on the default branch and same-repository PRs but currently excludes pull requests from forks | GitHub default-setup documentation; observed PR checks | `EXTERNAL CAPABILITY GAP / REVIEW-GAP-044` | keep scans enabled and reviewed, but require only the repository-owned Merge gate so public external contributions are not permanently blocked; never substitute `pull_request_target` |
| The first MAINT-001 current-state regression required the temporary maintenance authority text and rejected the mandatory transition to `NONE` | failed completion docs gate; `REVIEW-CONFLICT-045`; corrected traceability regression and subsequent repository gates | `REPO_STALE / COMPLETION-GATE CONFLICT / CORRECTED` | current-state tests must follow lifecycle transitions and must never force stale authority to remain active |
| Initial unaccepted TASK-010 draft used stale baselines, collided with occupied ADR-0013 and left diff/error/test/file-scope outcomes ambiguous | proposal v0.1.1; `REVIEW-CONFLICT-047/048`; current repository driver/classifier/workflow inspection | `REPO_STALE / CONFLICT / CORRECTED IN DRAFT` | TASK-010 remains blocked; candidate ADR number is 0014; only the stale workflow display name, not classifier/CI behavior, enters candidate scope; TASK-013 privileged enforcement stays outside Gate A |
| Full TASK-010 review found that durable host paths cannot bind a later macOS pathname launch, Gate A mixed pure code with filesystem/runtime services and full TASK-010 completion unnecessarily blocked TASK-011 on Admin persistence | proposal v0.2.0; candidate ADR-0014; `REVIEW-CONFLICT-049`; macOS 26.5 SDK header/API inventory; current package/platform/architecture inspection | `CONFLICT / PLATFORM_FEASIBILITY / CORRECTED IN DRAFT` | TASK-010 is narrowed to canonical in-memory package foundation with AC-098..AC-100; TASK-012 owns managed executable launch proof; TASK-013 owns install/migrations/grants/revocations/audit/AC-027; authority remains NONE pending independent review |
| TASK-010 v0.2.0 re-review found a real deny-policy/file-scope failure plus ambiguous identity diff, evidence ordering, parser cap counting, runtime resolver defense and downstream acceptance gates | proposal v0.2.1; `REVIEW-CONFLICT-050`; isolated copy of current lock selected 40 new packages; Rust 1.98 all-target online/offline checks; cargo-deny fail-before/pass-after exact candidate policy delta | `CONFLICT / SUPPLY / SPECIFICATION PRECISION / CORRECTED IN DRAFT` | retain jsonschema 0.56.0, add only exact deny entries during future STEP-1, require runtime offline validation and deterministic diff/caps; TASK-010 remains blocked with authority NONE pending independent review/start record |
| TASK-010 v0.2.1 re-review found that its lock omitted intended testkit edges, the tested MIT-0 allowance was global, schema/typed errors overlapped and requirement ownership was incomplete | proposal v0.2.2; `REVIEW-CONFLICT-051`; isolated final-manifest graph with 117 retained + 40 exact registry packages; `--locked --offline --workspace --all-targets`; cargo-deny with per-crate `borrow-or-share@0.2.4` exception | `CONFLICT / SUPPLY / SECURITY / TRACEABILITY / CORRECTED IN DRAFT` | freeze all Cargo edges/checksums/final lock, keep MIT-0 out of global allow, use one MANIFEST_INVALID instance result, record contributor/sub-scope ownership and add SEC-003/010/016 to TASK-013's terminal set; repository Cargo/lock/deny and implementation remain unchanged pending independent review/start |
| TASK-010 v0.2.2 independent review found duplicated weaker AC/TEST prose, unnamed remaining API-001 owners, an over-broad Phase-3 entry gate, cross-task AC-020/AC-023 terminal-owner conflicts, incomplete number classification and a shape-only dependency inventory check | proposal v0.2.3; `REVIEW-CONFLICT-052`; Specification v1.1.43 sole AC/TEST authority; exact API-001 and AC-020..AC-023 ownership; inventory digest `0d66568ef018ff7d68083bb8570a861bf24d40861a85d8965bda1a00c3283c7b` | `CONFLICT / SECURITY / SPECIFICATION PRECISION / TRACEABILITY / RESOLVED` | retain the verified supply graph, split Phase 3 by real prerequisites, classify invalid-number grammar separately from valid non-V1 numbers and mechanically pin exact content/ownership |
| TASK-010 v0.2.3 independent acceptance replayed the exact final graph and retained baseline | `REVIEW-CONFLICT-053`; isolated current-head copy; exact 117+40 package inventory; lock SHA-256 `302df8141acee77aa58ecb796a53ecbb4faf9f6cd55dc384667bb08e725c0b2e`; offline build; cargo-deny advisories/bans/licenses/sources; developer baseline | `FACT / VERIFIED / ACCEPTED START GATE` | ADR-0014 accepted and TASK-010 foundation authority active; all filesystem/persistence/Admin/install/activation/execution/TASK-011+ work remains forbidden |
| TASK-010 pure package foundation is implemented and its completion-gate review corrected stable-ID command ownership plus stale current-authority assertions | exact head `e2311ed1dea992ce85db2547a1af799d0d8cf045`; PR `#4`; reviewed arm64 `macos-26` run `34667611801`; CodeQL run `34667610304`; `REVIEW-CONFLICT-054`; `REVIEW-CONFLICT-055`; exact 28-path proposal §10 diff | `FACT / VERIFIED / TASK-010 FOUNDATION DONE` | AC-098..AC-100 and all nine TEST-*-010 obligations PASS; authority revoked to NONE; TASK-011+ and all privileged/executable behavior remain unauthorized |
| TASK-011 caller-supplied private protocol/session and hostile fixture are implemented and verified | exact PR head `fc817200d2a60c88c4d16cc5e4c60a6be2dd1cbf`; PR `#12`; reviewed run `34761111053`; merged main `8416e01335e4fb8ff58e3381cf888fbcf69b9005`; main run `34761648787`; CodeQL runs `34761109339`/`34761648812`; ADR-0017 | `FACT / VERIFIED / DONE` | AC-101..AC-103 and all twelve TASK-011 IDs PASS; authority NONE; production spawn/sandbox/Broker/Admin/install/activation/TASK-012+ remain unauthorized |
| Finder `.DS_Store` 与 Cargo `target/` 存在但被忽略；候选提交清单不包含这些文件 | `git status --ignored`; `git ls-files --cached --others --exclude-standard` | `FACT` | 环境与编译产物不得提交；忽略与强制添加两条路径都由 repository hygiene test 覆盖 |
| 规范 v1.0.1 proposed tree 把 spec/ADR 路径写成 root/`docs/adr`，与实际 `docs/spec` 不同 | document/repository comparison | `SPEC_STALE` | v1.1.0 repository map 已修正为当前 canonical doc path |

## Host and tool facts

| Observation | Evidence | Classification | Impact / gate |
|---|---|---|---|
| 当前检查主机快照为 arm64 macOS 26.6.2 (build 25G83, Darwin 25.6.0) | `uname`, `sw_vers` | `FACT / TRANSIENT EVIDENCE` | 仅为本次开发证据；安全补丁升级通过 developer gates 时不触发规范重写，也不接受 `OQ-001/OQ-002` |
| 当前本机 Xcode 26.6/SDK 26.5 通过 developer compatibility，但 clang/libtool bytes 与 active GitHub runner-XIP attestation 不同，正式构建按预期 fail closed | candidate helper; emitted local digests; rejected local attested build | `FACT / EXPECTED_GAP / TRANSIENT EVIDENCE` | 本机可继续功能开发但不得产生 formal/release 证明；active manifest 对应的托管 runner 必须为 PR 和 merged-main 提供正式证据 |
| rustup 1.29.0 已安装 Rust 1.98.0、Cargo 1.98.0、rustfmt 与 Clippy；shell PATH 未被安装器修改 | explicit rustup/toolchain commands | `FACT / VERIFIED` | 使用 pinned 1.98.0 toolchain；TASK-001 创建 `rust-toolchain.toml` 后提供 repository-local resolution |
| PATH 中可见的 `sqlite3` 来自 Android SDK，版本 3.50.6 | command lookup/version | `FACT` | 不是 approved bundled runtime；处于 SQLite 官方 WAL-reset bug 影响版本范围，MUST NOT 被 TASK-004 采用 |
| SQLite 官方 arm64 tools 3.53.4 已安装到用户 MengXia 工具目录；下载 SHA3-256 与官方值 `58d53e...776d` 匹配 | official download + local digest/version/architecture | `FACT / VERIFIED` | 仅用于开发诊断；不替代 application bundled library |
| SQLite 3.53.4 amalgamation 已下载；SHA3-256 `628a44...34e` 与官方值匹配；自编译 CLI 已验证 ADR-0003 compile options | official download + local digest + `PRAGMA compile_options` | `FACT / VERIFIED` | TASK-004 可从已验证源码构建 bundled runtime并加入启动断言 |
| Git 版本为 Apple Git 2.50.1 | version check | `FACT` | 足以进行仓库元数据操作；不是产品依赖决定 |
| 当前 workspace 位于 `/System/Volumes/Data` 的 local APFS | mount/df evidence | `FACT` | 只证明当前 workspace；未来 `MENGXIA_LIBRARY_ROOT` 仍需 exact-path filesystem validation |
| 约 89 GiB 可用空间（检查时） | `df` | `FACT / TRANSIENT` | 不构成性能、最大库或发布容量承诺 |

## Repository/target gaps

| Gap | Classification | Required action |
|---|---|---|
| Rust/MSRV 1.98.0 和 SQLite 3.53.4 工具/源码已接受并完成本机验证 | `DECISION / VERIFIED` | ADR-0003; TASK-001 preserves the pin; TASK-004 still owns application bundling/assertions |
| arm64 macOS foundation 已接受；sandbox backend 未决定 | `DECISION / LATER BLOCKING` | ADR-0004; close OQ-002 before TASK-012 |
| ordinary Client peer UID contract 已接受；Admin mechanism 延后且功能禁用 | `DECISION / FAIL-CLOSED` | ADR-0004; OQ-010 before Admin enablement |
| TASK-002..TASK-005 frame/queue/buffer/concurrency/staging caps 已接受 | `DECISION` | ADR-0005; later caps remain incremental gates |
| secret store、Provider、Rights、retention 未决定 | `UNKNOWN / LATER BLOCKING` | close OQ-004/OQ-005/OQ-008/OQ-009 at their documented gates |
| Repository-local TASK-001 build/lint/test/supply-chain/doc commands 已存在并在当前 arm64 macOS 复核通过 | `FACT / VERIFIED` | 保留这些 gates；后续 task 必须在开始前增加自身稳定 AC/TEST registry 与完成证据 |

## First safe next action

TASK-001/TASK-002/TASK-004/TASK-003/TASK-005/TASK-006/TASK-007/TASK-008/TASK-009 are implemented and verified. Reviewed
runner-XIP CI run `32695815747` proves TASK-004, reviewed real-second-UID run
`32914222948` proves TASK-003, and reviewed `macos-26` run `33073580258` proves the
exact TASK-005 formal candidate. Reviewed arm64 `macos-26` run `33257331689` proves
the exact TASK-006 candidate and retained gates. Reviewed arm64 `macos-26` run
`33401785647` proves the exact TASK-007 candidate, all nineteen stable mappings and
the retained real second-UID gate. Reviewed arm64 `macos-26` run `34188886713`
proves the exact TASK-008 implementation, all twenty-one stable mappings and the
retained real second-UID gate at head
`7aeb032a75edbe85050cf470d910bc53a85d74cf`. Reviewed arm64 `macos-26` run
`34552988098` proves original TASK-009 implementation head
`fa7a0047c95c8b8eba12e859284223a1a78f51e2`; reviewed run `34554608874` proves
completion-gate correction `decfc82fadfd2a26221007fc67a5bc189845985d`;
correction `c3fa74a` closes the reproduced ledger-validation defects and reviewed
run `34559210695` covers exact descendant head
`05bce461b18fad6da77efe085913c1142c98c9e6`. TASK-011 exact PR head
`fc817200d2a60c88c4d16cc5e4c60a6be2dd1cbf` and merged main
`8416e01335e4fb8ff58e3381cf888fbcf69b9005` passed runs `34761111053` and
`34761648787`; current implementation authority is `NONE`. ADR-0013's MAINT-001 authority was
revoked after PR `#1` and merged-main formal verification. TASK-010 proposal v0.2.3
and ADR-0014 are complete under exact head
`e2311ed1dea992ce85db2547a1af799d0d8cf045` and reviewed run `34667611801`.
ADR-0017 and proposal v0.1.2 remain the completed TASK-011 contract.
TASK-012+, root rebind and Android SDK/system SQLite remain forbidden until their
owning gates permit them.
