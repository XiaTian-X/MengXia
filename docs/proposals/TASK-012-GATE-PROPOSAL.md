---
title: "TASK-012 managed Plugin executable and macOS sandbox start-gate proposal"
project: "梦夏 / MengXia"
document_role: "Draft TASK-012 macOS-edition implementation supplement"
status: "DEFERRED_NATIVE_CANDIDATE_FEASIBILITY_BLOCKED"
version: "0.2.9"
date: "2026-09-20"
canonical_specification_reviewed: "IMPLEMENTATION_SPEC.md v1.1.56"
repository_head_reviewed: "4fcf3470a6a98ad09feaf12152cee8c69740e467"
---

# TASK-012 Gate Proposal

## 0. Strict-profile gate verdict

CURRENT_PROJECT_NEXT_ACTION: DRAFT_BROKER_FOUNDATION_GATE

Latest completion: reviewed plan §5 pure foundation is DONE with exact PR/main
evidence in its §9. Implementation and product authority are NONE. Next is the
non-executing BROKER_FOUNDATION gate draft; older routing below is historical.
No R0/VM experiment or strict-backend acceptance is authorized.

Current routing amendment / 当前路线更新：ADR-0021 已接受审核准入原生插件方向。
纯准入基础已经完成；下一步起草 BROKER_FOUNDATION 的有界非执行合同 gate；
不再要求选择 VM 或重复原生硬内存探针。reviewed profile 明确接受剩余内存 DoS
风险，但不放宽文件/网络/IPC、身份、Broker/凭据与生命周期边界。产品权限仍为 NONE。
本文件下文的研究结论、候选与当时下一步属于保留的历史/严格 profile 范围，
不能覆盖新的方向，也不能作为 reviewed runtime 已合格的证据。


Scheduling update: the user prioritizes bounded R0-B research; latest results and
next evidence conditions are indexed in MACOS-NATIVE-SUPPORT-DEVELOPMENT-PLAN.md
section 9, not this product proposal. ADR-0020 retains built-in-first
macOS delivery. This unaccepted
Native candidate is DEFERRED and remains feasibility-blocked; it is not the gate
for the pure/non-executing foundations in Specification §0.7. A built-in execution
profile needs its own accepted scope and evidence and cannot inherit approval from
this candidate. Version 0.2.6 corrects routing only; the reviewed technical baseline
and qualification requirements of v0.2.5 remain preserved, not newly accepted.

ADR-0019 accepts separate macOS and Ubuntu editions under Specification §0.6.
This proposal covers only the macOS candidate, not the entire cross-platform
TASK-012 contract. The user's amended schedule is macOS first: Ubuntu intake and
development are deferred until macOS completion, then require their own foundation
and backend gates. Ubuntu never inherits a macOS PASS and adds no current macOS gate.
See `docs/proposals/DUAL-EDITION-DEVELOPMENT-PLAN.md`. All backend blockers and
candidate limits below are macOS-scoped unless explicitly stated as shared rules.

The macOS TASK-012 lane is not yet authorized for production implementation. Its completed
TASK-011 prerequisite and repository baseline pass, but three real pre-start gates
remain open:

1. `OQ-001`: dual-edition direction is accepted, but the exact arm64 macOS
   candidate runtime tuples need acceptance for implementation; production/release
   qualification follows implementation under §11; macOS is first, Ubuntu deferred;
2. `OQ-002`: the deprecated Apple Seatbelt launcher is only a candidate filesystem,
   network, process and IPC backend. No public, effective hard per-process memory
   mechanism has yet been proven on a candidate macOS tuple, so the required
   `resources=ENFORCED` vector is not currently constructible;
3. `OQ-006`: the remaining finite process, CPU, descriptor, writable-space and
   termination ceilings require acceptance after the memory mechanism is resolved.

This document supplies a bounded candidate and records its unresolved feasibility
gate rather than treating an RSS observer as hard enforcement. It does not
activate a task, accept an ADR, modify canonical status, or authorize production
spawn, kill, sandbox, installation or activation.

```text
TASK012_CANONICAL_GATE: NOT_ACCEPTED
TASK012_LIFECYCLE: BLOCKED
TASK012_IMPLEMENTATION_AUTHORITY: NONE
TASK012_PROPOSAL_VERSION: 0.2.9
TASK012_REPOSITORY_HEAD_REVIEWED: 4fcf3470a6a98ad09feaf12152cee8c69740e467
TASK012_PREREQUISITE_TASK011: DONE
TASK012_OQ001: EDITION_DIRECTION_ACCEPTED_EXACT_MACOS_TUPLE_OPEN
TASK012_OQ002: OPEN_SECURITY_FEASIBILITY_AND_USER_DECISION
TASK012_OQ006_PROCESS_RESOURCE_CAPS: OPEN_USER_DECISION
TASK012_NEXT_ACTION: DEFERRED_REOPEN_ONLY_WITH_NEW_EVIDENCE
TASK012_QUALIFICATION_BOOTSTRAP: TEST_ONLY_OBSERVATIONS
TASK012_PRODUCT_EVIDENCE_CONSTRUCTION: REVIEWED_MANIFEST_AND_FINAL_VALIDATION
TASK012_FULL_HOSTILE_SUITE_REQUIRED_AT: COMPLETION_NOT_IMPLEMENTATION_START
```

## 1. Repository, host and evidence baseline

The historical 2026-09-14 pre-start inspection established the following facts.
Its OS/tool and backend observations are not the current local tuple; §1.1
supersedes only that current-host interpretation, not historical delivery evidence:

- `main` equals `origin/main` at
  `4fcf3470a6a98ad09feaf12152cee8c69740e467`; the tracked worktree was clean;
- TASK-001 through TASK-011 are complete and every temporary implementation
  authority is `NONE`;
- `scripts/verify-repository.sh docs` passes;
- `scripts/verify-ci-fast.sh` passes as local non-formal evidence;
- host is arm64 macOS 26.6.2 build 25G83, Darwin 25.6.0;
- Xcode 26.6 build 17F113, SDK 26.5 and Rust/Cargo 1.98.0 are selected;
- the current hosted `macos-26` arm64 image advertises the same macOS 26.6.2
  build and Xcode/SDK tuple, but hosted sandbox evidence has not yet run;
- `mengxia-platform-sandbox` is an empty package boundary;
- `mengxia-plugin-host` contains only the completed caller-supplied-stream
  TASK-011 session and deliberately has no process authority;
- the SDK's public `sandbox.h` marks `sandbox_init` deprecated and "No longer
  supported"; `/usr/bin/sandbox-exec` and its manual are also explicitly
  deprecated;
- this host's `/usr/bin/sandbox-exec` is root-owned on the sealed system volume,
  Apple-signed as `com.apple.sandbox-exec`, CDHash
  `4828e16826baf4052b8212b82d1f3f2c13216303`, and SHA-256
  `abc5bb136d6b5cce8fa85d789f78e3326c51ca60cae637b2064adfb67a1dcd9a`;
- a read-only local probe confirms that a custom Seatbelt profile importing
  `system.sb` can launch on this exact host. This proves availability only, not
  the required containment vector;
- the SDK exposes `task_set_phys_footprint_limit`, but an ordinary same-user
  self-task probe returns `KERN_NO_ACCESS` (`8`). Current Apple XNU first calls
  `proc_check_footprint_priv`, and the relevant kernel privilege is
  `PRIV_VM_FOOTPRINT_LIMIT`; TASK-012 has no root/private entitlement authority.

Apple's supported App Sandbox is kernel-enforced and code-signature driven, but
its public contract is designed for signed application targets and inherited
helpers. It does not provide a public per-launch API for the dynamic, run-specific
read-only/read-write path policy required here. Apple states that inheriting child
targets must contain exactly `com.apple.security.app-sandbox` and
`com.apple.security.inherit`; inherited dynamic file grants are not carried to the
child. See:

- <https://developer.apple.com/documentation/xcode/configuring-the-macos-app-sandbox>
- <https://developer.apple.com/library/archive/documentation/Miscellaneous/Reference/EntitlementKeyReference/Chapters/EnablingAppSandbox.html>
- local SDK `usr/include/sandbox.h` and `man sandbox-exec`.

Availability of a command or entitlement string is not acceptance evidence under
Specification §12.4. The real hostile matrix in §15 remains mandatory.

### 1.1 Upgraded local host and review evidence

Observed on 2026-09-15 and OS/Xcode/SDK rechecked on 2026-09-20: arm64 macOS
27.0 build 26A428, Darwin 27.0.0; full Xcode 27.0 build 27A266a and SDK 27.0.
This is an active full-Xcode change, not merely installation of standalone CLT.
The 2026-09-15 tool observation emitted fingerprint
`426c653e1d4d1b82909959202f2bda9bb88e273277dac321e6feb9fef4510841` and
`ATTESTATION_MATCH: NO`. Under ADR-0016 that mismatch is not a developer failure.

At repository head `4fcf3470a6a98ad09feaf12152cee8c69740e467`, the following
2026-09-15 commands passed: `scripts/dev-toolchain.sh verify` (fresh native build
and SQLite identity/hardening), `scripts/dev-toolchain.sh test` (workspace,
all-target/all-feature, locked/offline), `scripts/dev-toolchain.sh fast` (format,
check, Clippy and maintenance regressions), `scripts/verify-repository.sh docs`
and `scripts/verify-ci-supply.sh` (current Cargo advisory/license/source checks).
The workspace invocation retains the explicitly ignored formal scaling and real
second-UID tests; it did not execute those obligations. Tool-security coverage
remains PARTIAL/UNKNOWN, not a complete Apple/Rust/tool advisory assessment.
These results do not qualify TASK-012, change hosted attestation, prove release
compatibility or reopen any completed task.

The current local `sandbox-exec` observation was Apple identifier
`com.apple.sandbox-exec`, SHA-256 CDHash
`1266e34f192210d9f1ac6dbc9cf297ecff93e568` and file SHA-256
`58839ef01b4eef8aac0d2aa8f9d1c074ae45aafe3533965b030672450064acc8`.
These are observed bytes, not an accepted backend entry. Neither this tuple nor
the historical tuple in §4.2 has complete TASK-012 hostile-suite qualification.

The 2026-09-15 isolated probes established only these bounded counterexamples:

