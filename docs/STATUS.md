# Current status

Maximum 221969 is excluded by the self-contained arithmetic certificate in
[`evidence/221969`](../evidence/221969/README.md). Its independent checker passed
from only the initial members 1, 2, and the assumed maximum. It does not depend
on the historical seed list, maximum deletion, or previous NO results. This is
independently replayed computational evidence, not proof-assistant verification.

## Latest result

The final checked contradiction is the mandatory sum `221227+180181=401408`:
all 20 bounded factor pairs are excluded. The certificate has 29 events and
101,227 recorded post-root arithmetic steps, plus 26,114 initial prime-chain
exclusions. Run `python evidence/221969/verify.py` for standalone replay.

The previous Rust run at 221969 remains a historical UNKNOWN after 30.14
seconds; that output has not been rewritten. The optimized event-v2 continuation
then independently replayed complete NO certificates through 224929. The
accepted complete-residual boundary now reaches 224929, and 224999 is the next
unstarted residual. The user has paused execution to diagnose the runtime
shape: current `--root-strengthen` work uses a separate 120-second root
deadline, so `--seconds 1` does not stop root preparation; recent slow cases
spent all of their time in shared-root strengthening, serial propagation,
prime-chain passes, and batch tails. See
[`evidence/events-v2-optimized-continuation/LEDGER.md`](../evidence/events-v2-optimized-continuation/LEDGER.md).

## Included residual sequence

`progress/candidates.txt` contains 18,466 strictly increasing maxima. Its SHA-256 is
`6ac25230a2da7dbee72c4b9df06020e741cbc886d2e02f109532e6539860f3bb`.

The current classification has:

| Classification | Count | Meaning |
| --- | ---: | --- |
| PREDECESSOR_REPORTED_COMPLETE | 648 | Earlier reported prefix through 139817; not re-audited during export |
| SOLVER_NO_INHERITED | 84 | Later NO outcomes inherited by the latest ledger |
| NO | 908 | Solver NO outcomes plus the independently replayed 221969 exclusion |
| UNKNOWN | 0 | No outstanding started residual |
| NOT_STARTED | 16826 | Remaining residual entries beginning at 224999 |

These categories describe the included ledger, not a new theorem. In
particular, UNKNOWN never means impossible, and the absence of a new example
never establishes finiteness or complete million-range coverage.

## Pending corrections

The active Rust replacement now processes one candidate at a time and races all
logical processors inside that candidate. A complete root NO or validated YES
cancels the other solver threads. Because there is only one outer candidate,
an UNKNOWN cannot leave another candidate running. The Rust path has no hash,
provenance, memory-reservation, or RSS-monitoring layer. Execution is currently
paused at the user's request for runtime diagnosis.

The Rust code has been formatted, type-checked, linted, unit tested, and built
in release mode on Windows. The 29-test release suite passed before the
continuation. Further execution requires addressing the observed shared-root
serial bottleneck or an explicit choice to continue with the current behavior;
any algorithm revision still needs known-outcome regression checks.

## Candidate sequence semantics

The user clarified that `progress/candidates.txt` is the complete hard residual
after a first-stage argument directly excludes every omitted maximum. It is not
merely a sample of difficult integers. Consequently, processing the residual in
strict order and obtaining complete NO for every encountered candidate advances
a contiguous exclusion result over all integer maxima, including the gaps.

The current contiguous exclusion reaches 224929; 224999 is the next unstarted
residual. The complete-prefix assumption remains available where its numeric
threshold holds. The 221969 certificate does not use maximum deletion. For
`B = A \\ {n}`, require `n` to be in `B+B` and not in `B*B`.
