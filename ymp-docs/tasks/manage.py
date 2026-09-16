#!/usr/bin/env python3
"""Inspect and update YMP's file-based development task register."""

from __future__ import annotations

import argparse
import collections
import copy
import fcntl
import json
import os
import re
import sys
import tempfile
from contextlib import contextmanager
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Iterator


SCHEMA_VERSION = 1
STATUSES = (
    "new",
    "planned",
    "in_progress",
    "owner_question",
    "paused",
    "done",
    "rejected",
)
TERMINAL_STATUSES = {"done", "rejected"}
ID_PATTERN = re.compile(r"W([1-9][0-9]*)-([0-9]{4,})\Z")
WAVE_PATTERN = re.compile(r"W([1-9][0-9]*)\Z")
SLUG_PATTERN = re.compile(r"[a-z][a-z0-9-]*\Z")
RECORD_FIELDS = {
    "schema_version",
    "id",
    "area",
    "title",
    "type",
    "priority",
    "status",
    "depends_on",
    "owner",
    "goal",
    "scope",
    "acceptance",
    "evidence",
    "context",
    "revision",
    "history",
}
DRAFT_FIELDS = RECORD_FIELDS - {"revision", "history"}
PATCH_FIELDS = {
    "title",
    "type",
    "priority",
    "depends_on",
    "goal",
    "scope",
    "acceptance",
    "evidence",
    "context",
}
HISTORY_FIELDS = {"revision", "at", "actor", "status", "note"}
DEFAULT_PAGE = 20
MAX_PAGE = 100
DEFAULT_SHOW_CHARS = 12_000
MAX_SHOW_CHARS = 50_000
SUMMARY_TEXT_CHARS = 180
DETAIL_TEXT_CHARS = 1_000
MAX_SLUG_CHARS = 64
PROGRESS_FILE = "PROGRESS.md"
PROGRESS_ZONE = timezone(timedelta(hours=8), "HKT")
PROGRESS_MARKS = {
    "open": "[ ]",
    "done": "[x]",
    "active": "[~]",
    "rejected": "[!]",
    "held": "[=]",
}
PROGRESS_COLUMNS = ("done", "active", "open", "held", "rejected")
COMMIT_PATTERN = re.compile(r"(?:\bcommit\b\s*[:=]?\s*`?|/commits?/)([0-9a-f]{7,64})(?![0-9a-f])", re.IGNORECASE)
SHORT_COMMIT_CHARS = 7


class TaskError(Exception):
    """A readable data or command error."""


class ConflictError(TaskError):
    """A rejected optimistic or ownership update."""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="microseconds").replace(
        "+00:00", "Z"
    )


def parse_timestamp(value: str, label: str) -> None:
    if not isinstance(value, str) or not value.strip():
        raise TaskError(f"{label} must be a nonblank ISO 8601 timestamp")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise TaskError(f"{label} must be an ISO 8601 timestamp") from error
    if parsed.tzinfo is None:
        raise TaskError(f"{label} must include a timezone")


def task_id_parts(task_id: str) -> tuple[int, int]:
    match = ID_PATTERN.fullmatch(task_id)
    if match is None:
        raise TaskError(
            f"invalid task ID {task_id!r}; expected W<wave>-<number>, for example W1-0001"
        )
    try:
        wave = int(match.group(1))
        number = int(match.group(2))
    except ValueError as error:
        raise TaskError(f"task ID {task_id!r} has an unsupported numeric component") from error
    if number < 1 or task_id != f"W{wave}-{number:04d}":
        raise TaskError(f"task ID {task_id!r} is not in canonical form")
    return wave, number


def wave_argument(value: str) -> int:
    match = WAVE_PATTERN.fullmatch(value)
    if match is None:
        raise argparse.ArgumentTypeError("expected a wave such as W1 or W2")
    try:
        return int(match.group(1))
    except ValueError as error:
        raise argparse.ArgumentTypeError("wave number is too large") from error


def require_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise TaskError(f"{label} must be a nonblank string")
    return value


def require_slug(value: Any, label: str) -> str:
    text = require_string(value, label)
    if len(text) > MAX_SLUG_CHARS or SLUG_PATTERN.fullmatch(text) is None:
        raise TaskError(
            f"{label} must be a lowercase safe slug of at most {MAX_SLUG_CHARS} characters"
        )
    return text


def preview(values: list[str], limit: int = 10) -> str:
    shown = ", ".join(values[:limit])
    if len(values) > limit:
        return f"{shown}, and {len(values) - limit} more"
    return shown


def require_string_list(value: Any, label: str, *, nonempty: bool = False) -> list[str]:
    if not isinstance(value, list):
        raise TaskError(f"{label} must be an array")
    if nonempty and not value:
        raise TaskError(f"{label} must not be empty")
    for index, item in enumerate(value):
        require_string(item, f"{label}[{index}]")
    return value