| Probe | Observed result | Consequence |
|---|---|---|
| ordinary self-task `task_set_phys_footprint_limit(..., 128, ...)` | KERN_NO_ACCESS (8) | OS upgrade does not resolve hard-memory enforcement |
| guest lookup for the current process with architecture 7 and twenty zero hash bytes | lookup and dynamic validity both return 0 | selector presence/success is not architecture/hash verification |
| new group via POSIX_SPAWN_SETPGROUP, then child joins its parent's group | setpgid succeeds and group changes | group creation alone cannot prove cleanup containment |
| same escape after POSIX_SPAWN_SETSID | EPERM, group unchanged | candidate session isolation fixes this specific escape; full lifecycle proof still required |
| exact-path exec allow under deny-default/system.sb, with fork/network denied | same-image exec succeeds | exact-path permission is not one-shot exec permission |

These probes were not the complete production profile, launcher or resource
matrix. In particular, a simple same-image exec success does not prove persistent
containment or hard-memory enforcement across exec. §§6–9 and §15 state the
remaining mandatory tests. No temporary local probe path is a release dependency.

## 2. Gap and blocker classification

| ID | Classification | Scope | Disposition |
|---|---|---|---|
| `TASK012-BLOCKER-001` | `ARCHITECTURE / SECURITY / PLATFORM_FEASIBILITY` | macOS TASK-012 lane | prove a public effective hard memory boundary on the exact macOS tuple or separately accept an alternative macOS architecture; no VM is selected, and Ubuntu qualification is independent; RSS polling and `RLIMIT_AS` are insufficient |
| `TASK012-BLOCKER-002` | `DECISION_REQUIRED / SECURITY / RELIABILITY` | macOS TASK-012 lane | after blocker 001, user must accept or amend this backend's finite cap set in §10 |
| `TASK012-BLOCKER-003` | `DECISION_REQUIRED / PLATFORM_SCOPE` | macOS TASK-012 lane | accept candidate arm64 macOS tuples for bounded implementation; §11 full qualification follows, before production eligibility/DONE; Ubuntu remains deferred |
| `TASK012-GAP-001` | `EXPECTED_GAP` | implementation | empty platform sandbox crate is the work of this task after acceptance |
| `TASK012-GAP-002` | `EXPECTED_GAP` | implementation | managed executable primitive and exact launched-image proof are the work of this task |
| `TASK012-GAP-003` | `SPEC_STALE` | canonical traceability | TASK-012 has no stable task-specific TEST registry or start record |
| `TASK012-GAP-004` | `SPEC_STALE` | architecture evidence | TASK-011 architecture test must be refined from “host never has process authority” to “only accepted TASK-012 process module has authority” |
| `TASK012-GAP-005` | `EXPECTED_GAP` | CI evidence | real hosted sandbox/kill/resource evidence must be added before DONE |
| `TASK012-GAP-006` | `UNKNOWN / EXTERNAL_COMPATIBILITY` | future OS updates | deprecated launcher/profile compatibility is accepted only for closed runtime tuples and fails closed after drift |
| `TASK012-GAP-007` | `CONFLICT` | candidate launch contract | new-session ownership and explicit post-query identity comparisons replace disproven assumptions; production/hosted evidence remains required |
| `TASK012-GAP-008` | `CONFLICT` | candidate exec contract | one immutable image in one process replaces unsupported one-shot-exec claim; §8.1 containment/budget tests and future ADR acceptance required, not yet ENFORCED |
| `TASK012-GAP-009` | `SPEC_STALE / UNKNOWN` | current host versus historical tuple | §1.1 records macOS 27 separately; compatibility PASS is not sandbox qualification and must not rewrite CI attestation |
| `TASK012-DEFERRED-001` | `DEFERRED` | TASK-013 | authenticated install, durable package state, grants, revocations and audit |
| `TASK012-DEFERRED-002` | `DEFERRED` | TASK-014+ | tool-child execution; TASK-012 V1 denies fork and execution of other images; same-image exec is bounded by §8.1 |
| `TASK012-DEFERRED-003` | `DEFERRED` | TASK-016 | Network/Asset Broker and positive egress authorization |
| `TASK012-DEFERRED-004` | `DEFERRED` | TASK-023 | additional OS versions and release support matrix |
| `TASK012-DEFERRED-005` | `DEFERRED_BY_USER / OUTSIDE_MACOS_SCOPE` | Ubuntu foundation and backend work | starts after macOS completion; Linux filesystem, IPC, launched-image and sandbox still require independent exact kernel/distribution/architecture evidence |

No completed task is reopened. Absence of TASK-012 code is an expected gap, not a
repository defect.

## 2.1 Reviewed-profile supersession boundary

ADR-0021 introduces a separate scoped route. This proposal's hard-memory feasibility
block and all-ENFORCED evidence remain correct for its original strict candidate,
not prerequisites to the new pure admission foundation. Do not implement this
proposal's process path under the reviewed label or reuse its inactive start record.
Custody, pre-initialization file/network/IPC denial, descriptor hygiene and lifecycle
sections are design inputs requiring requalification; the selected mechanism and
caps must be fixed in the later reviewed execution gate. New acceptance is AC-104
through AC-108; old AC-020/AC-021/AC-022 evidence is unchanged. The reviewed plan §5
defines the first exact candidate scope, including compatibility-preserving
refactoring when justified. No TASK-012 full completion or product authority follows.

## 3. Exact ownership and non-ownership

TASK-012 owns only:

1. a lower-level, non-product managed executable import/custody primitive;
2. exact executable length, SHA-256, Mach-O and code-signature validation;
3. a typed `SandboxPolicyV1` and immutable `PluginProcessLimits` DTO;
4. one arm64 macOS Seatbelt backend with no fallback;
5. the trusted launch bridge, process-group ownership, process admission and
   bounded terminate/kill/reap lifecycle;
6. dynamic code-signature/process identity proof before a process is admitted to
   the TASK-011 protocol session;
7. `SandboxEvidence` for filesystem, network, process, IPC and resource dimensions;
8. real hostile, tamper, overload, cancellation and unavailable-backend evidence.

Crate direction is explicit. `mengxia-platform-fs` remains a downward-only safe
leaf and adds only descriptor-relative root, staging, intent, hash and durable
publication primitives over a locally defined `ExpectedExecutableBytes`
(target/length/SHA-256) value. It does not depend on `mengxia-plugin-package`, parse
Mach-O, call Security.framework or decide signature trust. The private macOS module
in `mengxia-platform-sandbox` consumes those safe retained tokens and owns Mach-O,
static/dynamic code-signature validation, launch and OS enforcement. The
`mengxia-plugin-host` adapter is the only production caller that maps a
`RuntimeDependencyDeclaration` into the lower-level expected-bytes value and invokes
the sandbox crate's closed `stage -> validate -> publish -> launch` orchestration.
The sandbox crate is the sole production caller of the filesystem publication
primitive. Architecture tests reject any alternative production publisher, skipped
validation step, reverse dependency or public raw path/fd bridge.

TASK-012 does not own:

- package acquisition from a user/API, installation or activation endpoints;
- Admin authority, grants, revocations, audit persistence or migrations 0003/0004;
- Core Client/Admin proto, CLI, daemon composition, SQLite or CAS access;
- Broker leases, positive network egress, Credentials or Provider integration;
- arbitrary helper/tool execution; all child creation is denied in this baseline;
- a `TRUSTED_NATIVE` shortcut for user-installed third-party code;
- cross-platform or macOS-build-wide release claims;
- Linux-specific namespaces, LSM, seccomp, cgroup or process APIs;
- terminal PASS for AC-020 or AC-023.

TASK-013 will compose the primitive with authenticated installation and audit.
TASK-014 or a later accepted ADR must add a brokered/host-owned tool-process model;
it may not silently widen this task's no-child policy.

## 4. `OQ-002` candidate decision

### 4.1 Alternatives

**A — exact Apple Seatbelt launcher plus a separately proven hard resource
boundary; current candidate, not yet feasible.** Use the Apple-signed
`/usr/bin/sandbox-exec` only on closed, tested arm64 macOS runtime tuples. Compile
one deny-default profile per launch; fail closed on any tuple, signature,
profile-compile, self-test or evidence mismatch. Basic Seatbelt launch is observable
on the historically inspected host, not proof of the complete candidate policy;
the backend is deprecated,
requires requalification after OS drift and does not itself establish a hard memory
ceiling. Option A remains blocked until §10.1 is closed with public API and real
cap/cap+1 evidence.

**B — App Sandbox only.** Rejected for this task because the public entitlement
model cannot independently express the required dynamic per-run filesystem view,
and inheritance does not carry dynamic file grants. A redesign around signed XPC
services, per-run containers or mounted volumes would change packaging and process
architecture and needs its own proposal.

**C — deny third-party Native execution on macOS V1.** This is the safest fallback
if A is not accepted. It preserves SEC-002 but cannot satisfy TASK-012's terminal
AC-021/AC-022 and therefore requires rescoping the task rather than pretending it
is complete.

**D — VM, capability runtime or Linux execution backend.** Potentially provides a
stronger long-term resource boundary, but changes the executable target and/or
deployment architecture. It requires a separate product decision and proposal; it
is not silently substituted into TASK-012.

### 4.2 Candidate backend identity

If A is accepted, backend identity must contain every field illustrated below.
This is the historical 2026-09-14 candidate observation, not a current-host claim
or an accepted allowlist. The upgraded local tuple is separately recorded in §1.1:

```text
backend_name = APPLE_SEATBELT_SANDBOX_EXEC
backend_contract_version = 1
architecture = arm64
os_product_version = 26.6.2
os_build = 25G83
kernel_release = Darwin 25.6.0
sandbox_exec_path = /usr/bin/sandbox-exec
sandbox_exec_identifier = com.apple.sandbox-exec
sandbox_exec_cdhash_algorithm = sha256
sandbox_exec_cdhash = 4828e16826baf4052b8212b82d1f3f2c13216303
sandbox_exec_sha256 = abc5bb136d6b5cce8fa85d789f78e3326c51ca60cae637b2064adfb67a1dcd9a
policy_language = Seatbelt profile version 1
policy_schema = mengxia-seatbelt-v1
hostile_suite = mengxia-task012-hostile-v1
```

The proposal does not assume the hosted binary has the same digest. Before the
implementation-start gate, record observed candidate local/hosted tuples and the
primitive feasibility evidence required by §10.1. Candidate acceptance authorizes
only bounded implementation and qualification, not a production allowlist entry.
The identical complete hostile suite is required after implementation for every
tuple promoted to production eligibility. Unknown/unqualified tuple means
`SANDBOX_UNAVAILABLE` on the product path, never a compatibility guess or fallback.
Do not replace the old tuple with macOS 27 merely because developer tests pass;
each proposed local/hosted runtime requires its own §11 qualification. No accepted
runtime allowlist exists at this draft stage.

The backend executable must be opened and inspected through a root-owned,
non-writable, non-symlink system path, successfully validated by Security.framework,
and rechecked immediately before launch. `PATH`, `DEVELOPER_DIR`, aliases and
caller-supplied backend paths are forbidden.

