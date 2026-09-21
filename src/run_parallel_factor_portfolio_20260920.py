#!/usr/bin/env python3
"""Bounded per-candidate racing with owned process groups and shared resources.

Python threads orchestrate independent single-core engines. Mutable solver
states are never shared. UNKNOWN/error/cancellation cannot win a race.
"""
import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import threading
import time

from run_factor_portfolio_20260920 import check_example, digest

ALGORITHM = "factor_parallel_portfolio_v1"


class ResourcePool:
    """Atomically reserve a whole case; never acquire partial slot bundles."""
    def __init__(self, slots=6, memory_mb=4096):
        self.slots = self.free_slots = slots
        self.memory_mb = self.free_memory_mb = memory_mb
        self.condition = threading.Condition()
        self.cancelled = threading.Event()
        self.peak_reserved_slots = 0
        self.peak_reserved_memory_mb = 0

    def cancel(self):
        self.cancelled.set()
        with self.condition:
            self.condition.notify_all()

    @contextmanager
    def reserve(self, slots, memory_mb):
        if slots > self.slots or memory_mb > self.memory_mb:
            raise ValueError("Case exceeds resource pool capacity")
        with self.condition:
            self.condition.wait_for(lambda: self.cancelled.is_set() or
                                    (self.free_slots >= slots and self.free_memory_mb >= memory_mb))
            acquired = not self.cancelled.is_set()
            if acquired:
                self.free_slots -= slots
                self.free_memory_mb -= memory_mb
                self.peak_reserved_slots = max(self.peak_reserved_slots, self.slots - self.free_slots)
                self.peak_reserved_memory_mb = max(self.peak_reserved_memory_mb,
                                                   self.memory_mb - self.free_memory_mb)
        if not acquired:
            yield False
            return
        try:
            yield True
        finally:
            with self.condition:
                self.free_slots += slots
                self.free_memory_mb += memory_mb
                self.condition.notify_all()


def group_memory(pgroups):
    """RSS is a conservative sum, including each wrapper's descendants."""
    ps = subprocess.run(["ps", "-axo", "pgid=,rss=,stat="], check=True,
                        capture_output=True, text=True, timeout=2)
    rss = {pgid: 0 for pgid in pgroups}
    for line in ps.stdout.splitlines():
        group, memory, state = line.split(maxsplit=2)
        group = int(group)
        if group in rss and not state.startswith("Z"):
            rss[group] += int(memory)
    return rss


def kill_group(process, sig):
    try:
        os.killpg(process.pid, sig)
    except ProcessLookupError:
        pass


def run_preparation(command, pool, timeout=15):
    """Root preparation has the same owned-group cancellation discipline."""
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               text=True, start_new_session=True)
    deadline = time.monotonic() + timeout
    try:
        while True:
            if pool.cancelled.is_set():
                raise RuntimeError("Root preparation cancelled")
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(command, timeout)
            try:
                stdout, stderr = process.communicate(timeout=min(.05, remaining))
                if process.returncode:
                    raise subprocess.CalledProcessError(process.returncode, command, stdout, stderr)
                return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
            except subprocess.TimeoutExpired:
                continue
    finally:
        kill_group(process, signal.SIGKILL)
        process.wait(timeout=2)


def reap_groups(lanes):
    """Never return from a race with a live owned computation left behind."""
    errors = []
    for lane in lanes:
        try:
            kill_group(lane["process"], signal.SIGTERM)
        except OSError as error:
            errors.append(f"{lane['name']} SIGTERM: {error}")
    deadline = time.monotonic() + .2
    while time.monotonic() < deadline:
        try:
            done = all(lane["process"].poll() is not None for lane in lanes)
        except Exception as error:
            errors.append(f"poll during cleanup: {error}")
            break
        if done:
            break
        time.sleep(.01)
    for lane in lanes:
        # Also reaches descendants when the wrapper leader has already exited.
        try:
            kill_group(lane["process"], signal.SIGKILL)
        except OSError as error:
            errors.append(f"{lane['name']} SIGKILL: {error}")
    for lane in lanes:
        try:
            lane["process"].wait(timeout=2)
        except Exception as error:
            errors.append(f"{lane['name']} wait: {error}")
        finally:
            try:
                lane["log"].close()
            except Exception as error:
                errors.append(f"{lane['name']} close: {error}")
    return errors


def check_trace_structure(path, maximum):
    """Check complete prefix-tree framing and factor arithmetic, not proof validity."""
    with path.open() as stream:
        if stream.readline().strip() != "APA_PORTFOLIO_V11":
            raise ValueError("Invalid trace header")
        pending = 1
        for line in stream:
            if pending <= 0:
                raise ValueError("Extra trace nodes")
            fields = line.split()
            pending -= 1
            if fields == ["C"]:
                continue
            if not fields or fields[0] != "B" or len(fields) < 5:
                raise ValueError("Invalid trace node")
            a, b, total, count = map(int, fields[1:5])
            pairs = list(map(int, fields[5:]))
            if not (1 <= a <= maximum and 1 <= b <= maximum and a + b == total
                    and count >= 2 and len(pairs) == 2 * count):
                raise ValueError("Invalid branch framing")
            for d, e in zip(pairs[::2], pairs[1::2]):
                if not (1 <= d <= e <= maximum and d * e == total):
                    raise ValueError("Invalid witness arithmetic")
            pending += count
        if pending:
            raise ValueError("Truncated trace")


