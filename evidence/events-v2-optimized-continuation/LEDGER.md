# Optimized event-v2 continuation ledger

Started: 2026-09-21T19:54:26+08:00

Authorization: explicit user request to continue with the optimized Rust
solver until a new resource bottleneck appeared.

The release binary used `solve-events` with `--threads 0`, which resolves to
all 24 logical processors on this host. The common options were:

```text
--root-strengthen --probes --root-seconds 120 --seconds 1 --node-limit 1
--prime-chain-steps 50000000 --probe-case-events 1000000 --probe-members 2000
--root-probe-width 8 --probe-events 200000000
```

Every result below has a fresh proof and a separate `verify-events` replay with
no external lemmas.

| Maximum | Threads | Status | Root seconds | Probe events | Parallel batches / workers | Verification |
|---:|---:|---|---:|---:|---:|---|
| 222443 | 1 | NO / VERIFIED_NO | 15.83 | 15,160,080 | prior serial record | `candidates/222443/verify.log` in the 20260921-192505 ledger |
| 222473 | 24 | NO / VERIFIED_NO | 13.59 | 77,025,743 | 7 / 24 | `20260921-195426/candidates/222473/verify.log` |
| 222559 | 24 | NO / VERIFIED_NO | 17.32 | 50,508,167 | 7 / 24 | `20260921-195545/candidates/222559/verify.log` |
| 222665 | 24 | NO / VERIFIED_NO | 4.21 | 14,371,992 | 2 / 23 | `20260921-195644/candidates/222665/verify.log` |
| 222679 | 24 | NO / VERIFIED_NO | 27.57 | 131,476,353 | 10 / 24 | `20260921-200402/candidates/222679/verify.log` |
| 222731 | 24 | NO / VERIFIED_NO | 31.39 | 159,291,454 | 18 / 24 | `20260921-200757/candidates/222731/verify.log` |
| 222733 | 24 | NO / VERIFIED_NO | 18.81 | 98,386,772 | 8 / 24 | `20260921-201148/candidates/222733/verify.log` |
| 222739 | 24 | NO / VERIFIED_NO | 20.00 | 99,679,992 | 7 / 24 | `20260921-201426/candidates/222739/verify.log` |
| 222743 | 24 | NO / VERIFIED_NO | 31.36 | 126,180,526 | 14 / 24 | `20260921-201641/candidates/222743/verify.log` |
| 222751 | 24 | NO / VERIFIED_NO | 9.07 | 51,694,918 | 5 / 24 | `20260921-201816/candidates/222751/verify.log` |
| 222803 | 24 | NO / VERIFIED_NO | 6.70 | 25,231,898 | 4 / 23 | `20260921-202035/candidates/222803/verify.log` |
| 222863 | 24 | NO / VERIFIED_NO | 8.02 | 50,286,345 | 3 / 24 | `20260921-202246/candidates/222863/verify.log` |
| 222893 | 24 | NO / VERIFIED_NO | 13.77 | 48,415,995 | 5 / 24 | `20260921-202402/candidates/222893/verify.log` |
| 222953 | 24 | NO / VERIFIED_NO | 15.97 | 50,476,773 | 5 / 24 | `20260921-202526/candidates/222953/verify.log` |
| 223073 | 24 | NO / VERIFIED_NO | 20.72 | 58,637,687 | 8 / 24 | `20260921-202636/candidates/223073/verify.log` |
| 223159 | 24 | NO / VERIFIED_NO | 13.91 | 56,843,852 | 5 / 24 | `20260921-202752/candidates/223159/verify.log` |
| 223385 | 24 | NO / VERIFIED_NO | 14.92 | 85,731,614 | 6 / 24 | `20260921-202859/candidates/223385/verify.log` |
| 223445 | 24 | NO / VERIFIED_NO | 10.18 | 26,080,574 | 3 / 24 | `20260921-203030/candidates/223445/verify.log` |
| 223723 | 24 | NO / VERIFIED_NO | 33.24 | 166,233,110 | 17 / 24 | `20260921-203134/candidates/223723/verify.log` |
| 223761 | 24 | NO / VERIFIED_NO | 9.10 | 16,481,084 | 1 / 19 | `20260921-203304/candidates/223761/verify.log` |
| 223789 | 24 | NO / VERIFIED_NO | 14.68 | 53,707,678 | 6 / 24 | `20260921-203616/candidates/223789/verify.log` |
| 223801 | 24 | NO / VERIFIED_NO | 18.12 | 104,205,638 | 9 / 24 | `20260921-203811/candidates/223801/verify.log` |
| 223859 | 24 | NO / VERIFIED_NO | 17.37 | 56,278,547 | 7 / 24 | `20260921-204106/candidates/223859/verify.log` |
| 223949 | 24 | NO / VERIFIED_NO | 18.25 | 86,188,338 | 7 / 24 | `20260921-204335/candidates/223949/verify.log` |
| 223981 | 24 | NO / VERIFIED_NO | 12.56 | 42,448,592 | 5 / 24 | `20260921-204512/candidates/223981/verify.log` |
| 224071 | 24 | NO / VERIFIED_NO | 42.21 | 199,681,293 | 38 / 24 | `20260921-204708/candidates/224071/verify.log` |
| 224075 | 24 | NO / VERIFIED_NO | 9.82 | 31,375,562 | 3 / 24 | `20260921-204921/candidates/224075/verify.log` |
| 224329 | 24 | NO / VERIFIED_NO | 26.42 | 109,396,854 | 10 / 24 | `20260921-205153/candidates/224329/verify.log` |
| 224375 | 24 | NO / VERIFIED_NO | 10.83 | 68,724,544 | 3 / 24 | `20260921-205322/candidates/224375/verify.log` |
| 224473 | 24 | NO / VERIFIED_NO | 6.99 | 33,871,912 | 3 / 24 | `20260921-205519/candidates/224473/verify.log` |
| 224531 | 24 | NO / VERIFIED_NO | 4.05 | 13,648,585 | 3 / 24 | `20260921-205629/candidates/224531/verify.log` |
| 224771 | 24 | NO / VERIFIED_NO | 10.90 | 26,282,145 | 3 / 24 | `20260921-205817/candidates/224771/verify.log` |
| 224869 | 24 | NO / VERIFIED_NO | 38.72 | 194,169,746 | 31 / 24 | `20260921-210006/candidates/224869/verify.log` |
| 224929 | 24 | NO / VERIFIED_NO | 14.45 | 87,722,404 | 8 / 24 | `20260921-210204/candidates/224929/verify.log` |

