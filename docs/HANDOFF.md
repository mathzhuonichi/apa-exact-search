# Handoff after paused optimized continuation through 224929

Current execution state: PAUSED_FOR_RUNTIME_DIAGNOSIS.

The user explicitly authorized continuation with the optimized Rust event-v2
solver after the 221969 resolution. The optimized `--threads 0`
configuration independently solved the continuation candidates through
224929 as complete `NO`/`VERIFIED_NO` results. Their proof, result, and replay
records are under `evidence/events-v2-optimized-continuation/`; the run ledger
records the exact options and resource observations.

The accepted complete-residual semantics now extend contiguous exclusion
through 224929. The next unstarted residual is 224999. This is a solver NO
boundary supported by independently replayed event certificates, not a formal
proof-assistant theorem.

The user then requested a pause to diagnose why current candidates often take
longer than the earlier approximately-one-second path. The current command
uses `--root-strengthen` with `--root-seconds 120` and `--probe-events 200000000`;
the `--seconds 1` value only bounds the lane search after root preparation.
Unlike the older `solve`/`campaign` path, the `solve-events` CLI also has no
`--complete-prefix-base` option, so this continuation did not apply the
authorized maximum-deletion root constraint.
All recent slow cases spent their time in shared-root strengthening and had
`lane_search_time: 0`. Root batches reach many processors, but batch joins,
proof-fragment import, serial `quiesce`, and serial `prime_chains` create long
tails and candidate-specific propagation work.

Two `sol` reviews classified the near-cap cases 224071 and 224869 as
candidate-specific propagation amplified by that known batch-barrier design,
not an algorithmic regression or a genuine solution. No solver process remains
running. Any future algorithm revision still needs known-outcome regression
checks.

Maximum 221969 has a complete seed-free arithmetic exclusion certificate in
`evidence/221969/`. Read its `README.md`, `result.json`, and `verification.json`.
Run `python evidence/221969/verify.py` to replay it without launching a search.
The checker derives its initial root from only 1, 2, and 221969; the proof uses
neither historical seed assumptions nor maximum deletion. This is independently
replayed computational arithmetic evidence, not a proof-assistant theorem.

The certificate for 221969 does not independently re-audit older campaign
results. The older 30-second UNKNOWN remains a historical record and was not
rewritten into a solver NO.

The older 30-second UNKNOWN and its profile remain historical records in
`outputs/automatic/run-20260921-154351-288/221969.json` and
`docs/GPT6_HANDOFF_221969.md`. They are not edited into retrospective solver
NO results. Preserve UNKNOWN versus complete NO, and distinguish conditional
branch refutations, independently replayed arithmetic evidence, and formal
proof.

## Included evidence and omissions

The latest nine roots, result records, and seven winning original-root traces
are included. Earlier campaign ledgers and validation summaries are included,
with original source hashes. Older per-probe states, bulk traces, executable
binaries, and unrelated research topics are not copied. Links or paths pointing
to `source-workspace` describe omitted historical artifacts; they are not files
claimed to exist in this standalone checkout.

The first 648 entries of the frozen sequence have only the earlier reported
completion status in this export; their complete proof corpus was not re-audited
or recopied. Keep this distinction when reporting cumulative coverage.