### 4.3 Cross-platform contract and independent Ubuntu backend

This proposal, once accepted for implementation, develops only its candidate arm64
macOS backend; production eligibility still requires §11 qualification. The boundary above
it must remain platform-neutral. `SandboxPolicyV1`, `PluginProcessLimits`, process
admission, termination outcomes and the five `SandboxEvidence` dimensions must not
contain Seatbelt profile text, macOS paths, Darwin signal/wait encodings, Mach code
objects or macOS-only raw handles. Backend-specific identity and diagnostic details
remain private, typed implementation data.

The macOS implementation lives behind a compile-time platform module and is the
only constructible production backend authorized by this proposal. Unsupported targets compile a
fail-closed capability probe where practical, but do not gain a fake or partial
implementation. Upper layers select no backend by string guessing and cannot
branch on OS-specific enforcement details.

The independent Ubuntu backend requires its own proposal, ADR, file scope, supply review
and real hostile evidence on a closed architecture/distribution/kernel tuple. Its
candidate enforcement stack may combine mount/user/PID/network namespaces,
Landlock or another accepted LSM, seccomp, capability removal, `no_new_privs`,
cgroup v2 and race-safe process supervision. No one item is sufficient evidence:
seccomp is syscall-surface reduction rather than a complete sandbox, namespaces
are not a fine-grained access-control policy, and cgroup v2 is a resource/process
controller rather than filesystem/network confinement. Missing kernel features,
unavailable delegation or incomplete evidence must produce
`SANDBOX_UNAVAILABLE` with no unsandboxed fallback.

The common hostile-suite specification and fixture behavior are reusable, but a
macOS PASS never implies a Linux PASS. Linux local-filesystem/ownership, IPC peer
identity and executable-custody rules likewise require platform-specific evidence;
this proposal neither chooses ext4/XFS/Btrfs nor authorizes a remote/network
filesystem for active SQLite.

## 5. Managed executable custody

The filesystem primitive consumes an already-open regular source descriptor, an
already-open `ManagedExecutableRootAuthority` and an `ExpectedExecutableBytes`
value containing the exact target, length and SHA-256. The host constructs that
value from the exact `RuntimeDependencyDeclaration`; the filesystem crate never
imports the package crate. No API accepts a source pathname or persisted raw root
path as authority. TASK-012 defines the opaque authority's validation/consumption
contract and a crate-private/test-only constructor sufficient for real hostile and
recovery evidence, but deliberately exposes no production root factory. TASK-013
must separately authorize the root request from the live Library owner/lock, prove
it disjoint by retained identity and ancestry from the Library and Blob roots,
choose/configure its canonical location, durably provision it, and expose the only
production constructor together with authenticated installation persistence. No
TASK-012 test helper or feature can construct a production authority.

When TASK-013 later supplies one, the provisioned root must be local APFS with
ownership checking enabled, owned by the durable Library owner, mode `0700`, empty
extended ACL and a retained stable device/inode. It contains exactly
`.mengxia-plugin.lock` (`0600`) and owner-only
`objects/sha256`, `staging` and `intents` directories (`0700`). Missing
lock with any other content, unknown entry, symlink, mount crossing, case alias,
wrong owner/mode/ACL, replacement or changed root instance fails closed before a
write. The implementation acquires the retained lock and then re-enumerates the
complete top-level state plus the bounded `intents`/`staging` recovery namespaces;
it never uses a pre-lock snapshot. It reads one recovery entry at a time and rejects
immediately on the 65th, so memory is not proportional to directory size. The
potentially growing `objects/sha256` directory is not globally enumerated on startup;
only the exact validated digest child is opened on demand and independently checked.

Import contract:

1. validate declaration role is `PLUGIN_ENTRYPOINT`, target is the accepted
   arm64 macOS target, byte length is nonzero and within §10;
2. treat the already-open source as untrusted bytes: require a readable regular
   file descriptor, capture its identity/size metadata, and never reopen a source
   pathname. Source owner, mode and ACL are not trust claims. V1 accepts only a
   platform-validated local mount (including a qualifying removable local volume)
   and rejects network/FUSE/unknown mounts because one blocking remote read cannot
   satisfy the joined lifecycle contract. A future NAS path must first use its own
   bounded acquisition/spool gate and then supply a local descriptor; neither such
   source may become the managed executable root or active Library authority;
3. under the post-lock enumeration, either independently validate and return an
   existing canonical object without mutation, or prove the exact digest directory
   is absent. An empty/pre-existing digest directory fails closed. Only the proven-
   absent case samples one private `ImportAttemptIdentity` from explicit clock and
   OS-entropy seams, then creates and durably publishes the exact import intent
   described below before the staging directory entry is created;
4. stream exactly the declared length plus a one-byte oversize probe with O(buffer)
   memory into an owner-only staging file, computing SHA-256; require exact
   length/digest and matching before/after descriptor identity/size metadata. Any
   unstable, short, long or mismatching source is rejected and never published;
5. pass the retained staged-file token to the private sandbox validator, which
   validates a thin arm64 Mach-O executable; universal/fat, script, interpreter,
   dylib and bundle inputs are rejected in V1;
6. obtain the non-constructible `StaticExecutableEvidence` only after a valid
   embedded Apple code signature with no entitlement dictionary and no resource
   envelope; publisher identity remains untrusted metadata;
7. require that exact staged-file token and `StaticExecutableEvidence` in the
   sandbox crate's sole production publication orchestration, then call the leaf
   filesystem transaction to change staging from `0600` to `0500`, full-sync it and
   promote by no-replace link into the exact digest namespace. A newly created digest
   directory is full-synced with its `sha256` parent before linking; after link, the
   digest directory is
   full-synced, canonical content/identity/signature evidence is revalidated in the
   transitional two-link state, staging is unlinked and its directory full-synced,
   canonical is revalidated at link count one, then intent is unlinked and its
   directory full-synced;
8. on dedup, the existing object must independently match length, SHA-256,
   filesystem policy, Mach-O and code signature before success;
9. any state whose ownership or ordering cannot be proven preserves evidence and
   closes that custody instance.

For V1, an embedded ad-hoc or certificate-backed Mach-O signature is accepted only
as content/page identity, never as publisher trust. The candidate static sequence is
`SecStaticCodeCreateWithPathAndAttributes` with the required arm64 architecture,
followed by `SecStaticCodeCheckValidityWithErrors` using exactly
`kSecCSStrictValidate | kSecCSCheckAllArchitectures`; none of
`kSecCSDoNotValidateExecutable`, `kSecCSDoNotValidateResources`,
`kSecCSBasicValidateOnly` or `kSecCSAllowNetworkAccess` is permitted. Signing and
content dictionaries are then obtained with exactly
`kSecCSSigningInformation | kSecCSContentInformation` and validated against a
closed type/field policy. The implementation requires one CodeDirectory covering
the complete thin main executable and rejects detached signatures, absent,
ambiguous or unknown digest algorithms, entitlement blobs whether empty or
non-empty, resource envelopes, library-validation exceptions and dynamic-code
allowances. `kSecCodeInfoUnique` is retained as algorithm-tagged opaque bytes; its
length is not assumed to be 32 bytes because Security.framework documents that the
algorithm may change. Certificate chain, Team ID and notarization do not grant
authority in TASK-012. Golden ad-hoc and certificate-backed fixtures plus every
rejected signature shape must pass before this sequence is frozen in ADR-0018.
The Mach-O parser performs checked offset/length arithmetic with no mmap of the whole
file: at most 4096 load commands, 16 MiB total load-command bytes, 16 MiB embedded
signature bytes and 64 SuperBlob index entries are accepted. Duplicate/overlapping/
out-of-order regions, trailing ambiguity, unknown mandatory slot or page coverage,
and any read outside the declared file fail validation. These parser caps are fixed
TASK-012 safety constants and are included in `TEST-CUSTODY-012` cap-1/cap/cap+1
and malformed-offset fixtures.

Canonical layout under an opaque owner-only Plugin executable root:

```text
objects/sha256/<lowercase-64-hex-digest>/entrypoint
staging/<128-bit-random-import-id>.partial
intents/<128-bit-random-import-id>.intent
```

Each import uses `<id>.intent` and `<id>.partial`. Intent V1 is exactly 256 bytes:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | ASCII `MXPOBJ01` |
| 8 | 2 | little-endian version `1` |
| 10 | 2 | little-endian record length `256` |
| 12 | 4 | zero flags; V1 means digest directory proven absent at admission |
| 16 | 16 | import UUID bytes |
| 32 | 32 | expected executable SHA-256 |
| 64 | 8 | little-endian expected byte length |
| 72 | 8 | little-endian root device |
| 80 | 8 | little-endian root inode |
| 88 | 8 | little-endian signed creation seconds |
| 96 | 4 | little-endian nanoseconds `0..999999999` |
| 100 | 124 | all-zero reserved bytes |
| 224 | 32 | SHA-256 over bytes `0..223` |

The mandatory golden vector uses UUID
`018bcfe5-687b-7c4d-8e9f-1029384756ab`, 32 digest bytes `0x11`, byte length
`1234`, device `42`, inode `99`, seconds `1700000000` and nanoseconds
`123456789`. Its checksum field is
`3c4b69196fa431552cefce6c12bedf302eb5f5fe99bd6a556f7555cacd025d2a` and
the SHA-256 of all 256 bytes is
`b873e407723d51b307d08af6992068635340a017310a0e159644092ecbf57170`.
Tests independently assemble and parse these bytes; they do not obtain expectations
from the production encoder.

The ID is a canonical RFC 4122 UUIDv7 whose timestamp is derived from the same one-
time clock sample stored in the intent; entropy fills its random fields. Clock-before-
epoch, UUIDv7 range, nanosecond-range or entropy failure occurs before mutation and
maps to `ID_GENERATION_UNAVAILABLE`. An `O_EXCL` collision is not regenerated or
cleaned blindly; it is an unsafe namespace state and fails closed as
`STORAGE_CONFIGURATION_ERROR`. Deterministic clock/entropy/collision seams are test-
only and cannot be selected by a production feature or environment variable.

Decode validates exact length/magic/version/flags/reserved/checksum, RFC variant and
version, UUID timestamp equality to the stored seconds/nanoseconds at millisecond
precision, nonzero bounded byte length, digest-to-directory equality and checked
native-device/inode conversion to `u64`. Any overflow, impossible timestamp or
mismatch is a malformed intent; no field is accepted merely because the checksum is
valid.

