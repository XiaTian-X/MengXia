# ADR-0019: Shared core with independently qualified macOS and Ubuntu editions

- Status: ACCEPTED
- Date: 2026-09-20
- Scope: development organization, platform qualification and release planning only
- Amended: 2026-09-20 by the user's follow-up: complete macOS before starting Ubuntu
- Preserves: ADR-0004's completed macOS foundation contract and evidence
- Implementation authority: NONE

Current first-delivery scope and sequencing are amended by ADR-0020: built-in macOS
functionality first, third-party Native support deferred. Specification §0.7 defines
the bounded dependency decomposition; this does not authorize Ubuntu development or
transfer missing execution evidence to built-ins.

## Context

The user accepts two editions and will develop Ubuntu directly on Ubuntu, without
a VM. Current implementation and formal evidence cover arm64 macOS foundation,
not Ubuntu. The macOS TASK-012 candidate still lacks proven hard-memory enforcement.
The user subsequently deferred Ubuntu until macOS is complete. Independent
qualification remains an architecture rule, not a current parallel-work instruction;
calling the existing project portable would incorrectly transfer security evidence.

## Decision

Current schedule: macOS first, Ubuntu DEFERRED. Do not begin Ubuntu intake,
version selection, implementation or product CI now. Resume only after the accepted
macOS feature scope and applicable TASK-023 gate are complete, with a fresh Ubuntu
intake and its own start gate. No speculative Linux refactoring or multi-platform
evidence tooling is required to complete current macOS work.

1. Maintain one repository and shared domain, application, data/migration and wire
   contracts. Produce separate macOS and Ubuntu artifacts with explicit supported
   platform tuples and capability lists. No permanent platform forks.
2. Isolate filesystem/durability/ACL, peer identity, native build evidence, executable
   custody, process/sandbox and later secret-store/Admin adapters. Share requirements
   and test scenarios; measure enforcement on each actual supported platform.
3. Preserve the stable task dependency graph. For future platform-scoped starts,
   every dependency must have accepted common-contract evidence and target-edition
   implementation evidence. Ubuntu's unfinished work is not a macOS prerequisite;
   the current macOS-first schedule still controls when Ubuntu starts.
   Historical DONE is never sufficient proof of Ubuntu support. Specification §0.6
   governs scoped versus aggregate status and stable-ID evidence.
   Current task-table DONE is macOS-scoped and may satisfy macOS downstream tasks
   and TASK-023 without Ubuntu evidence. Ubuntu gets additional qualification
   records later; only a dual-edition support claim needs both editions' evidence.
4. Ubuntu foundation qualification precedes Ubuntu production sandbox work. Existing
   TASK-012 draft remains the macOS candidate; Linux mechanisms, versions, limits and
   permissions require a separate accepted gate. Ubuntu version/architecture/kernel/
   filesystem are not selected by this ADR. VM/remote execution is not a prerequisite.
5. Each edition is qualified independently after its own applicable gates pass;
   current development and first delivery prioritize macOS. Missing enforcement
   denies the dependent capability; it never selects
   an unsandboxed fallback. No shared requirement is weakened to fit one backend.
   A reduced-capability preview is not whole-V1 completion and requires explicit
   accepted capability scope rather than silently omitting a required feature.
6. Platform-specific target/schema support is a compatibility change with its own
   gate; preserve historical bytes/digests and migration meaning. Shared storage
   schema does not authorize copying/rebinding an active Library across platforms.

## Consequences

Completed tasks are not reopened solely because Ubuntu is added. New work may
change existing adapters and verification tooling under a bounded start gate, with
macOS regression evidence. Contract defects affecting both platforms are fixed once
and tested on each supported edition; Ubuntu testing starts when that edition is
introduced, not as a requirement on current macOS changes. Platform-only failures
block that platform and any affected shared claim. Later Ubuntu adaptation retains
one owner for shared interfaces/migrations and does not fork state/schema histories.

This ADR does not change product code, CI required checks, tool pins or machine
lifecycle-record syntax. Their future extensions need executable negative tests:
missing edition evidence, a skipped applicable obligation and a mismatched tuple
must fail closed. No implementation starts from a prose-only scoped DONE claim.

## Verification

- Documentation traceability, stable IDs and canonical version synchronization pass.
- The development plan separates known macOS evidence from unknown Ubuntu evidence.
- Before Ubuntu starts, its gate declares exact file scope, baseline/toolchain,
  applicable AC/TEST IDs, native/second-UID/recovery proof and CI/evidence accounting.
- Before each edition releases, all enabled features and transitive prerequisites
  have current applicable evidence; disabling a feature cannot bypass its callers.
- See `docs/proposals/DUAL-EDITION-DEVELOPMENT-PLAN.md` for work packages and gates.
