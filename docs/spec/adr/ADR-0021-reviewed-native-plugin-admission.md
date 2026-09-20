# ADR-0021: Reviewed native plugin admission with explicit residual resource risk

- Status: ACCEPTED
- Date: 2026-09-20
- Scope: macOS policy and development direction; no implementation or activation authority
- Supersedes: ADR-0020's indefinite third-party deferral and unchanged-resource-target rule only for the explicitly qualified reviewed-native profile; strict SANDBOX_ONLY remains unchanged

CURRENT_PROJECT_NEXT_ACTION: DRAFT_BROKER_FOUNDATION_GATE

Foundation completion (2026-09-20): the pure reviewed-native foundation is DONE
with PR #16 and exact merged-main evidence in reviewed plan §9 and the scoped
lifecycle ledger. Its implementation authority is revoked to NONE; product authority
remains NONE. Old strict TASK-012 stays BLOCKED and its parent is not completed.
Next is drafting the non-executing BROKER_FOUNDATION gate, not implementing Broker,
launching plugins or repeating R0/VM experiments. Earlier start/validation/routing
statements below are historical and cannot grant current implementation authority.
REVIEWED_NATIVE_DECISION: ACCEPTED
REVIEWED_NATIVE_PRODUCT_AUTHORITY: NONE
REVIEWED_NATIVE_ADMISSION: EXACT_ARTIFACT_AND_DEPENDENCY_CLOSURE
REVIEWED_NATIVE_MEMORY: MONITORED_NOT_HARD_ENFORCED
REVIEWED_NATIVE_UNKNOWN_PACKAGE: DENY
REVIEWED_NATIVE_SECRET_ACCESS: BROKER_ONLY
REVIEWED_NATIVE_REVOCATION: RECHECK_BEFORE_LAUNCH_AND_AT_BROKER_SINK
REVIEWED_NATIVE_STRICT_EVIDENCE: NOT_INHERITED

## Context and user decision

The user proposed retaining native execution, accepting an imperfect boundary,
and admitting third-party plugins only after a complete prescribed review. After
the explanation of residual memory/DoS risk and retained runtime safeguards, the
user accepted: “可以，开始吧”. This accepts a curated risk-managed direction,
not proof of hostile-code containment, acceptance of a specific sandbox primitive,
or permission to run arbitrary plugins. It does not accept VM/Linux workers,
privilege escalation, raw-secret access or unrestricted TRUSTED_NATIVE execution.

R0 and both R0-B batches remain negative/inconclusive evidence for complete hard
physical-memory enforcement. Review cannot change those observations. There is no
qualified production backend today. The implementation plan is
`docs/proposals/REVIEWED-NATIVE-PLUGIN-DEVELOPMENT-PLAN.md`.

## Decision

### Admission is distinct from authority and containment

Define REVIEWED_NATIVE as a distinct execution profile, not an alias for
TRUSTED_NATIVE or SANDBOX_ONLY. It permits only an exact reviewed package, its
complete executable/runtime dependency closure and an accepted capability profile.
Unknown, unreviewed, changed, expired or revoked packages are denied. A publisher
name, signature, popularity, ProjectTrust or an earlier version's approval is never
sufficient. All third-party packages, including a future strict-sandbox class,
require review before admission. No developer switch bypasses this in product use.

An admission record is necessary but insufficient: local authenticated Admin
authorization, current grants, Run-bound leases, rights/egress decisions and a
qualified launch boundary still apply. This decision does not issue any such grant.
Review covers source, dependencies, build scripts and final artifacts; controlled
build provenance must link the reviewed source to what is shipped and launched.
Artifact identity must survive import, storage and launch, not only download hash
checking. Review/signature authorities and distribution formats are separate
implementation-gate decisions; no home-grown cryptographic scheme is authorized.

The existing PackageDigest identifies canonical manifest bytes, not the whole
distributed binary package. Approval must bind that manifest identity AND a
separate final-artifact identity AND its executable dependency closure; do not
repurpose PackageDigest or invalidate historical package fixtures. OS-supplied
libraries are pinned by the qualified platform/profile trust boundary; packaged
and external non-system executable dependencies require exact reviewed identities.

### Explicit reduction in assurance

For this profile only, complete hard physical-memory accounting of the worker,
shared/GPU/kernel allocations and worker-induced host cost is not a prerequisite.
The project accepts the residual possibility of host memory pressure, UI/system
unresponsiveness, process termination and loss of in-flight work from a reviewed
plugin's bug, exploited input or malicious behavior missed in review. No guarantee
that a watchdog always reacts before exhaustion, or that other applications remain
unaffected, may be made. A monitor is reported as MONITORED, never ENFORCED.

This is a bounded policy exception to the hard-memory requirement, not acceptance
of every missing resource/property. Accepted finite input/output/queue/concurrency/
time budgets, supported OS limits, monitor failure handling, cancellation, cleanup
and recovery still require implementation and measured evidence. Their numeric
values remain gate decisions, not fabricated performance guarantees. If another
required property proves infeasible, stop at that exact property and decide it
explicitly; do not accumulate implicit exceptions under this ADR.

