#!/usr/bin/env python3
"""Derive a compact run-NN.json from a retained ymp pilot run directory.

Usage:
    derive.py [--acceptance-path] RUN_DIR JOURNAL_COPY LOG_PATH NOTE_FILE OUTPUT

RUN_DIR       retained run directory (summary.json, experiment.json,
              discovery.json, view.json, workspace/). Read only.
JOURNAL_COPY  a copy of the run's journal.sqlite. The original is never
              opened, because opening a WAL database touches its sidecar.
LOG_PATH      the run's shell log, or "-" when none exists.
NOTE_FILE     UTF-8 text file holding the hand-written note for the run.
OUTPUT        file to write.

Everything except `note`, `runtime_source_commit` and `runtime_source_note`
is copied or computed from the run files. Fields under `derived` are computed.

Without the flag the output has exactly the structure of run-01.json to
run-05.json. With `--acceptance-path` one top-level key, `acceptance_path`, is
added after `workspace`. It holds the review, acceptance, commitment, session
change and mediated file access records that the first five runs either did
not have or did not retain. Nothing else changes.

A call can end without an invocation start record and without a cost record in
the projection (first seen in run 08). For such a call `invocation` and `cost`
are written as null, as in the source, and `derived.duration_ms` is null.

A run can reach the finalization stage and make a final-review call (first
seen in run 11). With `--acceptance-path`, and only for such a run, the key
`final_review` is added at the end of `acceptance_path`. It holds the recorded
purpose of that call, the finalization values of the projection, the `failure`
text of summary.json, every commitment end record of the run with the basis
stored in its outcome (`commitment_ends`), and the journal records from the
`Finalizing` phase change to the end in journal order (`records`). The values
under `final_review.derived` are computed by this script with the Python JSON
parser. They are not the kernel's decoding result. A run without a
final-review call gets no such key, so its file is unchanged.
"""

import hashlib
import json
import os
import re
import sqlite3
import sys

RUNTIME_SOURCE_COMMIT = "197aaa7bd8d1b8d85c996adec3bd29f1cefd0b0b"
RUNTIME_SOURCE_NOTE = (
    "Commit plus the uncommitted W1-0020 working tree. The working-tree state "
    "and build profile of each run are not recorded in run files."
)
REDACTED = "[redacted local path]"
# Built from parts so that this file does not itself contain the pattern.
HOME_PREFIXES = ("/" + "Users/", "/" + "home/")
SHELL_TIME = re.compile(r"cpu\s+(\S+)\s+total\s*$")


def load(path):
    with open(path, encoding="utf-8") as handle:
        return json.load(handle)


def pick(source, names):
    return {name: source[name] for name in names}


def redact(value):
    """Replace any string that contains a home-directory path."""
    if isinstance(value, dict):
        return {key: redact(item) for key, item in value.items()}
    if isinstance(value, list):
        return [redact(item) for item in value]
    if isinstance(value, str) and any(p in value for p in HOME_PREFIXES):
        return REDACTED
    return value


def read_journal(path):
    connection = sqlite3.connect("file:" + path + "?mode=ro", uri=True)
    try:
        events = []
        for seq, payload in connection.execute(
            "select e.seq, c.bytes from journal_events e "
            "join content_values c on c.digest = e.payload order by e.seq"
        ):
            event = json.loads(bytes(payload).decode("utf-8"))
            event["_seq"] = int.from_bytes(bytes(seq), "big")
            events.append(event)
        heads = connection.execute(
            "select session, last_seq, chain from journal_heads"
        ).fetchall()
        identity = connection.execute(
            "select identity from store_identity"
        ).fetchone()[0]
    finally:
        connection.close()
    return events, heads, identity


def event_kind(event):
    payload = event["payload"]
    return next(iter(payload)) if isinstance(payload, dict) else payload


def is_reference(value):
    return isinstance(value, dict) and set(value) == {"id", "version"}


