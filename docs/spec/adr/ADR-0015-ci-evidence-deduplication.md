# ADR-0015: CI evidence deduplication

- Status: ACCEPTED
- Date: 2026-09-12
- Authority: user-authorized MAINT-002 implementation after plan review
- Supersedes: ADR-0010/ADR-0013 only for the execution and feedback rules below

## Context

Review classified repeated identical execution and lifecycle/version constants as
REPO_STALE maintenance debt, not a product defect. The reviewed plan is
docs/proposals/MAINT-002-CI-SIMPLIFICATION-PLAN.md. Its initial design omitted a
concrete partial-result contract and assumed an existing TOML parser dependency;
both assumptions are corrected here before implementation.

## Decision

1. Keep all current events, classification, exact attestation, separate real second
   UID, main code revalidation, weekly full evidence and public-repository security
   settings. Draft suppression is NOT_ENABLED. No path-based product test selection.
2. Code PR feedback uses a new fast command: workspace format/check/Clippy plus
   developer build-boundary and orchestration regressions. This is not formal
   acceptance and does not replace the local complete developer command.
3. The repository driver runs each task in an explicit native-component mode.
   These components retain task-specific checks and emit COMPONENT_PASS, never
   complete task or supply PASS. The local developer/formal driver additionally
   executes the full shared supply check once and only then reports aggregate PASS.
   CI runs native formal and shared supply in separate jobs on the same checkout;
   the stable Merge gate requires both, real second UID, fast feedback and dependency
   review. main/schedule/dispatch also require a repository evidence aggregate.
4. Shared supply retains the exact cargo-deny version, fetch-current-database
   behavior, unavailable-database negative and all four policy categories. The
   supply job initially stays on macos-26, avoiding simultaneous graph/platform
   changes. Only classify, dependency review and merge aggregation move to Linux.
5. Identical pure task test groups may run once for several explicitly listed IDs.
   No automatic filter/features/environment equivalence or cross-run result cache
   is introduced. All task-specific shell assertions, compile-fail, doctests, E2E,
   release/ignored scaling, fault/SIGKILL and stress repetitions remain executable.
   Standalone commands retain their full supply/predecessor semantics.
6. Small declarative lifecycle/version records use a closed, bounded subset of
   TOML (sections and quoted scalar fields), parsed without new dependencies.
   Completed task evidence remains historical. Record consistency never claims
   remote run verification. Invalid or duplicate keys/states and missing evidence
   fail closed. The document-only record cannot specify commands, test exemptions
   or required-check policies; those remain code-reviewed execution inputs.
7. Preserve stable TEST obligations. Native results are not release/completion
   evidence alone. Failed, missing, cancelled, timed-out or unexpectedly skipped
   required components prevent aggregate PASS; all evidence records checkout SHA,
   run/attempt and mode. No configurable skip flag grants full PASS.

## Scope and verification

Use the plan's exact file list, additionally allowing scripts/ci-evidence.sh,
scripts/verify-ci-supply.sh, scripts/ci-baseline-mappings.txt and
crates/mengxia-testkit/tests/support/lifecycle.rs for explicit shared execution,
retained mapping inventory and bounded record parsing. No production code, Cargo
manifest/lock, dependency/tool version, deny policy, migration, protocol/schema,
third_party, provenance manifest, second-UID wrapper or product test body changes.

Repository-wide verification found legacy TASK-005..TASK-009 driver tests coupled
to inline FAST_PASS or one `run` spelling per ID (SPEC_STALE test-orchestration
assumption under this decision). Also authorize only the driver-mapping tests in
crates/mengxia-testkit/tests/task_005_foundation.rs through task_009_foundation.rs,
and new tests/support/ci_mappings.rs. Replace spelling checks with a closed parser
for actual top-level single/group mappings and exact ID ownership; do not remove
coverage, accept comment-only maps, or edit their product assertions.

The maintenance registry in Specification is authoritative. Validate failure
propagation and coverage, local full developer plus fast/docs gates, reviewed PR
native/supply/second-UID/aggregate jobs and exact merged-main evidence. Mark DONE
only after remote evidence is reviewed. MAINT-002 is not a TASK-011 dependency.

## Rollback

Restore affected orchestration through a reviewed revert if coverage is lost or an
aggregate can incorrectly pass; keep branch protection and mandatory evidence.
Missing hosted tooling is UNVERIFIABLE, not permission to relax attestation.
