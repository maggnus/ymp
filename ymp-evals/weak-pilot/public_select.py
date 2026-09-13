#!/usr/bin/env python3
"""Independent-attempt selection using visible evidence only, with no model.

No evaluator imports, hidden data, answers or quality oracle. Candidate arguments
are in preassigned participant order. Run after every candidate has been sealed.
The operator must bind --public-check to the frozen manifest's visible file.
"""

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def select(task, candidates, public_check):
    if len(candidates) not in (2, 3):
        raise ValueError("expected exactly two or three candidates in participant order")
    outputs = ["windows.py"] if task == "repair" else ["totals.csv", "exceptions.csv"]
    observations = []
    for ordinal, candidate in enumerate(candidates, 1):
        with tempfile.TemporaryDirectory(prefix="ymp-pilot-public-") as directory:
            work = Path(directory)
            record = {"participant_ordinal": ordinal, "public_passed": False, "files_sha256": {}, "error": None}
            try:
                for name in outputs:
                    source = (candidate / name).resolve(strict=True)
                    if not source.is_relative_to(candidate.resolve(strict=True)):
                        raise ValueError("candidate file escapes selected directory")
                    content = source.read_bytes()
                    record["files_sha256"][name] = hashlib.sha256(content).hexdigest()
                    (work / name).write_bytes(content)
                shutil.copyfile(public_check, work / "public_test.py")
                result = subprocess.run([sys.executable, "-B", "public_test.py"], cwd=work,
                                        capture_output=True, text=True, timeout=5)
                record.update(public_passed=result.returncode == 0, exit_code=result.returncode,
                              stdout=result.stdout, stderr=result.stderr)
            except (OSError, ValueError, subprocess.TimeoutExpired) as error:
                record["error"] = str(error)
            observations.append(record)
    winner = next((row["participant_ordinal"] for row in observations if row["public_passed"]), 1)
    return {"selection_rule": "first-public-pass-else-first-v1", "selected_participant_ordinal": winner,
            "public_check_sha256": hashlib.sha256(public_check.read_bytes()).hexdigest(),
            "observations": observations, "model_invocations": 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task", choices=["reconcile", "repair"], required=True)
    parser.add_argument("--public-check", type=Path, required=True)
    parser.add_argument("candidates", type=Path, nargs="+")
    args = parser.parse_args()
    print(json.dumps(select(args.task, args.candidates, args.public_check), indent=2))


if __name__ == "__main__":
    main()