The intent is `O_CREAT|O_EXCL|O_NOFOLLOW`, mode `0600`, fully written,
`F_FULLFSYNC`ed and followed by an `F_FULLFSYNC` of the `intents` directory before
the staging name is created.
UUID/time failure maps to `ID_GENERATION_UNAVAILABLE`. At most 64 incomplete
intents are admitted during one open; the 65th or any malformed/unknown entry fails
closed with `STORAGE_CONFIGURATION_ERROR` and performs no cleanup.

Recovery under the exclusive root lock is exact:

| Observed state for one ID | Result |
|---|---|
| valid intent only, no canonical | remove its optional exact empty digest directory and full-sync `sha256`; unlink intent and full-sync `intents` |
| valid intent + matching partial, no canonical | accept only mode `0600` pre-validation or `0500` pre-link; unlink partial/full-sync `staging`; remove optional exact empty digest directory/full-sync `sha256`; then unlink intent/full-sync `intents` |
| valid intent + partial + matching canonical | require both names identify one inode with link count two; revalidate bytes/signature, unlink partial/full-sync `staging`, require canonical link count one, then unlink intent/full-sync `intents` |
| valid intent, no partial, independently valid canonical | require canonical link count one, then unlink intent and full-sync `intents` |
| independently valid canonical with no intent/partial | normal dedup object |
| partial without valid intent; invalid intent; mismatch; unexpected link count/type/name | fail closed, delete nothing |

An empty digest directory is removable only when its valid intent names that digest
and every contained entry is exactly the expected staging/canonical state. Crash and
fault tests cover every create/write/full-sync/link/unlink boundary. Normal canonical
files are mode `0500`, link count one, owner-only, empty ACL, no xattrs or BSD flags;
staging is `0600` until final validation and changes once to `0500` immediately before
its final full-sync/no-replace publication sequence.

Every “full-sync” above reuses the accepted macOS `F_FULLFSYNC` primitive and its
startup support probe from the local-storage boundary; ordinary `fsync` is not
silently substituted. Deterministic syscall-fault and SIGKILL tests verify ordering
and same-running-OS recovery. Hardware power loss remains explicitly
`UNVERIFIABLE` in ordinary CI; the implementation claims the documented
`F_FULLFSYNC` ordering contract, not that CI simulated physical media failure.

All opens are descriptor-relative, no-follow and exact-case. The safe filesystem
token passed to the sandbox validator contains retained root/staged-object
descriptors plus digest, length and device/inode evidence; it contains no signature
claim. The sandbox's `StaticExecutableEvidence` is bound to that exact token and is
consumed by the sandbox publication orchestration before the leaf commit call.
Neither token has `Clone`, serialization, `Display`, a public raw fd/path or an
unchecked constructor. TASK-013 later owns authenticated
source acquisition and durable installation records; it must consume this primitive
without weakening it.

## 6. Object-to-launched-image binding

macOS has no public `fexecve`/`execveat`. A pre-spawn pathname hash is therefore
insufficient. The proposed candidate requires two independent bindings:

1. **custody binding:** retained descriptor identity, exact SHA-256/length and
   validated static code signature are rechecked before spawn;
2. **dynamic binding:** before protocol admission, Security.framework obtains the
   live code object for the still-owned, unreaped child through
   `SecCodeCopyGuestWithAttributes`, using its exact PID and, only when obtained
   from a validated OS source, its audit token. A Plugin-provided PID/audit token
   is never accepted. One supervisor owns wait/reap; no other waiter, SIGCHLD
   auto-reap setting or early reap may make that PID reusable during verification.
   A raw PID number without this retained child ownership is not authority.
   Architecture and `kSecGuestAttributeHash` selector values are not trusted
   constraints: the observed API can ignore them and still return success. The
   candidate must not depend on these extra selectors being recognized. The
   returned dynamic code is validated, its
   `SecCodeCheckValidityWithErrors` call uses exactly `kSecCSDefaultFlags` and a null
   requirement. `SecCodeCopyStaticCode` then uses `kSecCSDefaultFlags`, and
   signing/content extraction from that returned static object uses exactly the same
   closed `kSecCSSigningInformation | kSecCSContentInformation` flags as static
   validation. Its
   `kSecCodeInfoUnique`, digest algorithm, architecture and signing information must
   equal the retained managed object, and its path must resolve to the exact retained
   object whose current device/inode/length/file SHA-256 still match. Expected
   values come from custody, never from the query dictionary or Plugin report.
   Missing or unsupported returned evidence, multiple matches or any type/length
   mismatch fails closed. Arm64 verification uses the actual returned image/Mach-O
   evidence, not an assumed public dictionary key or requested architecture.

The negative matrix must include a valid PID with deliberately wrong architecture
and hash selector values. On the observed macOS 27 tuple lookup succeeds; never
treat that success as binding to those requested values. If another candidate tuple
rejects such selectors instead, record that behavior without a permissive fallback.
Independently mismatch each expected post-query field and
require rejection before stream consumption. Test exit, exec and pathname races
throughout lookup/validity/static extraction/final comparison. An error, stale
dynamic object or loss of child ownership yields no admission. Apple SDK headers
and the published `KernelCode::locateGuest` implementation explain why supported
guest-selection keys and returned identity must be treated separately:
<https://github.com/apple-oss-distributions/Security/blob/main/OSX/libsecurity_codesigning/lib/cskernel.cpp>.

The launcher first applies the sandbox and then executes the managed object. Both
the process permit and the existing TASK-011 protocol permit are acquired before
spawn. The host does not consume stdout or stderr before dynamic binding; the
kernel pipes are the bounded pre-admission buffer, so an early writer may block but
cannot allocate host memory or enter `PluginSession`. After successful binding the
unchanged streams and retained protocol permit are moved exactly once into
`SessionPermit::open`, using the original launch deadline. Mismatch triggers bounded
kill/reap and `STORAGE_CORRUPTION`; unknown, unavailable or timed-out dynamic
evidence triggers `SANDBOX_UNAVAILABLE` or `DEADLINE_EXCEEDED` as specified in §13.
In every failure case no Plugin frame is decoded and no session is admitted.
The consumed `SandboxRunView` is also irreversibly tainted on every failure before
admission: none of its output/tmp bytes may be inspected as a Plugin result,
published or reused. The supervisor retains its descriptors through reap and
returns only a redacted terminal disposition to the future owning composition,
which must perform descriptor-relative cleanup. A successfully admitted view remains
bound to that one process/session and likewise cannot be reused.

The test race replaces or mutates the pathname at every seam between final recheck,
launcher exec, Plugin exec and dynamic verification. A different image may execute
only inside the already-applied deny-default sandbox and must never cross admission.
The final accepted ADR must state whether code-directory equality is considered the
exact launched-image proof. If not, Option A is infeasible on current public macOS
APIs and the task remains blocked.

## 7. Launch bridge and descriptor inheritance

The host never calls a shell. It first consumes an opaque `SandboxRunView` containing
one retained owner-only local-APFS run-directory descriptor, the exact pre-created
input/output/tmp file descriptors and a checked process-supervisor writable-budget
permit. The permit reserves the configured logical worst-case bytes against all
admitted processes; it is not a false claim that other host activity cannot exhaust
the physical volume. Those file descriptors remain parent-side authority tokens and
are not inherited by the Plugin; only the retained run-directory descriptor is
inherited temporarily as fd 5.
The caller, eventually TASK-013/TASK-015 composition, owns creating that view;
TASK-012 only revalidates and consumes it. One small first-party
`mengxia-plugin-launcher` executable is launched by absolute verified path. It
performs only:

- decode one fixed-size versioned binary limit record from inherited fd 3;
- set hard and soft `RLIMIT_CPU`, `RLIMIT_FSIZE`, `RLIMIT_NOFILE` and
  `RLIMIT_CORE=0`, plus the separately accepted hard memory primitive required by
  §10.1; absence or failure of that primitive prevents spawn;
- verify the `POSIX_SPAWN_SETSID`-created session has `SID == PGID == PID`, then
  set the fixed umask and signal defaults. Use SETSID, not a SETPGROUP-only
  fallback or a post-spawn `setsid`/`setpgid` repair race. Every attribute call and
  resulting identity must be checked; unsupported session creation denies launch;
- close every descriptor except fd 0 (TASK-011 host-to-Plugin), fd 1 (Plugin-to-host),
  fd 2 (discard-only stderr), fd 3 (limit record, closed after decode), fd 4
  (launcher-status write end, `CLOEXEC`) and fd 5 (retained run directory, closed
  immediately after `fchdir`);
- clear environment, set the closed §12 environment, `fchdir` through fd 5 and
  `execve` the exact `/usr/bin/sandbox-exec` argv.

It performs no filesystem discovery, dynamic allocation after reading the fixed
record, logging, network, package parsing or policy decision. Failure writes one
fixed binary status code to a dedicated close-on-exec status pipe and `_exit`s.
The parent treats pipe EOF without a complete error record only as
`LAUNCHER_STATUS_CLOSED_WITHOUT_REPORTED_ERROR`; it does not prove that the helper's
error write succeeded, the frontend was executed, the sandbox was applied or the
Plugin was executed. The sole positive transition becomes admissible only when the
exact live managed Plugin image and complete sandbox evidence are observed and
validated within the launch deadline. The helper
is first-party product code, not a Plugin runtime dependency, and is covered by the
same locked/signed build evidence as other binaries.

The implementation must use `posix_spawn` with
`POSIX_SPAWN_CLOEXEC_DEFAULT | POSIX_SPAWN_SETSID`, a new session and its process
group, exact file actions and reset
signals. Rust unsafe/FFI is isolated in `mengxia-platform-sandbox` and backed by a
checked-in narrow C ABI shim/probe; no generic spawn or raw-fd API is public.
The private backend retains the child/session/group ownership record until reap.
The real matrix must attempt joining parent/sibling groups, creating a new session,
group changes across exec, and exit/signalling races. SETSID's successful local
counterexample alone is not evidence for the whole process dimension.

Limit-record V1 is exactly 128 bytes and is decoded without allocation:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | ASCII `MXLIM001` |
| 8 | 2 | little-endian version `1` |
| 10 | 2 | little-endian record length `128` |
| 12 | 4 | zero flags |
| 16 | 8 | little-endian `cpu_seconds` |
| 24 | 8 | little-endian `file_size_bytes` |
| 32 | 8 | little-endian `open_files` |
| 40 | 8 | little-endian `memory_bytes` |
| 48 | 4 | little-endian accepted memory-backend contract version |
| 52 | 44 | all-zero reserved bytes |
| 96 | 32 | SHA-256 over bytes `0..95` |

