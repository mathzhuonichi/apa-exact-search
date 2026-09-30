# Million-range continuation, 2026-09-23

The goal is to verify every integer maximum through 1,000,000. The frozen
`progress/candidates.txt` list contains the complete hard residual after the
first-stage exclusions, as identified by the user. The prior contiguous
exclusion ended at 224929. This run does not use maximum deletion or external
universal lemmas; every new NO requires an event certificate and a separate
`verify-events` replay returning `VERIFIED_NO`.

## Current pause

The user paused the campaign on 2026-09-26 for export. The last independently
replayed residual is 370039; the checkpoint records 2,303 `VERIFIED_NO` results
after 225023. Candidate 370169 was interrupted before the solver returned and
must be retried on resume. It is not a solver `UNKNOWN` or a completed result.
The runner uses a 600-second external wall timeout per candidate. The 648-entry
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
candidates/228961/attempts.json. The campaign later advanced through 370039
before the user paused it. Its state records 2,303 verified results since
225023 and identifies 370169 as the interrupted residual to retry. The runner
stops at the first UNKNOWN, YES, solver error, or replay failure, and records
the attention case. The seven complete results and certificates through
225431, plus the latest certificate for 370039, are retained under
`evidence/events-v2-20260923-continuation/`. The complete local bulk corpus is
not tracked in Git; the published classification and checkpoint summarize the
intervening independently replayed results, but those per-candidate proofs
cannot all be replayed from this repository alone.

The campaign state is the authority for the continuation checkpoint.
progress/STATUS.json records the verified boundary and pause state. The prior
evidence ledger describes the earlier snapshot through 224929; do not use it to
infer the current boundary. Launching the runner again is an explicit resume
action; it resumes after 370039 and retries 370169 first. Do not report a larger
contiguous boundary until the corresponding replay log and result have been
checked.

## Resumed run, block 1 (2026-09-29)

The user resumed the campaign with the concurrent runner. The first 1000
residuals after 370039, namely 370169 through 425663, were each accepted as a
fresh `VERIFIED_NO` with an independent replay, in residual order (see
`evidence/events-v2-concurrent/block-001/checkpoint.json`). 370169, the
interrupted candidate, was retried first and verified. One solver was killed by
the container's 14.3 GB memory limit at 372923 while four jobs ran; that was a
resource event and not a result, and it was retried and verified with three jobs
of two threads each. A container restart later interrupted in-flight candidates,
which were rerun. `progress/STATUS.json` and `progress/classification.csv` are
not updated by this change; the checkpoint file is authoritative for this block.

Block 2 continues contiguously: 425687 through 479783, 1000 residuals accepted in
residual order with fresh `VERIFIED_NO` results and independent replays
(`evidence/events-v2-concurrent/block-002/checkpoint.json`), with no resource
events. Through block 2 the concurrent run has verified 2000 residuals after
370039.

## Automatic relaunch after container restarts (2026-09-30)

Container restarts kill the runner but leave `state.json` and the certificates
on disk. `.claude/hooks/session-start.sh` (registered in `.claude/settings.json`)
relaunches an interrupted campaign when a remote session starts. It never starts
a new campaign and never resumes past a stop: it acts only when `state.json` is
`RUNNING` with no attention case, `PAUSED.json` has `paused: false` and
`automatic_resume_authorized: true`, and no runner process is alive (checked
under a lock, so simultaneous hooks start only one). The runner sets
`automatic_resume_authorized` to false whenever it stops, so a YES, UNKNOWN, or
solver error is never resumed automatically, and setting it to false or
`paused` to true disables the hook. Resumption reruns every residual after the
checkpoint with fresh evidence. The hook is skipped outside remote sessions and
when `outputs/20260929-concurrent/state.json` does not exist, so a fresh
checkout does nothing. `tools/test_session_start_hook.sh` checks these cases
against fake campaigns. A hook runs when a session starts; it cannot restart a
runner inside a session that stays open after the container has restarted.

## Candidate-parallel execution (2026-09-29)

Within one maximum, root strengthening is a dependency chain: each productive
probe changes the root used by the next. The recorded unlimited-budget runs used
one root worker per batch, and speculative batches can recover only part of the
idle processors. The runner therefore parallelizes across maxima. Each residual
is still an independent problem with its own certificate and replay; only the
scheduling changes.

`tools/run_events_campaign.py` now keeps up to `--jobs` maxima in flight
(default: every available processor) and gives each solver
`--threads-per-candidate` threads (default: the processors divided among the
jobs, so one each when every processor runs a job). Acceptance is unchanged and
strictly ordered:

- candidates start in residual order and at most `--lookahead` (default 1000)
  beyond the first unaccepted residual;
- a result is accepted, and the checkpoint written, only after every earlier
  residual was accepted, with the same fresh-evidence, `VERIFIED_NO`, complete
  scope, identity, and separate `verify-events` checks as before;
- when a candidate returns anything other than NO, later running candidates
  are cancelled (their folders record `cancelled.json`), earlier ones finish,
  and the campaign stops at the first non-NO in residual order;
- a cancelled or unaccepted result never advances the checkpoint, and resuming
  reruns every residual after the checkpoint with fresh evidence.

`--jobs 1` reproduces the previous runner, including the all-processor solver
command. The recorded `options` are unchanged; the resource settings are
recorded separately in `execution`, and each change is appended to
`execution_history` with the checkpoint at which it took effect. Several jobs
run the documented one-thread root algorithm per maximum, so search trajectories
differ from the multi-thread path while acceptance requirements do not.

Validation on this 4-processor cloud machine, without running any residual:

- 21 runner tests with fake subprocesses, including a randomized comparison of
  150 concurrent schedules with sequential acceptance, earliest-failure
  reporting, cancellation of only later candidates, lookahead, and resource
  recording;
- a CPU-bound stand-in solver over 16 maxima took 17.49 seconds with one job
  (0.97 busy processors) and 4.42 seconds with four (3.81 busy processors);
- the release binary over maxima 115-600 accepted all 486 with independently
  replayed `VERIFIED_NO` using one or four jobs (3.57 and 1.28 seconds), and a
  list containing 113 stopped at that validated YES with 112 as the checkpoint;
- the previous and new binaries agreed on all maxima 1-600 with the campaign
  options and one thread.

Memory is per process: at n = 1,000,000 the factor tables are about 670 MB, the
state allocates up to about 410 MB, and the proof arena grows with the search.
Four concurrent processes are expected to fit in 16 GB; lower `--jobs` if a
future range needs more memory. No residual timing on this machine has been
measured yet.

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