def validate_record(record: Any, source: str) -> None:
    if not isinstance(record, dict):
        raise TaskError(f"{source} must contain a JSON object")
    fields = set(record)
    if fields != RECORD_FIELDS:
        missing = sorted(RECORD_FIELDS - fields)
        extra = sorted(fields - RECORD_FIELDS)
        details = []
        if missing:
            details.append("missing " + preview(missing))
        if extra:
            details.append("unknown " + preview(extra))
        raise TaskError(f"{source} has invalid fields: {'; '.join(details)}")
    if (
        isinstance(record["schema_version"], bool)
        or not isinstance(record["schema_version"], int)
        or record["schema_version"] != SCHEMA_VERSION
    ):
        raise TaskError(f"{source} has unsupported schema_version")
    task_id = require_string(record["id"], f"{source}.id")
    task_id_parts(task_id)
    require_slug(record["area"], f"{task_id}.area")
    require_string(record["title"], f"{task_id}.title")
    require_slug(record["type"], f"{task_id}.type")
    priority = record["priority"]
    if isinstance(priority, bool) or not isinstance(priority, int) or priority < 0:
        raise TaskError(f"{task_id}.priority must be a nonnegative integer")
    if record["status"] not in STATUSES:
        raise TaskError(f"{task_id}.status must be one of {', '.join(STATUSES)}")
    dependencies = require_string_list(record["depends_on"], f"{task_id}.depends_on")
    if len(dependencies) != len(set(dependencies)):
        raise TaskError(f"{task_id} has duplicate dependencies")
    for dependency in dependencies:
        task_id_parts(dependency)
    owner = record["owner"]
    if owner is not None:
        require_string(owner, f"{task_id}.owner")
    require_string(record["goal"], f"{task_id}.goal")
    require_string_list(record["scope"], f"{task_id}.scope", nonempty=True)
    require_string_list(record["acceptance"], f"{task_id}.acceptance", nonempty=True)
    evidence = require_string_list(record["evidence"], f"{task_id}.evidence")
    require_string_list(record["context"], f"{task_id}.context")
    revision = record["revision"]
    if isinstance(revision, bool) or not isinstance(revision, int) or revision < 1:
        raise TaskError(f"{task_id}.revision must be a positive integer")
    history = record["history"]
    if not isinstance(history, list) or len(history) != revision:
        raise TaskError(f"{task_id}.history length must equal revision")
    for index, entry in enumerate(history, start=1):
        if not isinstance(entry, dict) or set(entry) != HISTORY_FIELDS:
            raise TaskError(f"{task_id}.history[{index - 1}] has invalid fields")
        if (
            isinstance(entry["revision"], bool)
            or not isinstance(entry["revision"], int)
            or entry["revision"] != index
        ):
            raise TaskError(f"{task_id}.history revisions must be contiguous from one")
        parse_timestamp(entry["at"], f"{task_id}.history[{index - 1}].at")
        require_string(entry["actor"], f"{task_id}.history[{index - 1}].actor")
        if entry["status"] not in STATUSES:
            raise TaskError(f"{task_id}.history[{index - 1}].status is invalid")
        require_string(entry["note"], f"{task_id}.history[{index - 1}].note")
    if history[-1]["status"] != record["status"]:
        raise TaskError(f"{task_id}.status must match its latest history entry")
    if record["status"] == "done" and not evidence:
        raise TaskError(f"{task_id} cannot be done without evidence")
    if record["status"] in {"owner_question", "paused"}:
        require_string(history[-1]["note"], f"{task_id} current status note")


def validate_records(records: dict[str, dict[str, Any]]) -> None:
    for task_id, record in records.items():
        validate_record(record, task_id)
        if record["id"] != task_id:
            raise TaskError(f"record key {task_id} does not match {record['id']}")

    reverse: dict[str, list[str]] = {task_id: [] for task_id in records}
    indegree: dict[str, int] = {}
    for task_id, record in records.items():
        dependencies = record["depends_on"]
        if task_id in dependencies:
            raise TaskError(f"{task_id} cannot depend on itself")
        for dependency in dependencies:
            if dependency not in records:
                raise TaskError(f"{task_id} has unknown dependency {dependency}")
            reverse[dependency].append(task_id)
        indegree[task_id] = len(dependencies)

    ready = collections.deque(
        sorted((task_id for task_id, degree in indegree.items() if degree == 0), key=task_id_parts)
    )
    visited = 0
    while ready:
        task_id = ready.popleft()
        visited += 1
        for dependent in reverse[task_id]:
            indegree[dependent] -= 1
            if indegree[dependent] == 0:
                ready.append(dependent)
    if visited != len(records):
        members = sorted(
            (task_id for task_id, degree in indegree.items() if degree > 0),
            key=task_id_parts,
        )
        cycle_preview = ", ".join(members[:5])
        suffix = "" if len(members) <= 5 else f" and {len(members) - 5} more"
        raise TaskError(f"dependency cycle includes {cycle_preview}{suffix}")

    for task_id, record in records.items():
        if record["status"] not in {"in_progress", "done"}:
            continue
        unfinished = [
            dependency
            for dependency in record["depends_on"]
            if records[dependency]["status"] != "done"
        ]
        if unfinished:
            raise TaskError(
                f"{task_id} cannot be {record['status']} with unfinished dependencies: "
                + preview(unfinished)
            )