def invocation(view, call_id):
    source = view["execution"]["invocations"][call_id]
    dispatch = source["dispatch"]
    prompt = json.loads(dispatch["prompt"]["text"])
    frame = prompt[-1]
    assignment = dispatch["assignment"]
    contribution = view["coordination"]["contributions"][
        assignment["contribution"]
    ]["value"]
    record = source["invocation"]
    return {
        "dispatch": {
            "invocation": dispatch["invocation"],
            "assignment": pick(
                assignment,
                [
                    "id",
                    "session",
                    "agent",
                    "profile",
                    "contribution",
                    "role",
                    "access",
                    "workspace",
                    "allowance",
                    "state",
                ],
            ),
            "kind": contribution["kind"],
            "port": frame.get("port"),
            "response_contract": frame.get("response"),
            "provider": dispatch["provider"],
            "settings": dispatch["settings"],
            "allowance": dispatch["allowance"],
            "backend": dispatch["backend"],
            "at": dispatch["at"],
            "deadline": dispatch["deadline"],
        },
        "invocation": record,
        "output": source["output"],
        "usage": source["usage"],
        "turns": source["turns"],
        "receipt": source["receipt"],
        "cost": (
            None
            if source["cost"] is None
            else pick(source["cost"][1], ["outcome"])
            | source["cost"][1]["effective"]
        ),
        "terminal": source["terminal"],
        "confirmed_terminal": source["confirmed_terminal"],
        "backend_terminal": (
            source["backend_terminal"][0]
            if isinstance(source["backend_terminal"], list)
            else source["backend_terminal"]
        ),
        "ended_at": source["ended_at"],
        "diagnostics": [
            [part for part in diagnostic if not is_reference(part)]
            for diagnostic in source["diagnostics"]
        ],
        "derived": {
            "duration_ms": (
                record["ended"] - record["started"]
                if record is not None
                and record.get("ended") is not None
                and record.get("started") is not None
                else None
            )
        },
    }


def account(source):
    settlement = source["settlement"]
    return {
        "reservation": source["reservation"],
        "demand": pick(
            source["demand"],
            ["contribution", "kind", "difficulty", "provider"],
        ),
        "allowance": source["allowance"],
        "invocation": source["invocation"],
        "receipt": source["receipt"],
        "complete_cost": source["complete_cost"],
        "observed_cost_floor": source["observed_cost_floor"],
        "upper_bound": source["upper_bound"],
        "revoked": source["revoked"],
        "settlement": (
            None
            if settlement is None
            else {
                "reservation": settlement["reservation"],
                "outcome": settlement["decision"]["outcome"],
            }
        ),
    }


def report(view):
    delivered = view["finalization"]["delivered"]
    if delivered is None:
        return None
    accounting = dict(delivered["accounting"])
    accounting["receipts"] = len(accounting["receipts"])
    return {
        "retained": delivered["retained"],
        "accepted_sources": delivered["accepted_sources"],
        "report": delivered["report"],
        "claims": delivered["claims"],
        "aggregate": delivered["aggregate"],
        "acceptance": delivered["acceptance"],
        "outcome": delivered["outcome"],
        "accounting": accounting,
        "unresolved": len(delivered["unresolved"]),
    }


def coordination(view, events):
    source = view["coordination"]
    opened = {}
    for event in events:
        if event_kind(event) == "SolicitationOpened":
            identifier = event["payload"]["SolicitationOpened"]["solicitation"]["id"]
            opened.setdefault(identifier, event["at"])
    solicitations = {}
    ordered = sorted(
        source["solicitations"].items(),
        key=lambda item: (opened.get(item[0], 0), item[0]),
    )
    for identifier, entry in ordered:
        value = entry["value"]
        opened_at = opened.get(identifier)
        offers = sorted(
            (
                offer["value"]
                for offer in source["offers"].values()
                if offer["value"]["solicitation"] == identifier
            ),
            key=lambda offer: (offer["at"], offer["id"]),
        )
        solicitations[identifier] = {
            "contribution": value["contribution"],
            "eligible": value["eligible"],
            "opened_at": opened_at,
            "deadline": value["deadline"],
            "final_state": value["state"],
            "offers": [pick(offer, ["agent", "at"]) for offer in offers],
            "derived": {
                "window_ms": (
                    None if opened_at is None else value["deadline"] - opened_at
                ),
                "offer_delay_ms": [
                    None if opened_at is None else offer["at"] - opened_at
                    for offer in offers
                ],
            },
        }
    return {
        "solicitations": solicitations,
        "awards": list(source["awards"].keys()),
        "commitments": list(source["commitments"].keys()),
    }