### Runtime safeguards retained

- A separate owned native worker process; no plugin dylib/media parser in Core.
- Verified OS-enforced filesystem, direct-network and IPC isolation on the exact
  supported tuple, established before any plugin initialization can run.
  Only package/read-only task inputs and bounded task output/tmp access; no direct
  Library DB/CAS, other Run, HOME/secret store or Client/Admin access.
- Broker-only network and credentials, including reviewed plugins; no Level-C
  raw static secret exception. Broker requests remain typed and bounded; read plus
  network permission does not authorize arbitrary upload or URL/body proxies.
- Managed exact-image/dependency custody, constrained argv/environment/descriptors,
  no ambient PATH, shell, self-update, undeclared executable or downloaded code.
  Scripts, interpreters and executable model formats count as code when applicable.
- Process/child ownership and bounded termination/reap must be demonstrated;
  a process group or closed stream alone is not proof. Admission stays occupied
  until owned work is confirmed stopped; unknown cleanup denies further launches.
- Output validation, safe audit, durable effect/recovery and all existing
  authentication, rights, storage and destructive-operation boundaries survive.

These are qualification requirements, not statements of implemented capabilities.
Native media/GPU access is not blanket-approved: each later capability must prove
its access profile and record resource observations and remaining memory risk.

### Review and continuous operation

Initially accept only source-available-to-reviewers packages with a maintainable
dependency inventory. A designated human maintainer owns approval; AI, SAST,
dependency/malware scans and Apple notarization assist but do not approve alone.
Every released version has its own decision, evidence and final digest. A reviewed
delta may reuse unchanged evidence only when dependency/build/profile changes and
advisory freshness have been checked. Permission increases require fresh local
authorization. Review of dependencies is risk-based and documented; “complete”
means all prescribed checks resolved, not a proof that every transitive line is safe.

Approval records bind package/closure digests, source/build evidence, permissions,
profile and supported tuples, reviewer/decision/evidence version, expiry and
revocation identity. Revocation/freshness is checked before launch and at sensitive
Broker operations; known revocation blocks locally even when offline. Learning new
revocations requires connectivity: only an unexpired last verified snapshot may be
used offline, and missing/expired state denies new runs and sensitive operations.
Clock rollback and snapshot rollback must not extend approval validity. Running
work on revocation/expiry loses leases, is cancelled and supervised to termination;
unknown cleanup is quarantined. Exact lifetimes and response bounds precede enablement.
Rollback may select only another currently approved, non-revoked digest.

Review infrastructure executes unknown submissions only in a separately isolated,
credential-free environment. This is independent of the product's native execution
choice; no untrusted build/test execution on this workstation is authorized now.

## Compatibility and acceptance ownership

OQ-007's direction is resolved in favor of the separate reviewed profile, not
general TRUSTED_NATIVE. OQ-001/OQ-002 exact-tuple/backend decisions, OQ-006 budgets,
OQ-010 Admin and later Credential/Rights decisions remain at their effectful gates.
The original TASK-012 proposal is retained as the blocked strict-sandbox candidate;
new scoped work does not mark that candidate or its full parent task DONE.

SEC-002's legacy universal rule and AC-020 are superseded only for this profile by
Specification §0.8, SEC-022 and AC-104 through AC-108. Old stable IDs and historical
evidence are not rewritten. AC-020 remains the strict/unknown-package obligation;
AC-021/AC-022 keep their original SANDBOX_ONLY semantics. New profile tests require
equivalent file/network denials but cannot borrow their PASS or all-ENFORCED evidence.

Existing TASK-001 through TASK-011 behavior, data, protocol/manifest bytes and
completion records are preserved. Do not serialize a new enum into an old protocol
or accept a new target by loosening validation. Schema/API evolution requires its
own versioned compatibility gate. Ubuntu remains deferred. The §0.7 built-in
dependency map is retained; the new profile can qualify BUILTIN_EXECUTION under
this ADR, while third-party admission requires its additional approval lifecycle.

## Consequences and verification

We retain native API/media options without promising hardware-acceleration
compatibility before tests, and avoid requiring a VM just to match the old resource
claim. Costs shift to human review, controlled builds, advisory response and
revocation operations; a small curated catalog is the initial target, not an open
marketplace with automatic approval. The result is deliberately weaker against
host-resource exhaustion than the original strict model.

The immediate deliverable is the bounded pure admission-foundation start proposal,
plus canonical synchronization and negative document tests. It grants no process,
package installation, signature authority, system mutation or product execution.
Run documentation traceability and regression checks. Runtime acceptance still
requires real final-path tests; document PASS is not security qualification.

## References

- [NIST SSDF 1.1](https://csrc.nist.gov/pubs/sp/800/218/final): vulnerability reduction and continuing response, not proof of zero vulnerabilities.
- [VS Code extension runtime security](https://code.visualstudio.com/docs/configure/extensions/extension-runtime-security): scanning, dynamic detection, signing and removal; broad extension permissions are not the proposed MengXia permissions.
- [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution): automated malicious-content/signing checks, not a substitute for application review.