class TaskStore:
    def __init__(self, root: Path):
        self.root = root.absolute()
        self.records_root = self.root / "records"

    def _check_root(self, *, create_records: bool = False) -> None:
        if not self.root.exists() or not self.root.is_dir():
            raise TaskError(f"task root does not exist or is not a directory: {self.root}")
        if self.root.is_symlink():
            raise TaskError(f"task root must not be a symlink: {self.root}")
        if create_records and not self.records_root.exists():
            self.records_root.mkdir(mode=0o755, exist_ok=True)
        if self.records_root.exists():
            if self.records_root.is_symlink() or not self.records_root.is_dir():
                raise TaskError("records must be a real directory inside the task root")

    def _read_json(self, path: Path) -> Any:
        flags = os.O_RDONLY
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        try:
            descriptor = os.open(path, flags)
            with os.fdopen(descriptor, encoding="utf-8") as handle:
                return json.load(handle)
        except ValueError as error:
            raise TaskError(f"invalid JSON in {path}: {error}") from error
        except UnicodeError as error:
            raise TaskError(f"record is not valid UTF-8: {path}") from error
        except OSError as error:
            raise TaskError(f"cannot read {path}: {error}") from error

    def load(self) -> dict[str, dict[str, Any]]:
        self._check_root()
        records: dict[str, dict[str, Any]] = {}
        if not self.records_root.exists():
            validate_records(records)
            return records
        for file_entry in sorted(os.scandir(self.records_root), key=lambda entry: entry.name):
            if file_entry.is_symlink():
                raise TaskError(f"symlinked record path is not allowed: {file_entry.path}")
            if not file_entry.is_file(follow_symlinks=False):
                raise TaskError(f"nested record paths are not allowed: {file_entry.path}")
            if file_entry.name == ".gitkeep":
                continue
            if file_entry.name.startswith(".") and ".tmp-" in file_entry.name:
                continue
            if not file_entry.name.endswith(".json"):
                raise TaskError(f"unexpected file in records: {file_entry.path}")
            task_id = file_entry.name.removesuffix(".json")
            task_id_parts(task_id)
            record = self._read_json(Path(file_entry.path))
            validate_record(record, file_entry.path)
            if record["id"] != task_id:
                raise TaskError(
                    f"record ID does not match its stable path: {file_entry.path}"
                )
            records[task_id] = record
        validate_records(records)
        return records

    @contextmanager
    def writer_lock(self) -> Iterator[None]:
        self._check_root(create_records=True)
        lock_path = self.root / ".manage.lock"
        if lock_path.is_symlink():
            raise TaskError("writer lock must not be a symlink")
        with lock_path.open("a+", encoding="utf-8") as handle:
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
            try:
                yield
            finally:
                fcntl.flock(handle.fileno(), fcntl.LOCK_UN)

    def path_for(self, record: dict[str, Any]) -> Path:
        task_id = require_string(record["id"], "id")
        task_id_parts(task_id)
        destination = self.records_root / f"{task_id}.json"
        if os.path.commonpath((str(self.root), str(destination.absolute()))) != str(self.root):
            raise TaskError("record path escapes the configured task root")
        return destination

    def write_record(self, record: dict[str, Any], *, create: bool = False) -> Path:
        destination = self.path_for(record)
        self._check_root(create_records=True)
        if create and destination.exists():
            raise ConflictError(f"task path already exists: {destination}")
        if destination.is_symlink():
            raise TaskError(f"record path must not be a symlink: {destination}")
        payload = json.dumps(record, indent=2, ensure_ascii=False) + "\n"
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{record['id']}.tmp-", dir=self.records_root, text=True
        )
        temporary = Path(temporary_name)
        try:
            with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
                handle.write(payload)
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(temporary, destination)
        except BaseException:
            temporary.unlink(missing_ok=True)
            raise
        return destination


def append_history(record: dict[str, Any], actor: str, note: str) -> None:
    actor = require_string(actor, "actor").strip()
    note = require_string(note, "note").strip()
    record["revision"] += 1
    record["history"].append(
        {
            "revision": record["revision"],
            "at": utc_now(),
            "actor": actor,
            "status": record["status"],
            "note": note,
        }
    )


def unfinished_dependencies(
    record: dict[str, Any], records: dict[str, dict[str, Any]]
) -> list[str]:
    return [
        dependency
        for dependency in record["depends_on"]
        if records[dependency]["status"] != "done"
    ]


def readiness(record: dict[str, Any], records: dict[str, dict[str, Any]]) -> str:
    status = record["status"]
    if status == "planned":
        return "ready" if not unfinished_dependencies(record, records) else "waiting"
    if status == "new":
        return "unscheduled"
    if status == "in_progress":
        return "active"
    if status in {"owner_question", "paused"}:
        return "attention"
    return "terminal"


def truncate(text: str, limit: int) -> str:
    normalized = " ".join(text.split())
    if len(normalized) <= limit:
        return normalized
    return normalized[: max(0, limit - 1)] + f"… [truncated; {len(normalized)} characters]"


def sort_key(record: dict[str, Any]) -> tuple[int, int, int]:
    wave, number = task_id_parts(record["id"])
    return record["priority"], wave, number


def page(items: list[Any], offset: int, limit: int, *, max_limit: int = MAX_PAGE) -> tuple[list[Any], dict[str, Any]]:
    if offset < 0:
        raise TaskError("offset must be nonnegative")
    if limit < 1 or limit > max_limit:
        raise TaskError(f"limit must be between 1 and {max_limit}")
    selected = items[offset : offset + limit]
    next_offset = offset + len(selected) if offset + len(selected) < len(items) else None
    return selected, {
        "offset": offset,
        "limit": limit,
        "returned": len(selected),
        "total": len(items),
        "next_offset": next_offset,
    }


def summary_for(record: dict[str, Any], records: dict[str, dict[str, Any]]) -> dict[str, Any]:
    return {
        "id": record["id"],
        "wave": f"W{task_id_parts(record['id'])[0]}",
        "area": record["area"],
        "type": record["type"],
        "priority": record["priority"],
        "status": record["status"],
        "readiness": readiness(record, records),
        "owner": truncate(record["owner"], SUMMARY_TEXT_CHARS) if record["owner"] else None,
        "revision": record["revision"],
        "title": truncate(record["title"], SUMMARY_TEXT_CHARS),
    }


def emit_json(value: Any) -> None:
    print(json.dumps(value, indent=2, ensure_ascii=False))


def emit_summaries(
    selected: list[dict[str, Any]], metadata: dict[str, Any], records: dict[str, dict[str, Any]], json_output: bool
) -> None:
    summaries = [summary_for(record, records) for record in selected]
    if json_output:
        emit_json({"page": metadata, "tasks": summaries})
        return
    print(
        f"Tasks {metadata['offset'] + 1 if metadata['returned'] else 0}-"
        f"{metadata['offset'] + metadata['returned']} of {metadata['total']}"
    )
    for item in summaries:
        owner = item["owner"] or "unowned"
        print(
            f"{item['id']} P{item['priority']} {item['status']} "
            f"({item['readiness']}, {owner}, r{item['revision']}) — {item['title']}"
        )
    if metadata["next_offset"] is not None:
        print(f"More results: repeat with --offset {metadata['next_offset']}")