Unknown version/flags/backend, nonzero reserved bytes, noncanonical integer range,
short/long record, checksum mismatch or trailing byte is a launcher setup failure.
Every applicable soft/hard rlimit is set to the same validated value and read back
before continuing. The 16-byte status record is ASCII `MXLS`, little-endian version
`1`, length `16`, one closed nonzero `u32` code and zero reserved `u32`. Codes distinguish
the following failures exactly:

| Code | Meaning | Parent ErrorCode |
|---:|---|---|
| 1 | limit record decode/canonicality failure | `INTERNAL_ERROR` |
| 2 | accepted hard-memory backend application failure | `SANDBOX_UNAVAILABLE` |
| 3 | rlimit application or readback mismatch | `SANDBOX_UNAVAILABLE` |
| 4 | session/process-group/child identity mismatch | `INTERNAL_ERROR` |
| 5 | inherited descriptor inventory mismatch | `INTERNAL_ERROR` |
| 6 | fixed environment/umask/signal setup failure | `INTERNAL_ERROR` |
| 7 | run-directory transition or revalidation failure | `STORAGE_CONFIGURATION_ERROR` |
| 8 | frontend `execve` failure | `SANDBOX_UNAVAILABLE` |

A partial, duplicate, trailing, zero or unknown status record is `INTERNAL_ERROR`;
clean EOF without bytes has only the non-authoritative meaning stated above. The accepted
hard-memory design must refine the backend-version field and parent/helper ownership
without changing the remaining wire bytes; until then no nonzero backend version is
valid for production launch.

## 8. Seatbelt policy and filesystem view

Every policy begins with `(version 1)`, `(deny default)` and the reviewed system
baseline import. Allow rules are generated only from retained fixed-child tokens;
no raw caller string enters profile syntax. Profile string escaping is not an
authorization mechanism.

The sandbox receives host paths corresponding to this logical view:

| Logical role | Rights | Exact rule |
|---|---|---|
| Plugin entrypoint | read/execute only | one exact managed file and required system loader pages |
| Inputs | read-only | bounded pre-created exact files; no parent traversal/write/create/delete |
| Outputs | write/read metadata | bounded pre-created slots only; no create/delete/rename/hardlink/symlink |
| Tmp | write/read metadata | bounded pre-created slots only; no create/delete/rename/hardlink/symlink |
| Control/stderr | inherited descriptors only | no named Client/Admin/Broker endpoint lookup |

The retained run directory is local APFS with ownership checking enabled, owned by
the durable Library owner, mode `0700`, empty ACL and no symlink/mount transition.
Its entire V1 namespace is the closed ASCII set `input-0000`..`input-0063`,
`output-0000`..`output-0015` and `tmp-0000`..`tmp-0003`, truncated to the exact
counts carried by the view. Indices are four lowercase decimal digits with no gaps.
Every slot is a regular owner-owned file with empty ACL/xattrs/BSD flags and link
count one; inputs are mode `0400`, while output/tmp slots are `0600` and initially
length zero. Unknown names, aliases, sparse/pre-sized output, count mismatch or
descriptor/name/inode mismatch rejects the view before permits or spawn. Parent-side
descriptors are revalidated after reap before any later task may inspect results.

There is no general HOME, `/tmp`, Library, CAS, config, browser, SSH, AWS, Keychain,
Client/Admin socket or another session rule. The generated policy explicitly denies
all network operations, process fork, non-entrypoint exec, Mach task inspection,
ptrace, Apple Events and namespace mutation. A deny-default profile compilation
failure or required runtime-service uncertainty denies launch.

Output/tmp disk accounting is structural: the host pre-creates the finite slot set,
each slot is capped by `RLIMIT_FSIZE`, the profile forbids creating further files,
and a supervisor permit reserves `slot_count * per_slot_bytes` in checked arithmetic
until reap. Therefore the per-process and admitted-global worst cases are finite.
Physical `ENOSPC` remains possible and is handled as a failed run; the logical permit
is not represented as a filesystem reservation. No application-only after-write
quota is represented as OS enforcement.

`system.sb` is not trusted as a security policy. Its only purpose is minimum OS
runtime compatibility; explicit TASK-012 denies and the real hostile matrix must
prove the resulting effective policy. Any OS change invalidates prior evidence.

### 8.1 One process and one immutable image, not one-shot exec

The revised candidate permits exec of the same managed image in the same owned
process; it does not claim that a static exact-path allow rule can allow the first
exec and deny the second. Fork/posix_spawn/other process creation, shells, scripts,
the launcher/frontend as re-exec targets, other managed objects and tool binaries
remain denied. This candidate change is subject to the future accepted sandbox
ADR and does not enable a production path now.

For the entire child lifetime, the admitted canonical object and its parent
namespace must remain immutable to the Plugin and must not be replaced, modified
or retired by custody/composition. Retained identity and custody ownership last
through reap; descriptor retention alone is not a write lock. A same-image exec
must preserve the effective sandbox, session/group, accepted hard memory boundary,
CPU usage/limits, fd/file-size limits and writable-slot budget. The supervisor's
original absolute launch/wall/cleanup budgets, process/protocol permits, cumulative
stderr quota and in-flight request state never restart. There is still exactly one
host-owned protocol session, at most one handshake, and no implicit retry or new
attempt. A restarted Plugin sending an out-of-state Hello or duplicate response
is rejected by the existing TASK-011 contract; silence ends at the original deadline.

The fixed host argv/environment in §12 describes initial launch, not a claim that
hostile code cannot change its own argv/environment or open permitted slot fds
before exec. Such changes cannot add a capability, restore the closed launcher fds,
reacquire admission or relax the sandbox. Tests must exercise them, including
preserved/closed stdio and permitted file descriptors across exec.

Apple documents preservation of process/group identity, resource usage and limits
across exec, but that does not prove this backend's complete enforcement:
<https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/execve.2.html>.
The exact-tuple real suite must repeat filesystem/network/IPC denials after exec,
exercise bounded repeated-exec loops with original wall timeout, demonstrate
cumulative CPU accounting and unchanged hard limits, and deny alternate-image
exec. The eventual hard-memory mechanism must explicitly survive exec without a
reset window. Unavailable/failing evidence keeps the process/resources dimensions
non-ENFORCED and launch disabled; no polling-based one-shot detection is substituted.

## 9. Process, session and termination state machine

```text
AVAILABLE
  -> PROCESS_PERMIT_HELD
  -> PROTOCOL_PERMIT_HELD
  -> LAUNCHER_SPAWNED
  -> LAUNCHER_STATUS_CLOSED_WITHOUT_REPORTED_ERROR
  -> PLUGIN_IMAGE_AND_POLICY_VERIFIED
  -> PLUGIN_SESSION_ACTIVE
  -> COOPERATIVE_SHUTDOWN
  -> TERM_PROCESS_GROUP
  -> KILL_PROCESS_GROUP
  -> REAPED
  -> AVAILABLE
```

- Process and protocol admission are acquired before spawn. Process admission is
  retained until observed reap, and protocol admission is moved into TASK-011 only
  after live verification; both remain owned on every failure/cancellation/panic
  path.
- Exactly one process in its dedicated session/group is allowed. Fork and other
  process creation are denied. Initial and repeated exec may target only the same
  immutable managed image under §8.1, without fresh admission or budget.
- A Plugin exit, protocol terminal result, deadline, caller cancellation, dropped
  future, resource signal or supervisor panic transfers ownership to the same
  cleanup state machine.
- Cleanup sends the TASK-011 cooperative request when a session exists, waits its
  earlier deadline, sends `SIGTERM` to the owned session's exact process group,
  waits, sends `SIGKILL`, waits, then reaps the exact child. The sole waiter must
  serialize observed exit/reap with signalling: an already exited child goes to
  reap without a later group signal; a reaped identity is never used again. PID or
  PGID numbers alone are never cleanup authority, and a changed group is not adopted.
- Failure to prove group identity or reap closes the whole process supervisor to new
  admission and returns `INTERNAL_ERROR`; it never signals an unverified PID/group.
- No `Drop` performs a blocking wait. Dropping the public owner triggers the
  already-running bounded supervisor. TASK-012 exposes an explicit bounded
  `shutdown_and_join` owner contract; later daemon composition must call and verify
  it, but daemon integration is not claimed by this task.
- There is no automatic retry of spawn or Plugin work.

## 10. `OQ-006` candidate production caps

All configurable values are immutable and may only tighten these defaults/hard
ceilings. TASK-012 defines a source-free `PluginProcessLimits` DTO and pure validator;
it does not read CLI arguments, environment variables, Library config or defaults.
The future daemon/composition task owns the canonical
`CLI > environment > Library config > default` resolution, performs the one-time
typed/textual capture and supplies complete typed constructor arguments. TASK-012
rejects zero, out-of-range, overflowed or impossible typed combinations before any
directory creation or process spawn; nonnumeric and missing source errors remain the
future resolver's obligation and cannot be claimed by this task. The `MENGXIA_*`
names below reserve future Specification §16 keys; their presence does not authorize
TASK-012 to capture or parse an environment.

