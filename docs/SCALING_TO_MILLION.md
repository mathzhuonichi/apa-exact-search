# Design for million-range throughput

## What the current data establish

The campaign creates a new process, factor table, root state, and local proof
arena for every maximum. Learned witness pairs and root facts are scoped to one
maximum. The current event engine never reads earlier candidate proofs or NO
results. Skipping easy maxima therefore cannot directly remove a learning
signal from this implementation: there is no cross-maximum learner to starve.
The filter can still create selection bias by leaving harder maxima.

The user's one-second reference is the oldest algorithm's easy-pruning path;
that path did not spend tens of seconds saturating a shared root. The frozen
residual list becomes denser with scale: 298 entries in 1–100,000,
1,409 in 200,001–300,000, and 2,640 in 900,001–1,000,000. In 33 consecutive
event-v2 results from 222473–224929, the Spearman rank correlation between
the previous residual gap and root time is approximately 0.09; the correlation
between probe events and root time is approximately 0.90. These are descriptive
statistics on a narrow range, not a causal test. The repository does not contain
comparable per-maximum timing records for that oldest fast path. Historical
small-range ledgers use different algorithms and premises.

At 225121, the old Rust solver returned NO in 2.00 seconds with its preset
members and maximum deletion, or 4.57 seconds without maximum deletion. The
seed-free event engine took 26.77 seconds; giving it the same 21 members as
external premises still took 21.86 seconds. Thus the mathematical premises
explain part of the gap, while the root-probe/DFS architecture explains another
large part. Preprocessing, final proof replay, and disk output are small beside
the root search. The active-sum bitset removes substantial serial propagation
work, but fixed probe batching changes search trajectories and gives mixed
wall-time gains.

## Separate three kinds of reuse

1. **Logical reuse of completed maxima.** If every maximum in `(M,n)` is
   excluded, then for a hypothetical solution of maximum `n`, with
   `B=A\\{n}`, require `n in B+B` and `n not in B*B` when
   `n > M(M-1)`. If either condition failed, `B` would itself satisfy the
   closure condition and have maximum `m <= M`. But the mandatory sum `n+m`
   exceeds `m^2`, while no product using `n` can equal `n+m`, a contradiction.
   This uses earlier exclusions as a real mathematical constraint. The event
   engine now accepts `--complete-prefix-base` as an explicit external premise,
   checks the derived additive witness and forbidden factor pairs, and labels
   any dependent NO as `SOLVER_NO_EXTERNAL_LEMMAS`.
   Re-auditing the historical prefix is required before making an independent
   million-range claim. The arithmetic argument does not require 113 to be a
   member; the old implementation's mandatory-113 check is stronger than needed.
2. **Logical reuse within one maximum.** Add reason-carrying clauses for a
   completely refuted decision, then use watched clauses, conflict analysis,
   backjumping, and restarts. Current chronological pair nogoods mostly help
   siblings and are rolled back. Root probe batches repeatedly work from stale
   snapshots. New root implications should be imported as workers finish;
   queued probes invalidated or reprioritized after a material root change.
   Every learned clause must carry a checkable derivation and valid scope.
3. **Heuristic reuse across maxima.** Normalize successful proof patterns by
   fixed small values and offsets such as `n+c`. Cache which factor covers,
   absence probes, and branch orders paid off for a given arithmetic signature
   (factor-domain sizes and primality of relevant `n+c`). Use this only to
   schedule work. A raw clause or root fact from maximum `n` is not automatically
   valid at another maximum, because bounded factor domains change. A proposed
   proof template can be instantiated only after the checker validates its
   arithmetic and complete cases for the new maximum.

## Solver architecture: restore a cheap first path

The current `--root-strengthen` configuration allows 120 seconds of root
construction before its one-second lane limit even begins. That policy cannot
deliver a one-second typical result. Use a proof-producing fast path first and
reserve broad root saturation for its failures:

1. Apply the verified first-stage rule and the prefix-dependent maximum-deletion
   domain before expensive root probes. The additive witnesses for `n in B+B`
   are now a rollback-safe domain with checked empty and unique-witness reasons;
   proper factor pairs of `n` become checked pair nogoods or unary bans.
2. Run cheap propagation with compact bitsets and two watched supports. Branch
   on the smallest live additive or product domain, including the maximum row.
   The old solver's same-maximum advantage makes this path worth porting, but
   it needs proof reasons for every force, ban, and conflict. Give this stage a
   small measured wall budget, initially around 0.1–0.3 seconds, and escalate
   only when its observed proof yield supports it.
