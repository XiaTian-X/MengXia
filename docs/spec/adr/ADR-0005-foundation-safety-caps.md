# ADR-0005: Foundation finite safety caps

- Status: ACCEPTED
- Date: 2026-08-21
- Clarified: 2026-08-21 (typed-key and tightening-only semantics; accepted defaults/maxima unchanged)
- Partially closes: `OQ-006` for `TASK-002` through `TASK-005`

## Context

Finite limits are correctness and abuse-resistance requirements, not performance SLOs. The foundation needs bounded metadata framing, database work and media ingest before representative benchmarks exist. V1 must still exercise the specification's 100 GiB streaming case without buffering media in memory.

## Decision

The following are conservative, versioned foundation defaults. They are configurable only through the typed keys in Specification §16; missing, zero, overflow or out-of-range values fail startup or disable the dependent capability. A `tightening-only` value may reduce a maximum or increase a reserve but cannot widen the accepted safety boundary.

| Boundary | Default | Hard accepted range / behavior |
|---|---:|---|
| Core Protobuf frame | 4 MiB | 64 KiB–16 MiB; media bytes forbidden |
| Decode/validation nesting | 64 levels | tightening-only range 1–64; deeper input rejected before persistence |
| DB writer queue | 256 commands | 16–4096; full queue returns `BACKPRESSURE` |
| DB read connections | 4 | 1–16 |
| DB busy wait budget | 5 seconds | tightening-only range 1–5000 ms; bounded typed timeout; no infinite wait |
| Stream buffer per ingest | 8 MiB | 1–32 MiB |
| Storage I/O workers | 2 | 1–8 |
| Hash workers | 2 | 1–8 |
| Concurrent ingests | 2 | 1–8 |
| Single ingest byte limit | 1 TiB | tightening-only range 1 byte–1 TiB; validated before copy and during stream |
| Aggregate staging limit | 2 TiB logical ceiling | tightening-only range 1 byte–2 TiB; also constrained by actual free space |
| Filesystem free-space reserve | max(10 GiB, 5% of volume) | byte and percentage floors may only be increased; an operation is rejected before violating reserve |

Limits are checked at admission and while streaming where source size can change or is unknown. A configured logical staging limit never authorizes allocation beyond verified filesystem free space. Boundary tests cover cap−1, cap and cap+1.

Later OQ-006 sub-decisions remain open for Plugin frames/logs/process/CPU/memory, Provider cost/rate/egress and release performance SLO/reference hardware.

## Consequences

- TASK-002..TASK-005 no longer need to invent unbounded or magic limits.
- These defaults may be revised after measured evidence, but widening a security/resource boundary requires a recorded decision and regression tests.
- The values are not latency, throughput or capacity promises.

## Verification

- Configuration parser rejects zero, overflow and out-of-range values.
- Frame, queue and concurrency tests cover cap−1/cap/cap+1.
- 1/10/100 GiB generated-file tests demonstrate O(buffer) memory.
- Disk-full/reserve and staging admission tests leave no canonical broken reference.
- Backpressure and timeout outcomes are typed and do not detach work.