def workspace(run_dir, view, summary):
    files = {}
    root = os.path.join(run_dir, "workspace")
    for name in ("input.json", "sorted.json"):
        path = os.path.join(root, name)
        if not os.path.exists(path):
            continue
        with open(path, "rb") as handle:
            data = handle.read()
        files[name] = {
            "text": data.decode("utf-8"),
            "derived": {
                "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
            },
        }
    results = {
        identifier: pick(
            value,
            ["id", "item", "producer", "before", "after", "artifacts", "summary"],
        )
        for identifier, value in view["results"]["results"].items()
    }
    check_runs = {
        identifier: pick(
            value, ["check", "target", "role", "observation", "outcome", "at"]
        )
        for identifier, value in view["check_runs"].items()
    }
    evidence = {
        identifier: entry["evidence"]
        for identifier, entry in view["evidence"].items()
    }
    return {
        "files": files,
        "input_unchanged": summary["input_unchanged"],
        "sorted_bytes_match": summary["sorted_bytes_match"],
        "results": results,
        "check_runs": check_runs,
        "evidence": evidence,
        "acceptances": len(view["acceptances"]),
        "reviews": len(view["reviews"]),
    }


def first_key(value):
    return next(iter(value)) if isinstance(value, dict) else value


def finalization_detail(name, body):
    """Compact content of one FinalizationRecorded journal record."""
    if name == "Started":
        return pick(body, ["contract", "snapshot", "workspace", "holder"])
    if name == "Captured":
        return pick(body, ["snapshot", "baseline", "sources", "producers"])
    if name == "Reviewer":
        return {
            "outcome": body["decision"]["outcome"],
            "policy": body["decision"]["effective"]["policy"]["impl"],
            "aggregate": body["input"]["aggregate"],
            "candidates": [
                pick(candidate, ["prior_reviews"])
                | {"agent": candidate["profile"]["agent"]}
                for candidate in body["input"]["candidates"]
            ],
            "producers": body["input"]["producers"],
        }
    if name == "Narrative":
        return {
            "input": pick(body["input"], ["outcome", "final_acceptance", "source"]),
            "outcome": body["decision"]["outcome"],
        }
    if name == "Audit":
        return {
            "outcomes": [decision["outcome"] for decision in body["decisions"]]
        }
    if name == "Delivered":
        return pick(
            body, ["outcome", "acceptance", "aggregate", "accepted_sources"]
        )
    if name in ("Control", "ReportPrepared"):
        return body
    return None