3. Use complete root probes selectively. Score a probe by expected new root
   facts per event, measured on the previous rounds. Import a completed probe
   immediately; stop reusing a stale snapshot after its relevant domains change.
   Switch to clause-learning search when root-probe yield falls. Preserve the
   fast path's learned reasons instead of starting a new root from scratch.
4. Persist immutable prime/factor tables across candidates within the campaign.
   This is secondary at 225k, where preprocessing is about 0.1 seconds, but is
   needed for a one-second target near one million.

The separate `research/apa/paper/trackI_findings.md` reports a machine proof
that the 21 preset members are universal for maxima at least 128. Its checker
and complete trees should be audited and imported once into the exact-search
evidence system. The current event engine labels these members as external
premises because that proof bundle is absent from its certificate checker.
The separate research project's two-power criterion is conditional on (H1),
so it cannot be used as an unconditional fast rejection rule here.

## Current prefix experiment and the next fast path

The release build with the checked prefix premise refuted 225121 again. Its
certificate independently replays with `--allow-external-lemmas` and is rejected
without that explicit trust flag. Root time was 8.76 seconds, including 7.97
seconds in parallel probe workers, 0.49 seconds in prime chains, 0.15 seconds
in parent propagation, and 0.02 seconds merging proof fragments. Preprocessing
was 0.09 seconds; final in-process verification was 0.004 seconds. The 66.6
million probe events dominate. The prefix is useful, but it has not restored
the approximately one-second path. These figures are one diagnostic run, not a
throughput distribution.

The next implementation should use the existing event proof vocabulary in a
bounded, sequential bitset/watch search before root probes. Make this an
optional stage that consumes the same root state and proof arena: every branch
refutation must discharge to a parent pair before it may prune a sibling, and
an interrupted branch yields UNKNOWN. A 0.1–0.3 second initial budget is a
measurement parameter, not a completeness shortcut. If the stage completes a
root NO, replay its certificate. If it does not, retain only proved root facts
and move to selected root probes with the remaining budget. Record the fast
stage's wall time, nodes, conflicts, discharged pairs, and proof bytes so its
cost can be compared with the root work it displaces.

The additive maximum domain can now be selected as a DFS branch when it has
fewer live choices than the selected product domain, or when no product demand
remains. This closes a search-path gap but is not the main hard-case speedup:
in a 225121 prefix diagnostic with root strengthening disabled and one second
of DFS, the solver visited nine nodes, made zero additive branches, and returned
UNKNOWN. Product demands remain the active bottleneck. The next concrete
pruning work is to translate the old solver's pairwise witness compatibility
and binary-demand implication closure into scoped, replayable reasons. A
heuristic-only copy may improve branch order, but it cannot soundly delete a
choice without a discharged refutation or another checked inference rule.

On the same maximum, schedule covers with small live domains and measured
productive history first. Import a finished probe's proved conclusions before
dispatching further work from that snapshot. Stop probing a snapshot when
recent batches yield no root fact per event and switch to certificate-producing
search. Cache successful cover order and normalized proof shapes across maxima
as hints only; arithmetic facts must be re-derived against the new maximum.
Compare with history disabled to test whether the residual filter actually
starves useful scheduling information. A one-second claim requires independently
replayed NO or validated YES at the measured percentile, including proof output
and replay time, with no UNKNOWN silently counted as a result.

## Tests that distinguish the hypotheses

Use one binary, one machine, fixed evidence requirements, and stratified samples
from 50–100k, 130–150k, 210–230k, 450–500k, and 900–1,000k. Within each band,
include both residual maxima and maxima excluded by the first-stage filter.
Record wall time through independent replay, root and lane times, event counts,
productive root facts, stale probe work, learned clauses by final scope, and
prefix/template hit rates. Compare four variants in paired runs:

- cold seed-free event baseline;
- bitset/watch search with proof recording;
- that search plus the checked prefix premise;
- that search plus proof-template or heuristic reuse.

Selection bias is supported if residual cases are slower than matched excluded
cases under the same solver. Lost transfer is supported only if the fourth
variant improves time over the same solver with its history cleared. A prefix
gain is measured separately because it changes the logical problem. Report
median, 95th percentile, worst unresolved case, and total machine time in each
band; a fast median with repeated 120-second UNKNOWNs does not meet the goal.
