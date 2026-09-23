# Algorithm-design handoff: bottleneck at maximum 221969

> Historical algorithm-design note. Maximum 221969 was subsequently excluded
> by the seed-free arithmetic certificate in `../evidence/221969/README.md`.
> The performance observations below describe the earlier UNKNOWN runs.

## Historical status

The trusted-solver contiguous NO boundary is 221923. Maximum 221969 returned
UNKNOWN in a 30-second, 24-thread portfolio run. A subsequent unlimited run
was interrupted for algorithm redesign after about 11 minutes; it produced no
result and must not be treated as evidence either way.

The preceding bottleneck, 218303, was resolved as a complete root NO after
224.81 seconds by lane 10. Maximum 217127 was resolved as a complete root NO
after 54.32 seconds by lane 9. These are solver results, not independently
replayed certificates or formal proofs.

## Exact model and preprocessing

For fixed maximum `n`, the solver searches for a finite set of positive
integers `A` with `max(A) = n` and `A+A` contained in `A*A`.

The active implementation is `src/main.rs`, `src/solver.rs`, and
`src/portfolio.rs`.

1. `Problem` sieves primes through `2*n`, stores smallest prime factors, and
   lazily caches every bounded factor-pair domain.
2. `State::prepared` starts with the mandatory seed values
   `1,2,3,5,7,11,13,17,19,23,31,43,47,53,61,71,73,83,103,109,113,n`.
3. Root propagation removes even values whose prime translates force a prime
   above `n`, and forces endpoints common to every remaining factor witness of
   a mandatory sum.
4. Complete-prefix mode applies the maximum-deletion lemma. With
   `B = A \ {n}`, it requires `n` to be in `B+B` and not in `B*B`. The additive
   witnesses are a reversible live bit domain. Proper factor pairs of `n` are
   unary bans or binary nogoods.

## Search core

Adding a member incrementally updates products, performs prime-based bans, and
activates every new member-pair sum. A prime sum must itself be a member not
exceeding `n`. A composite sum is a demand whose factor pairs are possible
product witnesses.

Each demand has two watched live factor supports. Losing both supports is a
conflict; one remaining support forces both endpoints. Propagation currently
rescans the complete historical demand vector at every fixpoint.

DFS prefers demands with exactly two live witnesses from a configurable
lookahead window. Otherwise it branches on the first demand with more than two
live witnesses. Factor options are sorted by the smaller endpoint and are
rescanned from the beginning by `choose`; the propagation watches are not
reused by the branching selector.

After a child is completely refuted, the selected endpoint pair becomes a
scoped nogood. Learning is chronological and rollback-scoped: there is no
conflict analysis, backjumping, restart policy, persistent clause database, or
cross-lane sharing.

The 24-thread portfolio varies insertion versus maximum-first demand order,
lookahead windows 16/32/64/128, witness order/rotation, and three compatibility
modes. A complete root NO or validated YES cancels the other lanes. A deadline
returns UNKNOWN and never refutes a branch.

Compatibility reasoning checks whether endpoints from two binary witnesses
produce a forbidden prime sum, followed in selected lanes by binary arc
consistency and a 2-SAT implication closure.

## Arithmetic structure of 221969

`221969 = 11 * 17 * 1187`.

The previous prime is 221957, a gap of 12. The next prime is 221987, a gap of
18. The next-prime gap is therefore not unusually large.

The important feature is the density of small positive offsets `b` for which
`221969+b` is prime:

```text
18, 20, 30, 38, 42, 54, 60, 72, 74, 90, 98, 104,
138, 140, 144, 158, 168, 180, 182, 192, ...
```

There are seven such offsets at most 64, compared with three for 217127 and
two for 218303. Because `n` is mandatory, every such offset is immediately
banned: including `b` would require the prime sum `n+b > n`.

All sums of `n` with the odd mandatory seeds are even. The only even seed is
2, and `n+2 = 221971 = 67 * 3313`, so there is no immediate seed contradiction.
The root is therefore heavily pruned without being closed.

The proper factor pairs of `n` are:

```text
(11, 20179), (17, 13057), (187, 1187)
```

Since 11 and 17 are mandatory, maximum deletion bans 20179 and 13057. The last
pair becomes a binary nogood. The additive side still begins as a very large
disjunction requiring one pair summing to 221969.

## Measured 30-second profile

The 24-lane run recorded:

```text
status                     UNKNOWN
wall time                   30.14 s
total nodes                 5,125
maximum nodes in one lane   1,627
factor visits               565,111,450
demand scans                47,431,975
conflicts                   838
learned pairs               4,368
compatibility checks        168,232
compatibility rejections    0
factor visits per node      110,265.6
demand scans per node       9,255.0
branch conflict rate        16.43%
```

For comparison, the 30-second 218303 run performed about 32,234 factor visits
per node. Maximum 221969 therefore spends 3.42 times as many factor visits per
node and advances far fewer nodes, even though it performs fewer demand scans
per node.

The leading diagnosis is that dense prime-offset bans remove many small
factor endpoints. Factor domains remain sorted from small to large endpoint,
so activation, watch repair, and especially `choose` repeatedly traverse long
prefixes of dead factor pairs before finding two or three live supports. The
cache stores immutable factor lists, not incremental live-domain state.

Compatibility produced zero support rejection, so its CPU cost did not prune
this instance. The large number of learned pairs relative to nodes indicates
mostly local chronological learning rather than reusable structural clauses.

## Failed optimization experiment

An experimental unresolved-demand list was implemented and fully reverted.

- A swap-remove vector changed demand ordering. It reproduced NO but made the
  217127 regression slower, 63.58 seconds instead of 54.32 seconds.
- A stable linked list reduced demand scans but damaged cache locality. The
  same 217127 regression remained UNKNOWN after 90 seconds.

This is strong evidence that pointer-heavy per-thread state and apparently
innocent demand-order changes can outweigh scan reductions. Future work should
preserve deterministic order and contiguous memory, and must be evaluated on
217081, 217127, 218303, and 221969 rather than microbenchmarks alone.

## Recommended design directions

1. **Incremental factor domains.** Keep a compact live-option bitset or support
   count per active demand. Build flat endpoint-to-option occurrence lists so a
   newly banned value invalidates only affected options. Trail changed words
   for rollback. Avoid linked structures and per-option heap allocation.
2. **Reuse supports in branching.** `choose` should use maintained live counts,
   watched positions, and a continuation cursor instead of rescanning each
   factor list from index zero. Maintain MRV buckets or a lazy versioned heap
   over domain sizes.
3. **Prime-offset-aware witness ordering.** Precompute the forbidden-offset
   bitset induced by `n+b` prime. Score witness endpoints by immediate prime
   consequences and expected support destruction rather than by a rotation of
   factor order alone.
4. **First-class additive domain.** Put the maximum-addition witness domain in
   the same MRV mechanism as product demands. In particular, branch on it when
   it becomes binary or otherwise smaller than the selected product domain;
   currently it is considered only when no binary product demand is available.
5. **Conflict-directed learning.** Record decision reasons for bans and forced
   members, derive clauses over decisions, and backjump. Share only clauses
   proven valid at the common root. UNKNOWN must never cause learning.
6. **Portfolio specialization.** Disable compatibility modes when a cheap root
   sample predicts zero pruning, and allocate more lanes to genuinely distinct
   value/domain branching, restart, or clause-learning policies rather than
   rotations of the same DFS.
7. **Progress preservation.** Add exact checkpoint/restart support or use
   escalating budgets for a repeated candidate. Every current rerun restarts
   all lanes from the root and loses scoped learning.
8. **Instrumentation before redesign.** Record root mandatory/banned counts,
   demand-domain histograms, scanned dead prefixes, maximum depth, live support
   counts, nogood scan cost, and per-phase CPU time. The current aggregate
   counters cannot distinguish root preprocessing, watch repair, and branch
   selection factor visits.

## Correctness constraints

- Only a complete root NO excludes a maximum.
- UNKNOWN from a deadline or internal bounded branch is not a refutation.
- Learned facts must be scoped to the assumptions under which the child was
  completely refuted unless a sound conflict analysis proves a wider scope.
- A reported YES must be checked against the original closure condition.
- Maximum deletion is valid only with a genuinely contiguous excluded prefix.
- Preserve the distinction between a solver NO, independent replay, and a
  formal proof.

## Required regression gate

Before production continuation, a replacement should at minimum:

1. pass formatting, release tests, and strict linting;
2. reproduce 217081 as NO within 30 seconds;
3. reproduce 217127 as NO (baseline 54.32 seconds);
4. reproduce 218303 as NO (baseline 224.81 seconds); and
5. improve the 30-second 221969 profile without weakening exactness.
