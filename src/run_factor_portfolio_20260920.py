#!/usr/bin/env python3
"""Run explicitly configured exact searches; reject mismatched solver metadata.

This runner consumes existing root states. It neither generates roots nor
independently replays NO certificates. Each fresh output directory is immutable
for this run. A 5-second k4 stage is followed by a 25-second k64 stage only when
the first result is UNKNOWN. Processing stops at the first unresolved maximum.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_example(path, maximum):
    fields = path.read_text().split()
    if len(fields) < 2 or fields[0] != "Y":
        raise ValueError("Missing YES witness")
    members = list(map(int, fields[2:]))
    if len(members) != int(fields[1]) or len(set(members)) != len(members):
        raise ValueError("Invalid witness cardinality")
    if not members or min(members) < 1 or max(members) != maximum:
        raise ValueError("Invalid witness domain")
    products = {a * b for a in members for b in members}
    if any(a + b not in products for a in members for b in members):
        raise ValueError("Witness does not satisfy A+A subset AA")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--solver", type=Path, required=True)
    parser.add_argument("--algorithm", choices=["factor_branch_runtime_v9", "factor_branch_prime_csp_v10"],
                        default="factor_branch_runtime_v9")
    parser.add_argument("--roots", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--maxima", type=int, nargs="+", required=True)
    args = parser.parse_args()
    if args.maxima != sorted(set(args.maxima)):
        parser.error("maxima must be strictly increasing")
    solver = args.solver.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    ledger = {"solver": str(solver), "solver_sha256": digest(solver),
              "algorithm": args.algorithm, "stages": [[4, 5], [64, 25]],
              "independent_replay": False, "rows": [], "status": "RUNNING"}
    ledger_path = args.output / "ledger.json"

    def save():
        temporary = ledger_path.with_suffix(".tmp")
        temporary.write_text(json.dumps(ledger, indent=2) + "\n")
        temporary.replace(ledger_path)

    save()
    try:
        for maximum in args.maxima:
            state = args.roots / str(maximum) / "root.state.txt"
            row = {"maximum": maximum, "root_path": str(state.resolve()),
                   "root_sha256": digest(state), "stages": []}
            ledger["rows"].append(row)
            for lookahead, cap in ledger["stages"]:
                directory = args.output / str(maximum) / f"k{lookahead}"
                directory.mkdir(parents=True)
                proof, result = directory / "proof.txt", directory / "result.json"
                command = [str(solver), str(maximum), str(state.resolve()), str(cap),
                           "1000000000", str(proof.resolve()), str(result.resolve()),
                           str(lookahead)]
                started = time.monotonic()
                with (directory / "solver.log").open("w") as log:
                    subprocess.run(command, check=True, stdout=log,
                                   stderr=subprocess.STDOUT, timeout=cap + 15)
                data = json.loads(result.read_text())
                if (data.get("algorithm") != ledger["algorithm"] or
                    data.get("branch_lookahead") != lookahead or
                    data.get("seconds_cap") != cap or
                    data.get("node_cap") != 1000000000 or
                    data.get("maximum") != maximum):
                    raise ValueError("Solver configuration mismatch; result rejected")
                if data["status"] not in ("NO", "YES", "UNKNOWN"):
                    raise ValueError("Invalid solver status")
                if data["status"] == "NO" and (not proof.exists() or not proof.stat().st_size):
                    raise ValueError("NO result has no proof artifact")
                if data["status"] == "YES":
                    check_example(proof, maximum)
                row["stages"].append({"command": command, "result": data,
                                      "wall_seconds": time.monotonic() - started,
                                      "proof_sha256": digest(proof) if proof.exists() else None})
                row["status"] = data["status"]
                save()
                if data["status"] != "UNKNOWN":
                    break
            print(json.dumps(row), flush=True)
            if row["status"] == "UNKNOWN":
                ledger["status"] = "STOPPED_UNKNOWN"
                save()
                return
        ledger["status"] = "COMPLETED"
        save()
    except Exception as error:
        ledger["status"] = "OPERATIONAL_ERROR"
        ledger["error"] = str(error)
        save()
        raise


if __name__ == "__main__":
    main()
