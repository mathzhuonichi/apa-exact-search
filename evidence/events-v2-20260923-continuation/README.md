# Verified continuation and paused checkpoint through 370039

The seven residual maxima 224999, 225023, 225121, 225181, 225187, 225413,
and 225431 each returned `NO` with `VERIFIED_NO` evidence. Each `candidates/<n>`
directory contains the result, the complete event certificate, and the output
of an independent `verify-events --proof` replay without external lemmas.

These results used the prime-chain generation-stamp optimization and the
event-v2 options recorded in each result. They preceded the later active-sum
bitset experiment. All seven are sequential entries in the frozen residual
list. Together with the accepted first-stage exclusions and prior boundary,
they extend the reported contiguous exclusion from 224929 through 225431.

The campaign has since independently replayed 2,303 sequential residuals after
225023, reaching 370039. Its latest result, proof, and replay log are included
under `candidates/370039/`; the paused checkpoint and next residual are recorded
in `campaign-state-paused.json` and `progress/STATUS.json`. Candidate 370169
was interrupted before the solver returned and must be retried on resume. This
is not a solver UNKNOWN or a completed result.

The full bulk corpus for the 2,303 residuals remains under the locally ignored
`outputs/20260923-million-campaign/` directory and is not included in this
repository. The classification ledger and checkpoint publish the reported
statuses, but remote users cannot replay every intervening proof from this
repository alone.

The full paused bulk runner state remains locally under
`outputs/20260923-million-campaign/state.json`. See `docs/MILLION_CAMPAIGN.md`
for performance diagnosis and evidence limitations of the earlier prefix.
