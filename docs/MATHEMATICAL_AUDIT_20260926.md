# Mathematical and implementation audit, 2026-09-26

## Scope and conclusion

The local checkout was fast-forwarded from `9f16e41` to upstream `f4a7e20`.
This audit examines the current Rust event engine, its arithmetic checker,
the original Rust solver, and the event campaign acceptance logic. Historical
C++ and Python solvers were not re-audited. The production campaign remains
paused; no new residual candidate was searched or classified.

One mathematical boundary error and three related campaign acceptance defects
were confirmed and corrected locally. No unsound inference was found in the
active event engine or its seed-free proof rules. This conclusion is a source
audit supported by finite controls and replay, not a formal correctness theorem.

## Confirmed corrections

### Original Rust solver incorrectly rejected the singleton at maximum 2

The public solver interface includes nonempty finite positive-integer sets,
including the exceptional solution `A={2}`. Its sum and product sets are both
`{4}`. The original engine nevertheless forced both 1 and 2 at every maximum,
which introduces the unwitnessable sum 3 at maximum 2 and produces NO.
The newer event engine already handles this case correctly.

`State::seeded` and validation in `src/solver.rs` now preserve `{2}`. The
membership requirement for 1 applies only when the maximum exceeds 2. Explicit
additional assumptions in a supplied root are retained: a root requiring both
1 and 2 at maximum 2 is still correctly rejected as a conditional problem.
Maximum deletion is rejected at maximum 2, where deleting the maximum leaves
the empty set. Three unit tests cover default/prepared roots, supplied roots,
and the deletion guard.

For completeness, the initial-membership lemma for `n>2` is sound. If
`m=min(A)`, representing `2m` gives `m^2 <= 2m`, so `m<=2`. If `m=2`, let
`b>2` be the least other member. Then `4<2+b<2b`; every product of two members
is either 4 or at least `2b`, a contradiction. Thus 1 belongs to A, and
representing `1+1` forces 2. The substantive research problem excludes the
trivial singleton; the correction makes the two solver interfaces consistent.

### A non-certified NO could advance the event campaign

In `tools/run_events_campaign.py`, the rejection branch for missing
`VERIFIED_NO` evidence or a non-root scope returned the report's status. When
that status was `NO`, the caller advanced the checkpoint without executing
proof replay. In particular, `SOLVER_NO_EXTERNAL_LEMMAS` could be accepted as
an unconditional result.

This branch now returns `ERROR`. Only complete original-problem evidence,
followed by successful replay without external-lemma permission, returns NO.
An integration regression verifies that a rejected NO leaves the previous
checkpoint and result count unchanged.

### Result and proof were not bound to the requested maximum

The controller did not compare `report.config.maximum` or
`certificate.maximum` with the candidate being processed. A valid proof of
another maximum could therefore be entered under the requested candidate.
Both identities are now required before acceptance and replay.

### Old files could substitute for missing current output

A zero exit code from a process that wrote no new result or proof could reuse
files from an earlier attempt. Before a retry, current output files are now
moved into `attempts/<timestamp>/`, preserving the historical evidence and
requiring fresh result and proof files. No runtime hashes or resource gates
were added.

Eight acceptance regressions failed on the upstream controller and passed
after correction. The final campaign test suite contains thirteen tests and
uses fake subprocesses only. These defects are demonstrated acceptance
failures; this audit found no evidence that they invalidated the published
event certificates.

## Mathematical rules reviewed

- **Factor completeness.** Every required sum has the full domain
  `{(d,e): 1 <= d <= e <= n, de=s}`, including `1*s` when in range and square
  witnesses. The verifier rebuilds this domain by trial division independently
  of the engine's factor tables.
- **Propagation.** Mandatory members activate sums. An excluded endpoint or
  a proved incompatible pair deletes its product witness. A product value with
  all witnesses deleted is impossible. A required sum with one witness forces
  its endpoints; one with none yields contradiction. Prime-chain steps are
  checked for exact addition and primality.