def load_json_file(path_text: str) -> Any:
    path = Path(path_text)
    try:
        with path.open(encoding="utf-8") as handle:
            return json.load(handle)
    except FileNotFoundError as error:
        raise TaskError(f"input file does not exist: {path}") from error
    except ValueError as error:
        raise TaskError(f"invalid JSON in {path}: {error}") from error
    except UnicodeError as error:
        raise TaskError(f"input file is not valid UTF-8: {path}") from error
    except OSError as error:
        raise TaskError(f"cannot read input file {path}: {error}") from error


def get_record(records: dict[str, dict[str, Any]], task_id: str) -> dict[str, Any]:
    task_id_parts(task_id)
    try:
        return records[task_id]
    except KeyError as error:
        raise TaskError(f"unknown task {task_id}") from error


def expected(record: dict[str, Any], revision: int) -> None:
    if revision < 1:
        raise TaskError("expected revision must be positive")
    if record["revision"] != revision:
        raise ConflictError(
            f"stale revision for {record['id']}: expected {revision}, current {record['revision']}"
        )


def create_task(store: TaskStore, args: argparse.Namespace) -> None:
    draft = load_json_file(args.file)
    if not isinstance(draft, dict):
        raise TaskError("task draft must contain a JSON object")
    forbidden = set(draft) - DRAFT_FIELDS
    if forbidden:
        raise TaskError("task draft contains unsupported fields: " + preview(sorted(forbidden)))
    with store.writer_lock():
        records = store.load()
        candidate = copy.deepcopy(draft)
        candidate.setdefault("schema_version", SCHEMA_VERSION)
        if "id" not in candidate:
            if args.wave is None:
                raise TaskError("create requires --wave W1 when the draft has no id")
            numbers = (
                number
                for wave, number in map(task_id_parts, records)
                if wave == args.wave
            )
            number = max(numbers, default=0) + 1
            candidate["id"] = f"W{args.wave}-{number:04d}"
        elif args.wave is not None:
            wave, _ = task_id_parts(require_string(candidate["id"], "task draft.id"))
            if wave != args.wave:
                raise TaskError(f"task ID {candidate['id']} does not match --wave W{args.wave}")
        candidate.setdefault("status", "new")
        candidate.setdefault("owner", None)
        candidate.setdefault("evidence", [])
        candidate.setdefault("context", [])
        actor = candidate["owner"] or "manage.py"
        candidate["revision"] = 1
        candidate["history"] = [
            {
                "revision": 1,
                "at": utc_now(),
                "actor": actor,
                "status": candidate["status"],
                "note": "Task created.",
            }
        ]
        validate_record(candidate, "task draft")
        if candidate["id"] in records:
            raise ConflictError(f"task {candidate['id']} already exists")
        proposed = dict(records)
        proposed[candidate["id"]] = candidate
        validate_records(proposed)
        path = store.write_record(candidate, create=True)
        refresh_progress(store, proposed)
    emit_json({"created": candidate["id"], "revision": 1, "path": str(path.relative_to(store.root))})


def update_task(store: TaskStore, args: argparse.Namespace) -> None:
    patch = load_json_file(args.file)
    if not isinstance(patch, dict) or not patch:
        raise TaskError("task patch must be a nonempty JSON object")
    unsupported = set(patch) - PATCH_FIELDS
    if unsupported:
        raise TaskError(
            "task patch contains immutable or unsupported fields: "
            + preview(sorted(unsupported))
        )
    with store.writer_lock():
        records = store.load()
        current = get_record(records, args.id)
        expected(current, args.expect_revision)
        candidate = copy.deepcopy(current)
        candidate.update(copy.deepcopy(patch))
        append_history(candidate, candidate["owner"] or "manage.py", args.note)
        proposed = dict(records)
        proposed[args.id] = candidate
        validate_records(proposed)
        store.write_record(candidate)
        refresh_progress(store, proposed)
    emit_json({"updated": args.id, "revision": candidate["revision"]})


def claim_task(store: TaskStore, args: argparse.Namespace) -> None:
    owner = require_string(args.owner, "owner").strip()
    with store.writer_lock():
        records = store.load()
        current = get_record(records, args.id)
        expected(current, args.expect_revision)
        if current["status"] != "planned":
            raise ConflictError(
                f"{args.id} cannot be claimed from status {current['status']}; expected planned"
            )
        missing = unfinished_dependencies(current, records)
        if missing:
            raise ConflictError(
                f"{args.id} is not ready; unfinished dependencies: {preview(missing)}"
            )
        candidate = copy.deepcopy(current)
        candidate["owner"] = owner
        candidate["status"] = "in_progress"
        append_history(candidate, owner, f"Claimed by {owner}.")
        proposed = dict(records)
        proposed[args.id] = candidate
        validate_records(proposed)
        store.write_record(candidate)
        refresh_progress(store, proposed)
    emit_json({"claimed": args.id, "owner": owner, "revision": candidate["revision"]})


def status_task(store: TaskStore, args: argparse.Namespace) -> None:
    evidence = args.evidence or []
    for index, reference in enumerate(evidence):
        require_string(reference, f"evidence[{index}]")
    with store.writer_lock():
        records = store.load()
        current = get_record(records, args.id)
        expected(current, args.expect_revision)
        if args.state in {"in_progress", "done"}:
            missing = unfinished_dependencies(current, records)
            if missing:
                raise TaskError(
                    f"{args.id} cannot be {args.state} with unfinished dependencies: "
                    + preview(missing)
                )
        if args.state == "in_progress" and current["status"] != "in_progress":
            raise TaskError(f"use claim to start {args.id} from a ready planned state")
        if args.state == "done" and current["status"] != "in_progress":
            raise TaskError(f"{args.id} can be completed only from in_progress")
        candidate = copy.deepcopy(current)
        candidate["status"] = args.state
        candidate["evidence"].extend(evidence)
        actor = args.actor or candidate["owner"] or "manage.py"
        append_history(candidate, actor, args.note)
        proposed = dict(records)
        proposed[args.id] = candidate
        validate_records(proposed)
        store.write_record(candidate)
        refresh_progress(store, proposed)
    emit_json(
        {
            "status": args.state,
            "task": args.id,
            "revision": candidate["revision"],
            "evidence_added": len(evidence),
        }
    )