def journal_record(event):
    """One journal record as sequence number, time, kind and detail."""
    kind = event_kind(event)
    payload = event["payload"]
    body = payload[kind] if isinstance(payload, dict) else None
    entry = {"seq": event["seq"], "at": event["at"], "kind": kind}
    if not isinstance(body, dict):
        return entry
    for field in ("change", "observation", "data"):
        if field in body:
            name = first_key(body[field])
            # Variant names are capitalized; field names are not.
            if isinstance(name, str) and name[:1].isupper():
                entry["detail"] = name
            break
    if "invocation" in body:
        entry["invocation"] = (
            body["invocation"]["id"]
            if isinstance(body["invocation"], dict)
            else body["invocation"]
        )
    if kind == "CheckRunRecorded":
        entry["content"] = pick(
            body["data"]["run"], ["id", "check", "role", "target", "outcome"]
        )
    elif kind == "EvidenceRecorded":
        entry["content"] = pick(
            body["data"]["evidence"],
            ["id", "criterion", "class", "polarity", "result"],
        )
    elif kind == "ProgressRecorded" and entry.get("detail") in (
        "Diagnosis",
        "Escalation",
    ):
        entry["content"] = body["data"][entry["detail"]]["decision"]["outcome"]
    elif kind == "FinalizationRecorded" and isinstance(body["data"], dict):
        name = entry["detail"]
        content = finalization_detail(name, body["data"][name])
        if content is not None:
            entry["content"] = content
    elif kind == "SessionChanged":
        entry["content"] = body["change"]
    elif kind == "CommitmentChanged" and isinstance(body["change"], dict):
        change = body["change"][entry["detail"]]
        entry["commitment"] = change.get("commitment", change.get("id"))
        if "outcome" in change:
            entry["outcome"] = first_key(change["outcome"])
    elif kind == "InvocationEnded":
        entry["content"] = pick(body, ["terminal", "confirmed"])
    elif kind == "ReportingStarted":
        entry["content"] = pick(body, ["mode"])
    return entry


def final_review(view, events, summary):
    """Final-review records; None for a run without a final-review call."""
    found = None
    for call_id, source in view["execution"]["invocations"].items():
        frames = json.loads(source["dispatch"]["prompt"]["text"])
        if frames[-1].get("operation") == "final_review":
            found = (call_id, source, frames)
    if found is None:
        return None
    call_id, source, frames = found
    purpose = frames[-1]
    final_evidence = sorted(
        identifier
        for identifier, entry in view["evidence"].items()
        if entry["evidence"]["result"] is None
    )
    earlier = json.dumps(frames[:-1])

    output = source["output"]
    whole = None
    tail = None
    prose = None
    if isinstance(output, str):
        try:
            whole = json.loads(output)
        except ValueError:
            whole = None
        start = output.find("{")
        if whole is None and start >= 0:
            try:
                tail = json.loads(output[start:])
                prose = start
            except ValueError:
                tail = None
    verdict = whole if isinstance(whole, dict) else tail
    subject = purpose.get("subject")
    supplied = purpose.get("evidence")
    basis = verdict.get("basis") if isinstance(verdict, dict) else None

    start_seq = None
    for event in events:
        if event_kind(event) == "SessionChanged":
            change = event["payload"]["SessionChanged"]["change"]
            if isinstance(change, dict) and change.get("Phase") == "Finalizing":
                start_seq = event["seq"]
    commitment_ends = []
    for event in events:
        if event_kind(event) != "CommitmentChanged":
            continue
        change = event["payload"]["CommitmentChanged"]["change"]
        if not isinstance(change, dict) or "Ended" not in change:
            continue
        ended = change["Ended"]
        outcome = ended["outcome"]
        commitment_ends.append(
            {
                "seq": event["seq"],
                "at": event["at"],
                "commitment": ended["commitment"],
                "outcome": first_key(outcome),
                "basis": (
                    outcome[first_key(outcome)].get("basis")
                    if isinstance(outcome, dict)
                    and isinstance(outcome[first_key(outcome)], dict)
                    else None
                ),
                "reason": ended.get("reason"),
            }
        )
    failure = summary["failure"]
    code = None if failure is None else failure.split(":")[0]
    finalization = view["finalization"]
    return {
        "invocation": call_id,
        "purpose": {
            "keys": sorted(purpose),
            "operation": purpose["operation"],
            "subject": subject,
            "evidence": supplied,
            "response": purpose.get("response"),
            "aggregate": pick(
                purpose["aggregate"],
                [
                    "snapshot",
                    "baseline",
                    "contract",
                    "criteria",
                    "producers",
                    "sources",
                ],
            )
            | {
                "checks": [
                    check["check"] for check in purpose["aggregate"]["checks"]
                ]
            },
            "runs": [
                pick(run, ["id", "check", "role", "target", "outcome", "at"])
                for run in purpose["runs"]
            ],
        },
        "projection": {
            "finalization": pick(
                finalization,
                [
                    "control",
                    "stopped",
                    "fence",
                    "outcome",
                    "reviews",
                    "acceptance",
                    "pending_work",
                ],
            ),
            "session_state": pick(
                view["session_state"], ["phase", "stopped", "control"]
            ),
            "final_evidence": final_evidence,
        },
        "failure": failure,
        "commitment_ends": commitment_ends,
        "records": [
            journal_record(event)
            for event in events
            if start_seq is not None and event["seq"] >= start_seq
        ],
        "derived": {
            "final_evidence_in_earlier_frames": {
                identifier: earlier.count(identifier)
                for identifier in final_evidence
            },
            "output_chars": len(output) if isinstance(output, str) else None,
            "whole_output_is_json": whole is not None,
            "json_after_prose": tail,
            "prose_chars_before_json": prose,
            "verdict": (
                None if not isinstance(verdict, dict) else verdict.get("verdict")
            ),
            "aggregate_equals_subject": (
                None
                if not isinstance(verdict, dict) or subject is None
                else verdict.get("aggregate") == subject
            ),
            "basis": basis,
            "basis_in_purpose_evidence": (
                None
                if basis is None or supplied is None
                else [item in supplied for item in basis]
            ),
            "basis_in_final_evidence": (
                None
                if basis is None
                else [item in final_evidence for item in basis]
            ),
            "failure_code_journal_records": (
                None
                if code is None
                else [
                    event["seq"]
                    for event in events
                    if code in json.dumps(event["payload"])
                ]
            ),
        },
    }


