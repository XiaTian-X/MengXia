# ADR-0001: V1 local authority and identity boundary

- Status: ACCEPTED
- Date: 2026-08-20
- Supersedes: caller-controlled `actor_principal` baseline in Specification v1.0.1

## Context

MengXia V1 is local-first and has no multi-user server, UI or remote transport. The earlier command envelope allowed a caller-provided actor while the security architecture required channel-bound authority and separate Client/Admin/Plugin/Broker domains. Project was also not explicitly distinguished from a tenant boundary.

## Decision

V1 is a single-Library, single-local-owner trust domain. Project is a work/policy context, not a tenant or owner of global Asset/Subject identity.

Core derives `PrincipalContext` exclusively from the authenticated local IPC channel. Request bodies cannot supply actor or Admin role. Verified owner UID/SID may authenticate the ordinary Client under V1's explicit local-owner trust assumption. This is not a claim of containment against arbitrary software already running with the owner's full OS authority.

Admin operations additionally require the target-platform mechanism accepted under `OQ-010`. Until accepted evidence exists, Admin-sensitive operations remain disabled. Sandboxed Plugins never inherit ordinary Client/Admin authority, even though the daemon host process belongs to the same OS account. TCP and remote sessions remain disabled.

## Consequences

- Protobuf field 3 in `CommandEnvelope` is reserved.
- CommandRecord and audit bind the server-derived principal.
- Multi-tenant deployment is prohibited without a new architecture/protocol version.
- A separate Admin socket is necessary routing separation but not sufficient authorization.
- ADR-0004 later accepted the arm64 macOS ordinary Client peer-auth path and frame-cap dependency while keeping Admin disabled; `TASK-003` no longer waits for OQ-010 unless enabling Admin behavior.

## Verification

- Actor-spoof request leaves attribution unchanged.
- Unauthorized peer is rejected before CommandRecord/state.
- Ordinary Client cannot invoke Admin operations.
- Plugin cannot connect to Client/Admin endpoints.
- Cross-Project policy tests do not turn Project into a tenant or Asset owner.