def command_list(records: dict[str, dict[str, Any]], args: argparse.Namespace) -> None:
    tasks = list(records.values())
    if args.wave:
        waves = set(args.wave)
        tasks = [record for record in tasks if task_id_parts(record["id"])[0] in waves]
    if args.area:
        areas = set(args.area)
        tasks = [record for record in tasks if record["area"] in areas]
    if args.status:
        statuses = set(args.status)
        tasks = [record for record in tasks if record["status"] in statuses]
    elif not args.all:
        tasks = [record for record in tasks if record["status"] not in TERMINAL_STATUSES]
    if args.readiness:
        readiness_values = set(args.readiness)
        tasks = [record for record in tasks if readiness(record, records) in readiness_values]
    tasks.sort(key=sort_key)
    selected, metadata = page(tasks, args.offset, args.limit)
    emit_summaries(selected, metadata, records, args.json)


def command_next(records: dict[str, dict[str, Any]], args: argparse.Namespace) -> None:
    tasks = [record for record in records.values() if readiness(record, records) == "ready"]
    if args.wave:
        waves = set(args.wave)
        tasks = [record for record in tasks if task_id_parts(record["id"])[0] in waves]
    if args.area:
        areas = set(args.area)
        tasks = [record for record in tasks if record["area"] in areas]
    tasks.sort(key=sort_key)
    selected, metadata = page(tasks, args.offset, args.limit)
    emit_summaries(selected, metadata, records, args.json)


def show_document(record: dict[str, Any], records: dict[str, dict[str, Any]]) -> str:
    lines = [
        f"# {record['id']} — {record['title']}",
        "",
        f"Wave: W{task_id_parts(record['id'])[0]}",
        f"Area: {record['area']}",
        f"Type: {record['type']}",
        f"Priority: {record['priority']}",
        f"Status: {record['status']}",
        f"Readiness: {readiness(record, records)}",
        f"Owner: {record['owner'] or 'unowned'}",
        f"Revision: {record['revision']}",
        "",
        "## Goal",
        "",
        record["goal"],
        "",
        "## Scope",
        "",
    ]
    lines.extend(f"- {item}" for item in record["scope"])
    lines.extend(["", "## Acceptance", ""])
    lines.extend(f"- {item}" for item in record["acceptance"])
    lines.extend(["", "## Evidence", ""])
    lines.extend(f"- {item}" for item in record["evidence"] or ["None recorded."])
    lines.extend(["", "## Context", ""])
    lines.extend(f"- {item}" for item in record["context"] or ["None recorded."])
    lines.extend(["", "## Immediate dependencies", ""])
    if record["depends_on"]:
        for dependency in record["depends_on"]:
            dependency_record = records[dependency]
            lines.append(
                f"- {dependency}: {dependency_record['status']} — "
                f"{truncate(dependency_record['title'], SUMMARY_TEXT_CHARS)}"
            )
    else:
        lines.append("- None.")
    lines.append("")
    return "\n".join(lines)


def command_show(records: dict[str, dict[str, Any]], args: argparse.Namespace) -> None:
    record = get_record(records, args.id)
    if args.offset < 0:
        raise TaskError("offset must be nonnegative")
    if args.limit < 1 or args.limit > MAX_SHOW_CHARS:
        raise TaskError(f"show limit must be between 1 and {MAX_SHOW_CHARS} characters")
    document = show_document(record, records)
    content = document[args.offset : args.offset + args.limit]
    next_offset = args.offset + len(content) if args.offset + len(content) < len(document) else None
    metadata = {
        "id": args.id,
        "offset": args.offset,
        "limit": args.limit,
        "returned_characters": len(content),
        "total_characters": len(document),
        "next_offset": next_offset,
        "history_included": False,
    }
    if args.json:
        emit_json(
            {
                "page": metadata,
                "task": summary_for(record, records),
                "content": content,
            }
        )
        return
    print(
        f"Task {args.id}; characters {args.offset}-"
        f"{args.offset + len(content)} of {len(document)}; history excluded."
    )
    print(content, end="" if content.endswith("\n") else "\n")
    if next_offset is not None:
        print(f"Output truncated. Continue with show {args.id} --offset {next_offset} --limit {args.limit}")


def dependency_summaries(
    record: dict[str, Any], records: dict[str, dict[str, Any]]
) -> list[dict[str, Any]]:
    distances: dict[str, int] = {}
    queue = collections.deque((dependency, 1) for dependency in record["depends_on"])
    while queue:
        task_id, distance = queue.popleft()
        previous = distances.get(task_id)
        if previous is not None and previous <= distance:
            continue
        distances[task_id] = distance
        for dependency in records[task_id]["depends_on"]:
            queue.append((dependency, distance + 1))
    result = []
    direct = set(record["depends_on"])
    for task_id, distance in sorted(distances.items(), key=lambda item: (item[1], task_id_parts(item[0]))):
        item = summary_for(records[task_id], records)
        item["distance"] = distance
        item["direct"] = task_id in direct
        result.append(item)
    return result