def validate_result(lane, maximum):
    data = json.loads(lane["result_path"].read_text())
    if data.get("maximum") != maximum or data.get("algorithm") != lane["algorithm"]:
        raise ValueError("Algorithm or maximum mismatch")
    if lane["name"] == "probe":
        if data.get("seconds_cap") != lane["cap"]:
            raise ValueError("Wrapper budget mismatch")
        if data["status"] == "NO":
            if not data.get("terminal") and not (data.get("stages") and
                    data["stages"][-1]["result"]["status"] == "NO"):
                raise ValueError("Wrapper has no global NO conclusion")
            for step in data["probes"] + data["stages"]:
                if step["result"]["status"] == "NO":
                    if digest(Path(step["proof_path"])) != step["proof_sha256"]:
                        raise ValueError("Wrapper trace hash mismatch")
                    check_trace_structure(Path(step["proof_path"]), maximum)
        elif data["status"] == "YES":
            terminal = data.get("terminal", {})
            step = (data["probes"][terminal["probe"]] if "probe" in terminal else data["stages"][-1])
            check_example(Path(step["proof_path"]), maximum)
    else:
        if (data.get("demand_order") != "insertion" or
            data.get("branch_lookahead") != lane["window"] or
            abs(data.get("seconds_cap", -1) - lane["cap"]) > .0001 or
            data.get("node_cap") != 1000000000):
            raise ValueError("Engine configuration mismatch")
        if data["status"] == "NO":
            check_trace_structure(lane["proof_path"], maximum)
        elif data["status"] == "YES":
            check_example(lane["proof_path"], maximum)
    if data["status"] not in ("NO", "YES", "UNKNOWN", "OPERATIONAL_ERROR"):
        raise ValueError("Invalid result status")
    return data


