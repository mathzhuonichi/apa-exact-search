# Working with this snapshot

Read `PAUSED.json`, `progress/STATUS.json`, and `docs/HANDOFF.md` first.
Maximum 221969 has been excluded by the independently replayed seed-free
arithmetic certificate in `evidence/221969/`. The event-v2 continuation then
reached maximum 224929. Execution is paused for runtime diagnosis. Do not
launch solvers, campaigns, or worker goals for later candidates until the user
explicitly asks to continue.
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
coverage across its gaps. The current contiguous exclusion reaches 224929, and
224999 is the next unstarted residual maximum. This authorizes the maximum-deletion
constraint whenever its stated numeric threshold holds.

Preserve the distinction between solver NO, independently replayed evidence,
formal proof, and UNKNOWN. Use the included checksums and actual result
records; do not infer coverage from directory names. Historical normalized
commands are provenance records, not ready-to-run shell instructions.

Keep repository records in professional English and preserve unrelated files.
The later event-v2 continuation changed the event engine. No search,
deployment, or publication is authorized merely by this file.
