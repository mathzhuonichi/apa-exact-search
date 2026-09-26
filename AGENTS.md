# Working with this snapshot

The user has paused the sequential event campaign. No campaign processes
should run until the user explicitly resumes. Its published checkpoint is
evidence/events-v2-20260923-continuation/campaign-state-paused.json; it records
replayed results through 370039. The full outputs corpus remains on the
originating machine. Candidate 370169 was interrupted before the solver returned
and must be retried; it is not a completed NO or solver UNKNOWN. See
docs/MILLION_CAMPAIGN.md and the per-candidate replay records.

Read `PAUSED.json`, `progress/STATUS.json`, and `docs/HANDOFF.md` first.
Maximum 221969 has been excluded by the independently replayed seed-free
arithmetic certificate in `evidence/221969/`. The event-v2 campaign is paused
at 370039, with candidate 370169 next to resolve. The user previously
authorized execution through 1,000,000 and has now explicitly paused it. Do
not start a run unless the user explicitly resumes.

Documentation, export, and static snapshot checks do not resume execution.

The Rust programs in `src/main.rs`, `src/solver.rs`, and `src/portfolio.rs`
provide the original `solve` and `campaign` commands. The newer event engine
is under `src/events/` and is selected by `solve-events`. Both process one outer
candidate at a time. A complete root NO or validated YES cancels other threads. UNKNOWN
from an internal bounded branch is not a completed candidate result and must
never be treated as a refutation.

The user explicitly authorized full-machine use for the Rust implementation and
asked to remove runtime SHA-256 checks, provenance gates, memory reservations,
and RSS limits. The dated C++ and Python files are historical references, not
the active runtime. Do not restore those controls unless the user asks.

The user also clarified that `progress/candidates.txt` is the complete hard
residual after first-stage rules directly exclude every omitted integer maximum.
Sequential complete-NO results on this list therefore establish contiguous
coverage across its gaps. The reported contiguous exclusion reaches
370039, conditional on the unrechecked first-stage and older-prefix evidence.
Candidate 370169 is the next residual to resolve. This authorizes the
maximum-deletion constraint whenever its stated numeric threshold holds.

Preserve the distinction between solver NO, independently replayed evidence,
formal proof, and UNKNOWN. Use the included checksums and actual result
records; do not infer coverage from directory names. Historical normalized
commands are provenance records, not ready-to-run shell instructions.

Keep repository records in professional English and preserve unrelated files.
The later event-v2 continuation changed the event engine. No search,
deployment, or publication is authorized merely by this file.