def acceptance_path(view, events, summary):
    """Records behind the acceptance decision; used for runs 06 and later."""
    reviews = {
        identifier: pick(
            entry, ["review", "assignment", "result", "criteria", "at"]
        )
        for identifier, entry in view["reviews"].items()
    }
    acceptances = {
        identifier: {
            "acceptance": entry["acceptance"],
            "result": entry["result"],
            "attempt": entry["attempt"],
            "contract": entry["contract"],
            "rules": entry["rules"],
            "credit_outcome": entry["credit"]["outcome"],
        }
        for identifier, entry in view["acceptances"].items()
    }
    ledger = {
        identifier: pick(entry, ["status", "belief", "evidence"])
        for identifier, entry in view["ledger"]["entries"].items()
    }

    activated = {}
    ended = {}
    session_changes = []
    file_access = []
    for event in events:
        kind = event_kind(event)
        if kind == "CommitmentChanged":
            change = event["payload"][kind]["change"]
            if not isinstance(change, dict):
                continue
            if "Activated" in change:
                body = change["Activated"]
                activated[body["commitment"]] = event
            if "Ended" in change:
                body = change["Ended"]
                outcome = body["outcome"]
                ended[body["commitment"]] = {
                    "seq": event["seq"],
                    "at": event["at"],
                    "outcome": (
                        next(iter(outcome))
                        if isinstance(outcome, dict)
                        else outcome
                    ),
                    "reason": body.get("reason"),
                }
        elif kind == "SessionChanged":
            change = event["payload"][kind]["change"]
            if isinstance(change, dict) and "Configured" in change:
                change = "Configured"
            session_changes.append(
                {"seq": event["seq"], "at": event["at"], "change": change}
            )
        elif kind == "LockChanged":
            change = event["payload"][kind]["change"]
            if isinstance(change, dict) and "FileAccessPrepared" in change:
                body = change["FileAccessPrepared"]
                file_access.append(
                    {
                        "seq": event["seq"],
                        "at": event["at"],
                        "assignment": body["assignment"],
                        "mode": body["mode"],
                        "path": body["target"]["path"],
                    }
                )

    commitments = {}
    ordered = sorted(
        view["coordination"]["commitments"].items(),
        key=lambda item: (
            activated[item[0]]["at"] if item[0] in activated else 0,
            item[0],
        ),
    )
    for identifier, value in ordered:
        start = activated.get(identifier)
        end = ended.get(identifier)
        commitments[identifier] = {
            "debtor": value["debtor"],
            "creditor": value["creditor"],
            "subject": value["subject"],
            "lease": value["lease"],
            "state": value["state"],
            "activated": (
                None
                if start is None
                else {"seq": start["seq"], "at": start["at"]}
            ),
            "ended": end,
            "derived": {
                "lease_ms": (
                    None
                    if start is None
                    else value["lease"]["expires"] - start["at"]
                ),
                "ended_after_activation_ms": (
                    None
                    if start is None or end is None
                    else end["at"] - start["at"]
                ),
            },
        }
    section = {
        "reviews": reviews,
        "acceptances": acceptances,
        "ledger": ledger,
        "commitments": commitments,
        "session_changes": session_changes,
        "file_access": file_access,
    }
    final = final_review(view, events, summary)
    if final is not None:
        section["final_review"] = final
    return section


