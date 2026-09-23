# Algorithm and implementation

This document records the original solver and historical C++/Python designs.
The latest event-v2 solver is documented in [EVENT_ENGINE.md](EVENT_ENGINE.md).

For a fixed maximum n, the search tracks mandatory members, excluded members,
required sums, and their complete bounded factor pairs. A required sum must
have at least one product witness inside the final set. No result is positive
until the original closure condition is satisfied.

## Maximum deletion (complete-prefix mode)

Let `n=max(A)`, let `M` be the largest previously possible maximum, and put
`B=A\\{n}`. This mode is enabled only when every maximum in `(M,n)` is already
excluded, `M` is a mandatory member, and `n>M(M-1)`.

Every sum in `B+B` is below `2n`. A product witness using `n` can therefore
only be `n*1=n`. If `n` were absent from `B+B`, then `B+B` would be contained
in `B*B`, so completeness of the excluded prefix would give `max(B)<=M`.
But `n+M` also needs a product witness. It cannot use `n`, and two factors from
`B` have product at most `M^2`, contradicting `n+M>M^2`. Hence `n` is in
`B+B`. If `n` were also in `B*B`, the same reasoning would make `B` a smaller
counterexample and yield the same contradiction. Therefore every surviving
search state must satisfy both `n in B+B` and `n not in B*B`.

The implementation represents the additive witnesses `(a,n-a)` by a live
bitmask. Bans and learned pair nogoods delete bits with rollback, an empty
domain is a conflict, and a singleton forces both endpoints. Proper factor
pairs of `n` are installed as root nogoods (or a unary ban for a square).

## Exact search core

The v11 C++ solver retains full member-pair propagation, cached factor tables,
bitsets, two watched factor supports, and a rollback trail. It adds scoped
learning from completely refuted witness choices and compatibility checks
between selected binary witness domains. A prime sum larger than n, or a
prime already excluded in the branch, certifies incompatible endpoints.
Removing a witness without support in another mandatory domain is a necessary
inference; surviving compatibility alone does not establish existence.

A failed witness may be learned only after complete rejection of its branch.
Learned facts are rolled back with their scope. Time and node caps return
UNKNOWN and never justify rejecting an option. The selected demand ordering
is explicit: `insertion` or `maximum_first`, together with a candidate window.

## Layered failed-witness lookahead

The v2 Python wrapper first probes short maximum-row factor domains with a
node cap of one and at most 0.25 seconds each. If needed, it retries surviving
options with a node cap of 31 and at most 0.5 seconds each. All probing shares
a two-second phase budget and a 32-probe cap. A unique surviving witness is
mandatory relative to the established root and can strengthen the subsequent
exact search. The complete case still shares a 30-second search budget.

Only complete NO probes remove witnesses. A YES is directly checked against
the original sum/product condition. Forgetting rejected non-unit pairs before
fallback weakens pruning but does not remove a valid solution.

## Controlled parallel portfolio

Three independent lanes race on each candidate: the layered wrapper, original-
root insertion/window 8, and original-root insertion/window 64. Python threads
coordinate isolated process groups; mutable search state is never shared.
An atomic resource reservation bounds the search pool, and the first valid
observed complete result wins. Other process groups receive termination,
forced termination if necessary, and explicit collection. Result metadata,
frozen source/binary hashes, and trace structure are checked.

The historical defaults were two outer candidates, six computation slots,
a 30-second shared search deadline, and sampled RSS limits. RSS polling is a
soft limit, not an OS hard memory ceiling. These defaults require reassessment
following the user's pause. The controller's old drain-in-flight behavior on
UNKNOWN also still requires correction before resumption.

## Complexity and certification boundaries

The global search tree has no demonstrated polynomial bound. Changing a
branching heuristic can make one instance much faster and another slower.
Parallelism improves strategy coverage but can spend more total CPU work.
Benchmark timings are finite evidence, not a complexity proof.

NO output is a trusted solver result. Prefix-tree framing and factor arithmetic
checks do not independently verify all learned-clause or compatibility
inferences. The preserved traces are not Lean proofs or standalone certificates
accepted by an independent checker. No independent replay was performed for
the latest campaign, consistent with the user's instructions.
