"""Public shape check only; does not decide correctness."""
import csv
from pathlib import Path

for name, header in (("totals.csv", ["account", "currency", "total_cents", "active_entries"]),
                     ("exceptions.csv", ["row_number", "entry_id", "reason"])):
    raw = Path(name).read_bytes()
    assert raw.endswith(b"\n") and b"\r" not in raw
    rows = list(csv.reader(raw.decode("utf-8").splitlines(), strict=True))
    assert rows[0] == header
    assert all(len(row) == len(header) for row in rows[1:])
print("public shape checks passed")
