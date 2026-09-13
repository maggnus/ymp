#!/usr/bin/env python3
"""Narrow offline staging, freezing and blind artifact checks for YMP-201.

No provider calls, native runner, output selection or runtime acceptance here.
"""

import argparse
import csv
import hashlib
import importlib.util
import io
import json
import random
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EXECUTION_SPEC = importlib.util.spec_from_file_location("restricted_python", ROOT / "restricted_python.py")
restricted_python = importlib.util.module_from_spec(EXECUTION_SPEC)
EXECUTION_SPEC.loader.exec_module(restricted_python)
PROBE_SPEC = importlib.util.spec_from_file_location("repair_probe", ROOT / "repair_probe.py")
repair_probe = importlib.util.module_from_spec(PROBE_SPEC)
PROBE_SPEC.loader.exec_module(repair_probe)
SPEC = importlib.util.spec_from_file_location("universal", ROOT.parent / "validators/universal.py")
universal = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(universal)
require, digest, read_json = universal.require, universal.digest, universal.read_json
TASKS = ("reconcile", "repair")
VARIANTS = ("preparation", "measured")
CONDITIONS = ("strong-solo", "weak-solo", "independent-2", "independent-3",
              "cooperation-2", "cooperation-3")


def fixture(task, variant):
    require(task in TASKS and variant in VARIANTS, "unknown task or variant")
    return ROOT / "fixtures" / variant / task


def files(task, variant):
    return sorted(path.name for path in (fixture(task, variant) / "visible").iterdir())


def outputs(task):
    return ["totals.csv", "exceptions.csv"] if task == "reconcile" else ["windows.py"]


def hashes(directory, names):
    return {name: digest(universal.local_file(directory, name)) for name in names}


def non_git_new(path):
    require(path.is_absolute(), "use an absolute directory")
    require(not path.exists(), "destination must not exist")
    require(not any((parent / ".git").exists() for parent in path.parents),
            "destination must be outside Git")


def stage(task, variant, destination):
    source = fixture(task, variant) / "visible"
    non_git_new(destination)
    shutil.copytree(source, destination)
    return {"task": task, "variant": variant, "visible_sha256": hashes(destination, files(task, variant))}


def check_inputs(task, variant, workdir):
    immutable = [name for name in files(task, variant) if name not in outputs(task)]
    expected = hashes(fixture(task, variant) / "visible", immutable)
    require(hashes(workdir, immutable) == expected, "visible input or public check changed")
    return expected