def run_parallel_case(solver, maximum, state, directory, seconds=30, pool=None,
                      memory_mb=2048, wrapper=None, expected_solver_sha256=None,
                      expected_wrapper_sha256=None):
    pool = pool or ResourcePool()
    wrapper = wrapper or Path(__file__).resolve().with_name("run_witness_probe_portfolio_20260920.py")
    directory.mkdir(parents=True, exist_ok=False)
    root = directory / "root.state.txt"
    shutil.copy2(state, root)
    record = {"algorithm": ALGORITHM, "maximum": maximum, "status": "UNKNOWN",
              "seconds_cap": seconds, "root_sha256": digest(root),
              "solver_sha256": digest(solver), "wrapper_sha256": digest(wrapper),
              "slots_reserved": 3, "memory_budget_mb": memory_mb, "lanes": [], "stages": [],
              "winner_policy": "first_valid_observed_completion", "poll_seconds": .01,
              "independent_replay": False}
    if ((expected_solver_sha256 and record["solver_sha256"] != expected_solver_sha256) or
        (expected_wrapper_sha256 and record["wrapper_sha256"] != expected_wrapper_sha256)):
        record.update(status="OPERATIONAL_ERROR", error="Frozen executable/source hash mismatch",
                      seconds=0, children_reaped=True)
        (directory / "result.json").write_text(json.dumps(record, indent=2) + "\n")
        return record
    queued = time.monotonic()
    with pool.reserve(3, memory_mb) as acquired:
        if not acquired:
            record.update(status="CANCELLED", stop_reason="cancelled_before_dispatch", seconds=0,
                          queue_seconds=time.monotonic() - queued, children_reaped=True, owned_groups_empty=True)
            (directory / "result.json").write_text(json.dumps(record, indent=2) + "\n")
            return record
        started = time.monotonic()
        record["queue_seconds"] = started - queued
        deadline = started + seconds
        lanes = []
        winner = None
        peak_rss = 0
        last_rss = 0
        try:
            for name, window in [("probe", None), ("insertion8", 8), ("insertion64", 64)]:
                if pool.cancelled.is_set() or deadline - time.monotonic() < .001:
                    break
                cap = round(deadline - time.monotonic(), 3)
                output = directory / name
                log = (directory / f"{name}.log").open("w")
                if name == "probe":
                    command = [sys.executable, str(wrapper.resolve()), "--solver", str(solver.resolve()),
                               "--maximum", str(maximum), "--state", str(root.resolve()),
                               "--output", str(output.resolve()), "--seconds", str(cap)]
                    algorithm = "factor_witness_probe_portfolio_v2"
                else:
                    output.mkdir()
                    command = [str(solver.resolve()), str(maximum), str(root.resolve()), str(cap),
                               "1000000000", str((output / "proof.txt").resolve()),
                               str((output / "result.json").resolve()), str(window), "insertion"]
                    algorithm = "factor_branch_portfolio_v11"
                try:
                    process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT,
                                               start_new_session=True)
                except BaseException:
                    log.close()
                    raise
                lanes.append({"name": name, "window": window, "cap": cap, "algorithm": algorithm,
                              "process": process, "log": log, "command": command,
                              "result_path": output / "result.json", "proof_path": output / "proof.txt",
                              "started": time.monotonic(), "status": "RUNNING"})
            while True:
                if pool.cancelled.is_set():
                    record.update(status="CANCELLED", stop_reason="controller_cancelled")
                    break
                completed = []
                for lane in lanes:
                    if lane["status"] != "RUNNING" or lane["process"].poll() is None:
                        continue
                    lane["finished"] = time.monotonic()
                    try:
                        if lane["process"].returncode != 0:
                            raise RuntimeError(f"Engine exited {lane['process'].returncode}")
                        data = validate_result(lane, maximum)
                        lane["status"], lane["result"] = data["status"], data
                        if data["status"] in ("NO", "YES"):
                            completed.append(lane)
                    except Exception as error:
                        lane["status"], lane["error"] = "OPERATIONAL_ERROR", str(error)
                if completed:
                    if len({lane["status"] for lane in completed}) != 1:
                        raise RuntimeError("Conflicting complete solver results")
                    winner = min(completed, key=lambda lane: lane["finished"])
                    record["status"] = winner["status"]
                    record["winner"] = winner["name"]
                    break
                now = time.monotonic()
                if now >= deadline:
                    record["stop_reason"] = "shared_search_deadline"
                    break
                if all(lane["status"] != "RUNNING" for lane in lanes):
                    record["status"] = "OPERATIONAL_ERROR" if any(
                        lane["status"] == "OPERATIONAL_ERROR" for lane in lanes) else "UNKNOWN"
                    record["stop_reason"] = "all_lanes_finished_without_conclusion"
                    break
                if now - last_rss >= .25:
                    rss = group_memory([lane["process"].pid for lane in lanes])
                    peak_rss = max(peak_rss, sum(rss.values()))
                    last_rss = now
                    if sum(rss.values()) > memory_mb * 1024:
                        record.update(status="RESOURCE_LIMIT", stop_reason="observed_case_rss_limit")
                        break
                time.sleep(.01)
        except Exception as error:
            record.update(status="OPERATIONAL_ERROR", error=str(error))
        finally:
            stopped = time.monotonic()
            cleanup_errors = reap_groups(lanes)
            try:
                record["owned_groups_empty"] = not sum(group_memory(
                    [lane["process"].pid for lane in lanes]).values())
                if not record["owned_groups_empty"]:
                    raise RuntimeError("Owned process group remains live after cancellation")
            except Exception as error:
                cleanup_errors.append(str(error))
            if cleanup_errors:
                record.update(status="OPERATIONAL_ERROR", cleanup_errors=cleanup_errors)
            for lane in lanes:
                if lane["status"] == "RUNNING":
                    lane["status"] = "CANCELLED_AFTER_WINNER" if winner else "CANCELLED_AT_LIMIT"
                entry = {k: lane[k] for k in ("name", "command", "status", "cap", "window")}
                entry["process_group"] = lane["process"].pid
                entry["wall_seconds"] = lane.get("finished", stopped) - lane["started"]
                if "result" in lane:
                    entry["result"] = lane["result"]
                    record["stages"].append({"result": lane["result"], "lane": lane["name"]})
                if "error" in lane:
                    entry["error"] = lane["error"]
                record["lanes"].append(entry)
            record["search_wall_seconds"] = stopped - started
            record["seconds"] = time.monotonic() - started
            record["sum_lane_wall_seconds"] = sum(lane["wall_seconds"] for lane in record["lanes"])
            record["peak_observed_rss_kb"] = peak_rss
            record["children_reaped"] = all(lane["process"].returncode is not None for lane in lanes)
            if record["status"] == "UNKNOWN" and any(
                    lane["status"] == "OPERATIONAL_ERROR" for lane in lanes):
                record["status"] = "OPERATIONAL_ERROR"
        (directory / "result.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--solver", type=Path, required=True)
    p.add_argument("--maximum", type=int, required=True)
    p.add_argument("--state", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--seconds", type=float, default=30)
    p.add_argument("--memory-mb", type=int, default=2048)
    a = p.parse_args()
    if not 2 <= a.maximum <= 1000000 or not a.seconds > 0 or not 1 <= a.memory_mb <= 4096:
        p.error("invalid maximum, time budget, or memory reservation")
    pool = ResourcePool()
    signal.signal(signal.SIGTERM, lambda *_: pool.cancel())
    signal.signal(signal.SIGINT, lambda *_: pool.cancel())
    result = run_parallel_case(a.solver, a.maximum, a.state, a.output, a.seconds,
                               pool=pool, memory_mb=a.memory_mb)
    print(json.dumps({k: result[k] for k in ("maximum", "status", "seconds", "lanes")}))


if __name__ == "__main__":
    main()
