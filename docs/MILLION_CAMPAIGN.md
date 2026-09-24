# Million-range continuation, 2026-09-23

The goal is to verify every integer maximum through 1,000,000. The frozen
`progress/candidates.txt` list contains the complete hard residual after the
first-stage exclusions, as identified by the user. The prior contiguous
exclusion ended at 224929. This run does not use maximum deletion or external
universal lemmas; every new NO requires an event certificate and a separate
`verify-events` replay returning `VERIFIED_NO`.

## Current pause

The user paused the campaign on 2026-09-24. The last independently replayed
residual is 335135; the checkpoint records 1,646 `VERIFIED_NO` results after
225023. Candidate 335177 is next and has not been started. The 648-entry
predecessor prefix and first-stage exclusions have not been re-audited, so they
remain a separate requirement for a final million-range claim.

## Runtime finding and change

The recent 222473–224929 continuation used 4–42 seconds of shared-root time
per maximum, and 14–200 million probe events. Lane search was usually zero.
Parallel root probes reached 24 workers, but parent propagation, proof merging,
prime-chain passes, and batch joins remained serial. Some candidates generated
far more propagation than their neighbors.

`prime_chains` formerly allocated and cleared a reachability array of length
`2n+1` for every unassigned even candidate. It now uses generation stamps in
one scratch array per pass and reuses the BFS queue. This preserves the same
reachability graph, proof steps, budgets, and result semantics. All 33 Rust
tests passed. The acceptance script regenerated and independently replayed
seed-free `VERIFIED_NO` certificates for 218303 and 221969. A single 224929
comparison gave 14.50 seconds on the previous binary and 9.93 seconds on the
new binary, with identical 87,722,404 probe events and 27,604,748 prime-chain
steps. The binaries used different Windows Rust toolchains, so that comparison
alone cannot isolate the speedup or establish its distribution across maxima.

The new binary solved 224999 and the campaign smoke test solved 225023;
both returned and independently replayed `VERIFIED_NO`. Their artifacts are
retained under `evidence/events-v2-20260923-continuation/candidates/`.

## Campaign checkpoint

`tools/run_events_campaign.py` runs the frozen residual in order, writes one
result, proof, and replay log per maximum, and checkpoints only after replay.
Its state is outputs/20260923-million-campaign/state.json. The runner certified
residuals through 228887 using its recorded options. The initial
228961 attempt exhausted the 200M probe cap with 14 cases unstarted; a
500M-cap retry completed at 171.3M events and passed independent replay. The
original UNKNOWN is preserved beside the verified result in
candidates/228961/attempts.json. The campaign later advanced through 335135
before the user paused it. Its state records 1,646 verified results since
225023 and identifies 335177 as the next unstarted residual. The runner stops
at the first UNKNOWN, YES, solver error, or replay failure, and records the
attention case. The seven complete results and certificates through 225431,
plus the latest certificate for 335135, are retained under
`evidence/events-v2-20260923-continuation/`. The complete local bulk corpus is
3.23 GB and is not tracked in Git; the published classification and checkpoint
summarize the intervening independently replayed results, but those
per-candidate proofs cannot all be replayed from this repository alone.

The campaign state is the authority for the continuation checkpoint.
progress/STATUS.json records the verified boundary and pause state. The prior
evidence ledger describes the earlier snapshot through 224929; do not use it to
infer the current boundary. Launching the runner again is an explicit resume
action; it resumes from the checkpoint at 335135 and starts with 335177.
Do not report a larger contiguous boundary until the corresponding replay log
and result have been checked.

## Root-cause comparison

Historical small-range results are mixed. In the 128k–140k factor-branch
portfolio ledger, the 107 NO rows have median 4.17 seconds and 90th percentile
15.96 seconds; ten rows completed within one second. The later 142k-range
layered-witness ledger has 53 NO rows, median 1.79 seconds and 90th percentile
3.18 seconds. These are historical solver NO records without independent event
certificate replay. They use different candidates, strategies, and premises.

A same-maximum diagnostic at 225121 is more useful. The original Rust `solve`
path returned NO in 2.00 seconds with its 21 preset members and the complete
prefix maximum-deletion mode; without maximum deletion it took 4.57 seconds.
These are trusted-solver results using the historical preset, not new seed-free
certificates. Event-v2 took 26.77 seconds seed-free. Enabling the same 21
members as external premises in event-v2 still took 21.86 seconds, so the
premise difference does not account for the whole gap. Event-v2 without shared
root strengthening returned UNKNOWN after a 30-second lane limit, showing that
its current DFS does not substitute for root saturation.

For seed-free event-v2 at 225121, preprocessing took 0.09 seconds and final
proof checking 0.005 seconds. An instrumented repeat measured 27.88 seconds
of shared-root time, including 12.24 seconds in parallel probe batches,
14.67 seconds in serial parent propagation, 0.79 seconds in prime chains, and
0.03 seconds in proof-fragment merging. The original propagation performed
492.8 million low-priority member-pair activations. Disk proof output was about
3.2 MB and the full campaign step took 27.0 seconds, consistent with a compute
bottleneck rather than I/O.

The event engine now scans member and active-sum bitsets to find only sums that
have not yet been activated. The 225121 parent propagation fell to 2.95 seconds
and low-priority activations to 17.3 million, but changed probe scheduling
increased probe events from 115.5 million to 169.0 million. Total root time
fell only from 27.88 to 25.30 seconds. On 225181 it fell from 13.63 to 8.23
seconds; on 224929 it remained near ten seconds. The change passed all 29 Rust
tests, the 218303/221969 certificate acceptance and replay, and a fresh
225121 replay. It is a useful propagation improvement, not a full throughput
solution.

An exploratory change from two queued covers per worker to one reduced 225121
from 25.30 to 18.87 seconds and 169.0 to 104.8 million probe events, but
increased 224929 from 10.51 to 14.63 seconds and 225181 from 8.23 to 9.41
seconds. The fixed smaller batch was reverted. This shows that stale-snapshot
work is instance dependent; a blanket batch-size change is not reliable.

## Next algorithm work

The detailed throughput and cross-maximum reuse design is in
`docs/SCALING_TO_MILLION.md`.

1. Replace the fixed batch barrier with completion-driven root imports so
   short probes can strengthen the parent while long probes continue against
   their immutable snapshot. Retain complete-case and proof-scope checks.
2. Reduce repeated state cloning and proof-fragment allocation; assess smaller
   batches only after measuring how often their early conclusions save work.
3. Prioritize probes by expected useful root facts and stop low-yield rounds
   instead of re-running broad 2–8 witness coverage on stale snapshots.
4. Evaluate porting the original bitset/watch search strategy with event proof
   recording, using same-maximum benchmarks to isolate the search-core gap.
5. Evaluate the maximum-deletion lemma behind an explicit complete-prefix
   premise and a matching replay rule. Results using that premise must be
   labeled separately from seed-free `VERIFIED_NO` until the prefix evidence
   is assembled and checked.
6. Re-audit the 648-entry predecessor prefix and first-stage exclusions before
   a final million-range coverage claim. Their reported status is not the
   same evidence level as the new event certificates.
