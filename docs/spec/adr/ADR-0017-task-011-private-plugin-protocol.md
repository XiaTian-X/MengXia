# ADR-0017: TASK-011 private Plugin protocol and bounded host session

- Status: ACCEPTED
- Date: 2026-09-13
- Owners: TASK-011 private protocol; TASK-012 process and sandbox integration
- Extends: ADR-0005, ADR-0010, ADR-0014, ADR-0015

## Context

TASK-010 established canonical Plugin package identity without execution or
authority. TASK-011 must establish a separate private protocol and hostile
conformance harness before process/sandbox integration. The previous canonical
configuration left Plugin log, frame and process/session limits open under OQ-006,
so implementation could not safely invent them.

The host boundary must also avoid granting process-launch, Core Client/Admin,
database, CAS or Broker authority. Protobuf parsers accept compatibility-oriented
unknown/duplicate representations that are unsuitable as an unreviewed authority
boundary without an additional closed validator.

## Decision

1. TASK-011 implements only proposal v0.1.2's `mengxia.plugin.v1` private control
   protocol, caller-supplied-stream host session and test-only hostile executable.
2. Production Plugin crates cannot spawn, locate or kill a process, open a path or
   network endpoint, import Core/app/ports/store/platform layers, or expose any
   Client/Admin/DB/CAS/Broker handle. TASK-012 owns managed launch, process-tree
   custody, resource enforcement, bounded kill/reap and sandbox proof.
3. The v1.0 protocol contains only host/plugin hello, ping, cooperative shutdown
   and a closed peer-failure enum. Host-side expected package/session context is
   never constructed from Plugin wire data and is not an authentication claim.
4. Control frames use the existing four-byte big-endian codec with a configurable
   tightening-only 64 KiB through 256 KiB limit, default/hard ceiling 256 KiB.
   Closed validation rejects unknown/reserved/duplicate/non-minimal/wrong-wire,
   group, depth and unknown-enum input before generated-message decode.
5. Decode depth defaults to and cannot exceed 16, with accepted tightening range
   2 through 16. V1.0 has exactly one application request in flight.
6. Inbound and outbound frame queues each default to and cannot exceed 16, with
   accepted tightening range 1 through 16. Full admission returns backpressure or
   suspends the sole owning producer; no unbounded side queue, dropped frame or
   detached retry exists.
7. Active private sessions default to and cannot exceed 4, with accepted
   tightening range 1 through 4. Held unopened permits count. TASK-012 retains an
   independent production process permit until observed reap even after protocol
   capacity is released.
8. Stderr is drained through a fixed 8192-byte scratch buffer, counted with checked
   arithmetic and discarded. `MENGXIA_PLUGIN_LOG_BYTES` is the inclusive lifetime
   quota per session, default/hard ceiling 1 MiB and tightening range 64 KiB through
   1 MiB. The first excess byte terminates the session; raw stderr is never logged,
   formatted or persisted.
9. Handshake, request and cooperative-shutdown budgets are respectively 5 seconds,
   30 seconds and 2 seconds, with tightening ranges 100 ms through their defaults.
   Each operation uses the earlier of caller and configured absolute deadlines
   across admission, queueing, write, flush, read and validation. Cancellation and
   drop close the session; no partial-frame resynchronization occurs.
10. One owning SessionDriver polls reader, writer, dispatcher and stderr branches
    without spawning internal tasks or threads. Terminal paths synchronously drop
    all owned futures/streams/queues and release the permit last. The caller owns
    any outer task handle; TASK-012 owns the process handle.
11. The testkit hostile harness runs its closed action matrix sequentially with at
    most one live child. Every result observes bounded kill and reap. This is
    protocol evidence only, not production execution or sandbox evidence.
12. Exact source, descriptor and provenance are checked in and regenerated only
    from the pinned protoc 35.1 arm64 artifact. Existing prost/Tokio versions and
    features are reused; no new third-party dependency or deny-policy exception is
    accepted.
13. TASK-010's historical whole-lock hash becomes a frozen fixture while current
    package/security closure, exact dependencies and supply policy remain checked.
    Only the three local plugin-host/plugin-proto/testkit lock blocks may change.
14. TASK-011 adds twelve evidence IDs disjoint from the immutable 150-ID baseline
    and five MAINT-003 IDs. CI topology, second-UID evidence, supply checks, runner,
    action pins, permissions and failure aggregation remain unchanged.
15. Specification §19.12 and §20.0.10 are the sole normative AC-101..AC-103 and
    stable-test obligation sources. Proposal v0.1.2 is the exact implementation and
    file-scope supplement.

This closes only TASK-011's protocol/log/session and test-process-fan-out portion
of OQ-006. TASK-012 production process-tree/CPU/memory/handle/sandbox caps,
TASK-016 Provider/egress/cost caps and release SLO/reference hardware remain open.

## Consequences

- TASK-011 can be implemented and attacked without enabling a real Plugin.
- Queue and log abuse has accepted finite memory/work ceilings.
- The private protocol cannot encode public Core/Admin operations or authority.
- A cooperative shutdown acknowledgement never proves process death.
- Later capabilities require additive, separately reviewed protocol fields and
  their owning authorization/Broker contracts.
- Tightening configuration is possible without widening the accepted ceilings.

## Rejected alternatives

### Reuse Core Client/Admin protocol

Rejected because protocol possession would collapse separate authority domains and
make accidental privileged operations representable to hostile Plugin code.

### Spawn or sandbox in TASK-011

Rejected because managed executable custody, exact launched-image binding and the
real sandbox backend are TASK-012 gates. A test fixture cannot establish them.

### Retain arbitrary Plugin stderr

Rejected because a generic byte buffer cannot prove secret redaction. TASK-011
counts and discards raw diagnostics; later structured diagnostics need a separate
schema and redaction review.

### Rely on generated Protobuf decode alone

Rejected because unknown fields, duplicate singular fields and open enum integers
could be silently normalized or discarded before policy checks.

### Pin every future workspace lock hash to TASK-010

Rejected because legitimate later local edges would require rewriting historical
evidence. The accepted replacement freezes the historical lock and checks the
current protected dependency closure and complete supply policy.

## Verification

- AC-101 through AC-103;
- all twelve stable TASK-011 tests in Specification §20.0.10;
- cap-1/cap/cap+1, checked aggregate payload and deadline/cancellation races;
- exact proto/descriptor/provenance regeneration and closed wire corpus;
- real test-child malformed/flood/crash/hang matrix with bounded reap;
- architecture and Cargo metadata negative tests;
- additive local/hosted CI attribution with every retained evidence requirement.