def command_deps(records: dict[str, dict[str, Any]], args: argparse.Namespace) -> None:
    record = get_record(records, args.id)
    dependencies = dependency_summaries(record, records)
    selected, metadata = page(dependencies, args.offset, args.limit)
    if args.json:
        emit_json({"task": args.id, "page": metadata, "dependencies": selected})
        return
    print(
        f"Dependencies for {args.id}: {metadata['returned']} shown of {metadata['total']} "
        f"from offset {metadata['offset']}"
    )
    for item in selected:
        relationship = "direct" if item["direct"] else f"distance {item['distance']}"
        print(f"{item['id']} {item['status']} ({relationship}) — {item['title']}")
    if metadata["next_offset"] is not None:
        print(f"More dependencies: repeat with --offset {metadata['next_offset']}")


def command_history(records: dict[str, dict[str, Any]], args: argparse.Namespace) -> None:
    record = get_record(records, args.id)
    entries = list(reversed(record["history"]))
    selected, metadata = page(entries, args.offset, args.limit)
    bounded = [
        {
            **entry,
            "actor": truncate(entry["actor"], SUMMARY_TEXT_CHARS),
            "note": truncate(entry["note"], DETAIL_TEXT_CHARS),
        }
        for entry in selected
    ]
    if args.json:
        emit_json({"task": args.id, "order": "newest_first", "page": metadata, "history": bounded})
        return
    print(
        f"History for {args.id}: {metadata['returned']} shown of {metadata['total']} "
        f"from offset {metadata['offset']}; newest first"
    )
    for entry in bounded:
        print(
            f"r{entry['revision']} {entry['at']} {entry['status']} by {entry['actor']} — {entry['note']}"
        )
    if metadata["next_offset"] is not None:
        print(f"More history: repeat with --offset {metadata['next_offset']}")


def command_summary(records: dict[str, dict[str, Any]], args: argparse.Namespace) -> None:
    groups = []
    for field in ("wave", "area", "type"):
        values: dict[str, list[dict[str, Any]]] = collections.defaultdict(list)
        for record in records.values():
            name = f"W{task_id_parts(record['id'])[0]}" if field == "wave" else record[field]
            values[name].append(record)
        names = sorted(values, key=wave_argument) if field == "wave" else sorted(values)
        for name in names:
            group_records = values[name]
            counts = collections.Counter(record["status"] for record in group_records)
            groups.append(
                {
                    "group": field,
                    "name": name,
                    "total": len(group_records),
                    "statuses": {status: counts[status] for status in STATUSES if counts[status]},
                }
            )
    selected, metadata = page(groups, args.offset, args.limit)
    if args.json:
        emit_json({"tasks": len(records), "page": metadata, "groups": selected})
        return
    print(f"Task summary: {len(records)} tasks; {metadata['returned']} groups shown of {metadata['total']}")
    for group in selected:
        counts = ", ".join(f"{status}={count}" for status, count in group["statuses"].items())
        print(f"{group['group']} {group['name']}: total={group['total']}; {counts}")
    if metadata["next_offset"] is not None:
        print(f"More groups: repeat with --offset {metadata['next_offset']}")


def command_render(records: dict[str, dict[str, Any]], args: argparse.Namespace) -> None:
    tasks = sorted(records.values(), key=sort_key)
    selected, metadata = page(tasks, args.offset, args.limit)
    counts = collections.Counter(record["status"] for record in records.values())
    print("# Development tasks")
    print()
    print(f"Canonical records: {len(records)}. This is a bounded view, not another status source.")
    print()
    print("| Status | Count |")
    print("| --- | ---: |")
    for status in STATUSES:
        if counts[status]:
            print(f"| `{status}` | {counts[status]} |")
    print()
    if selected:
        print("## Wave progress")
        print()
        print("| Wave | Done | Total | In progress |")
        print("| --- | ---: | ---: | ---: |")
        for wave in sorted({task_id_parts(record["id"])[0] for record in selected}):
            members = [record for record in records.values() if task_id_parts(record["id"])[0] == wave]
            done = sum(record["status"] == "done" for record in members)
            running = sum(record["status"] == "in_progress" for record in members)
            print(f"| W{wave} | {done} | {len(members)} | {running} |")
        print()
    active = [record for record in selected if record["status"] in {"in_progress", "owner_question", "paused"}]
    if active:
        print("## Current work")
        print()
        for record in active:
            latest = record["history"][-1]
            print(f"### {record['id']} — `{record['status']}`")
            print()
            print(f"Revision {record['revision']}; updated {latest['at']}.")
            print()
            print(truncate(latest["note"], 1_000))
            print()
    print(f"## Tasks ({metadata['returned']} shown of {metadata['total']}, offset {metadata['offset']})")
    print()
    for record in selected:
        item = summary_for(record, records)
        print(
            f"- [{item['id']}](records/{item['id']}.json) — "
            f"`{item['status']}` / `{item['readiness']}`; P{item['priority']}; {item['title']}"
        )
    if metadata["next_offset"] is not None:
        print()
        print(f"Output truncated. Continue with `render --offset {metadata['next_offset']} --limit {args.limit}`.")


def progress_kind(record: dict[str, Any], records: dict[str, dict[str, Any]]) -> str:
    status = record["status"]
    if status == "done":
        return "done"
    if status == "rejected":
        return "rejected"
    if status == "in_progress":
        return "active"
    if status in {"paused", "owner_question"}:
        return "held"
    if status == "planned" and unfinished_dependencies(record, records):
        return "held"
    return "open"


def progress_time(value: str) -> str:
    moment = datetime.fromisoformat(value.replace("Z", "+00:00"))
    try:
        return moment.astimezone(PROGRESS_ZONE).strftime("%Y-%m-%d %H:%M")
    except OverflowError:
        return f"{value} (outside HKT display range)"


