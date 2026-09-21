#!/usr/bin/env python3
"""Read-only snapshot checks. No solver process or campaign is launched."""
import ast
import csv
import hashlib
import json
from collections import Counter
from pathlib import Path
root = Path(__file__).resolve().parents[1]
manifest = json.loads((root / "provenance/FILES.json").read_text())
for entry in manifest:
    data = (root / entry["path"]).read_bytes()
    assert len(data) == entry["bytes"] and hashlib.sha256(data).hexdigest() == entry["sha256"], entry["path"]
status = json.loads((root / "progress/STATUS.json").read_text())
assert json.loads((root / "PAUSED.json").read_text())["paused"] is True
assert status["run_state"] == "PAUSED_BY_USER"
raw = (root / "progress/candidates.txt").read_bytes()
values = list(map(int, raw.split()))
assert values == sorted(set(values)) and len(values) == status["source_count"]
assert hashlib.sha256(raw).hexdigest() == status["source_sha256"]
with (root / "progress/classification.csv").open(newline="") as stream:
    rows = list(csv.DictReader(stream))
assert [int(r["maximum"]) for r in rows] == values
assert dict(Counter(r["status"] for r in rows)) == status["counts"]
assert [int(r["maximum"]) for r in rows if r["status"] == "UNKNOWN"] == status["unresolved"]
for path in (root / "evidence/latest").glob("*/root.state.txt"):
    n = int(path.parent.name)
    members, banned = set(), set()
    for line in path.read_text().splitlines():
        tag, v = line.split(); v = int(v)
        assert tag in ("F", "B") and 1 <= v <= n
        (members if tag == "F" else banned).add(v)
    assert {1, 2, n} <= members and not members & banned
    data = json.loads((path.parent / "result.json").read_text())
    assert hashlib.sha256(path.read_bytes()).hexdigest() == data["root_sha256"]
for path in root.rglob("*.py"):
    ast.parse(path.read_text(), filename=str(path))
for path in (root / "evidence/latest").glob("*/*/proof.txt"):
    lines = path.read_text().splitlines()
    assert lines.pop(0) == "APA_PORTFOLIO_V11"
    pending = 1
    n = int(path.parents[1].name)
    for line in lines:
        assert pending > 0
        pending -= 1
        f = line.split()
        if f == ["C"]: continue
        assert f[0] == "B"
        a, b, total, count = map(int, f[1:5]); pairs = list(map(int, f[5:]))
        assert 1 <= a <= n and 1 <= b <= n and a+b == total
        assert count >= 2 and len(pairs) == 2*count
        assert all(1 <= d <= e <= n and d*e == total for d,e in zip(pairs[::2], pairs[1::2]))
        pending += count
    assert pending == 0
print(f"Checked {len(manifest)} exported files, {len(values)} candidates, nine roots, and seven trace frames. Research remains paused.")
