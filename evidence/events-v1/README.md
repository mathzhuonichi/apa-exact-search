# Event-engine validation, 2026-09-21

This directory records development-time verification of the opt-in Rust event
engine. It does not extend historical candidate coverage or resume a campaign.

## Results

- `cargo test`: **24 passed**, including the four existing Rust controls.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.
- Release build: passed.
- Fixed matrix: **112 event-engine runs**, compared with 14 existing-engine
  baselines. Both policies, probes on/off, and all three universal modules
  on/off were tested with one thread and a five-second search/root allowance.
  There were **104 NO and 8 independently validated YES**. Every emitted NO
  certificate was replayed by the separate arithmetic checker. Certificates
  depending on optional universal lemmas are explicitly labeled conditional
  on those external lemmas, rather than seed-free VERIFIED_NO.
- Eight bounded large-case controls: **all UNKNOWN**. These covered 218303
  and 221969, both policies and probes on/off, without external universal
  lemmas, with one thread, five seconds of root preparation and five seconds
  of portfolio search. This is an incomplete large-instance regression; it
  is not a newly generated proof for either maximum.
- The existing seed-free 221969 certificate passed its original independent
  Python replay in a temporary copy. Its historical source files and original
  verification record were not modified. This is separate from the new engine.

The unit suite includes exhaustive subset comparisons for n = 1 through 14,
two positive examples from the research repository, 24 satisfiable extensions,
conditional absence refutations at maximum 113, the 39-clause/six-seed truth
table, count/XOR/product recomputation, rollback and paused-probe checks,
portfolio lifecycle checks, and six classes of deliberately broken evidence
or state: missing coverage, extra root assumption, false empty domain,
modified prime chain, cross-scope use, and omitted ban rollback.

## Files and reproduction

- `regression.json`: all 121 matrix/large/historical records, including exact
  configurations, counters, resource limits, timings, and evidence levels.
- `257.proof.json`: compact seed-free NO certificate produced by the new
  engine, retained as a standalone arithmetic replay example.
- `113-example.json`: newly found 48-element set, checked from raw arithmetic.
- `historical-221969-replay.json`: independent historical replay result.
- `development-probes.json`: earlier bounded exploratory configurations,
  retained to disclose the unsuccessful larger prime-chain/probe experiments.

```sh
cargo test
cargo build --release
target/release/apa-exact-search verify-events --proof evidence/events-v1/257.proof.json
python tools/regress_events.py --binary target/release/apa-exact-search \
  --output outputs/events-regression --large
```

The optional historical replay needs the local `evidence/221969/` proof bundle.
That bundle is a pre-existing workspace artifact and is not replaced by this
new solver revision. The baseline 112-case matrix does not need the bundle.

Timings are observations from this Windows host, not speedup claims. Peak
memory was not sampled. Large-instance readiness, a new 218303 certificate,
and independent universal-lemma bundles are not claimed. The engine remains
opt-in until these acceptance gaps are addressed.