| Field / future key | Default and hard ceiling | Tightening range | Enforcement |
|---|---:|---:|---|
| `max_processes` / `MENGXIA_PLUGIN_MAX_PROCESSES` | 4 | 1..4 | host semaphore held to reap |
| `import_concurrency` / `MENGXIA_PLUGIN_IMPORT_CONCURRENCY` | 1 | 1 fixed | bounded blocking-worker admission |
| `import_buffer_bytes` | 1048576 fixed | not configurable | one fixed reusable copy/hash buffer per admitted import |
| `incomplete_imports_per_open` | 64 fixed | not configurable | recovery rejects on entry 65 |
| `processes_per_tree` | 1 fixed | not configurable | new session + denied child creation/other-image exec; §8.1 same-image exec never creates new admission |
| `executable_bytes` / `MENGXIA_PLUGIN_EXECUTABLE_BYTES` | 536870912 | 1048576..536870912 | bounded import + exact length |
| `macho_load_commands` | 4096 fixed | not configurable | checked parser rejects command 4097 |
| `macho_load_command_bytes` | 16777216 fixed | not configurable | checked parser total |
| `embedded_signature_bytes` | 16777216 fixed | not configurable | checked parser/read buffer ceiling |
| `signature_index_entries` | 64 fixed | not configurable | checked parser rejects entry 65 |
| `memory_bytes` / `MENGXIA_PLUGIN_MEMORY_BYTES` | 2147483648 provisional | 134217728..2147483648 | **BLOCKED:** no accepted hard macOS primitive; value cannot enable launch |
| `cpu_seconds` / `MENGXIA_PLUGIN_CPU_SECONDS` | 600 | 1..600 | `RLIMIT_CPU`; wall supervisor remains separate |
| `wall_timeout_ms` / `MENGXIA_PLUGIN_WALL_TIMEOUT_MS` | 3600000 | 1000..3600000 | monotonic host deadline |
| `import_timeout_ms` / `MENGXIA_PLUGIN_IMPORT_TIMEOUT_MS` | 300000 | 1000..300000 | complete admission/read/hash/publish/recovery attempt |
| `launch_timeout_ms` / `MENGXIA_PLUGIN_LAUNCH_TIMEOUT_MS` | 5000 | 100..5000 | backend preflight through live-image proof and TASK-011 open |
| `live_image_attempts` | 100 fixed | not configurable | at most 100 observations within launch deadline; no busy loop |
| `live_image_interval_ms` | 50 fixed | not configurable | first observation immediate; subsequent observations wait at least 50 ms and never pass deadline |
| `policy_bytes` | 131072 fixed | not configurable | generated profile and compiler input hard bound |
| `policy_rules` | 512 fixed | not configurable | closed generator refuses a larger semantic rule set |
| `open_files` / `MENGXIA_PLUGIN_OPEN_FILES` | 64 | 16..64 | `RLIMIT_NOFILE`, inherited fds included |
| `input_slots` / `MENGXIA_PLUGIN_INPUT_SLOTS` | 64 | 0..64 | pre-created exact read-only files |
| `output_slots` / `MENGXIA_PLUGIN_OUTPUT_SLOTS` | 16 | 0..16 | pre-created exact files |
| `tmp_slots` / `MENGXIA_PLUGIN_TMP_SLOTS` | 4 | 0..4 | pre-created exact files |
| `writable_slot_bytes` / `MENGXIA_PLUGIN_WRITABLE_SLOT_BYTES` | 268435456 | 1048576..268435456 | one truthful process-wide `RLIMIT_FSIZE` for every output/tmp slot |
| `term_timeout_ms` / `MENGXIA_PLUGIN_TERM_TIMEOUT_MS` | 2000 | 100..2000 | process-group TERM wait |
| `kill_reap_timeout_ms` / `MENGXIA_PLUGIN_KILL_REAP_TIMEOUT_MS` | 5000 | 100..5000 | process-group KILL/reap wait |

Default maximum writable payload is truthfully 5 GiB per process: 16 output plus 4
tmp slots, all governed by the single 256 MiB process-wide file-size limit. With four
process permits the supervisor's maximum admitted logical writable budget is 20 GiB.
The caller must reserve the exact per-view logical budget through the opaque
`SandboxRunView`; a missing, overflowed or mismatched permit disables launch. These
are safety ceilings, not physical free-space guarantees, performance targets or
supported-media-size SLOs. TASK-014 must demonstrate its selected FFmpeg workload
fits or propose a separately reviewed widening before use.

The hostile suite uses tightened limits and tests cap-1/cap/cap+1 without allocating
hard-ceiling memory or disk. Formal tests separately verify arithmetic at the hard
ceiling.

Every deadline is one monotonic absolute deadline, never a resettable per-step
timeout. The import deadline starts when import admission is requested and includes
root locking/recovery, source read, hash/signature validation, publish, revalidation
and terminal cleanup. The launch deadline starts before process/protocol permit
acquisition and includes backend preflight, profile generation/compile, spawn,
frontend transition, every live-image observation and `SessionPermit::open`; the
fixed observation count cannot extend it. The wall deadline starts at successful
process admission and ends at terminal Plugin/session outcome; cleanup then receives
only the separately bounded cooperative remainder, TERM and KILL/reap deadlines.
Caller cancellation never replenishes a budget. Checked monotonic arithmetic failure
disables admission before side effects.

Import cancellation/deadline observation is cooperative at the same pre/post-I/O
boundaries already accepted for local Blob ingest. A currently executing local
filesystem syscall is not claimed to be preemptible; the admitted caller remains
joined to it and no worker is detached. This is why V1 rejects direct network/FUSE
sources. Launch/wall/termination supervision is process-based and uses the separate
bounded terminate/kill/reap state machine rather than abandoning a child.

### 10.1 Hard-memory feasibility gate

On the selected SDK, `RLIMIT_AS` aliases `RLIMIT_RSS`. The platform manual describes
that value as influencing which process loses physical memory under pressure, not
as a deterministic allocation-denial ceiling. It therefore cannot produce
`resources=ENFORCED`. Sampling RSS/footprint and killing after excess is also
rejected because allocation between samples has no proven overshoot bound.

The apparently closer Mach ledger API is not an accepted escape hatch. Although
`task_set_phys_footprint_limit` appears in the public SDK header, the exact local
ordinary-process probe returns `KERN_NO_ACCESS`; Apple's XNU implementation gates it
through `proc_check_footprint_priv`/`PRIV_VM_FOOTPRINT_LIMIT` and comments that the
call should probably be obsoleted. MengXia will not acquire root, private entitlement
or host-wide memory authority for this task.

The same result (8) was reproduced on the upgraded macOS 27.0 tuple in §1.1;
the old and new SDKs both alias RLIMIT_AS/RLIMIT_RSS. These observations rule out
these proposed substitutes, not every conceivable future public mechanism.

Evidence:

- <https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getrlimit.2.html>
- <https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/task.c>
- <https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/priv.h>

Before Option A can be accepted, the proposal must name a public, supported macOS
mechanism, its exact FFI/ownership contract and real cap-1/cap/cap+1 behavior for
anonymous mappings, file mappings, shared memory, allocator bursts and child
attempts. If none exists, the safe results are Option C or a separately reviewed
VM/capability-runtime/Linux architecture. No TASK-012 production launch path may be
compiled as available while this row is unresolved.

## 11. Sandbox evidence contract

```rust
pub enum Enforcement {
    Enforced,
    Partial,
    Unknown,
    Unavailable,
}

pub struct SandboxEvidence {
    backend: SandboxBackendIdentity,
    policy_sha256: Sha256Digest,
    managed_executable_sha256: Sha256Digest,
    launched_image: LaunchedImageEvidence,
    test_suite_version: u16,
    filesystem: Enforcement,
    network: Enforcement,
    process_tree: Enforcement,
    ipc: Enforcement,
    resources: Enforcement,
}
```

`LaunchedImageEvidence` is a non-serializable, non-constructible backend-neutral
proof token. The private macOS implementation retains CodeDirectory, audit/PID and
vnode evidence; independently qualified Linux may retain a different proof without changing this
contract. Fields have typed read-only accessors only where a consumer needs them.
Product construction is possible only after backend preflight, successful launch binding and
the exact runtime tuple's reviewed qualification record through the stages below. A configured backend name,
profile compilation, code
signature string, child self-report or one successful denial cannot construct
`Enforced`.

All five dimensions must be `Enforced` before returning an admitted sandboxed
process. Any other vector returns `SANDBOX_UNAVAILABLE` and boundedly reaps the
process. Evidence is in-process and non-persistent in TASK-012; TASK-013 later owns
auditable persistence/composition.

### 11.1 Non-circular implementation and qualification

**Stage 0 — pre-start feasibility and bounded authorization.** Name and demonstrate
the accepted hard-memory primitive and other critical launch mechanisms on candidate
tuples, accept finite caps/dependency scope and synchronize the task start. Targeted
isolated feasibility probes are evidence work, not product enablement. The complete
not-yet-written hostile suite is not a condition of this start. Stage 0 remains
BLOCKED today because §10.1 has no acceptable primitive; this sequencing correction
does not authorize an implementation start by itself.

**Stage A — implement and collect observations through a closed test runner.**
After Stage 0, implement the real private custody/profile/launch/identity/limits/
cleanup machinery. A crate-private `cfg(test)` qualification runner exercises that
same machinery with only repository-owned, digest-checked hostile fixtures and
bounded test roots/views. It emits a distinct qualification observation record,
not product `SandboxEvidence`, `LaunchedImageEvidence` or an admitted product handle.
It may reach the private launch/session test orchestration without a prior allowlist
entry, but MUST NOT bypass any actual enforcement, identity comparison, hard limit,
deadline, cancellation or cleanup check. Tests observe success/denial directly;
there is no fake `Enforced` constructor. The full candidate suite must include the
same session integration/hostile scenarios later replayed on the product path.

The runner is not a public API, Cargo feature, environment flag, CLI/daemon endpoint
or generic executable launcher. `--all-features` does not export it; normal/release
builds cannot call it. Existing §5 test-only root construction is not production
authority. Unit-test-side orchestration owns the runner; testkit may provide fixture
binaries/data and invoke the test target, but cannot acquire a product authority.
Negative architecture tests verify these boundaries and prove that an unqualified
tuple still fails on the product path. The runner produces a reviewed manifest with
candidate commit/tree, runtime tuple and enforcement implementation/profile/fixture/
suite digests. Failure produces no qualifying record or reusable run view.

**Stage B — validate production eligibility at the exact final head.** Add only the
reviewed Stage A manifest digest and tuple to the closed allowlist, then rerun the
entire suite through the product evidence/admission path using bounded test custody
roots. Do not reuse the Stage A observation-only route to claim a product PASS.
The final reviewed PR/merged-main evidence must cover the allowlist, test-only
boundary, negative unknown/stale/partial records and all real denial/resource tests.
No production composition or release is authorized before this passes; TASK-013
still owns its later factory/install/audit gate.

The Stage A manifest attests its candidate implementation, not a circular hash of
the final commit containing that manifest. Stage B records the final tested head
separately. A changed enforcement/profile/fixture/suite digest invalidates Stage A
and requires requalification; adding the manifest/allowlist is the expected promotion
delta, never an excuse to skip final testing. A docs-only manifest, stale tuple or
allowlist-only change is insufficient. TASK-012 DONE requires both stages' evidence.

## 12. Environment, arguments and secret boundary

At the initial trusted launch boundary, the Plugin receives only:

```text
LANG=C
LC_ALL=C
TZ=UTC
MENGXIA_PLUGIN_PROTOCOL=1
MENGXIA_INPUT_COUNT=<canonical u16>
MENGXIA_OUTPUT_COUNT=<canonical u16>
MENGXIA_TMP_COUNT=<canonical u16>
```

No inherited `HOME`, `PATH`, `TMPDIR`, `DYLD_*`, proxy, XPC, SSH, AWS, Git,
credential, Library, Developer, Cargo or arbitrary caller variable is present.
Arguments are a fixed argv array containing only the managed entrypoint and fixed
protocol marker; paths are supplied only through generated policy and fixed
relative slot naming. No shell, interpolation, search or user-supplied option is
used.
This constrains the trusted launch inputs, not the hostile process's ability to
change its own in-process strings. Same-image exec remains subject to §8.1 and
cannot turn a changed string into host or OS authority.