def seal(task, variant, workdir, destination, blind_id):
    """Freeze exactly one preselected candidate without consulting private checks."""
    require(re.fullmatch(r"[a-f0-9]{32}", blind_id) is not None, "blind ID must be 32 lowercase hex characters")
    check_inputs(task, variant, workdir)
    names = sorted(set(files(task, variant) + outputs(task)))
    before = hashes(workdir, names)
    non_git_new(destination)
    destination.mkdir(parents=True)
    for name in names:
        shutil.copyfile(universal.local_file(workdir, name), destination / name)
    require(hashes(destination, names) == before, "artifact changed during freeze")
    receipt = {"schema_version": 1, "blind_id": blind_id, "task": task, "variant": variant,
               "files_sha256": before, "runtime_acceptance": None, "model_quality": None}
    (destination / "submission.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def csv_bytes(header, rows):
    buffer = io.StringIO(newline="")
    writer = csv.writer(buffer, lineterminator="\n")
    writer.writerow(header)
    writer.writerows(rows)
    return buffer.getvalue().encode("utf-8")


def reconcile_expected(task_source):
    """Recompute from immutable inputs; checked-in answers independently cross-check this."""
    seen, posted, cancelled, exceptions = set(), {}, set(), []
    fields = ["entry_id", "account", "currency", "quantity", "unit_cents", "status", "ref_id"]
    for number, row in enumerate(universal.csv_rows(task_source, fields), 1):
        identifier, status, target = row["entry_id"], row["status"], row["ref_id"]
        reason = None
        if identifier in seen:
            reason = "duplicate_id"
        elif status == "void":
            reason = "void"
        elif status == "posted":
            posted[identifier] = row
        elif target not in posted:
            reason = "unknown_target"
        elif target in cancelled:
            reason = "inactive_target"
        elif (row["account"], row["currency"]) != (posted[target]["account"], posted[target]["currency"]):
            reason = "group_mismatch"
        else:
            cancelled.add(target)
        seen.add(identifier)
        if reason:
            exceptions.append((number, identifier, reason))
    totals = {}
    for identifier, row in posted.items():
        if identifier not in cancelled:
            key = (row["account"], row["currency"])
            amount, count = totals.get(key, (0, 0))
            totals[key] = (amount + int(row["quantity"]) * int(row["unit_cents"]), count + 1)
    return {
        "totals.csv": csv_bytes(["account", "currency", "total_cents", "active_entries"],
                                [(*key, *totals[key]) for key in sorted(totals)]),
        "exceptions.csv": csv_bytes(["row_number", "entry_id", "reason"], exceptions),
    }


def repair_checks(variant, artifact):
    cases = read_json(fixture("repair", variant) / "private/cases.json")
    calls = [{"function": case["function"], "args": case["args"]} for case in cases]
    try:
        result = restricted_python.run(
            repair_probe.PROBE_SOURCE.encode(), {"windows.py": artifact.read_bytes()},
            args=("windows.py",), input_text=json.dumps(calls), timeout=5)
    except restricted_python.RestrictedExecutionError as error:
        if error.kind == "timeout" and error.process_terminated:
            return [{"criterion_id": "candidate_execution", "passed": False,
                     "candidate_error": "timeout", "process_terminated": True}]
        raise
    if result.returncode != 0:
        return [{"criterion_id": "candidate_execution", "passed": False,
                 "candidate_error": "nonzero_exit", "exit_code": result.returncode,
                 "stderr": result.stderr[-1000:]}]
    try:
        actual = universal.parse_json(result.stdout)
    except ValueError:
        actual = None
    if not isinstance(actual, list) or len(actual) != len(cases):
        return [{"criterion_id": "candidate_execution", "passed": False,
                 "candidate_error": "missing_python_observations"}]
    checks = []
    for case, observation in zip(cases, actual):
        expected = {"value": case.get("expected"), "error": case.get("error"), "unchanged": True,
                    "repeatable": None if case.get("error") else True,
                    "return_type": None if case.get("error") else case["return_type"]}
        checks.append({"criterion_id": case["id"],
                       "passed": universal.first_difference(expected, observation) is None})
    return checks


def check_artifacts(task, variant, workdir):
    check_inputs(task, variant, workdir)
    if task == "repair":
        return repair_checks(variant, universal.local_file(workdir, "windows.py"))
    expected = reconcile_expected(fixture(task, variant) / "visible/ledger.csv")
    checks = []
    for name, content in expected.items():
        require((fixture(task, variant) / "private" / name).read_bytes() == content,
                "private reference disagrees with independent recomputation")
        checks.append({"criterion_id": name, "passed": universal.local_file(workdir, name).read_bytes() == content})
    return checks


def score(submission):
    receipt = read_json(submission / "submission.json")
    require(set(receipt) == {"schema_version", "blind_id", "task", "variant", "files_sha256",
                             "runtime_acceptance", "model_quality"}, "nonblind or unsupported receipt fields")
    require(type(receipt["schema_version"]) is int and receipt["schema_version"] == 1, "unsupported receipt version")
    require(receipt["runtime_acceptance"] is None and receipt["model_quality"] is None, "receipt invents an outcome")
    require(re.fullmatch(r"[a-f0-9]{32}", receipt["blind_id"]) is not None, "invalid blind ID")
    task, variant = receipt["task"], receipt["variant"]
    names = sorted(set(files(task, variant) + outputs(task)))
    require(receipt["files_sha256"] == hashes(submission, names), "stale or incomplete submission hashes")
    checks = check_artifacts(task, variant, submission)
    require(receipt["files_sha256"] == hashes(submission, names), "submission changed during scoring")
    return {"schema_version": 1, "blind_id": receipt["blind_id"], "task": task, "variant": variant,
            "objective_success": all(check["passed"] for check in checks), "checks": checks,
            "files_sha256": receipt["files_sha256"], "scope": "artifact_only",
            "runtime_acceptance": None, "model_quality": None}


def matrix(seed):
    proposal = read_json(ROOT / "manifest.template.json")
    generator = random.Random(seed)
    proposal["condition_order_seed"] = seed
    proposal["fixture_sha256"] = {
        str(path.relative_to(ROOT)): digest(path)
        for path in sorted((ROOT / "fixtures").rglob("*")) if path.is_file()
    }
    proposal["observer_sha256"] = {str(path.relative_to(ROOT.parent)): digest(path) for path in
                                   [ROOT / "observer.py", ROOT / "repair_probe.py", ROOT / "public_select.py",
                                    ROOT / "restricted_python.py",
                                    ROOT / "manifest.template.json", ROOT.parent / "validators/universal.py"]}
    for task in TASKS:
        order = list(CONDITIONS)
        generator.shuffle(order)
        for index, condition in enumerate(order, 1):
            attempt = next(item for item in proposal["attempts"] if item["task"] == task and item["condition"] == condition)
            attempt["order_within_task"] = index
    return proposal


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("stage", "seal"):
        command = sub.add_parser(name)
        command.add_argument("--task", choices=TASKS, required=True)
        command.add_argument("--variant", choices=VARIANTS, required=True)
        command.add_argument("--destination", type=Path, required=True)
        if name == "seal":
            command.add_argument("--workdir", type=Path, required=True)
            command.add_argument("--blind-id", required=True)
    sub.add_parser("matrix").add_argument("--seed", required=True, type=int)
    sub.add_parser("score").add_argument("--submission", required=True, type=Path)
    args = parser.parse_args()
    try:
        if args.command == "stage":
            result = stage(args.task, args.variant, args.destination)
        elif args.command == "seal":
            result = seal(args.task, args.variant, args.workdir, args.destination, args.blind_id)
        elif args.command == "score":
            result = score(args.submission)
        else:
            result = matrix(args.seed)
        print(json.dumps(result, indent=2, allow_nan=False))
        return 1 if result.get("objective_success") is False else 0
    except restricted_python.RestrictedExecutionError as error:
        print(json.dumps({"validator_passed": False, "measurement_valid": False,
                          "execution_status": error.kind, "error": str(error)}))
        return 2
    except (OSError, ValueError, KeyError, TypeError, AttributeError, csv.Error, subprocess.TimeoutExpired) as error:
        print(json.dumps({"validator_passed": False, "error": str(error)}))
        return 1


if __name__ == "__main__":
    sys.exit(main())
