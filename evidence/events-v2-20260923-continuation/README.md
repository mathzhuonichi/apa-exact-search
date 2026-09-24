# Verified continuation and paused checkpoint through 335135

The seven residual maxima 224999, 225023, 225121, 225181, 225187, 225413,
and 225431 each returned `NO` with `VERIFIED_NO` evidence. Each `candidates/<n>`
directory contains the result, the complete event certificate, and the output
of an independent `verify-events --proof` replay without external lemmas.

These results used the prime-chain generation-stamp optimization and the
event-v2 options recorded in each result. They preceded the later active-sum
bitset experiment. All seven are sequential entries in the frozen residual
list. Together with the accepted first-stage exclusions and prior boundary,
they extend the reported contiguous exclusion from 224929 through 225431.

The campaign has since independently replayed 1,646 sequential residuals after
225023, reaching 335135. Its latest result, proof, and replay log are included
under `candidates/335135/`; the paused checkpoint and next residual are recorded
in `campaign-state-paused.json` and `progress/STATUS.json`. Candidate 335177 is
next and has not been started. No unresolved UNKNOWN remains in this campaign.

The full bulk corpus for the 1,646 residuals occupies about 3.23 GB under the
locally ignored `outputs/20260923-million-campaign/` directory and is not
included in this repository. The classification ledger and checkpoint publish
the reported statuses, but remote users cannot replay every intervening proof
from this repository alone.

The full paused bulk runner state remains locally under
`outputs/20260923-million-campaign/state.json`. See `docs/MILLION_CAMPAIGN.md`
for performance diagnosis and evidence limitations of the earlier prefix.
