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
seconds; that output has not been rewritten. The event-v2 campaign later
advanced through 370039 and is now paused at the user's request. Candidate
370169 was interrupted before the solver returned and must be retried; it is
not a completed result or solver UNKNOWN. Candidate 228961 initially exhausted
the 200M probe-event cap, then returned NO with a 500M cap and passed
independent proof replay. See docs/MILLION_CAMPAIGN.md and the candidate retry
record.

## Included residual sequence

`progress/candidates.txt` contains 18,466 strictly increasing maxima. Its SHA-256 is
`6ac25230a2da7dbee72c4b9df06020e741cbc886d2e02f109532e6539860f3bb`.

The current classification has:

| Classification | Count | Meaning |
| --- | ---: | --- |
| PREDECESSOR_REPORTED_COMPLETE | 648 | Earlier reported prefix through 139817; not re-audited during export |
| SOLVER_NO_INHERITED | 84 | Later NO outcomes inherited by the latest ledger |
| NO | 3213 | Solver NO outcomes plus independently replayed event certificates, including 221969 and the current sequential continuation |
| INTERRUPTED | 1 | Candidate 370169 was stopped before the solver returned; retry on resume |
| UNKNOWN | 0 | No solver UNKNOWN in the current ledger snapshot |
| NOT_STARTED | 14520 | Remaining residual entries after the verified campaign boundary and interrupted candidate |

These categories describe the included ledger, not a new theorem. UNKNOWN never
means impossible, and the absence of a new example never establishes finiteness
or complete million-range coverage. Historical probe-cap attempts that returned
UNKNOWN were retried; there is no unresolved solver UNKNOWN in the current
sequential checkpoint. The full bulk certificate corpus remains in the local
ignored `outputs/` directory, so the intervening per-candidate proofs are not
all available for replay from this GitHub snapshot.

## Pending corrections

The active Rust replacement processes one candidate at a time and races all
logical processors inside that candidate. A complete root NO or validated YES
cancels the other solver threads. Because there is only one outer candidate, an
UNKNOWN cannot leave another candidate running. The Rust path has no hash,
provenance, memory-reservation, or RSS-monitoring layer. The user has paused
execution through 1,000,000 at checkpoint 370039. Candidate 370169 must be
retried on resume; the runner uses a 600-second external wall timeout per
candidate.
The checkpoint and pause state are recorded in PAUSED.json,
evidence/events-v2-20260923-continuation/campaign-state-paused.json, and the
local outputs/20260923-million-campaign/state.json.

The Rust code has been formatted, type-checked, linted, unit tested, and built
in release mode on Windows. The 29-test suite passed after the current
propagation change. Any algorithm revision still needs known-outcome regression
checks.

## Candidate sequence semantics

The user clarified that progress/candidates.txt is the complete hard residual
after a first-stage argument directly excludes every omitted maximum. It is not
merely a sample of difficult integers. Consequently, processing the residual in
strict order and obtaining complete NO for every encountered candidate advances
a contiguous exclusion result over all integer maxima, including the gaps.

The current contiguous exclusion reaches 370039; 370169 is the next residual
to resolve and must be retried because it was interrupted.
The complete-prefix assumption remains available where its numeric
threshold holds. The 221969 certificate does not use maximum deletion. For
B = A \ {n}, require n to be in B+B and not in B*B.
