# Handoff after paused million-range continuation

The [2026-09-26 local mathematical audit](MATHEMATICAL_AUDIT_20260926.md)
records the corrected singleton boundary and campaign acceptance defects,
51 successful event-certificate replays, and the remaining evidence limits.
The fixes do not resume the campaign or change its historical classifications.

## Current execution state

The event-v2 campaign is paused at the user's request. Its last independently
replayed residual is 370039; the campaign records 2,303 `VERIFIED_NO` results
after 225023. Candidate 370169 was interrupted before the solver returned and
must be retried on resume; it is not classified as `NO` or solver `UNKNOWN`.
No campaign processes should run until the user explicitly resumes. The
648-entry predecessor prefix and first-stage exclusions remain unaudited.
There are no solver `UNKNOWN` results in the current sequential campaign;
earlier probe-cap attempts that were retried remain as historical records.
The runner uses a 600-second external wall timeout per candidate.

This is a 2026-09-21 historical handoff. The 2026-09-23 continuation first
paused at 225431 for performance diagnosis; the user later explicitly resumed
the campaign through 1,000,000. See docs/MILLION_CAMPAIGN.md and
progress/STATUS.json for the live state.

The historical snapshot described below reached 228961. The current checkpoint
and pause state are recorded in evidence/events-v2-20260923-continuation/
campaign-state-paused.json, PAUSED.json, and progress/STATUS.json. The complete
bulk certificate corpus is in the local ignored outputs directory and is not
included in the GitHub repository.

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
