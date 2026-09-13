# ADR-0016: Toolchain compatibility and necessary security maintenance

- Status: ACCEPTED
- Date: 2026-09-13
- Authority: user requested full review followed by implementation
- Scope: MAINT-003 only; no product task authority
- Supersedes: ADR-0013 candidate-result vocabulary and ambient verifier lookup;
  ADR-0006 build-input tracking only. All attestation and runtime rules remain.

## Review and decision

The reviewed proposal is MAINT-003 v0.2.0. No unresolved product BLOCKER applies
to this maintenance scope. Review found the following corrections before code:

1. Existing CLI gates and build-script tests consume target/debug explicitly.
   Moving complete developer validation to a different build namespace would run
   stale binaries or require unrelated task edits. Instead, the trusted entry
   freshly computes an environment fingerprint and both native build scripts track
   that variable. Changes rebuild ACL and SQLite and relink their consumers in the
   existing target directory. Only isolated candidate verification uses a new
   target directory. The fingerprint is invalidation input, never trust evidence.
   Do not emit rerun-if-changed for Xcode directories or xcode_select_link: Cargo
   recursively scans a directory target. Track concrete tool/header files only;
   the external entry covers selection and OS changes without scanning all Xcode.
2. Reuse the existing shell preflight path/ownership/manifest predicates for a
   developer observation mode; only exact tuple equality is optional in that mode.
   Apple identity and all safety predicates remain mandatory. This observation
   alone is not ABI compatibility; a fresh native build/test must establish that.
3. Isolate cargo-deny 0.20.2 using its official arm64 release, with reviewed archive
   AND extracted executable SHA-256. Check bytes before execution, including cached
   copies. Prepare is an explicit network operation, not part of inspect/build.rs.
   Retain existing independently pinned protoc regeneration and rustup toolchains.
   No additional general installer, parser dependency, or privileged operation.
4. Pinned Rust is explicitly selected for maintained entries, not the user's global
   default. Reject compilation/environment overrides and noncanonical target paths;
   never silence them. Direct Cargo has best-effort external input tracking, not
   a guarantee of detecting every OS change. Document the required entry for that.
5. Compatibility, exact attestation match and security coverage are independent.
   Identity/preparation failure is not ABI failure. A source/build failure not
   attributable to the environment remains SOURCE_CHECK_FAILED or UNVERIFIABLE.
   There is no automatic unsafe downgrade or false PASS for unavailable evidence.
6. Stop ordinary Cargo Dependabot PR generation, retain security alerts/updates,
   Actions maintenance, current supply checks and all ADR-0015 aggregation rules.
   Provide a bounded source/coverage report and reviewed advisory-event records;
   applicable unresolved events block supply acceptance. Unknown/manual sources
   are not security PASS. A continuously running repair agent is NOT_ENABLED.
   No new write-enabled PR job, auto-merge, credentials or external automation.
7. Applicable security fixes remain mandatory under SEC-020. Version or frozen
   product-lock changes require a separate incident-specific reviewed patch, not
   test deletion. Preserve all historical delivery evidence. Current versions do
   not change in this maintenance implementation.

## Closed implementation scope

New files: scripts/dev-toolchain.sh, scripts/toolchain-environment.sh,
scripts/toolchain-tools.sh, scripts/toolchain-maintenance.sh,
scripts/verify-toolchain-maintenance.sh; docs/provenance/developer-tools-v1.toml,
docs/provenance/toolchain-security-events-v1.tsv;
docs/development/toolchain-maintenance.md;
crates/mengxia-testkit/tests/toolchain_maintenance.rs.

Existing files: scripts/verify-toolchain-candidate.sh,
scripts/verify-macos-acl-toolchain.sh, scripts/check-supply-chain.sh,
scripts/verify-ci-fast.sh, scripts/verify-ci-supply.sh, scripts/verify-repository.sh;
.github/workflows/ci.yml, .github/dependabot.yml;
crates/mengxia-platform-fs/build.rs and, as an explicit narrow exception,
third_party/libsqlite3-sys-0.38.2/build.rs (rerun tracking only);
crates/mengxia-testkit/tests/ci_orchestration.rs and document_traceability.rs
(maintenance integration only); canonical specification/decision/review/plan/intake,
task-lifecycle version records, this ADR, MAINT-003 proposal and AGENTS.md.

No Cargo manifest/lock, tool version, deny policy, historical provenance, protocol,
migration, SQLite source/bindings/options, product runtime or second-UID test change.
No storage/NAS/platform-support decision or new TASK-011 prerequisite.

## Verification and lifecycle

Specification owns TEST-MAINT3-ENV-001, TEST-MAINT3-INSTALL-001,
TEST-MAINT3-CACHE-001, TEST-MAINT3-SECURITY-001, TEST-MAINT3-INTEGRATION-001.
Require positive/negative/real executable cache tests, real host candidate evidence,
local docs/fast/full developer, performance measurements, and reviewed exact PR/main
evidence before DONE. Only one Xcode installation is currently available locally;
a second real environment must be evidenced remotely or recorded unavailable, not
mocked as a supported OS claim. Local completion is not remote acceptance.

## Recovery

Keep old safe tools/materials; reject tampered or partial installations. Never
delete user data, switch the OS, accept licenses, disable protections or return to
a known-affected tool automatically. Revert faulty maintenance code through review
without relaxing branch protection or required supply/second-UID evidence.
