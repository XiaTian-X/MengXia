# ADR-0020: Built-in-first macOS delivery with third-party Native execution deferred

- Status: ACCEPTED
- Date: 2026-09-20
- Authority: user decision, “可以，先完成内置功能吧”
- Scope: first-delivery feature scope and dependency decomposition
- Implementation authority: NONE; bounded implementation starts remain required
- Amends: first-delivery sequencing in Specification §18 and ADR-0019; does not supersede completed-task evidence

## Context

Classification: `SPEC_STALE / SCOPE`. The previous roadmap makes every runtime
feature transitively depend on qualifying arbitrary third-party Native execution.
The user now accepts prioritizing controlled built-in functionality. Keeping that
entire dependency chain would negate the decision; deleting all security
dependencies would instead expose assets, credentials and the host.

The current macOS candidate has no demonstrated ordinary-user hard-memory
mechanism. This is an unresolved execution-property issue, not evidence that pure
business validation, existing asset management or every built-in capability is
impossible. The feasibility report remains evidence, not backend qualification.

## Decision

CURRENT_PROJECT_NEXT_ACTION: DRAFT_BROKER_FOUNDATION_GATE

Foundation completion (2026-09-20): the pure reviewed-native foundation is DONE
with PR #16 and exact merged-main evidence in reviewed plan §9 and the scoped
lifecycle ledger. Its implementation authority is revoked to NONE; product authority
remains NONE. Old strict TASK-012 stays BLOCKED and its parent is not completed.
Next is drafting the non-executing BROKER_FOUNDATION gate, not implementing Broker,
launching plugins or repeating R0/VM experiments. Earlier start/validation/routing
statements below are historical and cannot grant current implementation authority.

Amendment: ADR-0021 supersedes this ADR's earlier immediate research routing and
indefinite third-party deferral for a distinct reviewed-native profile. It explicitly
accepts that profile's residual hard-memory/DoS risk, not general TRUSTED_NATIVE.
The dependency decomposition, migration order, other security boundaries and
historical decision below remain retained. Statements that this scope decision
did not accept resource risk refer to ADR-0020 itself; they cannot override the
later explicit exception. Review the new plan's §5; do not reopen R0-B/VM selection.
The text below describes the original decision and its earlier scheduling amendment.

Scheduling amendment (2026-09-20): the user's later explicit direction prioritizes
bounded native feasibility research, starting with MACOS-NATIVE-R0B-001.md. The
first-delivery scope and dependency decomposition below remain accepted. Research
does not authorize a production backend or third-party activation. The foundation
start proposal remains backlog, not the immediate action.
Its output-contract choice is still OPEN: candidate-only type or canonical
ExecutionPlan from an explicit typed catalog under Specification §0.7/§8.3.
This is not a reopening of the accepted built-in-first product direction.

### First-delivery scope

Deliver the macOS built-in workflow first: retain existing Asset/Project/Subject/
Work/Take functionality; add bounded Recipe/ExecutionPlan and recoverable Run
processing, a controlled FFmpeg capability, and selected built-in Provider adapters
behind their applicable Broker, credential, rights and authorization gates.
CLI/Core API parity, verification, observability and applicable release checks
remain required. Concrete Providers, executable versions and new resource values
are not selected here; their existing decision gates remain in force.

Third-party Native installation, activation and execution are DEFERRED and remain
disabled. No user package, user-selected executable, PATH lookup, arbitrary argv,
shell, environment variable or configuration toggle may turn into built-in code.
Built-ins are a closed, release-reviewed capability set, not a new trust label for
installed packages. Identity and allowed parameters must be verified at the
execution boundary; a friendly capability name is not authority. Distribution and
update provenance/digests remain necessary even for bundled third-party tools.

Ubuntu remains deferred until accepted macOS scope and applicable TASK-023 complete.
No VM, Wasm, privileged helper, OS rollback or toolchain change is selected.
TASK-010/TASK-011 are reusable completed foundations, not wasted work and not
proof that any production execution is enabled.

### Dependency decomposition

Specification §0.7 is the authoritative scoped dependency map. The original §18
task bodies and flat task table retain their full-feature dependencies and stable
acceptance ownership. A foundation scope can be implemented and verified without
claiming the parent task DONE. An accepted scope-specific start may replace only
the listed dependency, and only for the listed effects; it must not enable the
parent's deferred operations. No generic “built-in exception” is permitted.

The retained independent product-foundation work package is the pure Recipe/ExecutionPlan foundation of
TASK-015: bounded typed validation, deterministic graph/capability resolution and
immutable plans, with no parsing of media, process launch, network, credentials,
migrations, persistence, IPC operation or runnable execution authority. It depends
on completed TASK-009 semantics, not an executable FFmpeg implementation. Its own
gate must define stable AC/TEST obligations, finite budgets, exact files and tests
before implementation. Pure foundations must not be presented as a runnable Run.