def commit_references(record: dict[str, Any]) -> list[str]:
    references: list[str] = []
    for item in record["evidence"]:
        for match in COMMIT_PATTERN.findall(item):
            short = match[:SHORT_COMMIT_CHARS]
            if short not in references:
                references.append(short)
    return references


def progress_cell(text: str) -> str:
    return " ".join(text.split()).replace("|", "\\|") or "—"


def progress_state(record: dict[str, Any], kind: str) -> str:
    if record["status"] == "planned" and kind == "held":
        return "planned (blocked)"
    return record["status"]


def progress_dependencies(record: dict[str, Any], records: dict[str, dict[str, Any]]) -> str:
    cells = []
    for dependency in record["depends_on"]:
        status = records[dependency]["status"]
        suffix = " ✓" if status == "done" else " ✗" if status == "rejected" else ""
        cells.append(f"{dependency}{suffix}")
    return ", ".join(cells) or "—"


def render_progress(records: dict[str, dict[str, Any]]) -> str:
    tasks = sorted(records.values(), key=sort_key)
    kinds = {record["id"]: progress_kind(record, records) for record in tasks}
    counts = collections.Counter(kinds.values())
    lines = [
        "# Development task progress",
        "",
        "Generated from the canonical JSON records in [`records/`](records/) by "
        "`python3 ymp-docs/tasks/manage.py progress --write`. Task write commands "
        "and `make tasks-progress` refresh it; do not edit statuses here.",
        "",
    ]
    overview = (
        f"Tasks: {len(tasks)} ({counts['done']} done, {counts['active']} in progress, "
        f"{counts['open']} open, {counts['held']} paused or blocked, {counts['rejected']} rejected). "
        "Times are Hong Kong time (UTC+08:00)."
    )
    latest = max((record["history"][-1]["at"] for record in tasks), key=lambda value: datetime.fromisoformat(value.replace("Z", "+00:00")), default=None)
    if latest is not None:
        overview += f" Last record change: {progress_time(latest)}."
    lines += [overview, ""]
    lines += [
        "| Mark | Meaning |",
        "| :---: | --- |",
        f"| {PROGRESS_MARKS['open']} | not started (`new` is unscheduled; `planned` is ready only when dependencies are done) |",
        f"| {PROGRESS_MARKS['done']} | done |",
        f"| {PROGRESS_MARKS['active']} | in progress |",
        f"| {PROGRESS_MARKS['rejected']} | rejected |",
        f"| {PROGRESS_MARKS['held']} | paused or blocked (`paused`, `owner_question`, or `planned` with unfinished dependencies) |",
        "",
    ]
    if not tasks:
        lines += ["No tasks are recorded.", ""]
        return "\n".join(lines)
    waves = sorted({task_id_parts(record["id"])[0] for record in tasks})
    members = {wave: [record for record in tasks if task_id_parts(record["id"])[0] == wave] for wave in waves}
    lines += [
        "## Waves",
        "",
        "| Wave | Done | In progress | Open | Paused or blocked | Rejected | Total | Done % |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for wave in waves:
        wave_counts = collections.Counter(kinds[record["id"]] for record in members[wave])
        total = len(members[wave])
        percent = round(100 * wave_counts["done"] / total)
        cells = " | ".join(str(wave_counts[kind]) for kind in PROGRESS_COLUMNS)
        lines.append(f"| [W{wave}](#wave-w{wave}) | {cells} | {total} | {percent}% |")
    lines.append("")
    attention = [record for record in tasks if record["status"] in {"in_progress", "owner_question", "paused"}]
    if attention:
        lines += ["## Current work", ""]
        for record in attention:
            entry = record["history"][-1]
            lines += [
                f"### {record['id']} — {PROGRESS_MARKS[kinds[record['id']]]} {record['status']}",
                "",
                f"{progress_cell(record['title'])}. Owner: {record['owner'] or '—'}; "
                f"revision {record['revision']}; updated {progress_time(entry['at'])} by {progress_cell(entry['actor'])}.",
                "",
                truncate(entry["note"], DETAIL_TEXT_CHARS),
                "",
            ]
    for wave in waves:
        wave_counts = collections.Counter(kinds[record["id"]] for record in members[wave])
        lines += [
            f"## Wave W{wave}",
            "",
            f"{wave_counts['done']} of {len(members[wave])} done; {wave_counts['active']} in progress; "
            f"{wave_counts['held']} paused or blocked.",
            "",
            "| Status | Task | Title | Area | State | Depends on | Owner | Updated (HKT) | Rev | Evidence |",
            "| :---: | --- | --- | --- | --- | --- | --- | --- | ---: | --- |",
        ]
        for record in members[wave]:
            kind = kinds[record["id"]]
            evidence = []
            for reference in record["evidence"]:
                commits = commit_references({"evidence": [reference]})
                evidence.append(", ".join(f"commit `{commit}`" for commit in commits) if commits else progress_cell(truncate(reference, SUMMARY_TEXT_CHARS)))
            evidence_text = "; ".join(evidence) or "—"
            lines.append(
                f"| {PROGRESS_MARKS[kind]} "
                f"| [{record['id']}](records/{record['id']}.json) "
                f"| {progress_cell(truncate(record['title'], SUMMARY_TEXT_CHARS))} "
                f"| {progress_cell(record['area'])} "
                f"| {progress_state(record, kind)} "
                f"| {progress_dependencies(record, records)} "
                f"| {progress_cell(record['owner'] or '')} "
                f"| {progress_time(record['history'][-1]['at'])} "
                f"| {record['revision']} "
                f"| {evidence_text} |"
            )
        lines.append("")
    return "\n".join(lines)


def write_progress(store: TaskStore, document: str) -> Path:
    destination = store.root / PROGRESS_FILE
    if destination.is_symlink():
        raise TaskError(f"{PROGRESS_FILE} must not be a symlink: {destination}")
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{PROGRESS_FILE}.tmp-", dir=store.root, text=True)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(document)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, destination)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise
    return destination


def refresh_progress(store: TaskStore, records: dict[str, dict[str, Any]]) -> None:
    """Keep PROGRESS.md current after a record write without failing the write itself."""
    try:
        write_progress(store, render_progress(records))
    except Exception as error:
        # A derived presentation must never turn an already committed record
        # update into an apparent failed mutation.
        print(f"warning: {PROGRESS_FILE} was not refreshed: {error}", file=sys.stderr)


def command_progress(store: TaskStore, args: argparse.Namespace) -> None:
    if not args.write:
        sys.stdout.write(render_progress(store.load()))
        return
    with store.writer_lock():
        records = store.load()
        path = write_progress(store, render_progress(records))
    print(f"Refreshed {path.relative_to(store.root)}: {len(records)} tasks")


def add_page_arguments(parser: argparse.ArgumentParser, *, default: int = DEFAULT_PAGE) -> None:
    parser.add_argument("--limit", type=int, default=default)
    parser.add_argument("--offset", type=int, default=0)
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent,
        help="task root containing records/ (default: directory containing manage.py)",
    )
    commands = parser.add_subparsers(dest="command", required=True)

    next_parser = commands.add_parser("next", help="recommend ready planned tasks")
    next_parser.add_argument("--wave", action="append", type=wave_argument, metavar="W1")
    next_parser.add_argument("--area", action="append")
    add_page_arguments(next_parser, default=1)

    list_parser = commands.add_parser("list", help="list bounded task summaries")
    list_parser.add_argument("--wave", action="append", type=wave_argument, metavar="W1")
    list_parser.add_argument("--area", action="append")
    list_parser.add_argument("--status", action="append", choices=STATUSES)
    list_parser.add_argument(
        "--readiness",
        action="append",
        choices=("ready", "waiting", "unscheduled", "active", "attention", "terminal"),
    )
    list_parser.add_argument("--all", action="store_true", help="include terminal tasks")
    add_page_arguments(list_parser)

    show_parser = commands.add_parser("show", help="show one task without history")
    show_parser.add_argument("id")
    show_parser.add_argument("--limit", type=int, default=DEFAULT_SHOW_CHARS)
    show_parser.add_argument("--offset", type=int, default=0)
    show_parser.add_argument("--json", action="store_true")

    deps_parser = commands.add_parser("deps", help="show transitive dependency summaries")
    deps_parser.add_argument("id")
    add_page_arguments(deps_parser)

    history_parser = commands.add_parser("history", help="show paginated task history")
    history_parser.add_argument("id")
    add_page_arguments(history_parser)

    summary_parser = commands.add_parser("summary", help="group status counts by wave, area and type")
    add_page_arguments(summary_parser)

    render_parser = commands.add_parser("render", help="emit a bounded Markdown overview")
    render_parser.add_argument("--limit", type=int, default=DEFAULT_PAGE)
    render_parser.add_argument("--offset", type=int, default=0)

    create_parser = commands.add_parser("create", help="create one task from a JSON draft")
    create_parser.add_argument("--file", required=True)
    create_parser.add_argument("--wave", type=wave_argument, metavar="W1", help="required when the draft omits id")

    update_parser = commands.add_parser("update", help="update task definition fields")
    update_parser.add_argument("id")
    update_parser.add_argument("--file", required=True)
    update_parser.add_argument("--expect-revision", type=int, required=True)
    update_parser.add_argument("--note", required=True)

    claim_parser = commands.add_parser("claim", help="atomically claim a ready planned task")
    claim_parser.add_argument("id")
    claim_parser.add_argument("--owner", required=True)
    claim_parser.add_argument("--expect-revision", type=int, required=True)

    status_parser = commands.add_parser("status", help="change status and append history")
    status_parser.add_argument("id")
    status_parser.add_argument("state", choices=STATUSES)
    status_parser.add_argument("--expect-revision", type=int, required=True)
    status_parser.add_argument("--note", required=True)
    status_parser.add_argument("--actor")
    status_parser.add_argument("--evidence", action="append")

    progress_parser = commands.add_parser(
        "progress", help="human-readable Markdown progress tables; --write refreshes PROGRESS.md"
    )
    progress_parser.add_argument(
        "--write", action="store_true", help=f"atomically rewrite {PROGRESS_FILE} in the task root"
    )

    check_parser = commands.add_parser("check", help="validate every canonical record")
    check_parser.add_argument("--json", action="store_true")
    return parser