Inherited descriptor inventory is exact and checked both before spawn and from the
live child fixture. Every unrelated daemon descriptor is close-on-exec. Plugin
Control remains a separate authority domain and cannot represent Core/Admin/Broker
operations.

## 13. Error and retry mapping

No new `ErrorCode` is proposed.

| Condition | ErrorCode | Retry | Safe message / termination |
|---|---|---|---|
| unsupported/unknown runtime tuple, backend signature mismatch, profile compile/apply failure, incomplete evidence | `SANDBOX_UNAVAILABLE` | after operator/OS qualification only | static; no launch or bounded reap |
| declaration, target, cap or policy input invalid | `VALIDATION_ERROR` | no without corrected input | static; before mutation/spawn |
| unsupported source mount/type or stable source bytes fail declared length/digest/Mach-O/signature policy | `VALIDATION_ERROR` | no without corrected source/declaration | static; no source/path/signature detail |
| source identity/size changes, premature EOF or post-length byte appears during import | `SOURCE_MODIFIED_DURING_INGEST` | only after source stabilizes | static; cleanup owned staging before return |
| UUID or timestamp source unavailable for custody intent | `ID_GENERATION_UNAVAILABLE` | after platform condition changes | static; before staging creation |
| unsafe root owner/mode/ACL/filesystem/state or incomplete-intent bound exceeded | `STORAGE_CONFIGURATION_ERROR` | after operator/recovery action | static; no path/UID/ACL detail |
| import/root/staging filesystem failure | `STORAGE_IO_ERROR` | state-aware only | static; preserve uncertain evidence |
| existing managed object or live image conflicts with digest/signature/identity | `STORAGE_CORRUPTION` | no | static; bounded reap |
| process/protocol violates sandbox or private contract | `PLUGIN_PROTOCOL_VIOLATION` | no automatic retry | bounded terminate/reap |
| process/session/import admission or transient disk reservation unavailable | `BACKPRESSURE` | bounded caller delay and fresh admission | no work admitted |
| hard memory enforcement unavailable or configured backend cannot prove all dimensions | `SANDBOX_UNAVAILABLE` | only after platform qualification/config change | no process launched |
| launcher/profile/backend executable setup fails before untrusted image execution | `SANDBOX_UNAVAILABLE` | only after platform/operator correction | static; no fallback, bounded reap if a child exists |
| OS process/file-descriptor admission is transiently exhausted before spawn | `BACKPRESSURE` | bounded caller delay and fresh admission | static; no work admitted |
| launcher reports malformed/unknown fixed status or violates the fixed fd/state contract | `INTERNAL_ERROR` | no | close supervisor admission; bounded reap |
| child exits/is signalled before exact live-image proof | `SANDBOX_UNAVAILABLE` | only after platform/operator correction | static; cause is not inferred from stderr |
| proven managed Plugin exits/is signalled after live-image proof but before session admission | `PLUGIN_PROTOCOL_VIOLATION` | no automatic retry | static; bounded reap |
| wall/termination deadline expires | `DEADLINE_EXCEEDED` | no blind Plugin retry | supervisor closes on unreaped state |
| supervisor invariant, unknown wait status or unverified process-group ownership | `INTERNAL_ERROR` | no | close supervisor admission |
| caller cancellation before terminal result | `OPERATION_CANCELLED` | semantic caller decision | bounded terminate/reap |

Raw profile text, paths, argv, environment, Plugin stderr, signature diagnostics,
OS denial text and rejected bytes are never included in public errors or logs.
TASK-008's bounded observability schema may record only closed operation/backend/
reason labels and numeric duration/count values. TASK-013 owns persistent security
audit events.

## 14. Supply, FFI and build contract

- Prefer existing locked crates. Any new direct dependency or feature requires an
  isolated final-manifest lock and cargo-deny review before acceptance.
- `libc` may be promoted from the existing lock only at its exact current version
  after its direct feature/license/advisory inventory is recorded; no floating pin.
- All unsafe is confined to private `mengxia-platform-sandbox` macOS FFI modules.
- Checked-in bindings or a narrow C shim must list exact Security.framework,
  `posix_spawn`, wait/signal/resource and process-inspection signatures, constants,
  ownership and return semantics.
- A compile-time ABI probe includes the active SDK headers and static-asserts every
  size/alignment/constant used. Error and ownership tests exercise every allocation
  and release path.
- Build scripts reject ambient compiler/SDK/include/linker flags using the existing
  native-build policy. Framework links are exact and no private framework or
  undocumented symbol is permitted.
- `/usr/bin/sandbox-exec` is an external runtime dependency, not a Cargo artifact;
  exact runtime identity plus hostile behavior are both required.
- The profile generator is deterministic. Golden bytes/hash, injection corpus and
  compile-failure corpus are checked in.
- Test Plugin Mach-O files are built from checked-in hostile-source code at the
  exact reviewed head; no opaque executable fixture is committed. Ad-hoc positives
  use exact `/usr/bin/codesign -s - --timestamp=none` behavior. Certificate-backed
  positives use a per-test, non-production signing identity generated inside an
  isolated temporary keychain, pass that keychain explicitly to `codesign`, disable
  timestamp/network access, never modify the user's default/search keychain list,
  never log/export key bytes, and destroy the owned keychain after the process is
  reaped. The exact `codesign`, `security` and certificate-generation tool identities,
  argv templates and cleanup/failure matrix are part of the closed hosted manifest;
  unavailable isolation means formal evidence cannot pass.

## 15. Stable acceptance and test candidates

### 15.1 Acceptance ownership

TASK-012 terminally owns only existing `AC-021` and `AC-022`.

- `AC-021`: PASS only when real OS evidence denies HOME, Library DB, CAS and another
  run at the syscall boundary through the managed launch path.
- `AC-022`: PASS only when Internet, loopback, private, link-local, metadata,
  IPv4/IPv6, TCP/UDP/raw and Unix named socket bypass attempts are denied through
  the managed launch path.
- `AC-020`: contributor evidence only: unavailable/incomplete sandbox yields
  `SANDBOX_UNAVAILABLE` and no unsandboxed process. TASK-013 owns terminal audit and
  activation composition.
- `AC-023`: contributor evidence only: direct sockets are impossible. TASK-016 owns
  positive EgressAuthorization and Broker destination binding.

### 15.2 Stable TASK-012 test candidates

| ID | Exact responsibility | Evidence class |
|---|---|---|
| `TEST-CONFIG-012` | source-free typed DTO construction, reserved §16 key registry, cap-1/cap/cap+1 and impossible typed combinations before side effects; architecture negative proves no environment/CLI/source-precedence capture in TASK-012 | unit + integration |
| `TEST-CUSTODY-012` | descriptor source, exact 256-byte golden intent, bounded recovery count, bounded copy, no-clobber digest object, Mach-O/signature policy, dedup, every crash/fault boundary and cleanup uncertainty | real APFS + fault seams |
| `TEST-LAUNCH-012` | exact limit/status golden records, inherited-fd inventory, retained object to PID+architecture+algorithm-tagged live-code/path/inode binding and every replacement seam | real signed processes + deterministic barriers |
| `TEST-POLICY-012` | deterministic deny-default profile bytes/hash, token-only path construction and injection/unknown-rule failures | golden + negative compiler corpus |
| `TEST-FILESYSTEM-012` | exact input/output/tmp rights; HOME/Library/CAS/other-run/symlink/hardlink/namespace denials; failed pre-admission run view is tainted and never published/reused | real Seatbelt hostile fixture |
| `TEST-NETWORK-012` | IPv4/IPv6 TCP/UDP/raw, Internet/loopback/private/link-local/metadata and Unix endpoint denial | real Seatbelt hostile fixture |
| `TEST-PROCESS-012` | real new-session identity; denied child creation, other-image/shell/tool exec, process inspection and parent/sibling group escape; same-image exec retains containment and cannot renew admission or protocol state | real Seatbelt hostile fixture |
| `TEST-IPC-012` | only inherited private pipes survive; Client/Admin/Broker/Mach/AppleEvent/named IPC attempts denied | real descriptor/process inventory |
| `TEST-RESOURCE-012` | CPU, accepted hard-memory primitive, file size, fd, writable-slot and process/session admission cap-1/cap/cap+1 bounds, including cumulative CPU/hard-limit persistence across same-image exec | real OS enforcement + accounting |
| `TEST-TERMINATION-012` | absolute import/launch/wall budget origins including repeated-exec loops, cooperative remainder, TERM, KILL, serialized exit/reap/signalling, cancellation, panic, PID/session/group mismatch and bounded reap | subprocess/fault matrix |
| `TEST-EVIDENCE-012` | Stage A observations cannot construct product evidence/admission; Stage B replays full suite through product path; unqualified/stale/partial/self-report fails; changed enforcement digest forces requalification | typed matrix + two-stage real suite records |
| `TEST-HOSTILE-012` | applicable mandatory §20.1 attacks repeated through real managed launch and sandbox | real arm64 macOS only |
| `TEST-ERROR-012` | exact code/retry/redaction/metric mapping and canary scans | unit + integration |
| `TEST-LIFECYCLE-012` | process permit held through reap; receipt drop/shutdown/join; no detached work | deterministic concurrency |
| `TEST-ARCH-012` | exact dependency/unsafe/process authority; platform-neutral public types; private macOS details; qualification runner is cfg(test)-only and absent from normal/release/all-features product builds; no production root factory, Core/Admin/DB/CAS/Broker or generic spawn/raw-fd API | metadata + source/build negative fixtures |
| `TEST-SUPPLY-012` | locked/offline deps, ABI probe, system backend identity and fail-closed drift | local + hosted formal |
| `TEST-DOC-012` | accepted ADR/OQ/caps/AC/TEST/file-scope/start/completion consistency | document positives/negatives |

Each ID must directly execute every named responsibility in component mode before it
may print PASS. Formal-only tests print neither PASS nor skip locally. The repository
aggregate must add exactly these seventeen disjoint IDs and keep every retained ID.
TEST-LAUNCH-012 additionally owns the ignored-selector counterexample and independent
post-query-field mismatch matrix in §6. TEST-PROCESS-012 owns the §8.1 repeated
containment, argv/environment/fd and protocol-state checks; it must not report a
permitted same-image exec as a denied syscall. Existing TASK-011 protocol bytes and
rules remain unchanged.

## 16. Exact candidate file scope

If the proposal is accepted, only the following may change:

