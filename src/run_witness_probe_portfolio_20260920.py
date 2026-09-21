#!/usr/bin/env python3
"""Bounded failed-witness lookahead followed by an exact search portfolio.

Only complete NO probes delete witnesses. Unknown probes remain possible.
All refinements are consequences of the original root, never assumptions
silently promoted to unconditional facts. This is not an independent verifier.
"""
import argparse
import json
from math import isqrt
from pathlib import Path
import shutil
import subprocess
import time

from run_factor_portfolio_20260920 import check_example, digest

ALGORITHM = "factor_witness_probe_portfolio_v2"


def run_portfolio(solver, maximum, state, directory, seconds=30):
    directory.mkdir(parents=True, exist_ok=False)
    root = directory / "root.state.txt"
    shutil.copy2(state, root)
    forced, banned = set(), set()
    for line in root.read_text().splitlines():
        tag, value = line.split()
        value = int(value)
        if tag not in ("F", "B") or not 1 <= value <= maximum:
            raise ValueError("Invalid root record")
        (forced if tag == "F" else banned).add(value)
    if not {1, 2, maximum} <= forced or forced & banned:
        raise ValueError("Invalid mandatory root")
    started = time.monotonic()
    deadline = started + seconds
    probe_deadline = min(deadline, started + 2)
    record = {"algorithm": ALGORITHM, "maximum": maximum, "status": "UNKNOWN",
              "seconds_cap": seconds, "solver_sha256": digest(solver),
              "root_sha256": digest(root), "probes": [], "deductions": [],
              "stages": [], "independent_replay": False}
    rejected, tested = set(), set()
    probe_node_cap = 1

    def write_state(path, members):
        path.write_text("".join(f"F {v}\n" for v in sorted(members)) +
                        "".join(f"B {v}\n" for v in sorted(banned)))

    def invoke(input_state, stem, cap, node_cap, order, window):
        stem.mkdir()
        result, proof = stem / "result.json", stem / "proof.txt"
        cap = round(cap, 3)
        command = [str(solver.resolve()), str(maximum), str(input_state.resolve()), str(cap),
                   str(node_cap), str(proof.resolve()), str(result.resolve()), str(window), order]
        with (stem / "solver.log").open("w") as log:
            subprocess.run(command, check=True, stdout=log, stderr=subprocess.STDOUT,
                           timeout=cap + 2)
        data = json.loads(result.read_text())
        if (data.get("algorithm") != "factor_branch_portfolio_v11" or
            data.get("maximum") != maximum or data.get("demand_order") != order or
            data.get("branch_lookahead") != window or data.get("node_cap") != node_cap or
            abs(data.get("seconds_cap", -1) - cap) > .0001):
            raise ValueError("Solver configuration mismatch")
        if data["status"] == "YES":
            check_example(proof, maximum)
        elif data["status"] == "NO":
            if not proof.read_text().startswith("APA_PORTFOLIO_V11\n"):
                raise ValueError("Missing NO search trace")
        elif data["status"] != "UNKNOWN":
            raise ValueError("Invalid solver status")
        return {"command": command, "input_sha256": digest(input_state), "result": data,
                "proof_path": str(proof.resolve()) if proof.exists() else None,
                "proof_sha256": digest(proof) if proof.exists() else None}

    try:
        while (time.monotonic() < probe_deadline - .01 and
               len(record["probes"]) < 32 and len(forced) <= 256):
            for d, e in sorted(rejected):
                if d in forced and e in forced:
                    record.update(status="NO", terminal={"type": "rejected_pair_forced", "pair": [d, e]})
                    break
                # Keep these as rejected witness pairs. Eagerly turning them
                # into member bans changes propagation order substantially;
                # that stronger but slower variant is not used here.
            if record["status"] == "NO":
                break
            domains = []
            for a in sorted(forced):
                total = maximum + a
                options = [(d, total // d) for d in range(1, isqrt(total) + 1)
                           if total % d == 0 and total // d <= maximum and
                           d not in banned and total // d not in banned and
                           (d, total // d) not in rejected]
                if any(d in forced and e in forced for d, e in options):
                    continue
                if len(options) <= 3:
                    domains.append((total, options))
            refined = False
            selected = None
            for total, options in sorted(domains, key=lambda item: (len(item[1]), item[0])):
                if not options:
                    record.update(status="NO", terminal={"type": "empty_domain", "sum": total})
                    break
                if len(options) == 1:
                    option = options[0]
                    forced.update(option)
                    record["deductions"].append({"type": "force_unique", "sum": total, "pair": option})
                    # Check the newly entailed root before deriving more units.
                    # Otherwise a short propagation contradiction can remain
                    # hidden behind many further syntactic factor deductions.
                    cap = min(.25 if probe_node_cap == 1 else .5, probe_deadline - time.monotonic())
                    if option not in tested and cap >= .01:
                        index = len(record["probes"])
                        probe_state = directory / f"probe-{index}.state"
                        write_state(probe_state, forced)
                        result = invoke(probe_state, directory / f"probe-{index}", cap, probe_node_cap, "insertion", 16)
                        record["probes"].append({"sum": total, "option": option,
                                                 "entailed": True, **result})
                        tested.add(option)
                        if result["result"]["status"] in ("NO", "YES"):
                            record.update(status=result["result"]["status"],
                                          terminal={"type": "entailed_root", "probe": index})
                    refined = True
                    break
                if selected is None:
                    selected = next(((total, q) for q in options if q not in tested), None)
            if record["status"] in ("NO", "YES"):
                break
            if refined:
                continue
            if selected is None:
                if probe_node_cap == 1:
                    # Pure propagation has exhausted the short domains. Retry
                    # surviving witnesses with a small exact search tree.
                    probe_node_cap = 31
                    tested.clear()
                    continue
                break
            cap = min(.25 if probe_node_cap == 1 else .5, probe_deadline - time.monotonic())
            if cap < .01:
                break
            total, option = selected
            index = len(record["probes"])
            probe_state = directory / f"probe-{index}.state"
            write_state(probe_state, forced | set(option))
            result = invoke(probe_state, directory / f"probe-{index}", cap, probe_node_cap, "insertion", 16)
            record["probes"].append({"sum": total, "option": option, **result})
            tested.add(option)
            if result["result"]["status"] == "NO":
                rejected.add(option)
            elif result["result"]["status"] == "YES":
                record.update(status="YES", terminal={"type": "probe_example", "probe": index})
                break
        record["probe_wall_seconds"] = time.monotonic() - started
        refined_state = directory / "refined.state.txt"
        write_state(refined_state, forced)
        record["refined_state_sha256"] = digest(refined_state)
        if record["status"] == "UNKNOWN":
            # The last stage gets the actual remaining budget, including savings
            # from inexpensive probing. Refuted pairs not representable as units
            # need not be passed to the solver: forgetting them only weakens search.
            for order, window, nominal in [("maximum_first", 4, 2), ("insertion", 8, 3),
                                           ("insertion", 16, 3), ("insertion", 64, seconds)]:
                cap = min(nominal, deadline - time.monotonic())
                if cap < .01:
                    break
                stage = invoke(refined_state, directory / f"search-{order}-{window}",
                               cap, 1000000000, order, window)
                record["stages"].append(stage)
                record["status"] = stage["result"]["status"]
                if record["status"] != "UNKNOWN":
                    break
    except Exception as error:
        record.update(status="OPERATIONAL_ERROR", error=str(error))
    record["seconds"] = time.monotonic() - started
    (directory / "result.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--solver", type=Path, required=True)
    p.add_argument("--maximum", type=int, required=True)
    p.add_argument("--state", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--seconds", type=float, default=30)
    a = p.parse_args()
    if not 2 <= a.maximum <= 1000000 or not a.seconds > 0:
        p.error("invalid maximum or budget")
    r = run_portfolio(a.solver, a.maximum, a.state, a.output, a.seconds)
    print(json.dumps({k: r[k] for k in ("maximum", "status", "seconds")}))


if __name__ == "__main__":
    main()