External 0.5-second samples showed a 21.05-busy-logical-processor peak and
3,164.7 MiB peak RSS for 222559. For 222665, the sample average was 3.61
busy logical processors, the peak was 10.83, and 62% of samples were below two
busy processors; peak RSS was 1,938.8 MiB. The parallel probe bursts therefore
use the machine, but serial root propagation, prime-chain work, and short
batch tails leave most processors idle between bursts. For 222679, 70% of
0.5-second samples were below two busy logical processors, despite a 19.61
processor peak and approximately 2,908 MiB sampled RSS. A sol subagent
reviewed the evidence and classified this as algorithmic scheduler/work-
granularity overhead, not evidence of a genuine near-satisfiable candidate.
For 222731, the same pattern persisted: 31.39 seconds, 159.3M probe events,
18 batches, 43% of samples below two busy processors, and a 20.76-processor
peak; its root certificate also independently replayed as `VERIFIED_NO`.
For 222733, the peak RSS rose to approximately 3,881 MiB with 53% of samples
below two busy processors. A second sol subagent classified this as
state-clone/proof-fragment/allocator-residency and batch-barrier behavior, not
a candidate-specific genuine-solution signal. For 222739, the same resource
shape persisted: approximately 3,795 MiB peak RSS, 5.73 average busy logical
processors, and 59% of samples below two; its root proof independently replayed
as `VERIFIED_NO`.
For 222751, the 4,060.5 MiB peak was approximately 25.3% of the host's
16,073 MiB physical memory; post-run free memory was approximately 7,885 MiB.
A third sol subagent judged it a temporary clone/fragment/allocator peak, not
a candidate-specific solution signal. The agreed re-review thresholds are
6 GiB RSS, less than 4 GiB system-available memory, three consecutive
512 MiB RSS increases, or a root phase over 60 seconds/150M probe events
without a complete conflict. 222803 then fell back to approximately 2,883 MiB
RSS and also independently replayed as `VERIFIED_NO`.
For 222743, the repeated shape persisted: approximately 3,562 MiB peak RSS,
21.11 busy logical processors at peak, and 59% of samples below two; its
complete root certificate also independently replayed as `VERIFIED_NO`.
The fourth sol subagent reviewed 223723 and found no new algorithmic
regression: its approximately 5.00M probe-events/s throughput matched the
nearby high-workload cases, and its 3,782 MiB RSS stayed below prior peaks.
It was candidate-specific propagation/proof workload, not a genuine-solution
signal. Re-review the next high-workload case if it exceeds 60 seconds or
150M probe events without a complete root conflict, if two consecutive
100M-event cases fall below 3.5M events/s, or if the stated RSS/system-memory
thresholds are reached.
For 223859, the root completed in 17.37 seconds with 56,278,547 probe events,
21 complete conflicts, 7 parallel batches using 24 workers, and 1,776,982
proof events (9,441 compressed). External samples reached 23 busy logical
processors and 3,623.7 MiB RSS, with a 4.21 busy-processor average across 34
samples. Independent replay returned `VERIFIED_NO`; no new sol review threshold
was reached.
For 223949, the root completed in 18.25 seconds with 86,188,338 probe events,
36 complete conflicts, 7 parallel batches using 24 workers, and 2,518,997
proof events (11,679 compressed). External samples reached 23 busy logical
processors and 4,440.4 MiB RSS, with a 6.37 busy-processor average across 35
samples. Independent replay returned `VERIFIED_NO`; this was a single RSS rise
below the 6 GiB review line and did not trigger another sol review.
For 223981, the root completed in 12.56 seconds with 42,448,592 probe events,
42 complete conflicts, 5 parallel batches using 24 workers, and 1,758,765
proof events (6,572 compressed). External samples reached 24 busy logical
processors and 3,497.9 MiB RSS, with a 4.79 busy-processor average across 24
samples. Independent replay returned `VERIFIED_NO`; resource usage returned to
the established range and no new sol review threshold was reached.
For 224071, the root completed in 42.21 seconds with 199,681,293 probe
events, 27 complete conflicts, 38 parallel batches using 24 workers, 10
root-strengthen rounds, 59,958,126 prime-chain steps, and 1,339,156 proof
events (7,978 compressed). External samples reached 23 busy logical
processors and 3,019.4 MiB RSS, with a 4.66 busy-processor average across 82
samples. Independent replay returned `VERIFIED_NO`. This was the first case in
the continuation to approach the event cap, so a read-only sol review was
started; the next candidate can run concurrently.
The fifth sol subagent reviewed 224071 with approximately 95% confidence and
classified it as candidate-specific propagation workload amplified by the
known batch-barrier/serial-propagation design. Its approximately 4.73M
probe-events/s throughput matched nearby heavy cases (223723 and 223949), and
its 3,019.4 MiB RSS was below the 6 GiB review line. The complete root
`VERIFIED_NO` and independent replay provide no credible signal of a genuine
example. Re-review if the 200M probe budget is exhausted, lane search starts,
the result becomes `UNKNOWN`, two consecutive >100M-event cases fall below
3.5M events/s, or the established time/RSS/system-memory/checker thresholds
are reached.
For 224075, the root completed in 9.82 seconds with 31,375,562 probe events,
12 complete conflicts, 3 parallel batches using 24 workers, and 2,106,990
proof events (3,546 compressed). External samples reached 19 busy logical
processors and 3,544.5 MiB RSS, with a 3.84 busy-processor average across 19
samples. Independent replay returned `VERIFIED_NO`; the workload returned to
the established range.
For 224329, the root completed in 26.42 seconds with 109,396,854 probe
events, 34 complete conflicts, 10 parallel batches using 24 workers, 6
root-strengthen rounds, and 2,042,320 proof events (13,740 compressed).
External samples reached 23 busy logical processors and 3,752.7 MiB RSS, with
a 5.53 busy-processor average across 51 samples. Independent replay returned
`VERIFIED_NO`; throughput was approximately 4.14M probe events/s, above the
3.5M review threshold.
For 224375, the root completed in 10.83 seconds with 68,724,544 probe events,
18 complete conflicts, 3 parallel batches using 24 workers, and 4,354,159
proof events (6,015 compressed). External samples reached 23 busy logical
processors and 4,963.7 MiB RSS, with an 8.67 busy-processor average across 21
samples. Independent replay returned `VERIFIED_NO`; the RSS peak was isolated,
below the 6 GiB review line, and did not yet establish a rising-memory trend.
For 224473, the root completed in 6.99 seconds with 33,871,912 probe events,
41 complete conflicts, 3 parallel batches using 24 workers, and 1,573,836
proof events (5,596 compressed). External samples reached 22 busy logical
processors and 3,805.5 MiB RSS, with a 6.23 busy-processor average across 13
samples. Independent replay returned `VERIFIED_NO`; resource usage returned to
the established range.
For 224531, the root completed in 4.05 seconds with 13,648,585 probe events,
26 complete conflicts, 3 parallel batches using 24 workers, and 533,720 proof
events (898 compressed). External samples reached 17 busy logical processors
and 3,451.0 MiB RSS, with a 5.88 busy-processor average across 8 samples.
Independent replay returned `VERIFIED_NO`; no new resource or correctness
threshold was reached.
For 224771, the root completed in 10.90 seconds with 26,282,145 probe events,
16 complete conflicts, 3 parallel batches using 24 workers, and 1,734,021
proof events (3,411 compressed). External samples reached 18 busy logical
processors and 3,318.4 MiB RSS, with a 3.43 busy-processor average across 21
samples. Independent replay returned `VERIFIED_NO`; no new resource or
correctness threshold was reached.
For 224869, the root completed in 38.72 seconds with 194,169,746 probe
events, 45 complete conflicts, 31 parallel batches using 24 workers, 8
root-strengthen rounds, 42,634,994 prime-chain steps, and 1,900,517 proof
events (10,206 compressed). External samples reached 22 busy logical
processors and 2,902.8 MiB RSS, with a 4.88 busy-processor average across 75
samples. Independent replay returned `VERIFIED_NO`. This second near-cap case
started a read-only sol review; the next candidate can run concurrently.
The sixth sol subagent reviewed 224869 with approximately 97% confidence and
classified it as candidate-specific high propagation work amplified by the
known batch-barrier and serial-propagation phases. Its approximately 5.02M
probe-events/s throughput matched or exceeded the nearby heavy cases, and its
2,902.8 MiB RSS showed no memory pressure. The complete scope-0 root conflict
and independent `VERIFIED_NO` replay provide no credible signal of a genuine
example. Re-review if the 200M budget is exhausted, lane search starts, the
result becomes `UNKNOWN`, two consecutive >100M-event cases fall below 3.5M/s,
or the established time/RSS/system-memory/checker thresholds are reached.
For 224929, the root completed in 14.45 seconds with 87,722,404 probe events,
40 complete conflicts, 8 parallel batches using 24 workers, 4 root-strengthen
rounds, and 1,249,758 proof events (4,910 compressed). External samples reached
23 busy logical processors and 3,988.9 MiB RSS, with a 7.54 busy-processor
average across 28 samples. Independent replay returned `VERIFIED_NO`; no new
high-workload or memory threshold was reached before the user-requested pause.
