# Event-engine v2 acceptance: both maxima excluded

The upgraded Rust event engine independently generated complete, seed-free
root refutations for **218303** and **221969**. Both were replayed by the
separate arithmetic checker without enabling external lemmas. No historical
certificate was supplied to the solver, and no later candidate was searched.

| Maximum | Result | Shared-root time | Retained proof nodes | Root rounds | Independent replay |
|---|---|---:|---:|---:|---|
| 218303 | NO | 59.286 s | 6,693 | 11 | VERIFIED_NO |
| 221969 | NO | 27.721 s | 5,715 | 5 | VERIFIED_NO |

Both runs used the same options in `acceptance.json`. Each concluded during
shared-root preparation, before any portfolio lanes or DFS search were started.
The original constraints, all bounded factor witnesses, and the proof checker
were preserved. The only initial membership premises in these proofs are
1, 2, and the specified maximum. The minimum-element lemma justifies 1 and 2
for n > 2. Neither proof uses a universal-lemma bundle, a historical prefix,
maximum deletion, a fixed extra root member, or a candidate-specific schedule.

## What changed

The shared-root scheduler considers short factor domains throughout the current
unresolved set, including small sums, instead of probing only the maximum row.
It propagates case joins to quiet, renews prime-chain exclusions after new
members appear, and retries absence probes against the strengthened root.
Rounds continue while the parent changes, within a common root deadline and
candidate probe budget. Complete covers may now contain up to eight witnesses.

Propagation-only probes have separate event and additional-member limits.
Reaching either limit leaves a survivor whose already proved facts may join
with facts independently proved in every other survivor. The cap does not
create a contradiction or justify learning. Parent checkpoints remain restricted
to empty queues, and scope/proof IDs remain monotonically allocated.

This is a search-scheduling upgrade. The checker and its arithmetic inference
rules were not modified to accept these results.

## Reproduce

From the repository root, with the stable Rust toolchain and Python:

```sh
cargo build --release
python tools/accept_events.py --binary target/release/apa-exact-search \
  --output outputs/events-acceptance
```

On Windows, use `target/release/apa-exact-search.exe`. The acceptance script
regenerates each certificate, then starts a separate checker process. UNKNOWN,
conditional evidence, and failed replay are acceptance failures. No historical
proof files are required.

Replay the retained certificates without searching:

```sh
target/release/apa-exact-search verify-events --proof evidence/events-v2/218303.proof.json
target/release/apa-exact-search verify-events --proof evidence/events-v2/221969.proof.json
```

For an individual solve with the current defaults:

```sh
target/release/apa-exact-search solve-events --maximum 218303 \
  --root-strengthen --threads 1 --proof outputs/218303.proof.json
```

## Controls and interpretation

- `cargo test`: **27 passed**, including boundary cases, small exhaustive
  comparisons, positive max-113 sets and extensions, counter/rollback checks,
  scope/coverage tampering, and the new shared-root/probe-budget controls.
- `cargo fmt --check`, Clippy with warnings denied, and release build passed.
- `controls.json`: **112** fixed known-outcome runs with the new root loop,
  compared with the existing Rust baseline: **104 NO and 8 YES**. Both demand
  policies, DFS-node probes on/off, and external lemma modules on/off were
  exercised. These optional modules are off in both large acceptance proofs.
- `218303.json` and `221969.json` retain exact options, timings, counters,
  productive root-cover sums, and successfully discharged absence targets.

The timings are host measurements, not a claim about speed on every residual
candidate. Peak memory was not sampled. Temporary probe proof nodes remain in
the append-only arena until final dependency extraction, so transient storage
is much larger than the retained certificates. No memory limit or reservation
was added. These are independently checked arithmetic certificates, not
proof-assistant formalizations.

The v1 UNKNOWN measurements and the earlier independently generated 221969
certificate remain unchanged in their original directories. This acceptance
does not re-audit older campaign coverage or authorize continuing the residual
sequence.
