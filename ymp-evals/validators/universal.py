#!/usr/bin/env python3
"""Independent artifact checks and normalized runtime export checks for YMP-119.

This module never invokes ymp or a provider. Runtime exports are supplied by the
trusted integration harness; accepting a synthetic export only tests fixtures.
"""

import argparse
import csv
import hashlib
import io
import json
import sys
from pathlib import Path


EVALS = Path(__file__).resolve().parents[1]
FIXTURES = EVALS / "fixtures" / "universal"
SCENARIOS = EVALS / "scenarios"


class Invalid(ValueError):
    """A delivered artifact or observed runtime record violates the fixture."""


def require(condition, message):
    if not condition:
        raise Invalid(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def reject_constant(value):
    raise Invalid(f"non-finite JSON number: {value}")


def parse_json(text):
    return json.loads(text, object_pairs_hook=unique_object,
                      parse_constant=reject_constant)


def read_json(path):
    return parse_json(Path(path).read_text(encoding="utf-8"))


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def local_file(workdir, relative):
    require(isinstance(relative, str) and relative, "empty artifact path")
    requested = Path(relative)
    require(not requested.is_absolute() and ".." not in requested.parts,
            "artifact path must be relative and inside the working directory")
    root = Path(workdir).resolve(strict=True)
    artifact = (root / requested).resolve(strict=True)
    require(artifact.is_relative_to(root), "artifact escapes the working directory")
    require(artifact.is_file(), f"missing file: {relative}")
    return artifact


def scenario(case_id):
    cases = read_json(SCENARIOS / "universal-workflows.json")["workflows"]
    require(case_id in cases, f"unknown workflow: {case_id}")
    return cases[case_id]


def csv_rows(path, fields):
    return parse_csv(Path(path).read_text(encoding="utf-8"), fields)


def parse_csv(text, fields):
    reader = csv.DictReader(io.StringIO(text, newline=""), strict=True)
    require(reader.fieldnames == fields, "wrong CSV columns")
    rows = list(reader)
    require(all(None not in row and None not in row.values() for row in rows),
            "ragged CSV row")
    return rows


def expected_document(source):
    data = read_json(source)
    lines = [f"# {data['title']}", "", f"Date: {data['date']}",
             f"Location: {data['location']}", "",
             "| Time | Activity | Owner |", "| --- | --- | --- |"]
    lines.extend(f"| {item['time']} | {item['activity']} | {item['owner']} |"
                 for item in data["items"])
    return "\n".join(lines) + "\n"


def normalized_document(text):
    # Permit native newline and trailing-space differences, not missing content.
    return "\n".join(line.rstrip() for line in text.splitlines()).strip("\n")


def expected_totals(source):
    totals = {}
    for row in csv_rows(source, ["entry_id", "account", "quantity", "unit_cents", "status"]):
        if row["status"] == "posted":
            total, count = totals.get(row["account"], (0, 0))
            totals[row["account"]] = (
                total + int(row["quantity"]) * int(row["unit_cents"]), count + 1)
    return [{"account": account, "total_cents": str(total), "posted_entries": str(count)}
            for account, (total, count) in sorted(totals.items())]


def expected_claims(source):
    rows = {row["row_id"]: row for row in csv_rows(
        source, ["row_id", "site", "week", "completed", "scheduled"])}

    def rate(row_id):
        row = rows[row_id]
        numerator = int(row["completed"]) * 100
        denominator = int(row["scheduled"])
        require(numerator % denominator == 0, "fixture rate must be integral")
        return numerator // denominator

    def claim(identifier, value, unit, row_ids):
        return {"id": identifier, "value": value, "unit": unit,
                "source": "inputs/observations.csv", "rows": row_ids}

    return {"claims": [
        claim("harbor-w36-rate", rate("O03"), "percent", ["O03"]),
        claim("hill-w36-rate", rate("O04"), "percent", ["O04"]),
        claim("hill-w36-minus-w35", rate("O04") - rate("O02"),
              "percentage_points", ["O02", "O04"]),
    ]}


def check_artifact(case_id, workdir):
    spec = scenario(case_id)
    workdir = Path(workdir).resolve(strict=True)
    if case_id == "document":
        require(not any((parent / ".git").exists() for parent in (workdir, *workdir.parents)),
                "document fixture must run in a non-Git directory")
    artifact = local_file(workdir, spec["output"])
    artifact_bytes = artifact.read_bytes()
    artifact_text = artifact_bytes.decode("utf-8")
    sources = []
    for relative in spec["inputs"]:
        actual = local_file(workdir, relative)
        actual_bytes = actual.read_bytes()
        require(actual_bytes == (FIXTURES / relative).read_bytes(), f"fixture input changed: {relative}")
        sources.append({"path": relative, "sha256": hashlib.sha256(actual_bytes).hexdigest()})

    if case_id == "document":
        expected = expected_document(FIXTURES / spec["inputs"][0])
        require(normalized_document(artifact_text) ==
                normalized_document(expected), "document content does not match the supplied agenda")
    elif case_id == "transform":
        actual = parse_csv(artifact_text, ["account", "total_cents", "posted_entries"])
        require(actual == expected_totals(FIXTURES / spec["inputs"][0]),
                "totals, ordering, entry counts or account set are incorrect")
    elif case_id == "grounded":
        actual = parse_json(artifact_text)
        expected = expected_claims(FIXTURES / spec["inputs"][0])
        # JSON booleans and floats must not compare equal to integral metrics.
        require(all(type(claim.get("value")) is int for claim in actual.get("claims", [])),
                "claim values must be integers")
        require(actual == expected, "claim values or source/row grounding are incorrect")
    elif case_id == "qualitative":
        paragraphs = artifact_text.replace("\r\n", "\n").strip("\n").split("\n\n")
        require(len(paragraphs) == 3 and paragraphs[0] == "# Workshop opening" and
                all(len(paragraph.split()) >= 10 for paragraph in paragraphs[1:]),
                "qualitative artifact must contain the requested heading and two paragraphs")
    return {"case_id": case_id, "path": spec["output"], "sha256": hashlib.sha256(artifact_bytes).hexdigest(),
            "inputs": sources, "criterion_ids": spec["criterion_ids"],
            "objective_success": None if case_id == "qualitative" else True,
            "check_scope": "structure_only" if case_id == "qualitative" else "content"}


def nonempty(value, description):
    require(isinstance(value, str) and bool(value.strip()), f"missing {description}")


def check_workflow(case_id, workdir, receipt):
    checked = check_artifact(case_id, workdir)
    require(type(receipt["schema_version"]) is int and receipt["schema_version"] == 1 and receipt["case_id"] == case_id,
            "wrong workflow export version or case")
    nonempty(receipt["session_id"], "session identity")
    require(receipt["working_directory"] == str(Path(workdir).resolve()),
            "runtime used a different working directory")
    artifact = receipt["artifact"]
    require(artifact["path"] == checked["path"] and artifact["sha256"] == checked["sha256"],
            "runtime artifact path/version does not identify the delivered bytes")
    nonempty(artifact["result_id"], "result identity")
    require(type(artifact["result_version"]) is int and artifact["result_version"] >= 1,
            "invalid result version")
    producers = artifact["producer_agent_ids"]
    require(isinstance(producers, list) and len(producers) == 1,
            "this controlled workflow requires one producing agent")
    nonempty(producers[0], "producing agent")
    review = receipt["review"]
    nonempty(review["review_id"], "review identity")
    nonempty(review["reviewer_agent_id"], "reviewer identity")
    require(review["reviewer_agent_id"] not in producers, "self-review cannot accept a result")
    nonempty(review["rationale"], "independent review rationale")
    require(review["decision"] == "accepted", "result was not independently accepted")
    require(review["artifact_sha256"] == checked["sha256"], "review refers to stale artifact bytes")
    acceptance = receipt["acceptance"]
    for record in (review, acceptance):
        require(record["result_id"] == artifact["result_id"] and
                type(record["result_version"]) is int and
                record["result_version"] == artifact["result_version"],
                "review/acceptance refers to another result version")
    require(acceptance["state"] == "accepted" and acceptance["review_id"] == review["review_id"],
            "runtime acceptance is not linked to the independent review")
    grade = "unconfirmed" if case_id == "qualitative" else "confirmed"
    require(acceptance["confirmation"] == grade, "incorrect confirmation grade")
    evidence = receipt["confirmations"]
    observations = receipt["reputation_observations"]
    require(isinstance(evidence, list) and isinstance(observations, list),
            "confirmations and reputation observations must be arrays")
    if case_id == "qualitative":
        require(acceptance["confirmation_ids"] == [], "unconfirmed acceptance cannot cite confirmation")
        require(evidence == [], "format checks or agent agreement cannot confirm qualitative quality")
        require(observations == [], "unconfirmed acceptance must not increase reputation")
    else:
        require(len(evidence) == 1, "the controlled workflow needs one scoped confirmation")
        confirmation = evidence[0]
        nonempty(confirmation["confirmation_id"], "confirmation identity")
        require(acceptance["confirmation_ids"] == [confirmation["confirmation_id"]],
                "acceptance is not linked to its confirmation")
        require(confirmation["observer"] == "runtime" and confirmation["outcome"] == "passed",
                "confirmation was not observed by the runtime")
        require(confirmation["check_id"] == scenario(case_id)["check_id"],
                "an unrelated passing check cannot confirm this result")
        require(confirmation["result_id"] == artifact["result_id"] and
                type(confirmation["result_version"]) is int and
                confirmation["result_version"] == artifact["result_version"] and
                confirmation["artifact_sha256"] == checked["sha256"],
                "confirmation does not cover the current result version")
        require(confirmation["criterion_ids"] == checked["criterion_ids"] and
                confirmation["inputs"] == checked["inputs"],
                "confirmation does not cover the criterion and captured input bytes")
        seen = set()
        for observation in observations:
            nonempty(observation["observation_id"], "observation identity")
            require(observation["observation_id"] not in seen, "duplicate reputation observation")
            seen.add(observation["observation_id"])
            require(observation["agent_id"] in producers and
                    observation["result_id"] == artifact["result_id"] and
                    type(observation["result_version"]) is int and
                    observation["result_version"] == artifact["result_version"] and
                    observation["confirmation_id"] == confirmation["confirmation_id"],
                    "reputation is not attributable to a confirmed producing agent")
        require(len(observations) <= 1, "one outcome must not produce repeated reputation credit")

    usage = receipt["usage"]
    require(isinstance(usage, list) and len(usage) >= 2, "missing execution/review usage")
    invocation_ids = set()
    phases = set()
    for row in usage:
        for field in ("agent_id", "assignment_id", "invocation_id"):
            nonempty(row[field], f"usage {field}")
        require(row["invocation_id"] not in invocation_ids, "duplicate invocation usage")
        invocation_ids.add(row["invocation_id"])
        require(row["agent_id"] in producers + [review["reviewer_agent_id"]],
                "usage must identify a participating agent, not a provider aggregate")
        require(row["phase"] in {"planning", "communication", "execution", "review", "retry", "consultation"},
                "unknown charged phase")
        require(row["coverage"] in {"complete", "partial", "unknown"}, "unknown usage coverage")
        require(row["tokens"] is None or type(row["tokens"]) is int and row["tokens"] >= 0,
                "invalid token usage")
        require(row["coverage"] != "unknown" or row["tokens"] is None,
                "unknown token usage cannot be reported as zero")
        require(row["coverage"] != "complete" or row["tokens"] is not None,
                "complete coverage requires measured token usage")
        phases.add((row["agent_id"], row["phase"]))
    require((producers[0], "execution") in phases and
            (review["reviewer_agent_id"], "review") in phases,
            "execution and review must be charged to their actual agents")
    if case_id == "document":
        follow_up = receipt["follow_up"]
        require(follow_up["session_id"] == receipt["session_id"] and
                follow_up["result_id"] == artifact["result_id"], "location follow-up lost its session result")
        require(follow_up["answer_path"] == str(local_file(workdir, checked["path"])) and
                follow_up["artifact_sha256"] == checked["sha256"], "location follow-up returned a different artifact")
        require(type(follow_up["invocations_started"]) is int and follow_up["invocations_started"] == 0,
                "location follow-up must reuse the recorded outcome without inference or production rerun")
    measured_tokens = [row["tokens"] for row in usage if row["tokens"] is not None]
    return {"case_id": case_id, "accepted": True, "confirmation": grade,
            "objective_success": checked["objective_success"],
            "resource_metrics": {"invocations": len(usage),
                                 "reported_tokens": sum(measured_tokens) if measured_tokens else None,
                                 "usage_complete": all(row["coverage"] == "complete" for row in usage)}}


def expand(value, workdir):
    """Expand only fixture-owned placeholders; observed exports stay untouched."""
    if isinstance(value, dict):
        return {key: expand(item, workdir) for key, item in value.items()}
    if isinstance(value, list):
        return [expand(item, workdir) for item in value]
    if value == "$WORKDIR":
        return str(Path(workdir).resolve())
    if isinstance(value, str) and value.startswith("$SHA256:"):
        return digest(local_file(workdir, value.removeprefix("$SHA256:")))
    if isinstance(value, str) and value.startswith("$PATH:"):
        return str(local_file(workdir, value.removeprefix("$PATH:")))
    return value


def first_difference(expected, actual, location="export"):
    if type(expected) is not type(actual):
        return f"{location}: expected {type(expected).__name__}, got {type(actual).__name__}"
    if isinstance(expected, dict):
        if set(expected) != set(actual):
            return f"{location}: missing/extra fields {sorted(set(expected) ^ set(actual))}"
        for key, value in expected.items():
            difference = first_difference(value, actual[key], f"{location}.{key}")
            if difference:
                return difference
    elif isinstance(expected, list):
        if len(expected) != len(actual):
            return f"{location}: expected {len(expected)} entries, got {len(actual)}"
        for index, (left, right) in enumerate(zip(expected, actual)):
            difference = first_difference(left, right, f"{location}[{index}]")
            if difference:
                return difference
    elif expected != actual:
        return f"{location}: expected {expected!r}, got {actual!r}"
    return None


def check_protocol(case_id, workdir, observed):
    cases = read_json(SCENARIOS / "universal-protocol.json")["cases"]
    require(case_id in cases, f"unknown protocol case: {case_id}")
    spec = cases[case_id]
    for relative in spec.get("inputs", []):
        require(digest(local_file(workdir, relative)) == digest(FIXTURES / relative),
                f"fixture input changed: {relative}")
    for artifact_case in spec.get("artifact_cases", []):
        check_artifact(artifact_case, workdir)
    for relative, expected_text in spec.get("file_assertions", {}).items():
        require(local_file(workdir, relative).read_bytes() == expected_text.encode("utf-8"),
                f"scripted artifact state is incorrect: {relative}")
    expected = expand(spec["expected"], workdir)
    difference = first_difference(expected, observed)
    require(difference is None, difference)
    return {"case_id": case_id, "protocol_passed": True,
            "scope": "supplied_normalized_trace_only", "model_quality": None}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["artifact", "workflow", "protocol"])
    parser.add_argument("--case", required=True)
    parser.add_argument("--workdir", required=True, type=Path)
    parser.add_argument("--observed", type=Path, help="trusted harness JSON export; required except in artifact mode")
    args = parser.parse_args(argv)
    try:
        if args.mode == "artifact":
            result = check_artifact(args.case, args.workdir)
        else:
            require(args.observed is not None, "--observed is required")
            observed = read_json(args.observed)
            result = (check_workflow if args.mode == "workflow" else check_protocol)(
                args.case, args.workdir, observed)
        print(json.dumps({"validator_passed": True, **result}, sort_keys=True))
        return 0
    except (Invalid, OSError, ValueError, KeyError, TypeError, AttributeError, csv.Error) as error:
        print(json.dumps({"validator_passed": False, "case_id": args.case, "error": str(error)}))
        return 1


if __name__ == "__main__":
    sys.exit(main())
