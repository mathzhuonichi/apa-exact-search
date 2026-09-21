# Paused status

The user explicitly paused the research after reporting memory pressure and
clarifying the stopping rule. The subagent goal was set to paused. A process
check at the pause found no campaign controller, solver, wrapper, or root
preparation process. The publication task does not resume that goal.

## Latest campaign

Seven maxima completed with solver NO:

147241, 147353, 147467, 147503, 147587, 147743, 147829.

Two in-flight maxima remain UNKNOWN:

- 147907: first unresolved maximum; all three lanes were cancelled at the limit.
- 147923: also unfinished when the campaign ended. It is not an exclusion.

The next never-started source entry is 147983. A future continuation must first
handle both unresolved entries. Starting only after the largest directory or
recorded maximum would silently skip work.

## Included residual sequence

`progress/candidates.txt` contains 18,466 strictly increasing maxima. Its SHA-256 is
`6ac25230a2da7dbee72c4b9df06020e741cbc886d2e02f109532e6539860f3bb`.

The exported classification has:

| Classification | Count | Meaning |
| --- | ---: | --- |
| PREDECESSOR_REPORTED_COMPLETE | 648 | Earlier reported prefix through 139817; not re-audited during export |
| SOLVER_NO_INHERITED | 84 | Later NO outcomes inherited by the latest ledger |
| NO | 7 | Completed NO outcomes in the latest campaign |
| UNKNOWN | 2 | Unresolved latest cases |
| NOT_STARTED | 17725 | Remaining entries of this particular frozen sequence |

These categories describe the included ledger, not a new theorem. In
particular, UNKNOWN never means impossible, and the absence of a new example
never establishes finiteness or complete million-range coverage.

## Pending corrections

1. When any candidate returns final UNKNOWN, immediately cancel all other
   in-flight candidates as well as stopping new dispatch.
2. Reassess concurrency and memory policy before resuming. The previous design
   used two candidates with three lanes each and monitored RSS; those settings
   are historical measurements, not an approved resumed resource budget.
3. Obtain an explicit user instruction to resume. No unattended launch is
   configured in this repository.