def shell_wall_time(log_path):
    with open(log_path, encoding="utf-8") as handle:
        for line in handle:
            match = SHELL_TIME.search(line)
            if match:
                return match.group(1)
    return None


def main(argv):
    arguments = argv[1:]
    extended = "--acceptance-path" in arguments
    arguments = [item for item in arguments if item != "--acceptance-path"]
    run_dir, journal_copy, log_path, note_file, output = arguments[:5]
    run_dir = run_dir.rstrip("/")
    summary = load(os.path.join(run_dir, "summary.json"))
    view = load(os.path.join(run_dir, "view.json"))
    events, heads, identity = read_journal(journal_copy)
    with open(note_file, encoding="utf-8") as handle:
        note = handle.read().strip()

    treasury = view["treasury"]
    session, last_seq, chain = heads[0]
    document = {
        "summary": summary,
        "experiment": load(os.path.join(run_dir, "experiment.json")),
        "discovery": load(os.path.join(run_dir, "discovery.json")),
        "runtime_source_commit": RUNTIME_SOURCE_COMMIT,
        "runtime_source_note": RUNTIME_SOURCE_NOTE,
        "session_phase": view["session_state"]["phase"],
        "invocations": {
            call_id: invocation(view, call_id)
            for call_id in sorted(
                view["execution"]["invocations"],
                key=lambda call_id: view["execution"]["invocations"][call_id][
                    "dispatch"
                ]["at"],
            )
        },
        "budget": treasury["budget"],
        "totals": treasury["totals"],
        "unbounded": treasury["unbounded"],
        "unsettled_usage": treasury["unsettled_usage"],
        "reporting_mode": treasury["reporting_mode"],
        "accounts": {
            identifier: account(entry)
            for identifier, entry in treasury["accounts"].items()
        },
        "report": report(view),
        "coordination": coordination(view, events),
        "workspace": workspace(run_dir, view, summary),
        "journal": {
            "path": os.path.join(run_dir, "journal.sqlite"),
            "store_identity": identity,
            "session": session,
            "last_seq": int.from_bytes(bytes(last_seq), "big"),
            "chain": chain,
            "view_revision": view["revision"],
        },
        "log": {
            "path": None if log_path == "-" else log_path,
            "shell_wall_time": (
                None if log_path == "-" else shell_wall_time(log_path)
            ),
        },
        "evidence_class": "Native",
        "note": note,
    }
    if extended:
        ordered = {}
        for key, value in document.items():
            ordered[key] = value
            if key == "workspace":
                ordered["acceptance_path"] = acceptance_path(
                    view, events, summary
                )
        document = ordered
    document = redact(document)
    with open(output, "w", encoding="utf-8") as handle:
        json.dump(document, handle, indent=2, ensure_ascii=False)
        handle.write("\n")


if __name__ == "__main__":
    main(sys.argv)
