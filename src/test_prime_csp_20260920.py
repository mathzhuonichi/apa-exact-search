#!/usr/bin/env python3
"""Bounded differential and satisfiable-extension tests for the prime CSP solver."""
import argparse
import itertools
import json
from pathlib import Path
import random
import subprocess

from run_factor_portfolio_20260920 import check_example


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--solver", type=Path, required=True)
    p.add_argument("--baseline", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--demand-order", choices=["maximum_first", "insertion"])
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    records = []

    def solve(binary, n, forced, banned, k, tag):
        stem = a.output / tag
        state, proof, result = (stem.with_suffix(s) for s in (".state", ".proof", ".json"))
        state.write_text("".join(f"F {v}\n" for v in sorted(forced)) +
                         "".join(f"B {v}\n" for v in sorted(banned)))
        command = [str(binary.resolve()), str(n), str(state), "5", "1000000",
                   str(proof), str(result), str(k)]
        if binary == a.solver and a.demand_order:
            command.append(a.demand_order)
        subprocess.run(command, check=True,
                       capture_output=True, timeout=10)
        data = json.loads(result.read_text())
        if data["status"] == "YES":
            check_example(proof, n)
            witness = set(map(int, proof.read_text().split()[2:]))
            assert forced <= witness and not banned & witness
        records.append({"case": tag, **data})
        return data["status"], proof

    for n in range(2, 15):
        possible = False
        optional = list(range(3, n))
        for flags in itertools.product((False, True), repeat=len(optional)):
            members = {1, 2, n} | {x for x, flag in zip(optional, flags) if flag}
            products = {x*y for x in members for y in members}
            if all(x+y in products for x in members for y in members):
                possible = True
                break
        for k in (4, 64):
            status, _ = solve(a.solver, n, {1, 2, n}, set(), k, f"exhaustive-{n}-{k}")
            assert status == ("YES" if possible else "NO")

    for n in [31, 63, 64, 65, 97, 111, 112, 113, 114, 127, 128, 129, 191, 257]:
        expected, _ = solve(a.baseline, n, {1, 2, n}, set(), 64, f"baseline-{n}")
        assert expected != "UNKNOWN"
        actual, _ = solve(a.solver, n, {1, 2, n}, set(), 64, f"differential-{n}")
        assert actual == expected

    status, proof = solve(a.baseline, 113, {1, 2, 113}, set(), 64, "witness-source")
    assert status == "YES"
    witness = set(map(int, proof.read_text().split()[2:]))
    rng = random.Random(20260920)
    outside = sorted(set(range(1, 114)) - witness)
    for trial in range(24):
        forced = {1, 2, 113} | {x for x in sorted(witness) if rng.random() < .25}
        banned = {x for x in outside if rng.random() < .5}
        status, _ = solve(a.solver, 113, forced, banned, 4 if trial % 2 else 64,
                          f"satisfiable-extension-{trial}")
        assert status == "YES"
    (a.output / "summary.json").write_text(json.dumps(records, indent=2) + "\n")
    print(f"Passed {len(records)} solver invocations, including 24 known satisfiable extensions.")


if __name__ == "__main__":
    main()
