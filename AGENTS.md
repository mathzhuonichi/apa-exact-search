# Working with this snapshot

Read `PAUSED.json`, `progress/STATUS.json`, and `docs/HANDOFF.md` first.
The user paused the research. Do not launch solvers, campaigns, or worker goals
until the user explicitly resumes them. Documentation, export, and static
snapshot checks do not resume the research.

Before production resumption, fix the stopping behavior: a final candidate
UNKNOWN must cancel every in-flight candidate immediately, not only stop new
dispatch. UNKNOWN from an internal bounded probe is not itself a completed
candidate result and must never be treated as a refutation. Reassess resource
limits after the user's report of memory pressure.

Preserve the distinction between solver NO, independently replayed evidence,
formal proof, and UNKNOWN. Use the included checksums and actual result
records; do not infer coverage from directory names. Historical normalized
commands are provenance records, not ready-to-run shell instructions.

Keep repository records in professional English and preserve unrelated files.
No search, deployment, or publication is authorized merely by this file.
