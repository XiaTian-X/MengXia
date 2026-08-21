# ADR-0002: Copy-only initial Managed Asset ingest

- Status: ACCEPTED
- Date: 2026-08-20
- Supersedes: ambiguous copy/adopt/reference wording in Specification v1.0.1 TASK-007

## Context

Managed Asset registration requires verified, durable, user-controlled custody before canonical registration. The earlier plan named copy, adopt and reference modes without defining whether source files could be moved/deleted or whether an external reference satisfied custody.

## Decision

The initial `IngestAsset` vertical slice supports copy mode only. Core reads through a stable source handle into same-storage staging, hashes/verifies and durably promotes bytes before the canonical Asset transaction. It never removes or mutates the source.

External references are explicitly `UNMANAGED` Locations and cannot satisfy a `MANAGED` AssetRevision custody requirement. Adopt/move is a destructive semantic command outside the initial slice and requires its own authorization, recovery, cross-filesystem and source-preservation contract before introduction.

## Consequences

- Unknown, adopt and reference modes are rejected by the initial endpoint.
- Blob dedup remains byte-only; repeated copy ingests may create distinct Assets.
- A later reference/adopt feature requires a new contract and, if it changes these guarantees, a superseding ADR.

## Verification

- AC-001..009 pass.
- Source bytes/path remain unchanged on success, failure and retry.
- No Managed Asset can commit without verified durable custody.
- Crash after promotion and before DB commit produces only a safe orphan.
