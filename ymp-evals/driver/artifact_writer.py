#!/usr/bin/env python3
"""Scripted workload implementation. Reads only explicitly selected task inputs.

This is an execution backend, not an acceptance oracle. It never imports the
validator or accesses fixture reference artifacts or expected protocol traces.
"""
import csv
import json
import sys
from pathlib import Path


def execute(case, root):
    root = Path(root)
    output = root / "outputs"
    output.mkdir(exist_ok=True)
    if case == "document":
        data = json.loads((root / "inputs/document.json").read_text())
        agenda = ["# " + data["title"], "", "Date: " + data["date"],
                  "Location: " + data["location"], "", "| Time | Activity | Owner |",
                  "| --- | --- | --- |"]
        for item in data["items"]:
            agenda.append("| {} | {} | {} |".format(item["time"], item["activity"], item["owner"]))
        (output / "workshop.md").write_text("\n".join(agenda) + "\n")
    elif case == "transform":
        ledger = {}
        with (root / "inputs/ledger.csv").open(newline="") as source:
            for row in csv.DictReader(source):
                if row["status"] != "posted":
                    continue
                accumulator = ledger.setdefault(row["account"], [0, 0])
                accumulator[0] += int(row["quantity"]) * int(row["unit_cents"])
                accumulator[1] += 1
        with (output / "totals.csv").open("w", newline="") as destination:
            writer = csv.writer(destination, lineterminator="\n")
            writer.writerow(["account", "total_cents", "posted_entries"])
            for account in sorted(ledger):
                writer.writerow([account, *ledger[account]])
    elif case == "grounded":
        with (root / "inputs/observations.csv").open(newline="") as source:
            rows = list(csv.DictReader(source))
        records = {(r["site"], r["week"].rsplit("-", 1)[-1]): r for r in rows}
        harbor = records[("Harbor", "W36")]
        hill = records[("Hill", "W36")]
        earlier = records[("Hill", "W35")]
        def rate(row):
            return int(row["completed"]) * 100 // int(row["scheduled"])
        claims = []
        for name, value, unit, supporting in [
            ("harbor-w36-rate", rate(harbor), "percent", [harbor]),
            ("hill-w36-rate", rate(hill), "percent", [hill]),
            ("hill-w36-minus-w35", rate(hill) - rate(earlier), "percentage_points", [hill, earlier]),
        ]:
            claims.append({"id": name, "value": value, "unit": unit,
                           "source": "inputs/observations.csv",
                           "rows": sorted(r["row_id"] for r in supporting)})
        (output / "claims.json").write_text(json.dumps({"claims": claims}, indent=2) + "\n")
    elif case in {"observation-original", "observation-corrected"}:
        source_name = "observations.csv" if case.endswith("original") else "observations-corrected.csv"
        with (root / "inputs" / source_name).open(newline="") as source:
            row = next(r for r in csv.DictReader(source) if r["row_id"] == "O04")
        numerator = 100 * int(row["completed"])
        denominator = int(row["scheduled"])
        assert numerator % denominator == 0
        claim = {"row": row["row_id"], "site": row["site"], "week": row["week"], "value": numerator // denominator}
        (output / (case + ".json")).write_text(json.dumps(claim) + "\n")
    elif case == "qualitative":
        (output / "ideas.md").write_text(
            "# Workshop opening\n\n"
            "Invite each participant to share their name and one small discovery with a nearby partner, "
            "offering everyone a comfortable chance to speak before the wider group discussion.\n\n"
            "Place welcoming question cards on tables and let participants choose one to discuss together, "
            "then invite volunteers to share a common interest that emerged during their conversation.\n")
    else:
        raise ValueError("Unknown scripted workload: " + case)


if __name__ == "__main__":
    execute(sys.argv[1], sys.argv[2])