def run(args: argparse.Namespace) -> None:
    store = TaskStore(args.root)
    if args.command == "create":
        create_task(store, args)
        return
    if args.command == "update":
        update_task(store, args)
        return
    if args.command == "claim":
        claim_task(store, args)
        return
    if args.command == "status":
        status_task(store, args)
        return
    if args.command == "progress":
        command_progress(store, args)
        return

    records = store.load()
    if args.command == "next":
        command_next(records, args)
    elif args.command == "list":
        command_list(records, args)
    elif args.command == "show":
        command_show(records, args)
    elif args.command == "deps":
        command_deps(records, args)
    elif args.command == "history":
        command_history(records, args)
    elif args.command == "summary":
        command_summary(records, args)
    elif args.command == "render":
        command_render(records, args)
    elif args.command == "check":
        result = {"valid": True, "tasks": len(records), "schema_version": SCHEMA_VERSION}
        if args.json:
            emit_json(result)
        else:
            print(
                f"Valid task register: {len(records)} tasks; schema, paths, revisions, "
                "history and dependencies agree."
            )
    else:
        raise TaskError(f"unsupported command {args.command}")


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        run(args)
    except ConflictError as error:
        print(f"conflict: {error}", file=sys.stderr)
        return 4
    except (TaskError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 3
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
