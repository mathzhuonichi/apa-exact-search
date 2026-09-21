# APA exact search

## Opt-in Rust event engine

The new modular engine is available as `solve-events`; the previous Rust
engine remains available as `solve` and `campaign`. See
[the implementation and CLI guide](docs/EVENT_ENGINE.md) and
[the recorded regression results](evidence/events-v1/README.md).
Build with `cargo build --release` and run controls with `cargo test`.
This revision does not start or resume the residual campaign.

Exact fixed-maximum search for finite sets of positive integers satisfying

\[
A+A\subseteq A\cdot A,\qquad \max A=n.
\]

**Research is paused.** This repository is a standalone snapshot of the
algorithm and progress as of 2026-09-21. Uploading it does not authorize
resuming any search or worker goal.

The current unresolved maxima are **147907 and 147923**. The next never-started
entry in the included residual sequence is **147983**. The latest campaign
completed seven further cases with solver `NO`. These are software results,
not independently replayed or formally verified nonexistence certificates.
The full million-range problem remains unresolved.

## Contents

- `src/`: the v11 C++ exact solver, root propagator, v9 comparison solver,
  sequential/layered/parallel Python controllers, and existing test programs.
- `progress/`: the exact frozen residual list, a per-entry classification,
  and the authoritative paused status.
- `evidence/latest/`: roots and records for all nine latest campaign cases,
  including the seven winning NO traces and both unresolved cases.
- `evidence/ledgers/`: six campaign ledgers, with historical host paths normalized.
- `evidence/validation/`: saved prior validation and benchmark summaries.
- `docs/`: algorithm, status, validation limits, and a future handoff.
- `provenance/`: original source hashes and exported-file checksums.

The residual file has 18,466 entries. It is a selected previously processed
residual sequence, **not all unresolved integers up to one million**. The
648-entry predecessor prefix is reported complete by the earlier work and
has not been re-audited for this export. See [status details](docs/STATUS.md).

## Safe inspection

Only Python's standard library is needed for the controllers. The C++ code
requires a C++17 compiler. Process-group handling targets macOS/POSIX systems.
No automatic workflow, scheduled job, or search process is started by this repo.

```sh
python3 tools/status.py
make check
```

`make check` checks snapshot integrity and source syntax without executing any
solver. An optional build is also separate from running a search:

```sh
make build
```

**Do not resume the shipped controllers as-is.** They retain the behavior at
pause: after an UNKNOWN, new dispatch stops but in-flight candidates are
allowed to finish. The user now requires immediate cancellation of all
in-flight candidates when any candidate returns UNKNOWN. This correction,
resource-limit reassessment, and explicit user permission to resume are still
pending. `PAUSED.json` records the pause; it is an administrative marker, not a
runtime interlock inside the preserved source files.

See [algorithm](docs/ALGORITHM.md), [validation](docs/VALIDATION.md), and
[handoff](docs/HANDOFF.md). The original workspace and historical binary files
are not included; build from the exported source when a future run is authorized.
