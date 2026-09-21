# Event engine v1

The new implementation is an opt-in foundation, selected by `solve-events`.
The existing `solve` and `campaign` commands retain the previous Rust engine.
No campaign or later residual maximum was started during this revision.

## Build and use

```sh
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings

target/release/apa-exact-search solve-events --maximum 113 --threads 1 \
  --seconds 5 --root-seconds 5 --probes --output outputs/113.json

target/release/apa-exact-search solve-events --maximum 112 --threads 1 \
  --proof outputs/112.proof.json --output outputs/112.json
target/release/apa-exact-search verify-events --proof outputs/112.proof.json
```

The `.exe` suffix is needed when invoking the binary directly on Windows.
`--threads 0` uses all logical processors. Each lane clones the same completed
root and shares immutable factor tables and the root proof prefix. Mutable
state, learned pairs, and probe facts are private to the lane.

The `--seconds` limit applies to the shared portfolio search interval, including
lane creation. `--root-seconds` is separate and includes table construction and
root preparation. Both use zero for no deadline. Cancellation is checked at
atomic event boundaries. Table construction and final proof compression/replay
are not interruptible; their time is reported separately from lane execution.
An unfinished root returns UNKNOWN and creates no lanes.

## Implementation and switches

| Module | Implementation | Selection |
|---|---|---|
| Complete factors | Two-pass product enumeration, fixed witness IDs, CSR endpoint and endpoint-sum indices | Always in event engine |
| Propagation | Affected-domain events, count/XOR/product counters, dense unresolved set, member-pair jobs with fixed endpoints and resumable cursors | Always |
| Bad products | Both E-before-member and member-before-E; diagonal bans; independent E deletes its product domain and incompatible endpoint-sum witnesses | Always |
| Rollback | Undo trail for facts, products, domain counters, dense-set positions and pair adjacency; empty-parent-queue checkpoints; fresh queue epochs and monotonically allocated proof scopes | Always |
| MRV | `(live_count, outside_maximum_row, -sum)` | `--policy mrv` |
| Strict maximum row | `(outside_maximum_row, live_count, -sum)` | `--policy strict-maximum-row-first` |
| Portfolio | Alternating policies by default, deterministic witness rotations/reversal | `--threads`; optional single-policy override |
| Propagation-only probes | Complete 2–4 witness maximum-row covers and bounded absence probes | `--probes` |
| Six-seed probe | All six overlapping positive seed cases; no absent-group assumptions | `--seed-probe`, with probes, U, and clauses enabled |
| Prime chains | Pure temporary closure under steps 2 and the tested even member; exact arithmetic proof steps | `--prime-chain-steps`, default 0 |
| Universal members | Exactly the stated U, with the n > 2 applicability condition | `--universal-members` |
| Prime clauses | Exactly 39 cross-group positive clauses | `--prime-clauses` |
| Small even bound | At least seven members in `{2,4,...,384}` | `--even-bound` |
| Proofs | Scoped arithmetic DAG, dependency extraction, independent trial-division replay | Always constructed; `--proof` writes the certificate |

`dead[wid] == None` is the single authoritative alive flag; a dead witness
stores its first proof ID. A separate Boolean is unnecessary. Count and XOR
updates occur only on the first deletion. Product counts are updated through
endpoint incidence when a member is inserted, including square witnesses once.
There is no second `done` truth source or repeated scan of historical demands.
Domain enumeration is used for a chosen cover or for its complete proof, not
for repeatedly finding a singleton witness.

The six groups are `{29,37,41}`, `{89,97,101}`, `{59}`, `{67}`, `{79}`,
and `{107}`. A seed case asserts all Q primes outside its group. The cases
overlap and collectively follow from the 39 clauses. No integer outside U
is installed as an extra unconditional root member. Additional members require
a propagation proof, a discharged refutation, or complete case coverage.

Probe limits are propagation-event budgets, separate from the global clock.
`--probe-case-events`, `--probe-node-events`, and `--probe-events` bound the
case, node, and candidate totals; the candidate total is divided among lanes.
`--probe-domains` and `--absence-targets` bound scheduling work at a node.
An atomic operation may visit many incidence entries before the next budget
check. The first version makes one strengthening pass per DFS node, so it
needs no conditional-result cache. Prime-chain exclusion is a single bounded
root pass. Repeating or scheduling chains inside DFS is deferred.

