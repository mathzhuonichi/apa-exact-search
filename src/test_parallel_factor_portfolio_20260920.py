#!/usr/bin/env python3
"""Exercise result validity, cancellation, limits, and atomic reservations."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import signal
import subprocess
import sys
import time

from run_parallel_factor_portfolio_20260920 import (ResourcePool, check_trace_structure,
    group_memory, reap_groups, run_parallel_case, run_preparation)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--solver", type=Path, required=True)
    p.add_argument("--old-solver", type=Path, required=True)
    p.add_argument("--hard-root", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    summaries = []

    def check(r, expected):
        assert r["status"] == expected, (r["status"], expected, r.get("error"))
        assert r["children_reaped"]
        assert not sum(group_memory([x["process_group"] for x in r["lanes"]]).values())
        summaries.append({k: r.get(k) for k in ("maximum", "status", "winner", "seconds",
                                               "children_reaped", "stop_reason")})

    for n in (2, 7, 31, 113):
        state = a.output / f"root-{n}.txt"
        state.write_text("".join(f"F {x}\n" for x in sorted({1, 2, n})))
        r = run_parallel_case(a.solver, n, state, a.output / f"case-{n}", 5)
        check(r, "YES" if n == 113 else "NO")

    r = run_parallel_case(a.solver, 146621, a.hard_root, a.output / "deadline", .1)
    check(r, "UNKNOWN")
    assert r["seconds"] < 2
    r = run_parallel_case(a.solver, 146621, a.hard_root, a.output / "memory", 5, memory_mb=1)
    check(r, "RESOURCE_LIMIT")
    r = run_parallel_case(a.old_solver, 146621, a.hard_root, a.output / "wrong-engine", 5)
    check(r, "OPERATIONAL_ERROR")

    pool = ResourcePool(slots=3, memory_mb=2048)
    with ThreadPoolExecutor(max_workers=2) as executor:
        futures = [executor.submit(run_parallel_case, a.solver, 146621, a.hard_root,
                                   a.output / f"queued-{i}", .2, pool) for i in range(2)]
        rows = [f.result(timeout=10) for f in futures]
    for r in rows:
        check(r, "UNKNOWN")
    assert pool.peak_reserved_slots == 3 and pool.free_slots == 3
    assert pool.free_memory_mb == 2048 and max(r["queue_seconds"] for r in rows) >= .15

    # Cancel the real top-level controller; nested wrapper engines must die too.
    target = a.output / "sigterm"
    controller = Path(__file__).resolve().with_name("run_parallel_factor_portfolio_20260920.py")
    with (a.output / "sigterm.log").open("w") as log:
        process = subprocess.Popen([sys.executable, str(controller), "--solver", str(a.solver),
                                    "--maximum", "146621", "--state", str(a.hard_root),
                                    "--output", str(target), "--seconds", "30"], stdout=log, stderr=log)
        time.sleep(.5)
        process.send_signal(signal.SIGTERM)
        process.wait(timeout=5)
    check(json.loads((target / "result.json").read_text()), "CANCELLED")

    pool = ResourcePool(slots=3, memory_mb=2048)
    with pool.reserve(3, 2048):
        with ThreadPoolExecutor(max_workers=1) as executor:
            future = executor.submit(run_parallel_case, a.solver, 146621, a.hard_root,
                                     a.output / "queued-cancel", 5, pool)
            time.sleep(.1)
            pool.cancel()
            check(future.result(timeout=2), "CANCELLED")
    assert pool.free_slots == 3 and pool.free_memory_mb == 2048

    truncated = a.output / "truncated.proof"
    truncated.write_text("APA_PORTFOLIO_V11\nB 3 5 8 2 1 8 2 4\nC\n")
    try:
        check_trace_structure(truncated, 113)
        raise AssertionError("Accepted a truncated trace")
    except ValueError:
        pass

    # One failed wait must not prevent waiting for or closing other lanes.
    lanes = []
    for i in range(2):
        log = (a.output / f"wait-failure-{i}.log").open("w")
        process = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(20)"],
                                   stdout=log, stderr=log, start_new_session=True)
        lanes.append({"name": str(i), "process": process, "log": log})
    original_wait = lanes[0]["process"].wait
    def fail_wait(*args, **kwargs):
        raise RuntimeError("injected wait failure")
    lanes[0]["process"].wait = fail_wait
    errors = reap_groups(lanes)
    lanes[0]["process"].wait = original_wait
    original_wait(timeout=2)
    assert errors and all(lane["log"].closed for lane in lanes)
    assert not sum(group_memory([lane["process"].pid for lane in lanes]).values())

    pool = ResourcePool()
    pid_path = a.output / "preparation-pgid.txt"
    code = f"import os,time; open({str(pid_path)!r},'w').write(str(os.getpgrp())); time.sleep(20)"
    with ThreadPoolExecutor(max_workers=1) as executor:
        future = executor.submit(run_preparation, [sys.executable, "-c", code], pool)
        until = time.monotonic() + 2
        while not pid_path.exists() and time.monotonic() < until:
            time.sleep(.01)
        pool.cancel()
        try:
            future.result(timeout=3)
            raise AssertionError("Preparation ignored cancellation")
        except RuntimeError:
            pass
    assert not sum(group_memory([int(pid_path.read_text())]).values())
    (a.output / "summary.json").write_text(json.dumps(summaries, indent=2) + "\n")
    print(f"Passed {len(summaries)} lifecycle/outcome cases plus trace, wait-failure, and root-cancellation checks.")


if __name__ == "__main__":
    main()
