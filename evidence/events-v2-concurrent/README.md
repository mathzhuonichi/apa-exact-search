# Concurrent event-v2 continuation

Blocks of 1000 residuals verified by `tools/run_events_campaign.py` with
several maxima in flight. Acceptance is strictly in residual order, and each
result needs a fresh complete-scope `VERIFIED_NO` and a separate
`verify-events` replay returning `VERIFIED_NO`.

| Block | Residuals after 370039 | Count | Checkpoint |
|---|---|---:|---|
| 1 | 370169 through 425663 | 1000 | `block-001/checkpoint.json` |
| 2 | 425687 through 479783 | 1000 | `block-002/checkpoint.json` |
| 3 (partial) | 479917 through 514547 | 651 | `block-003-partial/checkpoint.json` |

Each checkpoint lists, per maximum, the result and proof SHA-256, proof size,
event counts, solver threads and root time. Only sample certificates are
tracked; the bulk corpus stays on the originating machine. As in earlier
evidence, contiguous coverage also depends on the first-stage exclusions and the
older prefix, which have not been re-audited here. Solver NO results are
independently replayed arithmetic evidence, not a formal proof.

The run was stopped at the user's request after 2651 verified residuals
(boundary 514547). The next residual to resolve is 514651. See the Handoff
section of `docs/MILLION_CAMPAIGN.md`.
