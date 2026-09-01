# ADR-0010: Layered non-recursive CI orchestration

- Status: ACCEPTED
- Date: 2026-09-01
- Applies to: repository verification orchestration after `TASK-007`
- Authority: user-authorized CI maintenance; no product or later-task authority

## Context

The repository workflow ran on every branch push and every pull request, so an open
pull request could start duplicate runs for the same commit. Every run also executed
the `TASK-007 -> TASK-006 -> TASK-005 -> TASK-003 -> TASK-001/002/004` retained-gate
chain. Those scripts independently repeated workspace build/test, document, naming,
lint and supply-chain work. A documentation-only evidence update therefore incurred
the same formal crash, stress and 1/10/100 GiB matrices as a product-code candidate.

This is a `REPO_STALE / CONFLICT` orchestration defect, not a reason to delete stable
test IDs or weaken formal evidence. The completed task scripts and reviewed historic
runs remain valid evidence for their exact candidates.

## Decision

1. CI has three fail-closed scopes:
   - `docs`: every changed path is exactly `AGENTS.md` or below the explicit prose
     allowlist `docs/spec/` and `docs/proposals/`;
   - `developer`: code/config changes on a pull request;
   - `formal`: code/config changes pushed to `main`, or an explicit
     `workflow_dispatch` run.
2. Branch pushes other than `main` do not trigger the push workflow. Pull requests
   remain the automatic pre-merge feedback path. Workflow concurrency cancels older
   feedback only for the same pull request. Push runs use the commit SHA as their
   group key, so a later documentation push cannot cancel earlier code formal
   evidence.
3. Empty diffs, invalid/unavailable comparison commits and every path outside the
   exact documentation allowlist classify as code. Classification failure is a hard
   failure; it never downgrades work to `docs`. In particular,
   `docs/provenance/` is machine-consumed build input and therefore always classifies
   as code despite its directory name. A future `docs/` subtree is also code until
   this ADR and the allowlist are explicitly extended.
4. The repository-level driver executes the common workspace baseline once and each
   completed task's owned stable-ID mappings once. A task script's explicit
   `component` mode omits only its retained predecessor/common-baseline calls; it
   does not omit an owned stable ID, formal fault matrix, stress matrix, supply-chain
   obligation or end-to-end test.
5. Existing task commands keep their standalone default semantics so historic
   completion evidence remains reproducible. The real second-UID job uses the
   component form because the formal repository job already executed the ordinary
   TASK-003 mappings. It remains a separate `macos-26` job and is still mandatory
   for formal code evidence.
6. Documentation validation runs document traceability, naming/inventory,
   orchestration regression tests and whitespace checks. It does not claim runtime,
   APFS, second-UID, crash, stress or scaling PASS evidence.
7. Formal evidence is still required before a code-bearing task candidate may be
   marked `DONE`. A later documentation-only completion-record update does not
   invalidate the already reviewed exact code candidate and must not rerun formal
   runtime evidence.

## Acceptance and rollback

- Static regression tests must prove the trigger matrix, fail-closed path
  classification, component commands, one common baseline, mandatory formal-only
  matrices and separate second-UID job.
- The documentation, developer and formal repository driver modes must be executable
  locally; platform-only attestation remains a reviewed `macos-26` CI fact.
- If any owned stable mapping disappears, a docs-only false positive is found, or
  formal code CI omits the second-UID job, revert the orchestration change and use
  the prior standalone formal commands until a corrected ADR change is reviewed.

## Consequences

- Most document-only updates no longer spend roughly ten minutes on unchanged
  runtime evidence, and feature-branch pushes no longer duplicate pull-request CI.
- Code pull requests retain fast deterministic feedback; the exact main candidate
  retains formal failure, stress, scaling, supply-chain and platform evidence.
- Adding a future task requires one component call in the repository driver and a
  regression update. It no longer creates a recursively growing gate graph.
- No production code, migration, dependency, public API, authority boundary,
  security cap, completed task status or `TASK-008+` authorization changes.
