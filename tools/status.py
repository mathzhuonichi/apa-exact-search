#!/usr/bin/env python3
"""Print the saved status without starting any search."""
import json
from pathlib import Path
print(json.dumps(json.loads((Path(__file__).resolve().parents[1] / "progress/STATUS.json").read_text()), indent=2))
