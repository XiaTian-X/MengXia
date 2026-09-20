# macOS Native R0-A research tools

This directory contains a bounded developer research harness. It is not a product
sandbox, TASK-012 implementation, formal attestation, or permission to run external
executables.

The only supported commands are:

```text
scripts/native-research/run.sh self-test
scripts/native-research/run.sh observe
scripts/native-research/run.sh run-a
scripts/native-research/run.sh verify <evidence-directory>
```

`run-a` compiles the checked-in controller and probe with the selected Apple SDK,
then runs the twelve closed cases from `cases.tsv` three times each. The probe acts
only on itself and on fixed names below a newly-created owner-only directory. It
does not use the network, a real Library, credentials, user plugins, signing,
registration, mounts, GPU resources, or arbitrary paths and commands.

Budgets are fixed by `MACOS-NATIVE-R0-RESEARCH-DESIGN.md`: at most 36 serial probe
runs; 10 seconds per normal run and 1 second for the deadline case; at most 32 MiB
per explicit allocation and 8 MiB of touched payload; at most two 96 KiB write
attempts; 32 extra descriptors; 32 KiB per stdout/stderr stream; and 4 MiB of
retained case evidence. These are experiment budgets, not product limits or proof
of complete OS enforcement.

Results are observations:

- `OBSERVED_EXPECTED` means the bounded case matched its reviewed expectation.
- `OBSERVED_COUNTEREXAMPLE` means it produced useful contrary evidence.
- `INCONCLUSIVE` means the required precondition or observation was unavailable.
- `NOT_RUN` means the case was not executed.

A structurally valid evidence set may contain counterexamples or inconclusive
results. Exit status 0 from `run-a` or `verify` means the evidence is complete and
internally consistent, not that a backend is qualified. Unresolved observations
return 2; malformed/inconsistent evidence or tool failure returns 1. Evidence stays in the
reported directory under `target/native-research`; it is not automatically removed
or treated as a trusted timestamp/signature.

Do not add free-form probe arguments, external PIDs, executable paths, sandbox
profiles, environment forwarding, or unbounded stress cases. Such changes require
a new reviewed research scope.

The corrected harness uses schema 2 / R0A-2. Older evidence is historical and is
not silently upgraded. The verifier checks unique probe fields, result/summary
agreement, resource errno, paired controls, and controller-observed file sizes.
It is an internal-consistency check, not cryptographic proof that a run occurred.

All seven inputs are copied into a read-only build snapshot before compilation;
both C units compile from that snapshot. Recorded source hashes describe those
inputs, and live-source drift prevents finalization. Do not edit the harness while
running it. A single monotonic 600-second deadline covers case scheduling and
evidence finalization after compilation; each controller inherits its remaining
budget. Filesystem/kernel stalls are not promised to resolve within that deadline.
Cancellation and overflow stop normal work but do not skip owned-child reaping.
Unconfirmed cleanup retains its controller rather than leaving a reusable PID.

Research launch uses the macOS 26+ public posix_spawn chdir API and
POSIX_SPAWN_CLOEXEC_DEFAULT with explicit stdio actions (stdin is /dev/null).
This does not implement or qualify a production sandbox. Self-test builds use
R0_SELF_TEST and never contribute real evidence: bounded cancellation, overflow,
TERM-to-KILL, high-FD inheritance across exec, expired/in-flight batch deadlines,
and error-classification checks supplement the semantic verifier mutants. Their
private temporary root is removed and its absence checked before success.
