# APA exact search

## Opt-in Rust event engine

The new modular engine is available as `solve-events`; the previous Rust
engine remains available as `solve` and `campaign`. See
[the implementation and CLI guide](docs/EVENT_ENGINE.md) and
[the recorded regression results](evidence/events-v1/README.md).
Build with `cargo build --release` and run controls with `cargo test`.
Residual execution is still an explicit operator action; the latest authorized
continuation is recorded under `evidence/events-v2-optimized-continuation/`.

Exact fixed-maximum search for finite sets of positive integers satisfying

\[
A+A\subseteq A\cdot A,\qquad \max A=n.
\]

**Maximum 221969 has been excluded.** The single-candidate research is complete.
The [self-contained arithmetic proof](evidence/221969/README.md) and its
independent checker start only from 1, 2, and the assumed maximum. They do not
use historical seeds, maximum deletion, or prior solver NO results. The
certificate has passed independent arithmetic replay; it is not a formal
proof-assistant theorem.

The optimized event-v2 continuation independently excluded the residuals
through **224929**. Including the first-stage exclusions between residual
entries, the accepted contiguous exclusion reaches **224929**; **224999** is
next. Execution is currently paused at the user's request to diagnose the
runtime difference from the earlier approximately-one-second path. Recent
slow cases spent their time in shared-root strengthening (not lane search):
serial propagation, prime-chain passes, proof-fragment merges, and batch tails
make the cost depend strongly on the candidate. Earlier campaign outcomes
retain their original evidence levels, and the full million-range problem
remains unresolved.

## Contents

- `src/events/`: the event-v2 solver used for the latest continuation, with
  independently replayable certificates and parallel shared-root probes.
- `src/main.rs`, `src/solver.rs`, and `src/portfolio.rs`: the original Rust
  `solve` and `campaign` path. It processes one candidate at a time and
  cancels other solver threads after a complete root NO or validated YES.
- The dated C++ and Python files under `src/` are retained as historical
  algorithm references; they are not part of the Rust runtime.
- `evidence/221969/`: the complete seed-free exclusion certificate, proof note, and standalone checker.
- `evidence/events-v2-optimized-continuation/`: independently replayed optimized event-engine results through 224929, the resource-bottleneck ledger, and sol analyses.
- `progress/`: the exact frozen residual list, a per-entry classification,
  and the authoritative execution status.
- `evidence/latest/`: roots and records for all nine latest campaign cases,
  including the seven winning NO traces and both unresolved cases.
- `evidence/ledgers/`: six campaign ledgers, with historical host paths normalized.
- `evidence/validation/`: saved prior validation and benchmark summaries.
- `docs/`: algorithm, status, validation limits, and a future handoff.
- `provenance/`: original source hashes and exported-file checksums.

The residual file has 18,466 entries. The user identified it as the complete
hard residual after first-stage rules excluded every omitted maximum. It is
not a claim that all maxima up to one million have been resolved. The 648-entry
predecessor prefix is reported complete by the earlier work and has not been
re-audited for this export. See [status details](docs/STATUS.md).

## Build and static checks

The active implementation requires the stable Rust toolchain. It uses all
logical processors by default, keeps mutable solver state private to each
thread, and performs no executable/source hashing. No automatic workflow,
scheduled job, or search process is started by this repo.

```sh
make check
make rust-build
```

`make check` formats, type-checks, and tests the Rust implementation without
starting a search. The historical snapshot checker is not part of the active
runtime or build path.

Build the optimized executable without starting a search:

```sh
cargo build --release
```

## Historical original-solver commands

The commands below describe the original `solve` and `campaign` path. They do
not represent the paused event-v2 continuation; read [the current handoff](docs/HANDOFF.md)
before any newly authorized run.

Solve one maximum using every logical processor (zero seconds means no time
limit):

```sh
target/release/apa-exact-search solve --maximum 221969 \
  --seconds 0 --complete-prefix-base 113
```

Run a sequential candidate campaign. Each candidate uses all logical
processors, and the campaign stops at the first result other than complete NO:

```sh
target/release/apa-exact-search campaign \
  --candidates progress/candidates.txt --roots evidence/latest \
  --start-after 221923 --output outputs/run --seconds 0 \
  --complete-prefix-base 113
```

`--complete-prefix-base` is deliberately opt-in. It enables the
maximum-deletion lemma only when the caller asserts that every intervening
maximum has already been excluded. The unattended supervisor passes 113 from
the recorded complete residual-sequence campaign. The solver then requires,
for `B = A \\ {n}`, both `n in B+B` and `n not in B*B`; the former is a
rollback-safe live bit domain and the latter is represented by root nogoods.

For unattended execution, launch the persistent PowerShell supervisor. It
checkpoints the last NO, restarts unexpected crashes up to three times, and
writes `attention.json` only for UNKNOWN, YES, or an operational failure:

```powershell
pwsh -File tools/start_automatic_campaign.ps1 `
  -StartAfter 210577 -Seconds 30 -Threads 24 -CompletePrefixBase 113
```

The launcher returns immediately; the supervisor and Rust process run hidden.
Inspect `outputs/automatic/state.json` for progress. The supervisor never skips
the first non-NO candidate. Once `state.json` exists, `StartAfter` cannot move
past its recorded contiguous NO boundary; resolve and record the attention case
before advancing it.

The Rust campaign processes one candidate at a time, so a candidate UNKNOWN
leaves no other candidate in flight. Within a candidate, only a complete
root-level NO can win the portfolio; conditional branch NO results remain local
pruning facts. `PAUSED.json` remains an administrative marker, not a runtime
interlock. Running a search still requires an explicit command.

See [algorithm](docs/ALGORITHM.md), [validation](docs/VALIDATION.md), and
[handoff](docs/HANDOFF.md). The original workspace and historical binary files
are not included; build from the exported source when a future run is authorized.
