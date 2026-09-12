#!/usr/bin/env python3
"""Trusted synthetic protocol assertions, separate from scripted agent context."""
import sys
import csv
import json
from pathlib import Path

if sys.argv[1] == "artifact-version":
    assert (Path(sys.argv[2]) / "outputs/version.txt").read_bytes() in {b"version 1\n", b"version 2\n"}
elif sys.argv[1] == "observation":
    root = Path(sys.argv[2])
    with (root / sys.argv[3]).open(newline="") as source:
        rows = {r["row_id"]: r for r in csv.DictReader(source)}
    row = rows["O04"]
    assert row["site"] == "Hill" and row["week"] == "2026-W36"
    value, remainder = divmod(100 * int(row["completed"]), int(row["scheduled"]))
    assert remainder == 0
    assert json.loads((root / sys.argv[4]).read_text()) == {"row": "O04", "site": "Hill", "week": "2026-W36", "value": value}
else:
    raise ValueError("Unknown protocol check")
