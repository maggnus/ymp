#!/usr/bin/env python3
"""Independent-attempt selection using visible evidence only, with no model.

No evaluator imports, hidden data, answers or quality oracle. Candidate arguments
are in preassigned participant order. Run after every candidate has been sealed.
The operator must bind --public-check to the frozen manifest's visible file.
"""

import argparse
import hashlib
import importlib.util
import json
import sys
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("restricted_python", Path(__file__).with_name("restricted_python.py"))
restricted_python = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(restricted_python)


def select(task, candidates, public_check):
    if len(candidates) not in (2, 3):
        raise ValueError("expected exactly two or three candidates in participant order")
    outputs = ["windows.py"] if task == "repair" else ["totals.csv", "exceptions.csv"]
    observations = []
    for ordinal, candidate in enumerate(candidates, 1):
        record = {"participant_ordinal": ordinal, "public_passed": False, "files_sha256": {},
                  "error": None, "execution_status": "not_started", "measurement_valid": True}
        try:
            contents = {}
            for name in outputs:
                source = (candidate / name).resolve(strict=True)
                if not source.is_relative_to(candidate.resolve(strict=True)):
                    raise ValueError("candidate file escapes selected directory")
                contents[name] = source.read_bytes()
                record["files_sha256"][name] = hashlib.sha256(contents[name]).hexdigest()
            result = restricted_python.run(public_check.read_bytes(), contents, timeout=5)
            record.update(public_passed=result.returncode == 0, exit_code=result.returncode,
                          stdout=result.stdout, stderr=result.stderr, execution_status="completed")
        except restricted_python.RestrictedExecutionError as error:
            record.update(error=str(error), execution_status=error.kind,
                          process_terminated=error.process_terminated,
                          measurement_valid=error.kind == "timeout" and error.process_terminated)
        except (OSError, ValueError) as error:
            record.update(error=str(error), execution_status="artifact_unavailable")
        observations.append(record)
    winner = next((row["participant_ordinal"] for row in observations if row["public_passed"]), 1)
    return {"selection_rule": "first-public-pass-else-first-v1", "selected_participant_ordinal": winner,
            "public_check_sha256": hashlib.sha256(public_check.read_bytes()).hexdigest(),
            "observations": observations, "model_invocations": 0,
            "measurement_valid": all(row["measurement_valid"] for row in observations)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task", choices=["reconcile", "repair"], required=True)
    parser.add_argument("--public-check", type=Path, required=True)
    parser.add_argument("candidates", type=Path, nargs="+")
    args = parser.parse_args()
    result = select(args.task, args.candidates, args.public_check)
    print(json.dumps(result, indent=2))
    return 0 if result["measurement_valid"] else 2


if __name__ == "__main__":
    sys.exit(main())