BROKER_FOUNDATION, owned by TASK-013, is pure in-memory caller/run-binding,
lease/policy decisions and audit-record contracts. It performs no IO, migrations,
persisted lease issuance, audit writes or product admission. Verify the restricted
identity contract; a caller-supplied PluginInstance ID is never proof of authority.

BUILTIN_EXECUTION follows that contract and TASK-011. Its accepted implementation
gate may qualify the real custody/enforcement path through restricted non-admitting
fixtures without a production Broker/Run database. It must preserve actual limits
and isolation; test fixtures are not a weaker enforcement profile or authority for
user workloads. Scoped qualification proves only the declared execution properties,
not product activation or integrated authorization/audit.

BROKER_PERSISTENCE, also owned by TASK-013, follows BROKER_FOUNDATION and verified
BUILTIN_EXECUTION. It owns reviewed 0003/0004 migrations and durable built-in
identity/policy/lease/audit integration and recovery. Its custody prerequisite is
the verified built-in scope, not full third-party TASK-012 DONE. Before applying
0003, review the additive Run-binding transition through 0005. Preserve 0000/0001/
0002 and migration order; no empty placeholders, fake Run rows or disabled foreign
keys. The schema does not enable external installation/grants. Admin remains
disabled unless OQ-010 closes for the privileged effect actually introduced.

FFMPEG_INTEGRATION consumes BROKER_PERSISTENCE and BUILTIN_EXECUTION for its bounded
adapter contract. RUN_INTEGRATION then owns 0005 and real caller/Run/lease/audit
composition plus recovery. Earlier contract/fixture tests do not require a live
product Run and cannot claim its end-to-end acceptance. Live Run-bound leases and
product execution remain disabled until this integration passes. Effectful Rights,
retention and Provider consumers need persisted Broker evidence, not pure contracts.

Real Provider access follows credential/egress/rights gates; mock adapters do not
authorize real secrets, uploads or cost-bearing requests. Native CLI adapters
require the same applicable execution qualification as other native built-ins.

### Security and residual risk

Bundled native tools process untrusted data. Their release gate must cover exact
executable custody, minimal file/descriptor access, secret-free environment,
network denial unless explicitly broker-authorized, process-tree ownership and
joined cleanup, cancellation/deadlines, bounded input complexity/concurrency/output,
malformed input and recovery tests. Do not load native media parsers into Core
as a shortcut around an unqualified child-process boundary.

This scope decision does NOT accept an unbounded host-memory/DoS risk. Soft RSS
monitoring is not hard enforcement; first-party provenance does not prove memory
safety. Before enabling a native built-in, accept and verify a specific execution
profile and disclose any remaining resource risk. If that cannot be done within
the accepted requirements, keep that capability disabled and escalate that exact
choice, while unrelated bounded foundations can proceed. Do not mark a partial
profile as all-ENFORCED SandboxEvidence or claim SANDBOX_ONLY equivalence.

Authentication, ownership, durability, policy/audit, Broker-only secret/egress,
SSRF defenses and privileged/destructive-operation controls are unchanged.
Rights/classification policy must be resolved before real asset upload, not only
before an eventual retention feature. No deletion or automatic purge is introduced.

### Completion accounting

Full-task DONE is not inferred from a completed foundation scope. Before consuming
a scoped completion in a downstream start, its gate must add machine-checked
scope/obligation/dependency accounting to the existing validator; tests must reject
missing prerequisites, a deferred capability being invoked and partial evidence
being treated as full-task PASS. Implementing that accounting is part of the first
bounded start, not an impossible pre-start requirement to write unauthorized code.

- Before the first scoped start, define the accounting contract, file scope and negative-test obligations; the accounting implementation need not exist.
- Implement and test the accounting during the first scoped implementation.
- Pass the accounting checks before recording or consuming any scoped completion.

The first start explicitly includes necessary validator and record files; no
completed-task check is disabled while these additions are being implemented.

AC-021/AC-022 retain their original SANDBOX_ONLY meaning. They are deferred/not
applicable to a release that enables no such execution, never PASS because plugins
are disabled. Any later SANDBOX_ONLY claim requires their full applicable evidence.
Applicable denial behavior, caller isolation and Broker obligations still need
evidence; absence of a Plugin activation endpoint does not prove the entire AC-020
audit composition. The release manifest must enumerate enabled and disabled
capabilities, applicable obligations, evidence and accepted exclusions. TASK-023
may accept this reduced scope, but must not call it whole-V1/plugin-platform PASS.

## Verification

This decision changes planning, not runtime behavior. Verify canonical traceability,
the scoped dependency map, preserved completed-task contracts and negative scope
mutations. No existing migration, protocol bytes, tool pin, CI required check or
production dependency is changed. Each later code slice runs its owning checks
and relevant regression gates before completion. Stop reopening the same rejected
Native candidate without new mechanism/evidence; the later user-authorized research
uses new bounded experiments, not a reversal of the accepted security requirements.
