#!/usr/bin/env python3
"""Run or resume an explicit three-strategy exact-search portfolio."""
import argparse
from concurrent.futures import FIRST_COMPLETED, ThreadPoolExecutor, wait
import json
from pathlib import Path
import shutil
import signal
import subprocess
import time

from run_factor_portfolio_20260920 import check_example, digest

ALGORITHM = "factor_branch_portfolio_v11"
STAGES = [("maximum_first", 4, 2), ("insertion", 16, 3), ("insertion", 64, 25)]


def run_case(solver, maximum, state, directory):
    """Read an existing root; cap outcomes never imply nonexistence."""
    directory.mkdir(parents=True, exist_ok=False)
    target = directory / "root.state.txt"
    shutil.copy2(state, target)
    row = {"maximum": maximum, "root_sha256": digest(target), "stages": []}
    for order, window, cap in STAGES:
        stage = directory / f"{order}-k{window}"
        stage.mkdir()
        proof, result = stage / "proof.txt", stage / "result.json"
        command = [str(solver.resolve()), str(maximum), str(target.resolve()), str(cap),
                   "1000000000", str(proof.resolve()), str(result.resolve()), str(window), order]
        with (stage / "solver.log").open("w") as log:
            subprocess.run(command, check=True, stdout=log, stderr=subprocess.STDOUT,
                           timeout=cap + 15)
        data = json.loads(result.read_text())
        expected = {"algorithm": ALGORITHM, "maximum": maximum, "demand_order": order,
                    "branch_lookahead": window, "seconds_cap": cap, "node_cap": 1000000000}
        if any(data.get(k) != value for k, value in expected.items()):
            raise ValueError("Solver configuration mismatch")
        if data["status"] == "YES":
            check_example(proof, maximum)
        elif data["status"] == "NO":
            if not proof.read_text().startswith("APA_PORTFOLIO_V11\n"):
                raise ValueError("Missing or incompatible search trace")
        elif data["status"] != "UNKNOWN":
            raise ValueError("Invalid solver status")
        row["stages"].append({"command": command, "result": data,
                              "proof_sha256": digest(proof) if proof.exists() else None})
        row["status"] = data["status"]
        (directory / "case.json").write_text(json.dumps(row, indent=2) + "\n")
        if data["status"] != "UNKNOWN":
            break
    return row


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--solver", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--roots", type=Path, action="append", default=[])
    p.add_argument("--maxima", type=int, nargs="+")
    p.add_argument("--previous-ledger", type=Path)
    p.add_argument("--source", type=Path)
    p.add_argument("--resolution", type=Path, action="append", default=[])
    p.add_argument("--root-propagator", type=Path)
    p.add_argument("--workers", type=int, default=1)
    p.add_argument("--probe", action="store_true",
                   help="Use failed-witness probing and a shared 30-second case deadline")
    p.add_argument("--parallel", action="store_true",
                   help="Race three strategies per candidate; at most two outer workers")
    a = p.parse_args()
    if not 1 <= a.workers <= 6:
        p.error("workers must be between 1 and 6")
    if a.parallel and (a.probe or a.workers > 2):
        p.error("parallel mode requires at most two workers and cannot combine with --probe")
    inherited, resolution_records = set(), []
    previous = None
    if a.previous_ledger:
        if a.maxima or not a.source or not a.root_propagator:
            p.error("continuation requires source and root-propagator, without maxima")
        previous = json.loads(a.previous_ledger.read_text())
        if previous["status"] == "RUNNING" or previous.get("active_batch"):
            raise ValueError("Previous campaign has not stopped")
        if digest(a.source) != previous["source_sha256"]:
            raise ValueError("Frozen source hash mismatch")
        values = list(map(int, a.source.read_text().split()))
        if values != sorted(set(values)):
            raise ValueError("Source must be strictly increasing")
        inherited.update(previous.get("inherited_completed", []))
        inherited.update(r["maximum"] for r in previous["rows"] if r["status"] == "NO")
        for path in a.resolution:
            data = json.loads(path.read_text())
            if data["status"] != "NO" or data["algorithm"] not in (
                    "factor_branch_prime_csp_v10", ALGORITHM,
                    "factor_witness_probe_portfolio_v1", "factor_witness_probe_portfolio_v2"):
                raise ValueError("Resolution must be a complete supported solver NO")
            if data["algorithm"].startswith("factor_witness_probe_portfolio_"):
                if digest(path.parent / "root.state.txt") != data["root_sha256"]:
                    raise ValueError("Resolution root hash mismatch")
                if not data.get("terminal") and not (data.get("stages") and
                        data["stages"][-1]["result"]["status"] == "NO"):
                    raise ValueError("Resolution lacks a global NO conclusion")
                for step in data["probes"] + data["stages"]:
                    if step["result"]["status"] == "NO":
                        artifact = Path(step["proof_path"])
                        if digest(artifact) != step["proof_sha256"]:
                            raise ValueError("Resolution trace hash mismatch")
                # This is provenance checking, not independent inference replay.
                proof = path
            else:
                proof = path.with_suffix(".proof")
                if not proof.exists():
                    proof = path.parent / "proof.txt"
                if not proof.exists() or not proof.stat().st_size:
                    raise ValueError("Resolution lacks its search trace")
            inherited.add(data["maximum"])
            resolution_records.append({"path": str(path.resolve()), "sha256": digest(path),
                                       "trace_sha256": digest(proof), "result": data})
        pending_previous = {r["maximum"] for r in previous["rows"] if r["status"] != "NO"}
        if pending_previous - inherited:
            raise ValueError("Unresolved predecessor must be explicitly resolved first")
        maxima = [n for n in values if n > previous["resume_after"] and n not in inherited]
    else:
        maxima = a.maxima or []
        if not maxima or maxima != sorted(set(maxima)):
            p.error("provide strictly increasing maxima or a predecessor ledger")
    a.output.mkdir(parents=True, exist_ok=False)
    solver = a.output / "solver"
    shutil.copy2(a.solver, solver)
    root_solver = None
    if a.root_propagator:
        root_solver = a.output / "root_propagator"
        shutil.copy2(a.root_propagator, root_solver)
    pool = None
    parallel_wrapper = None
    if a.parallel:
        from run_parallel_factor_portfolio_20260920 import ALGORITHM as case_algorithm, ResourcePool, run_parallel_case, run_preparation
        pool = ResourcePool(slots=6, memory_mb=4096)
        signal.signal(signal.SIGTERM, lambda *_: pool.cancel())
        signal.signal(signal.SIGINT, lambda *_: pool.cancel())
        code = a.output / "code"
        code.mkdir()
        for filename in ("run_parallel_factor_portfolio_20260920.py",
                         "run_witness_probe_portfolio_20260920.py", "run_factor_portfolio_20260920.py"):
            shutil.copy2(Path(__file__).resolve().with_name(filename), code / filename)
        parallel_wrapper = code / "run_witness_probe_portfolio_20260920.py"
    elif a.probe:
        from run_witness_probe_portfolio_20260920 import ALGORITHM as case_algorithm, run_portfolio
        wrapper_source = Path(__file__).resolve().with_name("run_witness_probe_portfolio_20260920.py")
        shutil.copy2(wrapper_source, a.output / "wrapper_snapshot.py")
    else:
        case_algorithm = ALGORITHM
    ledger = {"algorithm": case_algorithm,
              "solver_sha256": digest(solver), "stages": STAGES if not (a.probe or a.parallel) else None,
              "workers": a.workers, "rows": [], "status": "RUNNING", "independent_replay": False,
              "resolution_records": resolution_records, "inherited_completed": sorted(inherited)}
    if a.parallel:
        ledger["parallel_profile"] = {"case_seconds": 30, "global_slots": 6,
                                       "case_slots": 3, "global_memory_mb": 4096,
                                       "case_memory_mb": 2048, "memory_enforcement": "polled RSS",
                                       "lanes": ["probe", "insertion8", "insertion64"]}
        ledger["scheduling"] = "work_conserving_stop_dispatch_on_first_non_NO"
        ledger["source_hashes"] = {p.name: digest(p) for p in (a.output / "code").iterdir()}
    if a.probe:
        ledger["probe_profile"] = {"case_seconds": 30, "probe_wall_seconds": 2,
                                   "max_probes": 32, "probe_node_caps": [1, 31],
                                   "individual_probe_seconds": [.25, .5],
                                   "fallback": [["maximum_first", 4, 2], ["insertion", 8, 3],
                                                ["insertion", 16, 3], ["insertion", 64, "remaining"]]}
        ledger["wrapper_source_sha256"] = digest(a.output / "wrapper_snapshot.py")
    if previous:
        shutil.copy2(a.source, a.output / "candidates.txt")
        ledger.update(source_sha256=digest(a.source), resume_after=previous["resume_after"],
                      previous_ledger=str(a.previous_ledger.resolve()),
                      previous_ledger_sha256=digest(a.previous_ledger),
                      root_propagator_sha256=digest(root_solver))

    def save():
        temporary = a.output / "ledger.tmp"
        temporary.write_text(json.dumps(ledger, indent=2) + "\n")
        temporary.replace(a.output / "ledger.json")

    def process(n):
        started = time.monotonic()
        try:
            state = next((base / str(n) / "root.state.txt" for base in a.roots
                          if (base / str(n) / "root.state.txt").exists()), None)
            if state is None:
                if root_solver is None:
                    raise ValueError("No existing root and no root propagator")
                prep = a.output / "roots" / str(n)
                prep.mkdir(parents=True)
                state = prep / "root.state.txt"
                command = [str(root_solver.resolve()), str(n), str((prep / "root.trace.txt").resolve()),
                           str(state.resolve())]
                if a.parallel:
                    result = run_preparation(command, pool)
                else:
                    result = subprocess.run(command, check=True, capture_output=True, text=True, timeout=15)
                (prep / "root.log").write_text(result.stdout + result.stderr)
                info = json.loads(result.stdout)
                if info["status"] not in ("FIXPOINT", "CONTRADICTION"):
                    raise ValueError("Unexpected root status")
                # Let the exact solver also handle contradictory roots.
            if a.parallel:
                row = run_parallel_case(solver, n, state, a.output / str(n), 30,
                                        pool=pool, wrapper=parallel_wrapper,
                                        expected_solver_sha256=ledger["solver_sha256"],
                                        expected_wrapper_sha256=ledger["source_hashes"][parallel_wrapper.name])
            elif a.probe:
                row = run_portfolio(solver, n, state, a.output / str(n), 30)
            else:
                row = run_case(solver, n, state, a.output / str(n))
        except Exception as error:
            row = {"maximum": n, "status": "CANCELLED" if pool is not None and pool.cancelled.is_set()
                   else "OPERATIONAL_ERROR", "error": str(error), "stages": []}
        row["total_wall_seconds"] = time.monotonic() - started
        return row

    save()
    with ThreadPoolExecutor(max_workers=a.workers) as executor:
        if a.parallel:
            candidates = iter(maxima)
            pending = {}
            stopping = False

            def dispatch():
                while not stopping and not pool.cancelled.is_set() and len(pending) < a.workers:
                    maximum = next(candidates, None)
                    if maximum is None:
                        break
                    pending[executor.submit(process, maximum)] = maximum

            dispatch()
            while pending:
                ledger["active_batch"] = sorted(pending.values())
                save()
                finished, _ = wait(pending, return_when=FIRST_COMPLETED)
                for future in sorted(finished, key=lambda f: pending[f]):
                    pending.pop(future)
                    row = future.result()
                    ledger["rows"].append(row)
                    if row["status"] != "NO":
                        stopping = True
                    print(json.dumps({"maximum": row["maximum"], "status": row["status"],
                                      "seconds": row.get("seconds"), "winner": row.get("winner")}), flush=True)
                ledger["rows"].sort(key=lambda row: row["maximum"])
                dispatch()
            ledger["active_batch"] = []
            attention = [row for row in ledger["rows"] if row["status"] != "NO"]
            if attention:
                ledger["status"] = "STOPPED_" + attention[0]["status"]
                ledger["first_unresolved"] = attention[0]["maximum"]
            else:
                ledger["status"] = "STOPPED_CANCELLED" if pool.cancelled.is_set() else "COMPLETED"
            ledger["resource_pool"] = {"peak_slots": pool.peak_reserved_slots,
                                       "peak_reserved_memory_mb": pool.peak_reserved_memory_mb,
                                       "free_slots_at_end": pool.free_slots,
                                       "free_memory_mb_at_end": pool.free_memory_mb}
            save()
            return
        for start in range(0, len(maxima), a.workers):
            if pool is not None and pool.cancelled.is_set():
                ledger["status"] = "STOPPED_CANCELLED"
                save()
                return
            ledger["active_batch"] = maxima[start:start+a.workers]
            save()
            rows = list(executor.map(process, ledger["active_batch"]))
            ledger["rows"].extend(rows)
            ledger["active_batch"] = []
            for row in rows:
                print(json.dumps({"maximum": row["maximum"], "status": row["status"],
                                  "seconds": row.get("seconds", sum(s["result"]["seconds"] for s in row["stages"])),
                                  "probe_count": len(row.get("probes", [])),
                                  "nodes": [s["result"].get("nodes") for s in row["stages"]]}), flush=True)
            stopped = [r for r in rows if r["status"] != "NO"]
            if stopped:
                ledger["status"] = "STOPPED_" + stopped[0]["status"]
                ledger["first_unresolved"] = stopped[0]["maximum"]
                save()
                return
            save()
    ledger["status"] = "COMPLETED"
    save()


if __name__ == "__main__":
    main()