```text
Cargo.toml
Cargo.lock
deny.toml                         # only if exact audited dependency delta requires it
crates/mengxia-platform-fs/Cargo.toml
crates/mengxia-platform-fs/src/lib.rs
crates/mengxia-platform-fs/src/plugin_executable.rs
crates/mengxia-platform-fs/src/macos_ffi.rs # visibility/refactor only; no new unsafe ABI
crates/mengxia-platform-sandbox/Cargo.toml
crates/mengxia-platform-sandbox/build.rs
crates/mengxia-platform-sandbox/include/**
crates/mengxia-platform-sandbox/src/**
crates/mengxia-plugin-host/Cargo.toml
crates/mengxia-plugin-host/src/lib.rs
crates/mengxia-plugin-host/src/process.rs
crates/mengxia-plugin-host/src/error.rs
crates/mengxia-plugin-security/src/lib.rs
crates/mengxia-plugin-security/src/trust.rs
crates/mengxia-testkit/Cargo.toml
crates/mengxia-testkit/tests/architecture.rs
crates/mengxia-testkit/tests/ci_evidence.rs
crates/mengxia-testkit/tests/ci_orchestration.rs
crates/mengxia-testkit/tests/document_traceability.rs
crates/mengxia-testkit/tests/naming.rs
crates/mengxia-testkit/tests/task_012_foundation.rs
crates/mengxia-testkit/src/bin/task_012_hostile_plugin.rs
crates/mengxia-testkit/tests/support/plugin_sandbox_hostile.rs
crates/mengxia-testkit/tests/fixtures/task_012/**
bins/mengxia-plugin-launcher/Cargo.toml
bins/mengxia-plugin-launcher/src/main.rs
scripts/ci-task-012-mappings.txt
scripts/verify-task-012.sh
scripts/verify-repository.sh
.github/workflows/ci.yml          # TASK-012 formal component/job only
docs/provenance/task-012-macos-sandbox-v1.toml
docs/spec/adr/ADR-0018-task-012-macos-sandbox.md
docs/spec/task-lifecycle-records.toml
docs/spec/IMPLEMENTATION_SPEC.md
docs/spec/DECISIONS.md
docs/spec/IMPLEMENTATION_REVIEW.md
docs/spec/IMPLEMENTATION_PLAN.md
docs/spec/PROJECT_INTAKE_REPORT.md
AGENTS.md
docs/proposals/TASK-012-GATE-PROPOSAL.md
```

Before acceptance, isolated metadata must confirm whether the new internal launcher
requires adding a nineteenth workspace package and update the canonical package
inventory tests accordingly. `creative`, storage, store, app, ports, Core proto,
Plugin proto, CLI and daemon files are not authorized.

Any need for migration, persistent install state, Admin, audit, Broker, public API,
positive network egress, generic executable launch or tool-child support stops the
task and requires a new gate.

Linux implementation, Linux production dependency selection and Linux release
claims are also outside this file scope. Adding them requires a separately accepted
platform gate; it must reuse the common contract without weakening the macOS
evidence or treating a generic container as automatic sandbox proof.

## 17. Implementation order after acceptance

1. After §11 Stage 0 feasibility passes, synchronize canonical docs, accept ADR-0018
   for bounded implementation of candidate tuples/limits, close only the corresponding
   TASK-012 decision portions, add the exact start record and make document gates pass.
   No candidate tuple is thereby a production-qualified tuple.
2. Freeze the final dependency/package inventory, ABI/provenance manifest and
   hosted/local runtime tuples; stop if isolated supply checks differ.
3. Implement pure typed config, trust and evidence values with no spawn path.
4. Implement the leaf filesystem custody transaction, private sandbox static
   validator and their exact host-only stage/validate/publish composition with
   fault/tamper tests.
5. Implement deterministic profile compiler and real policy-compile negatives.
6. Implement the narrow launcher/FFI boundary, dedicated session and sole-owner
   process admission/reap lifecycle.
7. Implement dynamic launched-image verification before protocol admission.
8. Exercise the TASK-011 session through the Stage A closed test orchestration,
   retaining process/session permits and all real enforcement checks; collect
   observations without product admission/evidence and keep product entry unavailable.
9. Complete Stage A's real hostile/resource/drift matrix and review its manifest;
   perform Stage B promotion and replay the full suite through the product path.
   Only the qualified product path may construct all-ENFORCED evidence.
10. Run format, Clippy `-D warnings`, workspace all-target/all-feature offline tests,
    every TASK-012 ID, retained developer/formal gates, cargo-deny, diff/security
    review and exact AC/security verification.
11. Obtain reviewed arm64 `macos-26` PR and merged-main evidence before DONE.

No step automatically starts TASK-013.

## 18. Candidate start record — inactive

This block is a template only. It must not be copied into the Plan until independent
review passes and the user explicitly closes all three pre-start decisions.

```text
TASK012_CANONICAL_GATE: ACCEPTED
TASK012_LIFECYCLE: IN_PROGRESS
TASK012_IMPLEMENTATION_AUTHORITY: TASK_012_MANAGED_SANDBOX_ONLY
TASK012_PROPOSAL_VERSION: <accepted-version>
TASK012_ADR: ADR-0018_ACCEPTED
TASK012_OQ001: ACCEPTED_CANDIDATE_ARM64_MACOS_TUPLES
TASK012_OQ002: ACCEPTED_IMPLEMENTATION_DESIGN_QUALIFICATION_PENDING
TASK012_OQ006: CLOSED_FOR_TASK012_PROCESS_RESOURCE_CAPS_ONLY
TASK012_MEMORY_ENFORCEMENT: <accepted-public-hard-mechanism>

SCOPE: TASK-012 lower-level managed executable custody, exact launched-image
       binding, macOS sandbox/process lifecycle and hostile evidence only.
FEATURES: FUNC-006 and FUNC-007 contributors
REQUIREMENTS: SEC-001, SEC-002, SEC-005, SEC-009, SEC-017, SEC-020,
              SEC-021, REL-001, REL-006, CFG-003
CONTRIBUTOR_REQUIREMENTS: CFG-001 typed immutable DTO only; production source
                          precedence/capture remains a later composition task
PREREQUISITES: TASK-011 DONE; ADR-0018 ACCEPTED; candidate local/hosted backend
               preflight and primitive feasibility; all seventeen TEST IDs canonical
PRODUCTION_ELIGIBILITY: NONE_UNTIL_STAGE_B_COMPLETES
ACCEPTANCE: AC-021, AC-022
CONTRIBUTOR_ONLY: AC-020 no-unsandboxed-launch; AC-023 direct-socket-denial
TESTS: all seventeen TASK-012 IDs in canonical Specification
DEVELOPER_GATE: scripts/verify-task-012.sh developer
FORMAL_COMPLETION_GATE: scripts/verify-task-012.sh formal plus reviewed PR/main CI
AUTHORIZED_FILES: accepted proposal §16 exact list
FORBIDDEN: install/activation/Admin/grant/revoke/audit/migration/Core/CLI/daemon/
           DB/CAS/Broker/Credential/positive-egress/tool-child/TASK-013+
```

## 19. Independent review checklist

- [ ] Decide whether deprecated `sandbox-exec` is acceptable for an exact-build,
      fail-closed V1 backend; do not treat availability as proof.
- [ ] Independently reproduce local and hosted backend identity/provenance.
- [ ] Prove the effective profile, not just its source, denies every required class.
- [ ] Decide whether static SHA + live CodeDirectory + retained vnode is sufficient
      for ADR-0014's exact launched-image requirement.
- [ ] Verify scripts/fat Mach-O/dylibs/entitlements cannot bypass code identity.
- [ ] Verify the launcher has no unsandboxed untrusted-code window.
- [ ] Verify dedicated session/group escape denial and serialized signal/reap
      ownership prevent targeting an unrelated process.
- [ ] Verify guest-query success with ignored selectors is never identity proof;
      every actual post-query mismatch fails before protocol admission.
- [ ] Accept and prove §8.1 same-image re-exec containment, cumulative limits and
      original deadlines without another handshake, admission or implicit retry.
- [ ] Verify fixed slots plus RLIMIT_FSIZE form a true aggregate disk bound.
- [ ] Verify the accepted public hard-memory primitive and `RLIMIT_CPU` behavior on
      exact macOS, including cap-1/cap/cap+1 and signals; `RLIMIT_AS` is not accepted.
- [ ] Verify no fork/child path is required by TASK-012 or completed TASK-011.
- [ ] Verify every inherited descriptor and environment key has one reason.
- [ ] Verify all errors are static/redacted and no raw stderr is retained.
- [ ] Verify exact file scope includes every mechanically required CI/test update.
- [ ] Verify canonical AC ownership does not overclaim AC-020/AC-023.
- [ ] Verify TASK-013 can consume the opaque custody/process seam without changing
      TASK-012 security boundaries.
- [ ] Verify formal tests cannot report PASS on an unsupported or partially tested
      host and cannot silently skip real Seatbelt evidence.
- [ ] Verify Stage A needs no existing allowlist or product evidence but cannot
      bypass real controls; normal/release builds expose no qualification runner.
- [ ] Verify Stage B uses the product path and separately attests the final head;
      stale manifests and implementation changes cannot reuse qualification.
- [ ] Verify public policy/evidence/lifecycle APIs contain no macOS-only type or
      semantic assumption and that unsupported targets fail closed.
- [ ] Verify the independent Ubuntu contract requires its own complete five-dimension
      evidence rather than inheriting macOS results or trusting a container label.

## 20. Deferred Native candidate resumption conditions

The current project action is bounded R0-B native research under the later user
scheduling amendment, not implementing this candidate or drafting TASK-015.
The built-in-first direction is accepted, not awaiting another product-scope choice.
Only new mechanism/evidence may justify reopening this deferred Native candidate;
if reopened, independent security/feasibility review must either:

1. accept candidate arm64 macOS tuples for implementation, identify and prove a
   public hard-memory mechanism that closes `TASK012-BLOCKER-001`, then accept
   Option A, the launched-image proof and the remaining §10 finite caps; only after
   that may the proposal be revised and an explicit canonical start record synced;
   or
2. reject/rescope the macOS Native backend (Option C), or separately review an
   alternative macOS execution architecture while this proposal remains `DRAFT / BLOCKED`.

Ubuntu intake/foundation and backend qualification are deferred under the amended
ADR-0019 until macOS completion. No Ubuntu version or host information is requested
now; no VM/remote bridge is selected or required. Current built-in work follows §0.7;
this unaccepted macOS draft supplies no such completion evidence.

Codex must not implement production code from this draft and must not start
TASK-013 automatically.

The separate `TASK-012-MACOS-FEASIBILITY.md` records the latest feasibility result
and the accepted built-in direction alongside unaccepted backend alternatives.
It does not close Stage 0 or authorize a fallback.
