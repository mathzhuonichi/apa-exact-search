# Future handoff — do not resume yet

Current administrative state: PAUSED_BY_USER.

1. Read `progress/STATUS.json`. The immediate unresolved cases are 147907 and
   147923; preserve the latest seven NO results and inherited completed entries.
2. Correct and test the first-UNKNOWN global cancellation policy before any
   production continuation. No such code change was made during publication.
3. Reassess concurrency and the memory monitor after the reported machine
   sluggishness. Do not assume the historical defaults are authorized.
4. Obtain the user's explicit instruction to resume. Then build the binaries
   locally and record their new hashes, compiler, flags, and platform.
5. Use the included roots to investigate unresolved cases. Any conditional
   probe NO is only conditional; it must not be passed off as a whole-root NO.
6. Do not pass normalized historical commands straight to a shell. Those
   commands are provenance records. Create fresh output directories and
   regenerate local paths for any newly authorized run.

The worker/coordinator protocol remains: the worker reports a bottleneck and
its full evidence to the coordinating agent; the coordinator diagnoses and
validates improvements. An execution-stage goal ending is not the end of the
research request. At present, both research and worker execution remain paused.

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
