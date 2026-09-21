# Validation record

The following checks were reported before the pause; this export does not
repeat the research runs:

- The exact-core control suite included exhaustive small cases, comparisons
  with the v9 solver, and known satisfiable extensions at maximum 113.
- The layered wrapper passed 37 known-outcome controls and 15 regression cases.
- Parallel lifecycle checks covered complete NO and directly checked YES,
  deadline cancellation, observed RSS limits, incompatible engines, atomic
  reservations, external SIGTERM, and queued cancellation.
- Additional tests rejected truncated traces, injected a failed wait while
  ensuring other processes were collected, and cancelled root preparation.
- Controller tests checked stopping new dispatch on a non-NO result, collection
  of in-flight cases, and complete return of reserved resources.

The last item describes the behavior that was actually implemented and tested.
It does not satisfy the user's newer requirement to cancel every in-flight
candidate immediately after the first UNKNOWN. That remains a documented
blocker, not a silently claimed completed fix.

A six-case comparison using six computation slots recorded 30.097 seconds with
five NO results and one UNKNOWN under serial-within-case strategies, versus
20.826 seconds with six NO results under rolling internal parallelism. These
are selected-case, host-specific measurements; not all cases improved and no
universal speedup or mathematical certification follows.

Publication validation is limited to checksums, classification consistency,
root/trace framing, Python AST parsing, and C++ syntax checks. It launches no
solver or worker campaign. The saved evidence summaries are under
`evidence/validation/`. Original host paths in JSON evidence have been
normalized; original and exported hashes are distinguished in the provenance
manifest. Historical runtime hash fields remain historical measurements and
need not match a normalized JSON file byte-for-byte.