Probe outcomes own their facts and proofs before rollback. Paused cases remain
survivors; unstarted cases have no new facts. A join needs a proof of its fact
in every survivor and a refutation in every excluded case. If exactly one
case remains, its starting assumptions can be installed even if it was not
started. Case-derived E retains its prior join proof; witness deletions caused
by that E cannot be used to manufacture its original justification.

DFS propagates to quiet before taking a checkpoint. A full child refutation
can discharge its witness pair into a parent-scoped nogood; UNKNOWN and ERROR
cannot. Subsequent siblings may use that nogood, and its proof remains in the
append-only arena. The final empty-domain proof enumerates all original
factor pairs, including those removed before branching. Checkpoint rollback
also handles probes interrupted with partially consumed low-priority jobs.

## Proof and result meanings

The independent checker in `src/events/proof.rs` does not call the event engine,
use its factor tables, or trust its list of surviving witnesses. It rebuilds
bounded factors by trial division, verifies scope ancestry, verifies exact
cover assumptions, checks every removed witness and discharges only complete
child contradictions. Proof IDs refer only backward. Small boundary cases are
explicit: n = 1 is NO, n = 2 is the directly validated set `{2}`.

The only built-in seed-free premises for n > 2 are 1, 2, and the maximum.
The minimum-element argument is the same as in the existing arithmetic
certificate: a nontrivial solution has minimum 1, then 1+1 forces 2.
Raw YES validation rebuilds member products independently and tests every sum,
including squares and self-addition.

The three optional universal-lemma modules preserve the supplied statements,
but their original independent proof bundles were not present in this checkout.
They therefore default off. Their use is recorded as an external premise;
checking the resulting derivation does not prove those universal statements.
The user authorized regenerating missing evidence, but this version does not
claim to have regenerated their universal proofs. Such a certificate requires
`verify-events --allow-external-lemmas` and reports
`VALID_DERIVATION_WITH_EXTERNAL_LEMMAS`, rather than VERIFIED_NO.

Reports distinguish:

- `status: YES`, `evidence: RAW_VALIDATED_YES`.
- `status: NO`, `evidence: VERIFIED_NO`: complete seed-free arithmetic replay.
- `status: NO`, `evidence: SOLVER_NO_EXTERNAL_LEMMAS`: checked inference with
  explicitly named external universal premises in the dependency closure.
- `status: UNKNOWN`: no complete root result within the supplied limits.
- `status: ERROR`: internal inconsistency, invalid certificate, or failed YES.

Verified arithmetic evidence is not a proof-assistant theorem. A complete root
result cancels other lanes; all threads are joined. Any lane error overrides a
winning result. Certificate verification failure becomes ERROR. Output failure
returns a process error before printing a successful result. With no `--proof`,
the certificate is still checked in memory, but no durable proof file is saved.

## Validation and current limits

```sh
python tools/regress_events.py --binary target/release/apa-exact-search \
  --output outputs/events-regression --large
```

This runs fixed known-outcome controls only. It never reads or advances the
residual candidate sequence. The large cases are 218303 and 221969 only.
All retained measurements and limitations are in `evidence/events-v1/`.

Tests cover complete arithmetic tables, both reverse indices, square and
zero-ID witnesses, event direction, duplicate deletion, random rollback,
pending-queue rejection, paused probes and unstarted cases, strict-policy
selection, exhaustive small subsets, all 39 clauses/six-case cover assignments,
GitHub's positive examples, proof corruption, and portfolio completion.

The 113 examples come from
[RESULTS.md at research commit 3867395](https://github.com/mathzhuonichi/research/blob/3867395440384a07fae542a85035f7bb423809a1/apa/paper/RESULTS.md).
The two listed examples are independently validated in tests; the reported
110,592-example classification is not re-enumerated by this revision.

This first event engine is not yet a replacement for production campaigns.
Its bounded large-case controls can return UNKNOWN, and no speed improvement
is claimed. Peak memory is currently `null` (not measured); no RSS limit or
reservation is introduced. Proof arenas can grow substantially during search.
Bitset optimizations, proof-storage reduction, deeper prime-chain scheduling,
regenerated universal certificates, and a new independently replayed 218303
certificate remain future work. The existing 221969 certificate remains a
separate historical result, not a newly generated event-engine refutation.

Maximum-deletion pruning remains available in the old engine under its
existing explicit option. It is not silently added to this event engine.
No provenance gate, runtime source hash, or campaign authorization change is
introduced.