- **Branch coverage and learning.** Factor branches cover every remaining
  witness. Only complete child contradictions discharge assumptions into a
  parent fact. A common consequence requires every surviving case; unfinished
  and unstarted probes remain survivors. Caps are not refutations.
- **Scopes and parallelism.** Proof references point backward. Premises must
  be valid in the current scope, except through checked discharge or complete
  case joins. Workers export parent-scoped conclusions, with remapped node and
  scope identifiers. Rollback restores mutable branch state.
- **Acceptance.** YES is checked against the original arithmetic condition.
  Event NO requires a complete root contradiction and checker acceptance.
  External membership, prime-clause, even-bound, and complete-prefix rules
  require explicit external-premise permission and retain a distinct label.

The maximum-deletion argument is valid under its declared excluded-prefix
premise. Put `B=A\{n}` and suppose every maximum in `(M,n)` is excluded, with
`M>=2` and `n>M(M-1)`. If B were closed, `m=max(B)<=M`. Representing `n+m`
cannot use n, because `n<n+m<2n`, so `n+m<=m^2`, a contradiction. Products
representing sums in B+B can involve n only at the value n. Consequently,
`n in B+B` and `n not in B*B` are necessary. Membership of the numerical bound
M itself is unnecessary; `docs/ALGORITHM.md` now gives this argument.

The original engine still trusts its historical 21-member seed theorem or
the supplied root's F/B facts. Its bare NO is a trusted-root solver result,
not a seed-free event certificate. The missing universal-membership proof
bundle is discussed in `docs/SCALING_TO_MILLION.md`. The event campaign leaves
those external modules disabled.

## Executed checks

The local tools were Rust/Cargo 1.98.0 and Python 3.13.15 on macOS.

| Check | Result |
| --- | --- |
| Rust unit controls, including four added audit tests | 37 passed |
| Campaign acceptance controls with fake subprocesses | 13 passed |
| Published event certificates, without external-lemma permission | 51 passed, 347,020 proof nodes |
| Separate Python arithmetic replay at 221969 | Passed: 29 events, 101,227 arithmetic steps, 26,114 initial prime-chain bans |
| Python certificate corruption controls | Four rejected: omitted factor case, unfinished absence, unfinished root contradiction, unproved initial member |
| Frozen sequence and classification | 18,466 sorted unique candidates; recorded SHA-256 and counts match; 2,303 entries after 225023 through 370039 |

The added event regression checks two directly validated maximum-113 sets
through 240 deterministic trials and 5,760 mixed fact updates, with interrupted
propagation, rollback, prime chains, optional prefix constraints, and twelve
four-worker root-strengthening passes. Every derived fact is checked against
the actual arithmetic set, independently of the certificate checker.

`make check`, `cargo clippy --all-targets -- -D warnings`, and
`git diff --check` are the final code checks. Per-certificate replay records
are saved in `evidence/validation/mathematical-audit-20260926.json`. To repeat
the replay after `cargo build`, use each record's `path` with
`target/debug/apa-exact-search verify-events --proof <path>`. For the separate
Python checker, copy `evidence/221969/` into a temporary directory first if
the saved historical `verification.json` must remain unchanged.

## Remaining evidence limits and handoff

The exported checkpoint reports verified results through 370039 and an
interrupted attempt at 370169. The classification and checkpoint agree.
Current README and instruction text have been corrected from the stale
335135/335177 snapshot. PAUSED.json and the historical classification were
preserved.

The 51 event certificates physically included here were replayed; the full
2,303-result continuation corpus is not present in this checkout. Nor were
the 648-entry predecessor prefix or first-stage exclusions independently
re-established. Contiguous coverage through 370039 still depends on those
reported inputs. This audit neither proves total finiteness nor completes
the million-range classification.

The user authorized publication of these fixes and audit records after the
local audit. The campaign remains paused. A future explicitly authorized continuation must retry
370169 before moving to the next never-started residual, 370279.
